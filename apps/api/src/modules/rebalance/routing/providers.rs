//! Builds the live `aegis_routing::LiquidityGraph` from the token registry +
//! `Config`. This is the only place apps/api's domain (ChainKey / TokenSpec /
//! Config) meets the pure routing crate: each Circle rail contributes typed
//! edges, and the crate's solver does the rest. Adding a chain/token/venue means
//! a registry entry resolving an address here — zero solver changes (M10).

use aegis_routing::cost::BridgeComponent;
use aegis_routing::{
    assemble, BridgeCurve, ConstProductCurve, EdgeKind, GraphBuilder, LiquidityGraph, ProviderId,
    RouteProvider,
};
use rust_decimal::Decimal;

use super::asset;
use crate::config::Config;
use crate::domain::chain::ChainKey;
use crate::domain::token::{self, TokenClass, TOKEN_REGISTRY};

// Nominal cost parameters for the planning-time graph. The executor still
// re-quotes every leg live against QuoterV2 / Circle's fee table before signing
// (two-pass exactness, spec §7.3); these seed the *relative* route ranking. The
// in-crate M8 test proves a bucket-calibrated curve tracks the quoter to ≤25 bps.
const AMM_FEE_BPS: i64 = 5;
const CCTP_FEE_BPS: i64 = 1;

fn nominal_depth() -> Decimal {
    Decimal::from(5_000_000)
}
fn amm_gas() -> Decimal {
    Decimal::new(40, 2)
}
fn bridge_gas() -> Decimal {
    Decimal::new(30, 2)
}

fn usdc_on(cfg: &Config, chain: ChainKey) -> bool {
    token::token(token::USDC)
        .and_then(|u| u.address_for(cfg, chain))
        .is_some()
}

/// Assemble the directed liquidity multigraph: AMM markets (token↔USDC per
/// chain where both addresses resolve), USYC subscribe/redeem, and CCTP USDC
/// bridges across the execution chains.
pub fn liquidity_graph(cfg: &Config) -> LiquidityGraph {
    let token_markets = TokenMarketProvider { cfg };
    let cctp = CctpProvider { cfg };
    assemble(&[&token_markets, &cctp])
}

struct TokenMarketProvider<'a> {
    cfg: &'a Config,
}

impl RouteProvider for TokenMarketProvider<'_> {
    fn provider_id(&self) -> ProviderId {
        ProviderId::new("token_markets")
    }

    fn contribute(&self, b: &mut GraphBuilder) {
        let amm = ProviderId::new("uniswap_v3");
        let usyc = ProviderId::new("usyc_teller");

        for spec in TOKEN_REGISTRY {
            if spec.symbol == token::USDC {
                continue;
            }
            // USYC is gated by the kill-switch: while `USYC_ENABLED` is off the
            // Teller is allowlist-only, so the sleeve is track-only — it must not
            // appear as a routable edge or the graph would over-report routability.
            if spec.class == TokenClass::Yield && !self.cfg.usyc_enabled {
                continue;
            }
            for chain in spec.supported_chains() {
                if spec.address_for(self.cfg, chain).is_none() || !usdc_on(self.cfg, chain) {
                    continue;
                }
                let amm_curve = || {
                    Box::new(ConstProductCurve::new(
                        nominal_depth(),
                        Decimal::from(AMM_FEE_BPS),
                        amm_gas(),
                    ))
                };
                // USYC parks/redeems through the Hashnote Teller; everything else
                // (volatiles + EURC) trades on a DEX. Both are token↔USDC edges;
                // only the EdgeKind + provider differ — the solver treats them uniformly.
                let (buy_kind, sell_kind, provider) = if spec.class == TokenClass::Yield {
                    (EdgeKind::UsycSubscribe, EdgeKind::UsycRedeem, usyc.clone())
                } else {
                    (EdgeKind::AmmSwap, EdgeKind::AmmSwap, amm.clone())
                };
                b.add_edge(
                    asset(token::USDC, chain),
                    asset(spec.symbol, chain),
                    buy_kind,
                    provider.clone(),
                    amm_curve(),
                );
                b.add_edge(
                    asset(spec.symbol, chain),
                    asset(token::USDC, chain),
                    sell_kind,
                    provider,
                    amm_curve(),
                );
            }
        }
    }
}

struct CctpProvider<'a> {
    cfg: &'a Config,
}

impl RouteProvider for CctpProvider<'_> {
    fn provider_id(&self) -> ProviderId {
        ProviderId::new("cctp_v2")
    }

    fn contribute(&self, b: &mut GraphBuilder) {
        // CCTP USDC across every execution chain with a resolved USDC address.
        let chains: Vec<ChainKey> = ChainKey::ALL
            .iter()
            .copied()
            .filter(|c| c.is_execution() && usdc_on(self.cfg, *c))
            .collect();
        for &from in &chains {
            for &to in &chains {
                if from == to {
                    continue;
                }
                b.add_edge(
                    asset(token::USDC, from),
                    asset(token::USDC, to),
                    EdgeKind::CctpStandard,
                    self.provider_id(),
                    Box::new(BridgeCurve::new(
                        Decimal::from(CCTP_FEE_BPS),
                        Decimal::ZERO,
                        bridge_gas(),
                        BridgeComponent::Bridge,
                    )),
                );
            }
        }
    }
}
