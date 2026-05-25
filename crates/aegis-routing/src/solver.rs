//! The route finder + min-cost-flow solver (spec §7.3).
//!
//! * [`find_route`] — Dijkstra over all-in cost: the single cheapest route for a
//!   `(source, target, size)` query, or honest `None` when disconnected.
//! * [`min_cost_flow`] — successive-shortest-paths over **finite-difference
//!   marginal** cost: augments flow along the cheapest residual path until the
//!   demand is met, splitting across routes exactly when convex price-impact
//!   makes a split cheaper, then a dominance clamp guarantees the plan is never
//!   worse than the best single route.
//!
//! Both share one deterministic Dijkstra (lexicographic tie-break: cost → hops →
//! edge-index path), so identical inputs always yield an identical plan.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use rust_decimal::Decimal;

use crate::cost::{EdgeCost, ValueUsd};
use crate::domain::Asset;
use crate::graph::{EdgeIdx, LiquidityGraph, NodeIdx};

/// A single resolved route at a fixed size.
#[derive(Debug, Clone)]
pub struct Route {
    /// Edge indices, source→target order.
    pub legs: Vec<EdgeIdx>,
    pub size: ValueUsd,
    /// Decomposed all-in cost, summed component-wise over the legs.
    pub cost: EdgeCost,
}

impl Route {
    pub fn all_in(&self) -> Decimal {
        self.cost.all_in()
    }
    pub fn hops(&self) -> usize {
        self.legs.len()
    }
}

/// One route carrying a share of a split trade.
#[derive(Debug, Clone)]
pub struct FlowAllocation {
    pub legs: Vec<EdgeIdx>,
    pub value: ValueUsd,
}

/// The result of a min-cost-flow solve over one `(source → target, demand)`.
#[derive(Debug, Clone)]
pub struct FlowPlan {
    pub allocations: Vec<FlowAllocation>,
    /// True all-in cost of the flow (every edge priced at its total flow).
    pub total_cost: Decimal,
    pub delivered: ValueUsd,
    /// Demand that could not be routed (no residual path) — surfaced, never
    /// silently dropped.
    pub deferred: ValueUsd,
    /// Whether the trade was split across more than one route.
    pub split: bool,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SolveError {
    #[error("source asset {0:?} is not a node in the graph")]
    UnknownSource(Asset),
    #[error("target asset {0:?} is not a node in the graph")]
    UnknownTarget(Asset),
}

/// Knobs for the flow solver. More increments → smaller optimality gap (the gap
/// is O(demand / increments)); the default meets the ≥95% saving gate (M2) with
/// margin. `max_routes` caps candidate diversity (Yen-style), keeping the solve
/// sub-quadratic at scale (M6).
#[derive(Debug, Clone, Copy)]
pub struct FlowConfig {
    pub increments: u32,
    pub max_routes: u32,
}

impl Default for FlowConfig {
    fn default() -> Self {
        Self {
            increments: 256,
            max_routes: 4,
        }
    }
}

/// Component-wise sum of edge costs into a route's aggregate.
fn add_cost(acc: &mut EdgeCost, e: &EdgeCost) {
    acc.amm_fee += e.amm_fee;
    acc.price_impact += e.price_impact;
    acc.bridge_fee += e.bridge_fee;
    acc.gateway_fee += e.gateway_fee;
    acc.forwarding_fee += e.forwarding_fee;
    acc.protocol_fee += e.protocol_fee;
    acc.gas_usdc += e.gas_usdc;
    acc.slippage_budget += e.slippage_budget;
}

/// Heap entry for the deterministic Dijkstra. Ordered by (cost, hops, node, via)
/// so that, among equal-cost relaxations, the fewest-hops then smallest-node
/// then smallest-edge candidate is finalized first — fully reproducible, with no
/// per-push path allocation (predecessors are tracked separately).
struct State {
    cost: Decimal,
    hops: usize,
    node: NodeIdx,
    /// Edge that reached `node` (`usize::MAX` for the source).
    via: EdgeIdx,
}

impl State {
    fn key(&self) -> (Decimal, usize, NodeIdx, EdgeIdx) {
        (self.cost, self.hops, self.node, self.via)
    }
}
impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}
impl Eq for State {}
impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse so the BinaryHeap (a max-heap) pops the *smallest* key first.
        other.key().cmp(&self.key())
    }
}
impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Deterministic Dijkstra from `source` to `target`, with a caller-supplied
/// non-negative edge weight. Returns the cheapest edge path (reconstructed from
/// predecessors), or `None` if `target` is unreachable. The first time a node is
/// popped it is final (weights are non-negative); the total heap order makes that
/// pop deterministic across runs.
fn dijkstra<F>(
    graph: &LiquidityGraph,
    source: NodeIdx,
    target: NodeIdx,
    weight: F,
    forbidden: &HashSet<EdgeIdx>,
    forbidden_nodes: &HashSet<NodeIdx>,
) -> Option<Vec<EdgeIdx>>
where
    F: Fn(EdgeIdx) -> Decimal,
{
    let n = graph.node_count();
    let mut finalized = vec![false; n];
    let mut pred: Vec<EdgeIdx> = vec![usize::MAX; n];
    let mut heap = BinaryHeap::new();
    heap.push(State {
        cost: Decimal::ZERO,
        hops: 0,
        node: source,
        via: usize::MAX,
    });

    while let Some(state) = heap.pop() {
        if finalized[state.node] {
            continue;
        }
        finalized[state.node] = true;
        pred[state.node] = state.via;
        if state.node == target {
            break;
        }
        for &e in graph.out_edges(state.node) {
            if forbidden.contains(&e) {
                continue;
            }
            let edge = graph.edge(e);
            if finalized[edge.to] || forbidden_nodes.contains(&edge.to) {
                continue;
            }
            heap.push(State {
                cost: state.cost + weight(e),
                hops: state.hops + 1,
                node: edge.to,
                via: e,
            });
        }
    }

    if !finalized[target] {
        return None;
    }
    // Walk predecessors back from target to source.
    let mut legs = Vec::new();
    let mut node = target;
    while node != source {
        let e = pred[node];
        if e == usize::MAX {
            return None; // unreachable safety net (source != target with no pred)
        }
        legs.push(e);
        node = graph.edge(e).from;
    }
    legs.reverse();
    Some(legs)
}

/// Resolve the indices of `source`/`target` assets, erroring if either is not a
/// node in the graph.
fn endpoints(
    graph: &LiquidityGraph,
    source: &Asset,
    target: &Asset,
) -> Result<(NodeIdx, NodeIdx), SolveError> {
    let s = graph
        .node_index(source)
        .ok_or_else(|| SolveError::UnknownSource(source.clone()))?;
    let t = graph
        .node_index(target)
        .ok_or_else(|| SolveError::UnknownTarget(target.clone()))?;
    Ok((s, t))
}

/// The single minimum-all-in-cost route for `size` flowing `source → target`,
/// or `None` if no path exists (honest disconnection).
pub fn find_route(
    graph: &LiquidityGraph,
    source: &Asset,
    target: &Asset,
    size: ValueUsd,
) -> Result<Option<Route>, SolveError> {
    let (s, t) = endpoints(graph, source, target)?;
    if s == t {
        return Ok(Some(Route {
            legs: Vec::new(),
            size,
            cost: EdgeCost::default(),
        }));
    }
    let weight = |e: EdgeIdx| graph.edge(e).curve.all_in(size);
    let Some(legs) = dijkstra(graph, s, t, weight, &HashSet::new(), &HashSet::new()) else {
        return Ok(None);
    };
    let mut cost = EdgeCost::default();
    for &e in &legs {
        add_cost(&mut cost, &graph.edge(e).curve.cost(size));
    }
    Ok(Some(Route { legs, size, cost }))
}

/// Total all-in cost of a flow: every edge priced at its *total* flow (so shared
/// edges are charged once, on their combined load).
fn flow_cost(graph: &LiquidityGraph, edge_flow: &HashMap<EdgeIdx, Decimal>) -> Decimal {
    edge_flow
        .iter()
        .map(|(&e, &f)| graph.edge(e).curve.all_in(ValueUsd::usd(f)))
        .sum()
}

fn path_cost<F>(path: &[EdgeIdx], weight: &F) -> Decimal
where
    F: Fn(EdgeIdx) -> Decimal,
{
    path.iter().map(|&e| weight(e)).sum()
}

fn path_nodes(graph: &LiquidityGraph, source: NodeIdx, path: &[EdgeIdx]) -> Vec<NodeIdx> {
    let mut nodes = Vec::with_capacity(path.len() + 1);
    nodes.push(source);
    let mut current = source;
    for &e in path {
        debug_assert_eq!(graph.edge(e).from, current);
        current = graph.edge(e).to;
        nodes.push(current);
    }
    nodes
}

/// Up to `k` cheapest simple routes, allowing shared prefixes/suffixes.
///
/// This is a deterministic Yen-style candidate generator. It keeps the solver
/// bounded for production-sized graphs, but unlike the previous edge-disjoint
/// seeding it does not forbid shared bridges or USDC hubs. Shared edge costs are
/// accounted later by `edge_flow`, so convex splits across routes with common
/// prefixes/suffixes are valid candidates. Like Yen's algorithm, each outer
/// iteration spurs from the route most recently accepted into `routes` while
/// retaining the global candidate pool from earlier accepted routes.
fn candidate_routes(
    graph: &LiquidityGraph,
    s: NodeIdx,
    t: NodeIdx,
    size: ValueUsd,
    k: u32,
) -> Vec<Vec<EdgeIdx>> {
    let weight = |e: EdgeIdx| graph.edge(e).curve.all_in(size);
    let empty_edges = HashSet::new();
    let empty_nodes = HashSet::new();
    let Some(first) = dijkstra(graph, s, t, weight, &empty_edges, &empty_nodes) else {
        return Vec::new();
    };
    if first.is_empty() {
        return Vec::new();
    }

    let mut routes = vec![first];
    let mut seen: HashSet<Vec<EdgeIdx>> = routes.iter().cloned().collect();
    let mut candidate_pool: Vec<Vec<EdgeIdx>> = Vec::new();
    let limit = k.max(1) as usize;

    while routes.len() < limit {
        let previous = routes[routes.len() - 1].clone();
        let prev_nodes = path_nodes(graph, s, &previous);

        for spur_pos in 0..previous.len() {
            let spur_node = prev_nodes[spur_pos];
            let root = &previous[..spur_pos];

            let mut forbidden_edges: HashSet<EdgeIdx> = HashSet::new();
            for route in &routes {
                if route.len() > spur_pos && route[..spur_pos] == *root {
                    forbidden_edges.insert(route[spur_pos]);
                }
            }

            let mut forbidden_nodes: HashSet<NodeIdx> =
                prev_nodes[..spur_pos].iter().copied().collect();
            forbidden_nodes.remove(&spur_node);

            let Some(spur) = dijkstra(
                graph,
                spur_node,
                t,
                weight,
                &forbidden_edges,
                &forbidden_nodes,
            ) else {
                continue;
            };
            if spur.is_empty() {
                continue;
            }
            let mut candidate = root.to_vec();
            candidate.extend(spur);
            if seen.insert(candidate.clone()) {
                candidate_pool.push(candidate);
            }
        }

        if candidate_pool.is_empty() {
            break;
        }
        let best_idx = candidate_pool
            .iter()
            .enumerate()
            .min_by_key(|(_, path)| (path_cost(path, &weight), path.len(), (*path).clone()))
            .map(|(idx, _)| idx)
            .expect("candidate pool is non-empty");
        let best = candidate_pool.swap_remove(best_idx);
        routes.push(best);
    }
    routes
}

/// Min-cost flow with route splitting (successive shortest paths over a bounded
/// candidate set).
///
/// Generates the cheapest few simple routes (shared hubs/bridges allowed), then
/// water-fills `demand` across them one increment at a time, each increment
/// extending the route whose **marginal** cost is currently lowest. Convex
/// impact makes an over-used route's marginal rise, so flow spreads to a second
/// route exactly when that is cheaper. A final dominance clamp guarantees the
/// plan is **never worse** than the best single route.
pub fn min_cost_flow(
    graph: &LiquidityGraph,
    source: &Asset,
    target: &Asset,
    demand: ValueUsd,
    cfg: FlowConfig,
) -> Result<FlowPlan, SolveError> {
    let (s, t) = endpoints(graph, source, target)?;
    let routes = candidate_routes(graph, s, t, demand, cfg.max_routes);
    if routes.is_empty() {
        return Ok(FlowPlan {
            allocations: Vec::new(),
            total_cost: Decimal::ZERO,
            delivered: ValueUsd::ZERO,
            deferred: demand,
            split: false,
        });
    }

    let increments = cfg.increments.max(1);
    let step = demand.amount() / Decimal::from(increments);
    let mut edge_flow: HashMap<EdgeIdx, Decimal> = HashMap::new();
    let mut alloc_value = vec![Decimal::ZERO; routes.len()];

    for _ in 0..increments {
        // Pick the candidate with the lowest marginal cost for the next step.
        // Strict `<` keeps the earliest (cheapest-overall) route on ties →
        // deterministic.
        let mut best = 0usize;
        let mut best_marginal: Option<Decimal> = None;
        for (i, route) in routes.iter().enumerate() {
            let marginal: Decimal = route
                .iter()
                .map(|&e| {
                    let f = edge_flow.get(&e).copied().unwrap_or(Decimal::ZERO);
                    graph.edge(e).curve.marginal(ValueUsd::usd(f), step)
                })
                .sum();
            if best_marginal.is_none_or(|bm| marginal < bm) {
                best = i;
                best_marginal = Some(marginal);
            }
        }
        for &e in &routes[best] {
            *edge_flow.entry(e).or_insert(Decimal::ZERO) += step;
        }
        alloc_value[best] += step;
    }

    let split_total = flow_cost(graph, &edge_flow);

    // Single-route baseline (route 0 carrying the whole demand) for the clamp.
    let single = find_route(graph, source, target, demand)?;
    let used_routes = alloc_value.iter().filter(|v| **v > Decimal::ZERO).count();
    let use_single = match &single {
        Some(r) => split_total >= r.all_in() || used_routes <= 1,
        None => false,
    };

    if use_single {
        if let Some(r) = single {
            return Ok(FlowPlan {
                allocations: vec![FlowAllocation {
                    legs: r.legs,
                    value: demand,
                }],
                total_cost: r.cost.all_in(),
                delivered: demand,
                deferred: ValueUsd::ZERO,
                split: false,
            });
        }
    }

    let mut allocations: Vec<FlowAllocation> = routes
        .into_iter()
        .zip(alloc_value)
        .filter(|(_, v)| *v > Decimal::ZERO)
        .map(|(legs, value)| FlowAllocation {
            legs,
            value: ValueUsd::usd(value),
        })
        .collect();
    allocations.sort_by(|a, b| a.legs.cmp(&b.legs));
    let split = allocations.len() > 1;

    Ok(FlowPlan {
        allocations,
        total_cost: split_total,
        delivered: demand,
        deferred: ValueUsd::ZERO,
        split,
    })
}
