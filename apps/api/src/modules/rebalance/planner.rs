//! Pure-function rebalance planner.
//!
//! Given the user's portfolio (current allocation + goal target), Gateway
//! unified USDC across chains, and a drift threshold, produce a minimum
//! sequence of legs that brings the portfolio to its target. Output is a
//! deterministic `Vec<PlannedLeg>` — no DB writes, no SSE side effects.
//!
//! Decisions encoded here:
//!
//! - Symbols in `ARC_NATIVE_SYMBOLS` land on Arc; anything in
//!   `BASE_NATIVE_SYMBOLS` lands on Base (Uniswap V3 venue).
//! - A buy that needs USDC on Base while liquidity is on Arc emits a
//!   `cross_chain_burn` + `cross_chain_mint` pair instead of two legs.
//! - Sells route through `redeem_usyc` (USYC → USDC) or a local swap into USDC
//!   first; the planner never produces a swap directly between two non-USDC
//!   assets — every leg stages through USDC for auditability.
//! - Drifts smaller than `drift_threshold` are dropped before legs are emitted.
//! - Dust deltas (USD value < `dust_threshold_usd`) are dropped.
//!
//! Per-user signal: `current_weights` and `target_weights` come from each
//! user's portfolio — the planner respects each user's goal verbatim.

use std::collections::HashMap;

use super::models::{
    ChainKey, LegKind, PlanInput, PlannedLeg, ARC_NATIVE_SYMBOLS, BASE_NATIVE_SYMBOLS,
};

/// Compute the minimum set of legs to bring `current_weights` to
/// `target_weights`. Returns an empty Vec if no leg exceeds the drift / dust
/// thresholds — that's the no-op signal callers expect.
pub fn plan_legs(input: &PlanInput) -> Vec<PlannedLeg> {
    let mut deltas: Vec<SymbolDelta> = symbol_deltas(input)
        .into_iter()
        .filter(|d| d.weight_drift.abs() >= input.drift_threshold)
        .filter(|d| d.value_delta_usd.abs() >= input.dust_threshold_usd)
        .collect();

    if deltas.is_empty() {
        return Vec::new();
    }

    // Sells first — they free up USDC that buys can spend in the same plan.
    deltas.sort_by(|a, b| {
        a.value_delta_usd
            .partial_cmp(&b.value_delta_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut legs: Vec<PlannedLeg> = Vec::new();
    let mut next_idx: i32 = 0;
    let mut usdc_pool: HashMap<ChainKey, f64> = input.usdc_per_chain.clone();

    for d in &deltas {
        if d.value_delta_usd < 0.0 {
            // Sell: convert non-USDC asset back to USDC on its native chain.
            append_sell_legs(&mut legs, &mut next_idx, d, &mut usdc_pool);
        }
    }

    for d in deltas.iter().filter(|d| d.value_delta_usd > 0.0) {
        append_buy_legs(&mut legs, &mut next_idx, d, &mut usdc_pool);
    }

    legs
}

#[derive(Debug, Clone)]
struct SymbolDelta {
    symbol: String,
    weight_drift: f64,
    value_delta_usd: f64,
}

fn symbol_deltas(input: &PlanInput) -> Vec<SymbolDelta> {
    let mut symbols: Vec<String> = input
        .target_weights
        .keys()
        .chain(input.current_weights.keys())
        .cloned()
        .collect();
    symbols.sort();
    symbols.dedup();
    symbols
        .into_iter()
        .map(|symbol| {
            let current = input.current_weights.get(&symbol).copied().unwrap_or(0.0);
            let target = input.target_weights.get(&symbol).copied().unwrap_or(0.0);
            let weight_drift = target - current;
            let value_delta_usd = weight_drift * input.portfolio_value_usd;
            SymbolDelta {
                symbol,
                weight_drift,
                value_delta_usd,
            }
        })
        .collect()
}

fn native_chain(symbol: &str) -> ChainKey {
    if ARC_NATIVE_SYMBOLS.contains(&symbol) {
        ChainKey::Arc
    } else if BASE_NATIVE_SYMBOLS.contains(&symbol) {
        ChainKey::Base
    } else {
        // Default: USDC stays where it lives; anything unknown lands on Base.
        ChainKey::Base
    }
}

fn append_sell_legs(
    legs: &mut Vec<PlannedLeg>,
    next_idx: &mut i32,
    d: &SymbolDelta,
    usdc_pool: &mut HashMap<ChainKey, f64>,
) {
    let amount = d.value_delta_usd.abs();
    let chain = native_chain(&d.symbol);
    let kind = match d.symbol.as_str() {
        "USYC" => LegKind::RedeemUsyc,
        "EURC" => LegKind::FxStablefx,
        _ => LegKind::LocalSwap,
    };
    legs.push(PlannedLeg {
        leg_index: *next_idx,
        kind,
        src_chain: Some(chain),
        dest_chain: Some(chain),
        src_symbol: Some(d.symbol.clone()),
        dest_symbol: Some("USDC".into()),
        amount_usdc: amount,
        min_out: None,
    });
    *next_idx += 1;
    *usdc_pool.entry(chain).or_insert(0.0) += amount;
}

fn append_buy_legs(
    legs: &mut Vec<PlannedLeg>,
    next_idx: &mut i32,
    d: &SymbolDelta,
    usdc_pool: &mut HashMap<ChainKey, f64>,
) {
    let amount = d.value_delta_usd;
    let target_chain = native_chain(&d.symbol);
    let available_on_target = usdc_pool.get(&target_chain).copied().unwrap_or(0.0);

    let mut to_acquire = amount;
    let used_local = available_on_target.min(to_acquire);
    if used_local > 0.0 {
        *usdc_pool.entry(target_chain).or_insert(0.0) -= used_local;
        to_acquire -= used_local;
    }

    // Bridge any shortfall from the other chain.
    let other_chain = if target_chain == ChainKey::Arc {
        ChainKey::Base
    } else {
        ChainKey::Arc
    };
    if to_acquire > 0.0 {
        let available_other = usdc_pool.get(&other_chain).copied().unwrap_or(0.0);
        let to_bridge = available_other.min(to_acquire);
        if to_bridge > 0.0 {
            legs.push(PlannedLeg {
                leg_index: *next_idx,
                kind: LegKind::CrossChainBurn,
                src_chain: Some(other_chain),
                dest_chain: Some(target_chain),
                src_symbol: Some("USDC".into()),
                dest_symbol: Some("USDC".into()),
                amount_usdc: to_bridge,
                min_out: None,
            });
            *next_idx += 1;
            legs.push(PlannedLeg {
                leg_index: *next_idx,
                kind: LegKind::CrossChainMint,
                src_chain: Some(other_chain),
                dest_chain: Some(target_chain),
                src_symbol: Some("USDC".into()),
                dest_symbol: Some("USDC".into()),
                amount_usdc: to_bridge,
                min_out: None,
            });
            *next_idx += 1;
            *usdc_pool.entry(other_chain).or_insert(0.0) -= to_bridge;
        }
    }

    // Finally, the swap on the destination chain.
    let kind = match d.symbol.as_str() {
        "USYC" => LegKind::ParkUsyc,
        "EURC" => LegKind::FxStablefx,
        _ => LegKind::LocalSwap,
    };
    legs.push(PlannedLeg {
        leg_index: *next_idx,
        kind,
        src_chain: Some(target_chain),
        dest_chain: Some(target_chain),
        src_symbol: Some("USDC".into()),
        dest_symbol: Some(d.symbol.clone()),
        amount_usdc: amount,
        // Planner does not have Uniswap quoter access — executor fills minOut
        // before submitting the transaction.
        min_out: None,
    });
    *next_idx += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weights(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| ((*k).to_string(), *v)).collect()
    }

    fn input(
        portfolio_value: f64,
        current: &[(&str, f64)],
        target: &[(&str, f64)],
        arc_usdc: f64,
        base_usdc: f64,
    ) -> PlanInput {
        let mut usdc_per_chain = HashMap::new();
        usdc_per_chain.insert(ChainKey::Arc, arc_usdc);
        usdc_per_chain.insert(ChainKey::Base, base_usdc);
        PlanInput {
            portfolio_value_usd: portfolio_value,
            current_weights: weights(current),
            target_weights: weights(target),
            usdc_per_chain,
            drift_threshold: 0.05,
            dust_threshold_usd: 5.0,
        }
    }

    #[test]
    fn no_op_when_within_drift() {
        let i = input(
            10_000.0,
            &[("BTC", 0.50), ("ETH", 0.50)],
            &[("BTC", 0.52), ("ETH", 0.48)],
            0.0,
            0.0,
        );
        assert!(plan_legs(&i).is_empty());
    }

    #[test]
    fn dust_is_dropped() {
        let i = PlanInput {
            portfolio_value_usd: 100.0,
            current_weights: weights(&[("BTC", 0.95)]),
            target_weights: weights(&[("BTC", 0.85), ("ETH", 0.10)]),
            usdc_per_chain: HashMap::new(),
            drift_threshold: 0.01,
            dust_threshold_usd: 50.0,
        };
        // Both deltas are $10 — below the $50 dust floor.
        assert!(plan_legs(&i).is_empty());
    }

    #[test]
    fn single_chain_buy_uses_local_usdc() {
        let i = input(
            10_000.0,
            &[("BTC", 0.50), ("ETH", 0.50)],
            &[("BTC", 0.40), ("ETH", 0.60)],
            0.0,
            5_000.0,
        );
        let legs = plan_legs(&i);
        // Sell BTC → USDC on Base, buy ETH → USDC on Base. No cross-chain.
        assert_eq!(legs.len(), 2);
        assert!(legs.iter().all(|l| l.src_chain == Some(ChainKey::Base)));
        assert!(legs
            .iter()
            .all(|l| !matches!(l.kind, LegKind::CrossChainBurn | LegKind::CrossChainMint)));
    }

    #[test]
    fn cross_chain_buy_emits_burn_and_mint() {
        // Arc has USDC, target asset lives on Base, no Base USDC available.
        let i = input(
            10_000.0,
            &[("USYC", 1.0)],
            &[("USYC", 0.50), ("ETH", 0.50)],
            5_000.0,
            0.0,
        );
        let legs = plan_legs(&i);
        let kinds: Vec<LegKind> = legs.iter().map(|l| l.kind).collect();
        assert!(
            kinds.contains(&LegKind::CrossChainBurn) && kinds.contains(&LegKind::CrossChainMint),
            "expected burn+mint, got {kinds:?}"
        );
        let mint = legs
            .iter()
            .find(|l| l.kind == LegKind::CrossChainMint)
            .unwrap();
        assert_eq!(mint.dest_chain, Some(ChainKey::Base));
    }

    #[test]
    fn park_only_when_target_increases_usyc() {
        let i = input(
            10_000.0,
            &[("USYC", 0.20), ("BTC", 0.80)],
            &[("USYC", 0.50), ("BTC", 0.50)],
            0.0,
            3_000.0,
        );
        let legs = plan_legs(&i);
        let kinds: Vec<LegKind> = legs.iter().map(|l| l.kind).collect();
        assert!(kinds.contains(&LegKind::ParkUsyc));
        assert!(kinds.contains(&LegKind::LocalSwap)); // BTC sell
    }

    #[test]
    fn redeem_only_when_target_decreases_usyc() {
        let i = input(
            10_000.0,
            &[("USYC", 0.50), ("ETH", 0.50)],
            &[("USYC", 0.20), ("ETH", 0.80)],
            0.0,
            0.0,
        );
        let legs = plan_legs(&i);
        let first = &legs[0];
        assert_eq!(first.kind, LegKind::RedeemUsyc);
    }

    #[test]
    fn fx_only_when_eurc_changes() {
        let i = input(
            10_000.0,
            &[("USYC", 1.0)],
            &[("USYC", 0.80), ("EURC", 0.20)],
            5_000.0,
            0.0,
        );
        let legs = plan_legs(&i);
        let kinds: Vec<LegKind> = legs.iter().map(|l| l.kind).collect();
        assert!(kinds.iter().any(|k| matches!(k, LegKind::FxStablefx)));
        let fx = legs.iter().find(|l| l.kind == LegKind::FxStablefx).unwrap();
        assert_eq!(fx.dest_chain, Some(ChainKey::Arc));
    }

    #[test]
    fn weights_are_in_zero_to_one_range_not_percent() {
        // Regression for the H1 audit finding: planner expects fractions.
        // A 50→40 shift on a $10k portfolio is a $1000 sell, not $1000000.
        let i = input(
            10_000.0,
            &[("BTC", 0.50)],
            &[("BTC", 0.40), ("ETH", 0.10)],
            0.0,
            5_000.0,
        );
        let legs = plan_legs(&i);
        let total: f64 = legs.iter().map(|l| l.amount_usdc).sum();
        assert!(
            total < 10_000.0,
            "leg amounts must sum to less than portfolio value (got {total})"
        );
    }

    #[test]
    fn mixed_plan_orders_sells_before_buys() {
        let i = input(
            10_000.0,
            &[("BTC", 0.40), ("USYC", 0.30), ("EURC", 0.30)],
            &[("BTC", 0.50), ("USYC", 0.30), ("ETH", 0.20)],
            500.0,
            0.0,
        );
        let legs = plan_legs(&i);
        // The first leg must be a sell (negative delta -> liquidation).
        let first = &legs[0];
        assert!(matches!(
            first.kind,
            LegKind::LocalSwap | LegKind::FxStablefx | LegKind::RedeemUsyc
        ));
        assert_eq!(first.dest_symbol.as_deref(), Some("USDC"));
        // The final leg must be the ETH buy (positive delta -> acquisition).
        let last = legs.last().unwrap();
        assert_eq!(last.dest_symbol.as_deref(), Some("ETH"));
    }
}
