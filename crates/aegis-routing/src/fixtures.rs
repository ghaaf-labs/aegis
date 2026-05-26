//! Synthetic graph fixtures for the latency benchmark + scale tests (spec §16,
//! M6/M7). Generates a graph at a chosen scale with the real topology shape: per
//! chain a USDC hub + `tokens_per_chain` AMM markets, and a CCTP USDC ring
//! across chains, so cross-chain planning exercises multi-hop paths.

use rust_decimal::Decimal;

use crate::cost::BridgeComponent;
use crate::{
    Asset, BridgeCurve, ChainId, ConstProductCurve, EdgeKind, GraphBuilder, LiquidityGraph,
    ProviderId,
};

const USDC: &str = "USDC";

/// A graph with `chains × (tokens_per_chain + 1)` nodes.
pub fn scale_graph(chains: u32, tokens_per_chain: u32) -> LiquidityGraph {
    let mut b = GraphBuilder::new();
    let amm = ProviderId::new("uniswap_v3");
    let cctp = ProviderId::new("cctp_v2");

    for c in 0..chains {
        let usdc = Asset::new(ChainId(c), USDC);
        for t in 0..tokens_per_chain {
            let token = Asset::new(ChainId(c), format!("T{c}_{t}").as_str());
            let depth = Decimal::from(1_000_000 + i64::from(t) * 50_000);
            let mk = || {
                Box::new(ConstProductCurve::new(
                    depth,
                    Decimal::from(5),
                    Decimal::new(40, 2),
                ))
            };
            b.add_edge(
                token.clone(),
                usdc.clone(),
                EdgeKind::AmmSwap,
                amm.clone(),
                mk(),
            );
            b.add_edge(usdc.clone(), token, EdgeKind::AmmSwap, amm.clone(), mk());
        }
    }
    // CCTP USDC ring: chain c ↔ chain c+1 (and wrap), so any two chains connect.
    for c in 0..chains {
        let next = (c + 1) % chains;
        if next == c {
            continue;
        }
        let mk = || {
            Box::new(BridgeCurve::new(
                Decimal::from(1),
                Decimal::ZERO,
                Decimal::new(30, 2),
                BridgeComponent::Bridge,
            ))
        };
        b.add_edge(
            Asset::new(ChainId(c), USDC),
            Asset::new(ChainId(next), USDC),
            EdgeKind::CctpStandard,
            cctp.clone(),
            mk(),
        );
        b.add_edge(
            Asset::new(ChainId(next), USDC),
            Asset::new(ChainId(c), USDC),
            EdgeKind::CctpStandard,
            cctp.clone(),
            mk(),
        );
    }
    b.build()
}

/// A representative cross-chain planning query for `scale_graph`: a token on the
/// first chain → a token on the last chain (forces sell → bridge(s) → buy).
pub fn cross_chain_query(chains: u32, tokens_per_chain: u32) -> (Asset, Asset) {
    let last = chains.saturating_sub(1);
    let src_tok = format!("T0_{}", tokens_per_chain / 2);
    let dst_tok = format!("T{last}_{}", tokens_per_chain / 2);
    (
        Asset::new(ChainId(0), src_tok.as_str()),
        Asset::new(ChainId(last), dst_tok.as_str()),
    )
}
