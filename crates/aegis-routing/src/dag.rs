//! The execution leg DAG (spec §7.3 step 4). A solved set of routes compiles to
//! legs with **explicit `depends_on`** edges — not an implicit `leg_index`
//! order. Within a route the legs chain (sell → bridge → buy); independent
//! routes are parallel branches the executor may run concurrently. A Kahn
//! topological sort yields a valid execution order, and conservation is asserted
//! at compile (value in = value out + fees).

use rust_decimal::Decimal;

use crate::cost::ValueUsd;
use crate::domain::{Asset, EdgeKind};
use crate::graph::LiquidityGraph;
use crate::solver::FlowAllocation;

/// One executable step: traverse a single edge.
#[derive(Debug, Clone)]
pub struct Leg {
    pub id: usize,
    pub kind: EdgeKind,
    pub from: Asset,
    pub to: Asset,
    /// Value entering this leg.
    pub value_in: ValueUsd,
    /// Value leaving this leg (`value_in` − this leg's all-in cost).
    pub value_out: ValueUsd,
    /// Ids of legs that must complete before this one — the real dependency
    /// edges, so the executor never assumes sequential `leg_index` order.
    pub depends_on: Vec<usize>,
}

/// A directed acyclic graph of settlement legs.
#[derive(Debug, Clone, Default)]
pub struct LegDag {
    pub legs: Vec<Leg>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DagError {
    #[error("the leg graph contains a cycle (no topological order)")]
    Cycle,
    #[error("conservation violated: in {input} != out {output} + fees {fees}")]
    Conservation {
        input: Decimal,
        output: Decimal,
        fees: Decimal,
    },
}

impl LegDag {
    /// Compile a set of routed allocations into a leg DAG. Each allocation is one
    /// route carrying `value`; its legs chain in order, and distinct routes get
    /// no dependency between them (parallel branches).
    pub fn compile(graph: &LiquidityGraph, allocations: &[FlowAllocation]) -> Self {
        let mut legs: Vec<Leg> = Vec::new();
        for alloc in allocations {
            let mut value = alloc.value;
            let mut prev: Option<usize> = None;
            for &e in &alloc.legs {
                let edge = graph.edge(e);
                let cost = edge.curve.cost(value).all_in();
                let value_out = ValueUsd::usd(value.amount() - cost);
                let id = legs.len();
                legs.push(Leg {
                    id,
                    kind: edge.kind,
                    from: graph.asset(edge.from).clone(),
                    to: graph.asset(edge.to).clone(),
                    value_in: value,
                    value_out,
                    depends_on: prev.into_iter().collect(),
                });
                prev = Some(id);
                value = value_out;
            }
        }
        Self { legs }
    }

    /// A valid execution order via Kahn's algorithm, or `Cycle` if impossible.
    /// Ready legs are emitted in ascending id for determinism.
    pub fn topological_order(&self) -> Result<Vec<usize>, DagError> {
        let n = self.legs.len();
        let mut indegree = vec![0usize; n];
        let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); n];
        for leg in &self.legs {
            indegree[leg.id] = leg.depends_on.len();
            for &dep in &leg.depends_on {
                dependents[dep].push(leg.id);
            }
        }
        // Min-heap by id (a sorted ready set) keeps the order deterministic.
        let mut ready: Vec<usize> = (0..n).filter(|&i| indegree[i] == 0).collect();
        ready.sort_unstable_by(|a, b| b.cmp(a)); // pop smallest from the back
        let mut order = Vec::with_capacity(n);
        while let Some(id) = ready.pop() {
            order.push(id);
            for &d in &dependents[id] {
                indegree[d] -= 1;
                if indegree[d] == 0 {
                    ready.push(d);
                    ready.sort_unstable_by(|a, b| b.cmp(a));
                }
            }
        }
        if order.len() == n {
            Ok(order)
        } else {
            Err(DagError::Cycle)
        }
    }

    /// Legs with no dependencies — the parallel roots the executor may start at
    /// once.
    pub fn roots(&self) -> Vec<usize> {
        self.legs
            .iter()
            .filter(|l| l.depends_on.is_empty())
            .map(|l| l.id)
            .collect()
    }

    /// Whether `a` must wait for `b` (transitively) — used to assert that two
    /// independent routes carry no false dependency.
    pub fn depends_transitively(&self, a: usize, b: usize) -> bool {
        let mut stack = self.legs[a].depends_on.clone();
        let mut seen = vec![false; self.legs.len()];
        while let Some(x) = stack.pop() {
            if x == b {
                return true;
            }
            if seen[x] {
                continue;
            }
            seen[x] = true;
            stack.extend(self.legs[x].depends_on.iter().copied());
        }
        false
    }

    /// Conservation check (M4): summed route inputs equal summed sink outputs
    /// plus the total fees burned across every leg, to within `tolerance`.
    pub fn check_conservation(&self, tolerance: Decimal) -> Result<(), DagError> {
        // Route heads = legs nothing in this set depends on as a chain parent.
        // A leg is a "tail" if no other leg lists it in depends_on.
        let mut is_parent = vec![false; self.legs.len()];
        for leg in &self.legs {
            for &dep in &leg.depends_on {
                is_parent[dep] = true;
            }
        }
        let input: Decimal = self
            .legs
            .iter()
            .filter(|l| l.depends_on.is_empty())
            .map(|l| l.value_in.amount())
            .sum();
        let output: Decimal = self
            .legs
            .iter()
            .filter(|l| !is_parent[l.id])
            .map(|l| l.value_out.amount())
            .sum();
        let fees: Decimal = self
            .legs
            .iter()
            .map(|l| l.value_in.amount() - l.value_out.amount())
            .sum();
        if (input - output - fees).abs() <= tolerance {
            Ok(())
        } else {
            Err(DagError::Conservation {
                input,
                output,
                fees,
            })
        }
    }
}
