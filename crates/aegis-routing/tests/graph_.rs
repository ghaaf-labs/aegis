//! Metric 1 — typed liquidity graph: 100% universe coverage + deterministic
//! fingerprint. Run: `cargo test -p aegis-routing graph_ -- --nocapture`.

mod common;

use aegis_routing::GraphBuilder;
use common::{add_amm, add_cctp, three_chain_graph, three_chain_universe};

#[test]
fn graph_covers_every_universe_asset_with_no_drops_or_dupes() {
    let g = three_chain_graph();
    let universe = three_chain_universe();

    // Every declared (chain, token) is a node …
    let mut missing = Vec::new();
    for a in &universe {
        if !g.contains(a) {
            missing.push(a.clone());
        }
    }
    assert!(missing.is_empty(), "uncovered assets: {missing:?}");

    // … and there are no extra / duplicated nodes (bijection universe → nodes).
    assert_eq!(
        g.node_count(),
        universe.len(),
        "node count must equal the distinct-asset count exactly"
    );

    let covered = universe.iter().filter(|a| g.contains(a)).count();
    println!(
        "graph_ coverage: {covered}/{} assets (100%), 0 dropped, 0 duplicated; {} edges",
        universe.len(),
        g.edge_count()
    );
    assert_eq!(covered, universe.len());
}

#[test]
fn graph_fingerprint_is_deterministic_across_build_order() {
    // Build the SAME graph two ways, inserting markets in a different order.
    let mut b1 = GraphBuilder::new();
    add_amm(&mut b1, 6, "ETH", 5_000_000, 5, 40);
    add_cctp(&mut b1, 26, 6, 1, 30);
    add_amm(&mut b1, 26, "USYC", 10_000_000, 2, 10);
    let g1 = b1.build();

    let mut b2 = GraphBuilder::new();
    add_amm(&mut b2, 26, "USYC", 10_000_000, 2, 10);
    add_amm(&mut b2, 6, "ETH", 5_000_000, 5, 40);
    add_cctp(&mut b2, 26, 6, 1, 30);
    let g2 = b2.build();

    println!("graph_ fingerprint #1: {}", g1.fingerprint_hex());
    println!("graph_ fingerprint #2: {}", g2.fingerprint_hex());
    assert_eq!(
        g1.fingerprint(),
        g2.fingerprint(),
        "identical graphs must fingerprint identically regardless of build order"
    );
}

#[test]
fn graph_fingerprint_changes_when_topology_changes() {
    let g1 = three_chain_graph();
    let mut b = GraphBuilder::new();
    add_amm(&mut b, 6, "ETH", 5_000_000, 5, 40);
    let g2 = b.build();
    assert_ne!(
        g1.fingerprint(),
        g2.fingerprint(),
        "a different graph must produce a different fingerprint"
    );
    println!("graph_ fingerprint distinguishes distinct topologies: OK");
}
