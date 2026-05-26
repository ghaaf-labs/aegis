//! Metric 2 — route finder: min-all-in-cost for every connected pair within
//! ≤0.5% of the brute-force oracle, honest `None` for disconnected pairs.
//! Run: `cargo test -p aegis-routing route_finder -- --nocapture`.

mod common;

use aegis_routing::oracle::brute_force_route;
use aegis_routing::{find_route, GraphBuilder, ValueUsd};
use common::{add_amm, asset, dec, three_chain_graph};
use rust_decimal::Decimal;

#[test]
fn route_finder_matches_oracle_for_every_connected_pair() {
    let g = three_chain_graph();
    let size = ValueUsd::usd(dec(10_000));
    let max_hops = g.node_count();

    let assets: Vec<_> = g.nodes().to_vec();
    let mut pairs_connected = 0usize;
    let mut pairs_disconnected = 0usize;
    let mut worst_gap_bps = Decimal::ZERO;
    let mut false_routes = 0usize;

    for src in &assets {
        for dst in &assets {
            if src == dst {
                continue;
            }
            let oracle = brute_force_route(&g, src, dst, size, max_hops).unwrap();
            let found = find_route(&g, src, dst, size).unwrap();
            match (oracle, found) {
                (Some(opt), Some(route)) => {
                    pairs_connected += 1;
                    // gap = (solver − opt) / opt, in bps. Dijkstra is exact → ~0.
                    let gap_bps = if opt > Decimal::ZERO {
                        (route.all_in() - opt) / opt * dec(10_000)
                    } else {
                        Decimal::ZERO
                    };
                    assert!(
                        gap_bps <= dec(50),
                        "{src:?}->{dst:?}: gap {gap_bps} bps exceeds 0.5% (solver {}, opt {opt})",
                        route.all_in()
                    );
                    if gap_bps > worst_gap_bps {
                        worst_gap_bps = gap_bps;
                    }
                }
                (None, None) => pairs_disconnected += 1,
                (None, Some(_)) => false_routes += 1, // solver invented a route
                (Some(_), None) => panic!("{src:?}->{dst:?}: solver missed a real route"),
            }
        }
    }

    println!(
        "route_finder: {pairs_connected} connected pairs routed, worst optimality gap {worst_gap_bps} bps (cap 50), {pairs_disconnected} disconnected, {false_routes} false routes"
    );
    assert!(pairs_connected > 0, "expected some connected pairs");
    assert_eq!(false_routes, 0, "solver must never invent a route");
    assert!(
        worst_gap_bps <= dec(50),
        "worst gap {worst_gap_bps} bps must be ≤ 0.5%"
    );
}

#[test]
fn route_finder_returns_none_for_a_disconnected_pair() {
    // Add an isolated market on a chain with no bridge to the rest.
    let mut b = GraphBuilder::new();
    add_amm(&mut b, 6, "ETH", 5_000_000, 5, 40); // Base island
    add_amm(&mut b, 99, "LINK", 1_000_000, 5, 40); // unreachable chain 99
    let g = b.build();

    let from = asset(6, "ETH");
    let to = asset(99, "LINK");
    let route = find_route(&g, &from, &to, ValueUsd::usd(dec(1_000))).unwrap();
    assert!(
        route.is_none(),
        "disconnected pair must yield None, got {route:?}"
    );
    println!("route_finder: disconnected pair Base.ETH -> chain99.LINK -> None (honest)");
}
