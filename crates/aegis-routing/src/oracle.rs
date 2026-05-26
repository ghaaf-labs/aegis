//! Exhaustive brute-force optima — the *independent* reference the solver is
//! graded against (spec §22 B4). Deliberately simple and slow: it enumerates
//! every simple path (and, for flows, every discrete allocation across them),
//! so it cannot share a bug with the Dijkstra/SSP solver. Bounded to small
//! instances; intended for verification, not production use.

use rust_decimal::Decimal;

use crate::cost::ValueUsd;
use crate::domain::Asset;
use crate::graph::{EdgeIdx, LiquidityGraph, NodeIdx};
use crate::solver::SolveError;
use std::collections::HashMap;

/// Every simple (no repeated node) path `source → target`, up to `max_hops`.
pub fn enumerate_paths(
    graph: &LiquidityGraph,
    source: &Asset,
    target: &Asset,
    max_hops: usize,
) -> Result<Vec<Vec<EdgeIdx>>, SolveError> {
    let s = graph
        .node_index(source)
        .ok_or_else(|| SolveError::UnknownSource(source.clone()))?;
    let t = graph
        .node_index(target)
        .ok_or_else(|| SolveError::UnknownTarget(target.clone()))?;

    let mut out = Vec::new();
    let mut visited = vec![false; graph.node_count()];
    let mut path = Vec::new();
    visited[s] = true;
    dfs(graph, s, t, max_hops, &mut visited, &mut path, &mut out);
    Ok(out)
}

fn dfs(
    graph: &LiquidityGraph,
    node: NodeIdx,
    target: NodeIdx,
    max_hops: usize,
    visited: &mut [bool],
    path: &mut Vec<EdgeIdx>,
    out: &mut Vec<Vec<EdgeIdx>>,
) {
    if node == target {
        out.push(path.clone());
        return;
    }
    if path.len() == max_hops {
        return;
    }
    for &e in graph.out_edges(node) {
        let to = graph.edge(e).to;
        if visited[to] {
            continue;
        }
        visited[to] = true;
        path.push(e);
        dfs(graph, to, target, max_hops, visited, path, out);
        path.pop();
        visited[to] = false;
    }
}

/// The exact minimum single-route all-in cost for `size` (∞ if disconnected).
pub fn brute_force_route(
    graph: &LiquidityGraph,
    source: &Asset,
    target: &Asset,
    size: ValueUsd,
    max_hops: usize,
) -> Result<Option<Decimal>, SolveError> {
    let paths = enumerate_paths(graph, source, target, max_hops)?;
    let best = paths
        .iter()
        .map(|p| {
            p.iter()
                .map(|&e| graph.edge(e).curve.all_in(size))
                .sum::<Decimal>()
        })
        .min();
    Ok(best)
}

/// Brute-force optimal *split* cost: enumerate every way to distribute `grains`
/// integer units of `demand` across the enumerated paths, price each edge at its
/// summed flow, and take the minimum. Independent of the SSP solver — it shares
/// no path-selection or augmentation logic — so agreement is real evidence.
pub fn brute_force_flow(
    graph: &LiquidityGraph,
    paths: &[Vec<EdgeIdx>],
    demand: ValueUsd,
    grains: u32,
) -> Decimal {
    let grain = demand.amount() / Decimal::from(grains.max(1));
    let mut best: Option<Decimal> = None;
    let mut alloc = vec![0u32; paths.len()];
    compositions(grains, 0, paths.len(), &mut alloc, &mut |alloc| {
        let mut edge_flow: HashMap<EdgeIdx, Decimal> = HashMap::new();
        for (pi, &count) in alloc.iter().enumerate() {
            if count == 0 {
                continue;
            }
            let value = grain * Decimal::from(count);
            for &e in &paths[pi] {
                *edge_flow.entry(e).or_insert(Decimal::ZERO) += value;
            }
        }
        let total: Decimal = edge_flow
            .iter()
            .map(|(&e, &f)| graph.edge(e).curve.all_in(ValueUsd::usd(f)))
            .sum();
        best = Some(best.map_or(total, |b: Decimal| b.min(total)));
    });
    best.unwrap_or(Decimal::ZERO)
}

/// Enumerate all integer compositions of `total` into `bins` non-negative parts.
fn compositions(
    total: u32,
    bin: usize,
    bins: usize,
    alloc: &mut Vec<u32>,
    visit: &mut impl FnMut(&[u32]),
) {
    if bin + 1 == bins {
        alloc[bin] = total;
        visit(alloc);
        return;
    }
    for give in 0..=total {
        alloc[bin] = give;
        compositions(total - give, bin + 1, bins, alloc, visit);
    }
}
