//! Metric 3 — min-cost flow with route splitting: splits when convex impact
//! warrants, captures ≥95% of the achievable saving, and is never worse than the
//! best single route.
//! Run: `cargo test -p aegis-routing mincost_flow -- --nocapture`.

mod common;

use aegis_routing::cost::{BridgeComponent, BridgeCurve, ConstProductCurve};
use aegis_routing::oracle::{brute_force_flow, enumerate_paths};
use aegis_routing::{
    find_route, min_cost_flow, Asset, ChainId, EdgeKind, FlowConfig, GraphBuilder, LiquidityGraph,
    ProviderId, ValueUsd,
};
use common::dec;
use rust_decimal::Decimal;

/// A 2-node graph with `depths.len()` parallel AMM routes S→T (pure convex,
/// no fee/gas), so splitting is governed purely by price-impact convexity.
fn parallel_graph(depths: &[i64]) -> (LiquidityGraph, Asset, Asset) {
    let s = Asset::new(ChainId(0), "X");
    let t = Asset::new(ChainId(0), "USDC");
    let mut b = GraphBuilder::new();
    for (i, &d) in depths.iter().enumerate() {
        b.add_edge(
            s.clone(),
            t.clone(),
            EdgeKind::AmmSwap,
            ProviderId::new(&format!("venue_{i}")),
            Box::new(ConstProductCurve::new(dec(d), Decimal::ZERO, Decimal::ZERO)),
        );
    }
    (b.build(), s, t)
}

/// Two routes that share S→H, then split over parallel H→T venues. A solver
/// that only seeds edge-disjoint paths cannot see the second route because both
/// candidates need the same prefix edge.
fn shared_prefix_graph() -> (LiquidityGraph, Asset, Asset) {
    let s = Asset::new(ChainId(0), "X");
    let h = Asset::new(ChainId(0), "HUB");
    let t = Asset::new(ChainId(0), "USDC");
    let mut b = GraphBuilder::new();
    b.add_edge(
        s.clone(),
        h.clone(),
        EdgeKind::CctpStandard,
        ProviderId::new("shared_prefix"),
        Box::new(BridgeCurve::new(
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            BridgeComponent::Bridge,
        )),
    );
    for i in 0..2 {
        b.add_edge(
            h.clone(),
            t.clone(),
            EdgeKind::AmmSwap,
            ProviderId::new(&format!("venue_{i}")),
            Box::new(ConstProductCurve::new(
                dec(1_000_000),
                Decimal::ZERO,
                Decimal::ZERO,
            )),
        );
    }
    (b.build(), s, t)
}

fn ratio(num: Decimal, den: Decimal) -> Decimal {
    if den == Decimal::ZERO {
        Decimal::ZERO
    } else {
        num / den
    }
}

#[test]
fn mincost_flow_splits_and_captures_the_convex_saving() {
    // Three equal pools: the optimum is an even three-way split.
    let (g, s, t) = parallel_graph(&[1_000_000, 1_000_000, 1_000_000]);
    let demand = ValueUsd::usd(dec(300_000));

    let single = find_route(&g, &s, &t, demand).unwrap().unwrap().all_in();
    let paths = enumerate_paths(&g, &s, &t, 1).unwrap();
    let optimum = brute_force_flow(&g, &paths, demand, 60);
    let plan = min_cost_flow(&g, &s, &t, demand, FlowConfig::default()).unwrap();

    let achievable = single - optimum;
    let captured = single - plan.total_cost;
    let captured_pct = ratio(captured, achievable) * dec(100);

    println!(
        "mincost_flow[3x equal]: single={single} optimum={optimum} solver={} → captured {captured_pct}% of saving, split={}, routes={}",
        plan.total_cost, plan.split, plan.allocations.len()
    );

    assert!(plan.split, "an even three-way split must be taken");
    assert!(
        plan.total_cost <= single,
        "split plan must never be worse than the best single route"
    );
    assert!(
        captured >= achievable * dec(95) / dec(100),
        "must capture ≥95% of the achievable saving (got {captured_pct}%)"
    );
}

#[test]
fn mincost_flow_splits_across_routes_with_a_shared_prefix() {
    let (g, s, t) = shared_prefix_graph();
    let demand = ValueUsd::usd(dec(200_000));

    let single = find_route(&g, &s, &t, demand).unwrap().unwrap().all_in();
    let paths = enumerate_paths(&g, &s, &t, 2).unwrap();
    let optimum = brute_force_flow(&g, &paths, demand, 40);
    let plan = min_cost_flow(&g, &s, &t, demand, FlowConfig::default()).unwrap();

    let achievable = single - optimum;
    let captured = single - plan.total_cost;
    let captured_pct = ratio(captured, achievable) * dec(100);
    println!(
        "mincost_flow[shared prefix]: paths={} single={single} optimum={optimum} solver={} → captured {captured_pct}% split={}",
        paths.len(), plan.total_cost, plan.split
    );

    assert_eq!(paths.len(), 2, "fixture must have two shared-prefix routes");
    assert!(
        plan.split,
        "shared-prefix routes must still be split candidates"
    );
    assert!(
        captured >= achievable * dec(95) / dec(100),
        "must capture ≥95% of shared-prefix saving (got {captured_pct}%)"
    );
}

#[test]
fn mincost_flow_splits_optimally_across_asymmetric_pools() {
    let (g, s, t) = parallel_graph(&[5_000_000, 1_000_000, 500_000]);
    let demand = ValueUsd::usd(dec(200_000));

    let single = find_route(&g, &s, &t, demand).unwrap().unwrap().all_in();
    let paths = enumerate_paths(&g, &s, &t, 1).unwrap();
    let optimum = brute_force_flow(&g, &paths, demand, 60);
    let plan = min_cost_flow(&g, &s, &t, demand, FlowConfig::default()).unwrap();

    let achievable = single - optimum;
    let captured = single - plan.total_cost;
    let captured_pct = ratio(captured, achievable) * dec(100);
    println!(
        "mincost_flow[asym]: single={single} optimum={optimum} solver={} → captured {captured_pct}% of saving, split={}",
        plan.total_cost, plan.split
    );
    assert!(plan.total_cost <= single, "never worse than single route");
    assert!(
        captured >= achievable * dec(95) / dec(100),
        "must capture ≥95% of the achievable saving (got {captured_pct}%)"
    );
}

#[test]
fn mincost_flow_does_not_split_when_one_pool_dominates() {
    // One very deep pool, two shallow ones: splitting only adds impact.
    let (g, s, t) = parallel_graph(&[100_000_000, 10_000, 10_000]);
    let demand = ValueUsd::usd(dec(1_000));
    let single = find_route(&g, &s, &t, demand).unwrap().unwrap().all_in();
    let plan = min_cost_flow(&g, &s, &t, demand, FlowConfig::default()).unwrap();
    println!(
        "mincost_flow[dominant]: single={single} solver={} split={}",
        plan.total_cost, plan.split
    );
    assert!(!plan.split, "must not split when no convex saving exists");
    assert!(plan.total_cost <= single, "never worse than single route");
}

#[test]
fn mincost_flow_is_never_worse_than_single_across_many_instances() {
    let mut worse = 0usize;
    for depths in [
        vec![2_000_000, 2_000_000],
        vec![3_000_000, 1_000_000, 1_000_000],
        vec![10_000_000, 5_000_000, 2_500_000, 1_250_000],
        vec![500_000, 500_000],
        vec![9_000_000, 100_000],
    ] {
        let (g, s, t) = parallel_graph(&depths);
        let demand = ValueUsd::usd(dec(250_000));
        let single = find_route(&g, &s, &t, demand).unwrap().unwrap().all_in();
        let plan = min_cost_flow(&g, &s, &t, demand, FlowConfig::default()).unwrap();
        if plan.total_cost > single {
            worse += 1;
        }
    }
    println!("mincost_flow: {worse} split-worse-than-single cases across 5 instances (must be 0)");
    assert_eq!(
        worse, 0,
        "splitting must never beat itself into a worse plan"
    );
}
