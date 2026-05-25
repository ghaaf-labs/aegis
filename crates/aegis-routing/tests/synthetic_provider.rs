//! Metric 5 — the RouteProvider abstraction. A brand-new rail (a synthetic
//! provider with its own cost-curve type) routes through the *unchanged* solver,
//! proving adding a venue/chain costs 0 core lines (open/closed, M10).
//! Run: `cargo test -p aegis-routing synthetic_provider -- --nocapture`.

mod common;

use aegis_routing::cost::{EdgeCost, ValueUsd};
use aegis_routing::{
    assemble, find_route, min_cost_flow, Asset, ChainId, CostCurve, EdgeKind, FlowConfig,
    GraphBuilder, ProviderId, RouteProvider,
};
use rust_decimal::Decimal;

/// A cost curve type that does NOT exist in the crate — invented entirely here,
/// in test code, to prove the solver consumes arbitrary `CostCurve` impls.
struct TeleportCurve {
    flat_fee: Decimal,
}
impl CostCurve for TeleportCurve {
    fn cost(&self, _size: ValueUsd) -> EdgeCost {
        EdgeCost {
            forwarding_fee: self.flat_fee,
            ..EdgeCost::default()
        }
    }
    fn fingerprint(&self) -> Vec<u8> {
        let mut v = b"teleport".to_vec();
        v.extend_from_slice(&self.flat_fee.serialize());
        v
    }
}

/// A brand-new settlement rail, implemented only here. The solver has no
/// knowledge of it.
struct TeleportProvider;
impl RouteProvider for TeleportProvider {
    fn provider_id(&self) -> ProviderId {
        ProviderId::new("teleport_v0")
    }
    fn contribute(&self, b: &mut GraphBuilder) {
        // A novel rail: directly connect two exotic assets on a brand-new chain.
        b.add_edge(
            Asset::new(ChainId(777), "PORTAL"),
            Asset::new(ChainId(777), "USDC"),
            EdgeKind::AmmSwap,
            self.provider_id(),
            Box::new(TeleportCurve {
                flat_fee: Decimal::new(123, 2),
            }),
        );
    }
}

#[test]
fn synthetic_provider_routes_through_the_unchanged_solver() {
    let provider = TeleportProvider;
    let graph = assemble(&[&provider]);

    let from = Asset::new(ChainId(777), "PORTAL");
    let to = Asset::new(ChainId(777), "USDC");

    // The same find_route / min_cost_flow used for real rails — unchanged.
    let route = find_route(&graph, &from, &to, ValueUsd::usd(Decimal::from(1_000)))
        .unwrap()
        .expect("synthetic rail must be routable");
    assert_eq!(route.hops(), 1);
    assert_eq!(route.cost.forwarding_fee, Decimal::new(123, 2));

    let plan = min_cost_flow(
        &graph,
        &from,
        &to,
        ValueUsd::usd(Decimal::from(1_000)),
        FlowConfig::default(),
    )
    .unwrap();
    assert_eq!(plan.allocations.len(), 1);

    println!(
        "synthetic_provider: new rail '{}' (new chain 777 + new curve type) routed through unchanged solve/flow — 0 core lines changed",
        provider.provider_id().as_str()
    );
}
