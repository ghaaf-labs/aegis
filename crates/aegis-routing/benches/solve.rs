//! M6 latency benchmark — route planning across node scales {10, 50, 100, 500,
//! 1000}. "Planning" = the full call the API makes: find the min-cost route,
//! solve the split flow, and compile the leg DAG. Run: `cargo bench -p
//! aegis-routing`.

use std::hint::black_box;

use aegis_routing::dag::LegDag;
use aegis_routing::fixtures::{cross_chain_query, scale_graph};
use aegis_routing::{find_route, min_cost_flow, FlowConfig, ValueUsd};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rust_decimal::Decimal;

/// One end-to-end planning call.
fn plan(
    graph: &aegis_routing::LiquidityGraph,
    from: &aegis_routing::Asset,
    to: &aegis_routing::Asset,
) {
    let size = ValueUsd::usd(Decimal::from(50_000));
    let route = find_route(graph, from, to, size).unwrap();
    if let Some(r) = route {
        let plan = min_cost_flow(graph, from, to, size, FlowConfig::default()).unwrap();
        let dag = LegDag::compile(graph, &plan.allocations);
        black_box(r.all_in());
        black_box(dag.topological_order().unwrap());
    }
}

fn bench_route_planning(c: &mut Criterion) {
    // (chains, tokens_per_chain) → node count = chains × (tokens+1).
    let scales = [(2u32, 4u32), (5, 9), (10, 9), (10, 49), (10, 99)];
    let mut group = c.benchmark_group("route_planning");
    for (chains, tokens) in scales {
        let nodes = chains * (tokens + 1);
        let graph = scale_graph(chains, tokens);
        let (from, to) = cross_chain_query(chains, tokens);
        group.bench_with_input(BenchmarkId::from_parameter(nodes), &nodes, |b, _| {
            b.iter(|| plan(&graph, &from, &to));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_route_planning);
criterion_main!(benches);
