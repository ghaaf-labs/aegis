//! Shared fixtures for the metric tests: small, realistic liquidity graphs
//! built the same way apps/api will (AMM edges token↔USDC per chain, CCTP edges
//! USDC across chains).

#![allow(dead_code)]

use aegis_routing::cost::BridgeComponent;
use aegis_routing::{
    Asset, BridgeCurve, ChainId, ConstProductCurve, EdgeKind, GraphBuilder, LiquidityGraph,
    ProviderId,
};
use rust_decimal::Decimal;

pub const USDC: &str = "USDC";

pub fn dec(n: i64) -> Decimal {
    Decimal::from(n)
}
/// `n` cents as a Decimal dollar amount (e.g. `cents(50)` = $0.50).
pub fn cents(n: i64) -> Decimal {
    Decimal::new(n, 2)
}

pub fn asset(chain: u32, token: &str) -> Asset {
    Asset::new(ChainId(chain), token)
}

/// Add a same-chain AMM market `token ↔ USDC` (both directions) priced by an
/// exact constant-product curve of the given depth.
pub fn add_amm(
    b: &mut GraphBuilder,
    chain: u32,
    token: &str,
    depth_usd: i64,
    fee_bps: i64,
    gas_cents: i64,
) {
    let provider = ProviderId::new("uniswap_v3");
    let curve = || {
        Box::new(ConstProductCurve::new(
            dec(depth_usd),
            dec(fee_bps),
            cents(gas_cents),
        ))
    };
    b.add_edge(
        asset(chain, token),
        asset(chain, USDC),
        EdgeKind::AmmSwap,
        provider.clone(),
        curve(),
    );
    b.add_edge(
        asset(chain, USDC),
        asset(chain, token),
        EdgeKind::AmmSwap,
        provider,
        curve(),
    );
}

/// Add a CCTP USDC bridge between two chains (both directions).
pub fn add_cctp(b: &mut GraphBuilder, a_chain: u32, b_chain: u32, fee_bps: i64, gas_cents: i64) {
    let provider = ProviderId::new("cctp_v2");
    let curve = || {
        Box::new(BridgeCurve::new(
            dec(fee_bps),
            Decimal::ZERO,
            cents(gas_cents),
            BridgeComponent::Bridge,
        ))
    };
    b.add_edge(
        asset(a_chain, USDC),
        asset(b_chain, USDC),
        EdgeKind::CctpStandard,
        provider.clone(),
        curve(),
    );
    b.add_edge(
        asset(b_chain, USDC),
        asset(a_chain, USDC),
        EdgeKind::CctpStandard,
        provider,
        curve(),
    );
}

/// A three-chain, multi-token graph (Arc=26, Base=6, Eth=0) mirroring the real
/// topology: each chain has USDC; tokens trade against their chain's USDC; USDC
/// bridges across all three chains.
pub fn three_chain_graph() -> LiquidityGraph {
    let mut b = GraphBuilder::new();
    // Base (6): the live volatile venue.
    add_amm(&mut b, 6, "ETH", 5_000_000, 5, 40);
    add_amm(&mut b, 6, "cbBTC", 3_000_000, 5, 40);
    add_amm(&mut b, 6, "EURC", 8_000_000, 5, 40);
    // Eth (0).
    add_amm(&mut b, 0, "ETH", 2_000_000, 30, 60);
    // Arc (26): USDC-native; USYC sleeve.
    add_amm(&mut b, 26, "USYC", 10_000_000, 2, 10);
    // Cross-chain USDC.
    add_cctp(&mut b, 26, 6, 1, 30);
    add_cctp(&mut b, 6, 0, 1, 30);
    add_cctp(&mut b, 26, 0, 1, 30);
    b.build()
}

/// The set of `(chain, token)` assets the three-chain graph is expected to
/// cover — the "universe" coverage is checked against.
pub fn three_chain_universe() -> Vec<Asset> {
    vec![
        asset(6, "ETH"),
        asset(6, "cbBTC"),
        asset(6, "EURC"),
        asset(6, USDC),
        asset(0, "ETH"),
        asset(0, USDC),
        asset(26, "USYC"),
        asset(26, USDC),
    ]
}
