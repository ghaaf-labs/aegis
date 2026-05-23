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
//!   `BASE_NATIVE_SYMBOLS` lands on Base (Uniswap V3 / Aerodrome venue). EURC is
//!   Base-native here — the EUR sleeve trades on the permissionless USDC/EURC
//!   pool, superseding the KYB-gated Arc StableFX rail.
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
    let idle_total: f64 = input.usdc_per_chain.values().copied().sum();

    // First-deploy: a freshly-funded user has zero invested positions but real
    // USDC sitting in Gateway. `portfolio_value_usd` is the planning basis
    // after idle USDC is included, so detect this from current exposure rather
    // than total basis. Otherwise a target USDC sleeve becomes a bogus
    // USDC->USDC buy leg.
    let has_current_exposure = input
        .current_weights
        .values()
        .any(|w| (w * input.portfolio_value_usd).abs() > input.dust_threshold_usd);
    let first_deploy = !has_current_exposure
        && idle_total > input.dust_threshold_usd
        && !input.target_weights.is_empty();

    let deltas_source = if first_deploy {
        first_deploy_deltas(input, idle_total)
    } else {
        symbol_deltas(input)
    };

    // "Let winners run" — regime-aware asymmetric drift bands. A winner that
    // grew above its target produces a SELL delta (weight_drift < 0); in
    // `risk_on` we widen its band so we don't trim a rallying position too
    // eagerly, and in `risk_off` we tighten it to de-risk sooner. Buys (adding
    // to underweight sleeves, weight_drift > 0) always use the base threshold.
    // First-deploy is exempt (every leg should fire).
    let (sell_band, buy_band) = if first_deploy {
        (0.0, 0.0)
    } else {
        match input.regime.as_deref() {
            Some("risk_on") => (input.drift_threshold * 2.0, input.drift_threshold),
            Some("risk_off") => (input.drift_threshold * 0.5, input.drift_threshold),
            _ => (input.drift_threshold, input.drift_threshold),
        }
    };
    let mut deltas: Vec<SymbolDelta> = deltas_source
        .into_iter()
        // USDC is the settlement unit, not a tradeable position. A USDC weight
        // delta is absorbed by the other legs (buys consume USDC, sells produce
        // it), never its own USDC->USDC swap. `first_deploy_deltas` already drops
        // it; `symbol_deltas` does not, so an over-weight USDC sleeve would emit a
        // bogus self-swap the adapter rejects ("USDC<->token swaps only").
        .filter(|d| !d.symbol.eq_ignore_ascii_case("USDC"))
        .filter(|d| {
            let band = if d.weight_drift < 0.0 {
                sell_band
            } else {
                buy_band
            };
            d.weight_drift.abs() >= band
        })
        .filter(|d| d.value_delta_usd.abs() >= input.dust_threshold_usd)
        .collect();

    if deltas.is_empty() {
        return Vec::new();
    }

    // Sells first (negative deltas) so they free up USDC for same-plan buys.
    // Among buys: USYC is routed *last*. Its final leg calls an external
    // integration (Hashnote Teller) — when that reverts on testnet the executor
    // halts the plan, so putting it at the tail keeps a Teller failure from
    // blocking the BTC/ETH/SOL/EURC legs. EURC now trades on the Base DEX like
    // the volatiles, so it is no longer deferred.
    deltas.sort_by(|a, b| {
        use std::cmp::Ordering;
        let a_sell = a.value_delta_usd < 0.0;
        let b_sell = b.value_delta_usd < 0.0;
        match (a_sell, b_sell) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (true, true) => a
                .value_delta_usd
                .partial_cmp(&b.value_delta_usd)
                .unwrap_or(Ordering::Equal),
            (false, false) => {
                let a_yield = matches!(a.symbol.as_str(), "USYC");
                let b_yield = matches!(b.symbol.as_str(), "USYC");
                match (a_yield, b_yield) {
                    (true, false) => Ordering::Greater,
                    (false, true) => Ordering::Less,
                    _ => a
                        .value_delta_usd
                        .partial_cmp(&b.value_delta_usd)
                        .unwrap_or(Ordering::Equal),
                }
            }
        }
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
        append_buy_legs(&mut legs, &mut next_idx, d, &mut usdc_pool, &input.prices);
    }

    legs
}

#[derive(Debug, Clone)]
struct SymbolDelta {
    symbol: String,
    weight_drift: f64,
    value_delta_usd: f64,
}

/// First-deploy deltas: treat the idle USDC pool as the deployable capital and
/// emit a buy for every non-USDC target symbol weighted at `target * idle`.
/// USDC is excluded because it's the source of funds — no swap needed.
fn first_deploy_deltas(input: &PlanInput, idle_total: f64) -> Vec<SymbolDelta> {
    let mut symbols: Vec<&String> = input.target_weights.keys().collect();
    symbols.sort();
    symbols
        .into_iter()
        .filter(|s| s.as_str() != "USDC")
        .map(|symbol| {
            let target = input.target_weights.get(symbol).copied().unwrap_or(0.0);
            SymbolDelta {
                symbol: symbol.clone(),
                weight_drift: target,
                value_delta_usd: target * idle_total,
            }
        })
        .collect()
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
        // EURC sells route through the Base USDC/EURC DEX pool, not Arc StableFX.
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
    prices: &HashMap<String, f64>,
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

    // Tokens acquired via an AMM swap (volatiles + EURC, which now trades on the
    // Base USDC/EURC pool). These get a same-chain swap for the local portion and
    // a cross-chain hook swap for the bridged portion. Only USYC (Teller park)
    // and USDC (no swap) take the special stable final-leg path below.
    let is_swap_acquired = !matches!(d.symbol.as_str(), "USYC" | "USDC");

    // Bridge any shortfall from the chain holding the most idle USDC (greedy,
    // single-source). With two chains this is just "the other chain"; with N it
    // picks the richest non-target source so one burn+mint covers the shortfall
    // when possible. Splitting a shortfall across multiple source chains is left
    // to the durable saga (out of scope here). Ties break on `as_str()` so the
    // plan stays deterministic.
    let other_chain = if to_acquire > 0.0 {
        usdc_pool
            .iter()
            .filter(|(chain, bal)| **chain != target_chain && **bal > 0.0)
            .max_by(|(a_chain, a_bal), (b_chain, b_bal)| {
                a_bal
                    .partial_cmp(b_bal)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| b_chain.as_str().cmp(a_chain.as_str()))
            })
            .map(|(chain, _)| *chain)
    } else {
        None
    };
    let to_bridge = match other_chain {
        Some(chain) if to_acquire > 0.0 => {
            let available_other = usdc_pool.get(&chain).copied().unwrap_or(0.0);
            available_other.min(to_acquire)
        }
        _ => 0.0,
    };

    // For swap-acquired assets in a mixed buy (some USDC already local on target
    // chain, some bridged from the other chain) the bridge+hook only swaps the
    // bridged amount — the local portion still needs its own swap leg. Older code
    // returned early after the hook and silently dropped the local portion.
    if is_swap_acquired && used_local > 0.0 {
        let min_out = prices.get(&d.symbol).map(|&price| {
            if price > 0.0 {
                (used_local / price) * 0.95
            } else {
                0.0
            }
        });
        legs.push(PlannedLeg {
            leg_index: *next_idx,
            kind: LegKind::LocalSwap,
            src_chain: Some(target_chain),
            dest_chain: Some(target_chain),
            src_symbol: Some("USDC".into()),
            dest_symbol: Some(d.symbol.clone()),
            amount_usdc: used_local,
            min_out,
        });
        *next_idx += 1;
    }

    if let (Some(source_chain), true) = (other_chain, to_bridge > 0.0) {
        // Swap-acquired (volatiles + EURC): attach hook params to the burn so
        // the destination RebalanceExecutor swaps USDC→asset atomically with the
        // mint. USYC: plain USDC bridge; a separate special leg below handles the
        // final park on the destination chain.
        let (burn_dest_symbol, burn_min_out) = if is_swap_acquired {
            let min_out = prices.get(&d.symbol).map(|&price| {
                if price > 0.0 {
                    (to_bridge / price) * 0.95
                } else {
                    0.0
                }
            });
            (Some(d.symbol.clone()), min_out)
        } else {
            (Some("USDC".into()), None)
        };

        legs.push(PlannedLeg {
            leg_index: *next_idx,
            kind: LegKind::CrossChainBurn,
            src_chain: Some(source_chain),
            dest_chain: Some(target_chain),
            src_symbol: Some("USDC".into()),
            dest_symbol: burn_dest_symbol,
            amount_usdc: to_bridge,
            min_out: burn_min_out,
        });
        *next_idx += 1;

        legs.push(PlannedLeg {
            leg_index: *next_idx,
            kind: LegKind::CrossChainMint,
            src_chain: Some(source_chain),
            dest_chain: Some(target_chain),
            src_symbol: Some("USDC".into()),
            dest_symbol: Some("USDC".into()),
            amount_usdc: to_bridge,
            min_out: None,
        });
        *next_idx += 1;

        *usdc_pool.entry(source_chain).or_insert(0.0) -= to_bridge;
    }

    if is_swap_acquired {
        // Swap-acquired final leg(s) already emitted above (local_swap for the
        // used_local portion + cross-chain hook for the bridged portion).
        return;
    }

    // Stable final leg: park USDC → USYC, or a local USDC swap. Amount =
    // used_local + to_bridge; the bridged USDC has arrived on the target chain
    // via the mint above. (EURC no longer reaches here — it is swap-acquired.)
    let consumed = used_local + to_bridge;
    if consumed <= 0.0 {
        return;
    }
    let kind = match d.symbol.as_str() {
        "USYC" => LegKind::ParkUsyc,
        _ => LegKind::LocalSwap,
    };

    let min_out = prices.get(&d.symbol).map(|&price| {
        if price > 0.0 {
            (consumed / price) * 0.95
        } else {
            0.0
        }
    });

    legs.push(PlannedLeg {
        leg_index: *next_idx,
        kind,
        src_chain: Some(target_chain),
        dest_chain: Some(target_chain),
        src_symbol: Some("USDC".into()),
        dest_symbol: Some(d.symbol.clone()),
        amount_usdc: consumed,
        min_out,
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
            prices: HashMap::new(),
            regime: None,
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
            prices: HashMap::new(),
            regime: None,
        };
        // Both deltas are $10 — below the $50 dust floor.
        assert!(plan_legs(&i).is_empty());
    }

    #[test]
    fn risk_on_lets_winners_run() {
        // BTC sits 8% above target (a winner). Neutral trims it; risk_on's
        // widened sell band (2x = 10%) leaves it to run.
        let mut i = input(
            10_000.0,
            &[("BTC", 0.58), ("USDC", 0.42)],
            &[("BTC", 0.50), ("USDC", 0.50)],
            0.0,
            0.0,
        );
        let trims_btc =
            |legs: &[PlannedLeg]| legs.iter().any(|l| l.src_symbol.as_deref() == Some("BTC"));
        assert!(trims_btc(&plan_legs(&i)), "neutral should trim the winner");
        i.regime = Some("risk_on".into());
        assert!(
            !trims_btc(&plan_legs(&i)),
            "risk_on should let the winner run (no BTC trim)"
        );
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
    fn over_weight_usdc_never_self_swaps() {
        // Current 100% USDC, target 60% USDC / 40% ETH: the only action is one
        // USDC->ETH swap. The 40% USDC reduction is absorbed by that buy and
        // must NOT emit a USDC->USDC leg. Regression for the real-exec failure
        // "swap adapter handles USDC<->token swaps only" (a live Base run halted
        // on a bogus self-swap leg the planner had emitted).
        let i = input(
            100.0,
            &[("USDC", 1.0)],
            &[("USDC", 0.60), ("ETH", 0.40)],
            0.0,
            100.0,
        );
        let legs = plan_legs(&i);
        assert_eq!(legs.len(), 1, "expected exactly one leg, got {legs:?}");
        assert_eq!(legs[0].kind, LegKind::LocalSwap);
        assert_eq!(legs[0].src_symbol.as_deref(), Some("USDC"));
        assert_eq!(legs[0].dest_symbol.as_deref(), Some("ETH"));
        assert!(
            legs.iter()
                .all(|l| !(l.src_symbol.as_deref() == Some("USDC")
                    && l.dest_symbol.as_deref() == Some("USDC"))),
            "no USDC->USDC self-swap may be emitted, got {legs:?}"
        );
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
    fn cross_chain_buy_sources_from_richest_chain() {
        // First-deploy of 100% ETH (Base-native) with idle USDC spread across
        // three chains and none on Base. The bridge must greedily source from
        // the chain holding the most idle USDC (EthSepolia: $3000 > Arc: $1000),
        // not a hardcoded "other chain". No current holdings ⇒ no sell leg
        // injects USDC and skews the comparison.
        let mut usdc_per_chain = HashMap::new();
        usdc_per_chain.insert(ChainKey::Arc, 1_000.0);
        usdc_per_chain.insert(ChainKey::Base, 0.0);
        usdc_per_chain.insert(ChainKey::EthSepolia, 3_000.0);
        let i = PlanInput {
            portfolio_value_usd: 0.0,
            current_weights: weights(&[]),
            target_weights: weights(&[("ETH", 1.0)]),
            usdc_per_chain,
            drift_threshold: 0.05,
            dust_threshold_usd: 5.0,
            prices: HashMap::new(),
            regime: None,
        };
        let legs = plan_legs(&i);
        let burn = legs
            .iter()
            .find(|l| l.kind == LegKind::CrossChainBurn)
            .expect("expected a cross-chain burn for the ETH buy");
        assert_eq!(
            burn.src_chain,
            Some(ChainKey::EthSepolia),
            "burn must source from the richest non-target chain"
        );
        assert_eq!(burn.dest_chain, Some(ChainKey::Base));
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
    fn eurc_buy_routes_through_base_dex_swap() {
        // EURC is now Base-native and acquired via the permissionless USDC/EURC
        // DEX pool, so a buy emits a LocalSwap (USDC→EURC) on Base — never the
        // gated Arc StableFX leg. USDC liquidity on Arc bridges to Base first.
        let i = input(
            10_000.0,
            &[("USYC", 1.0)],
            &[("USYC", 0.80), ("EURC", 0.20)],
            5_000.0,
            0.0,
        );
        let legs = plan_legs(&i);
        let kinds: Vec<LegKind> = legs.iter().map(|l| l.kind).collect();
        assert!(
            !kinds.iter().any(|k| matches!(k, LegKind::FxStablefx)),
            "EURC must not route via Arc StableFX, got {kinds:?}"
        );
        // The EURC acquisition is a swap. With idle USDC only on Arc it bridges
        // to Base and swaps there (CrossChainBurn carries the hook to EURC), so
        // assert the EURC destination lands on Base via a swap-bearing leg.
        let eurc_leg = legs
            .iter()
            .find(|l| l.dest_symbol.as_deref() == Some("EURC"))
            .expect("expected a leg acquiring EURC");
        assert!(matches!(
            eurc_leg.kind,
            LegKind::LocalSwap | LegKind::CrossChainBurn
        ));
        assert_eq!(eurc_leg.dest_chain, Some(ChainKey::Base));
    }

    #[test]
    fn eurc_buy_with_local_base_usdc_is_a_single_local_swap() {
        // With USDC already on Base, the EURC buy is a same-chain LocalSwap.
        let i = input(
            10_000.0,
            &[("USYC", 1.0)],
            &[("USYC", 0.80), ("EURC", 0.20)],
            0.0,
            5_000.0,
        );
        let legs = plan_legs(&i);
        let eurc = legs
            .iter()
            .find(|l| l.dest_symbol.as_deref() == Some("EURC"))
            .expect("expected a EURC buy leg");
        assert_eq!(eurc.kind, LegKind::LocalSwap);
        assert_eq!(eurc.src_symbol.as_deref(), Some("USDC"));
        assert_eq!(eurc.dest_chain, Some(ChainKey::Base));
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
    fn first_deploy_emits_buys_from_idle_usdc() {
        // Fresh portfolio: $0 invested, $200 USDC sitting in Gateway, target
        // 50/30/10/10 BTC/ETH/SOL/USYC. Planner must emit buy legs covering
        // each non-USDC target weighted at target*idle.
        let i = input(
            0.0,
            &[],
            &[("BTC", 0.50), ("ETH", 0.30), ("SOL", 0.10), ("USYC", 0.10)],
            50.0,
            150.0,
        );
        let legs = plan_legs(&i);
        assert!(!legs.is_empty(), "first-deploy must produce legs");
        let total_buy_usdc: f64 = legs
            .iter()
            .filter(|l| {
                matches!(
                    l.kind,
                    LegKind::LocalSwap | LegKind::ParkUsyc | LegKind::CrossChainBurn
                )
            })
            .map(|l| l.amount_usdc)
            .sum();
        assert!(
            (total_buy_usdc - 200.0).abs() < 0.01,
            "buy legs must consume ~$200 of idle USDC, got {total_buy_usdc}"
        );
        // USDC is not bought from USDC — skip.
        assert!(legs.iter().all(|l| l.dest_symbol.as_deref() != Some("USDC")
            || matches!(l.kind, LegKind::CrossChainMint)));
    }

    #[test]
    fn first_deploy_keeps_usdc_target_as_cash_reserve() {
        let i = input(
            100.0,
            &[],
            &[("EURC", 0.10), ("USDC", 0.70), ("USYC", 0.20)],
            40.0,
            60.0,
        );
        let legs = plan_legs(&i);
        assert!(
            legs.iter()
                .all(|l| !(l.src_symbol.as_deref() == Some("USDC")
                    && l.dest_symbol.as_deref() == Some("USDC")
                    && matches!(l.kind, LegKind::LocalSwap))),
            "USDC target sleeve must stay reserve cash, got {legs:?}"
        );
        let routed: f64 = legs
            .iter()
            .filter(|l| l.kind != LegKind::CrossChainMint)
            .map(|l| l.amount_usdc)
            .sum();
        assert!(
            (routed - 30.0).abs() < 0.01,
            "only non-USDC targets should consume wallet USDC, got {routed}"
        );
        assert!(
            legs.iter()
                .any(|l| l.dest_symbol.as_deref() == Some("EURC")),
            "EURC target should still produce a buy leg (via the Base DEX swap), got {legs:?}"
        );
        assert!(
            legs.iter()
                .any(|l| l.dest_symbol.as_deref() == Some("USYC")),
            "USYC target should still route through park leg, got {legs:?}"
        );
    }

    #[test]
    fn partial_deploy_includes_idle_usdc_in_target_basis() {
        let i = input(
            106.14,
            &[
                ("SOL", 0.099),
                ("ETH", 0.298),
                ("BTC", 0.199),
                ("USYC", 0.0),
            ],
            &[("BTC", 0.50), ("ETH", 0.30), ("SOL", 0.10), ("USYC", 0.10)],
            16.98,
            25.48,
        );
        let legs = plan_legs(&i);
        assert!(
            legs.iter().any(|l| l.dest_symbol.as_deref() == Some("BTC")),
            "idle USDC should produce a BTC buy leg, got {legs:?}"
        );
        assert!(
            legs.iter()
                .any(|l| l.dest_symbol.as_deref() == Some("USYC")),
            "idle USDC should produce a USYC park leg, got {legs:?}"
        );
        let routed: f64 = legs
            .iter()
            .filter(|l| l.kind != LegKind::CrossChainMint)
            .map(|l| l.amount_usdc)
            .sum();
        assert!(
            (routed - 42.46).abs() < 0.25,
            "partial deploy should route the idle USDC, got {routed}"
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
        // With the CCTP + Hook design, the final acquisition of a volatile asset
        // on another chain is expressed via the CrossChainBurn leg (which carries
        // the hook parameters: dest_symbol = "ETH", min_out for ETH).
        // The last leg in the plan for this ETH buy is the CrossChainMint (USDC arrival).
        let eth_legs: Vec<_> = legs
            .iter()
            .filter(|l| {
                l.dest_symbol.as_deref() == Some("ETH") || l.kind == LegKind::CrossChainMint
            })
            .collect();
        assert!(
            eth_legs
                .iter()
                .any(|l| l.dest_symbol.as_deref() == Some("ETH")),
            "expected a CrossChainBurn with dest_symbol=ETH for the hook swap"
        );
    }
}
