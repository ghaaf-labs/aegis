//! M6 — deterministic p95 latency gate for route planning. A checked-in harness
//! (the "or equivalent" the spec allows alongside `cargo bench`): it times the
//! full planning call across node scales, computes p95, prints it, and asserts
//! ≤ 50 ms. Run: `cargo test -p aegis-routing --release latency -- --nocapture`.

use std::time::Instant;

use aegis_routing::dag::LegDag;
use aegis_routing::fixtures::{cross_chain_query, scale_graph};
use aegis_routing::{find_route, min_cost_flow, FlowConfig, LiquidityGraph, ValueUsd};
use rust_decimal::Decimal;

fn plan_once(graph: &LiquidityGraph, from: &aegis_routing::Asset, to: &aegis_routing::Asset) {
    let size = ValueUsd::usd(Decimal::from(50_000));
    if find_route(graph, from, to, size).unwrap().is_some() {
        let plan = min_cost_flow(graph, from, to, size, FlowConfig::default()).unwrap();
        let dag = LegDag::compile(graph, &plan.allocations);
        let _ = dag.topological_order().unwrap();
    }
}

fn p95_ms(
    graph: &LiquidityGraph,
    from: &aegis_routing::Asset,
    to: &aegis_routing::Asset,
    iters: usize,
) -> f64 {
    // Warm up.
    for _ in 0..16 {
        plan_once(graph, from, to);
    }
    let mut samples: Vec<f64> = Vec::with_capacity(iters);
    for _ in 0..iters {
        let t = Instant::now();
        plan_once(graph, from, to);
        samples.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((iters as f64) * 0.95).ceil() as usize - 1;
    samples[idx.min(iters - 1)]
}

#[test]
fn latency_route_planning_p95_under_50ms_across_scales() {
    let scales = [(2u32, 4u32), (5, 9), (10, 9), (10, 49), (10, 99)];
    let mut worst = 0.0f64;
    for (chains, tokens) in scales {
        let nodes = chains * (tokens + 1);
        let graph = scale_graph(chains, tokens);
        let (from, to) = cross_chain_query(chains, tokens);
        let p95 = p95_ms(&graph, &from, &to, 400);
        worst = worst.max(p95);
        println!(
            "latency: {nodes:>4} nodes / {} edges → p95 {p95:.3} ms",
            graph.edge_count()
        );
    }
    println!("latency: worst p95 across scales = {worst:.3} ms (cap 50 ms)");
    assert!(
        worst <= 50.0,
        "p95 route planning latency {worst:.3} ms exceeds the 50 ms gate (M6)"
    );
}
