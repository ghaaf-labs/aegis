use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::modules::rebalance::models::{ChainKey, LegKind};
use crate::modules::rebalance::registry::tokens;

use super::legs::LegRow;

/// A leg whose funds moved but whose final action failed — its asset is stranded
/// as idle USDC in the user's wallet. Used by `remaining_delta_after_strand` to
/// replan the still-needed exposure.
#[derive(Debug, Clone, PartialEq)]
pub struct StrandedLeg {
    pub dest_symbol: String,
    pub amount_usdc: f64,
}

/// The exposure a follow-up rebalance still needs after some legs stranded.
#[derive(Debug, Clone, PartialEq)]
pub struct RemainingDelta {
    pub dest_symbol: String,
    pub amount_usdc: f64,
}

/// Given the original plan and which legs stranded (funds landed as idle USDC
/// instead of reaching their destination asset), compute the per-symbol
/// exposure a follow-up rebalance still needs to acquire.
///
/// Pure: no DB, no side effects — this is the verifiable core of recovery. The
/// returned deltas are *not* auto-executed; they surface for user approval via
/// the same two-gate model. A stranded leg's USDC is already sitting in the
/// wallet, so the follow-up only needs to re-acquire the destination asset for
/// the stranded notional (the bridge/sell portion of the plan has already
/// settled). USDC destinations and non-positive amounts are dropped — there's
/// nothing to re-buy. Same-symbol strands are summed so a split buy that
/// stranded on two chains replans as one delta. Output is sorted by symbol for
/// determinism.
pub fn remaining_delta_after_strand(stranded: &[StrandedLeg]) -> Vec<RemainingDelta> {
    use std::collections::BTreeMap;

    let mut by_symbol: BTreeMap<String, f64> = BTreeMap::new();
    for leg in stranded {
        if leg.amount_usdc <= 0.0 {
            continue;
        }
        if leg.dest_symbol.eq_ignore_ascii_case("USDC") || leg.dest_symbol.is_empty() {
            continue;
        }
        *by_symbol.entry(leg.dest_symbol.clone()).or_insert(0.0) += leg.amount_usdc;
    }

    by_symbol
        .into_iter()
        .map(|(dest_symbol, amount_usdc)| RemainingDelta {
            dest_symbol,
            amount_usdc,
        })
        .collect()
}

/// Whether a failed leg leaves funds stranded as idle USDC in the user's wallet.
///
/// A `cross_chain_mint` whose companion burn already settled means USDC has
/// landed at the destination; if the *acquiring* action (the hook swap on the
/// burn, or a follow-on local swap) then fails, that USDC is stranded — not
/// lost. We mark the leg `stranded_asset` and record the USDC as cash rather
/// than failing the whole plan. A pre-funds-moved failure (e.g. the burn itself
/// reverts, or a local swap reverts before any token leaves the wallet) is a
/// clean halt with nothing stranded.
pub(super) fn leg_strands_funds_on_failure(
    kind: LegKind,
    leg: &LegRow,
    prior_confirmed: &[LegRow],
) -> bool {
    match kind {
        // Plain-bridge baseline: burn → mint → local swap. The mint is the leg
        // that *lands* the destination USDC, so a swap only strands funds when it
        // explicitly depends on a confirmed mint for the same chain. Independent
        // same-chain swaps revert atomically and leave no bridged cash stranded.
        LegKind::LocalSwap => {
            let chain = leg.src_chain.as_deref().and_then(ChainKey::parse);
            chain.is_some_and(|chain| {
                depends_on_confirmed_mint_to_chain(leg, prior_confirmed, chain)
            })
        }
        // A failed mint means the destination USDC never landed (it's still in
        // CCTP transit, recoverable by re-mint via the existing attestation), and
        // a failed burn leaves the source USDC in place — neither leaves idle
        // cash to strand. (A source-burn confirmation alone does NOT mean the
        // mint landed: that is the mint leg's job.)
        LegKind::CrossChainMint | LegKind::CrossChainBurn => false,
        // Park / FX: funds leave the wallet atomically with the acquire, so a
        // revert returns them — nothing stranded.
        LegKind::ParkUsyc | LegKind::RedeemUsyc | LegKind::FxStablefx => false,
    }
}

/// If this leg spends USDC on a chain that a prior confirmed cross-chain mint
/// just delivered USDC to, return `(chain, min_usdc)` the dependent leg must
/// wait to credit before submitting. This is the timing-race guard (B1): a
/// bridge mint reports `CONFIRMED` before Circle's balance indexer reflects the
/// new USDC, so a swap that spends it would fail closed.
///
/// Pure (DB-free) so the dependency decision is unit-testable. Returns `None`
/// when the leg doesn't spend USDC or no explicit mint dependency targeted its
/// spend chain.
pub(super) fn pending_funding_dependency(
    leg: &LegRow,
    confirmed: &[LegRow],
) -> Option<(ChainKey, f64)> {
    // Only a USDC-spending leg (a buy swap, or a burn) can race a fresh mint.
    if leg.src_symbol.as_deref() != Some(tokens::USDC) {
        return None;
    }
    let spend_chain = leg.src_chain.as_deref().and_then(ChainKey::parse)?;
    if !depends_on_confirmed_mint_to_chain(leg, confirmed, spend_chain) {
        return None;
    }
    Some((spend_chain, leg.amount_usdc.to_f64().unwrap_or(0.0)))
}

fn depends_on_confirmed_mint_to_chain(leg: &LegRow, confirmed: &[LegRow], chain: ChainKey) -> bool {
    confirmed.iter().any(|c| {
        c.kind == LegKind::CrossChainMint.as_str()
            && leg.depends_on.contains(&c.leg_index)
            && c.dest_chain.as_deref().and_then(ChainKey::parse) == Some(chain)
    })
}

pub(super) fn protocol_fee_notional_from_legs(legs: &[LegRow]) -> f64 {
    legs.iter()
        .filter(|leg| leg.kind != LegKind::CrossChainMint.as_str())
        .map(|leg| leg.amount_usdc.to_f64().unwrap_or(0.0))
        .sum()
}

/// Deterministic per-leg fingerprint, stamped once at plan creation and
/// persisted in `rebalance_legs.idempotency_key`.
///
/// Its job is the DB-level `(rebalance_id, idempotency_key)` UNIQUE index: if a
/// plan-creation is ever retried for the same logical leg, the same fingerprint
/// collides instead of admitting a duplicate row. The amount is rounded to whole
/// USDC cents so a sub-cent notional re-fetch maps to the same key. (The
/// at-submit, cross-resume dedup against Circle is separate — that uses the
/// stable leg-id-derived key in `circle_exec`, not this column.)
///
/// Shape: `rebalance_id:leg_index:kind:src>dest:rounded_amount`.
pub(super) fn idempotency_key_for_leg(
    rebalance_id: Uuid,
    leg_index: i32,
    kind: &str,
    src_symbol: Option<&str>,
    dest_symbol: Option<&str>,
    amount_usdc: Decimal,
) -> String {
    let src = src_symbol.unwrap_or("none");
    let dest = dest_symbol.unwrap_or("none");
    let rounded_cents = (amount_usdc * Decimal::from(100))
        .round()
        .to_i64()
        .unwrap_or(0);
    format!("{rebalance_id}:{leg_index}:{kind}:{src}>{dest}:{rounded_cents}")
}

#[cfg(test)]
mod tests {
    use rust_decimal::prelude::FromPrimitive;
    use uuid::Uuid;

    use crate::modules::rebalance::models::{ChainKey, LegKind};

    use super::super::legs::test_helpers::{make_leg, make_mint_leg, make_swap_leg};
    use super::*;

    fn strand(dest: &str, amount: f64) -> StrandedLeg {
        StrandedLeg {
            dest_symbol: dest.to_string(),
            amount_usdc: amount,
        }
    }

    fn usd(amount: f64) -> Decimal {
        Decimal::from_f64(amount).unwrap()
    }

    #[test]
    fn protocol_fee_notional_excludes_cctp_mint_receive_side() {
        let legs = vec![
            make_leg(LegKind::CrossChainBurn, 100.0),
            make_leg(LegKind::CrossChainMint, 100.0),
            make_leg(LegKind::LocalSwap, 25.0),
        ];

        assert_eq!(protocol_fee_notional_from_legs(&legs), 125.0);
    }

    #[test]
    fn protocol_fee_notional_counts_single_chain_and_usyc_legs() {
        let legs = vec![
            make_leg(LegKind::ParkUsyc, 50.0),
            make_leg(LegKind::RedeemUsyc, 20.0),
            make_leg(LegKind::FxStablefx, 10.0),
        ];

        assert_eq!(protocol_fee_notional_from_legs(&legs), 80.0);
    }

    // ── Idempotency key derivation ────────────────────────────────────────

    #[test]
    fn idempotency_key_is_deterministic_for_same_leg() {
        let id = Uuid::new_v4();
        let a = idempotency_key_for_leg(id, 2, "local_swap", Some("USDC"), Some("ETH"), usd(600.0));
        let b = idempotency_key_for_leg(id, 2, "local_swap", Some("USDC"), Some("ETH"), usd(600.0));
        assert_eq!(a, b, "same logical leg must derive the same key");
        assert_eq!(a, format!("{id}:2:local_swap:USDC>ETH:60000"));
    }

    #[test]
    fn idempotency_key_rounds_subcent_amount_drift_to_same_key() {
        // A price re-fetch nudges the notional by a fraction of a cent on a
        // resume; the rounded key must still match so we don't double-submit.
        let id = Uuid::new_v4();
        let a =
            idempotency_key_for_leg(id, 0, "local_swap", Some("USDC"), Some("BTC"), usd(600.001));
        let b =
            idempotency_key_for_leg(id, 0, "local_swap", Some("USDC"), Some("BTC"), usd(599.999));
        assert_eq!(a, b, "sub-cent drift must collapse to the same key");
    }

    #[test]
    fn idempotency_key_differs_across_legs_and_plans() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let base =
            idempotency_key_for_leg(id1, 0, "local_swap", Some("USDC"), Some("ETH"), usd(100.0));
        // Different leg index.
        assert_ne!(
            base,
            idempotency_key_for_leg(id1, 1, "local_swap", Some("USDC"), Some("ETH"), usd(100.0))
        );
        // Different kind.
        assert_ne!(
            base,
            idempotency_key_for_leg(
                id1,
                0,
                "cross_chain_burn",
                Some("USDC"),
                Some("ETH"),
                usd(100.0)
            )
        );
        // Different token pair.
        assert_ne!(
            base,
            idempotency_key_for_leg(id1, 0, "local_swap", Some("USDC"), Some("BTC"), usd(100.0))
        );
        // Different amount (≥ 1 cent).
        assert_ne!(
            base,
            idempotency_key_for_leg(id1, 0, "local_swap", Some("USDC"), Some("ETH"), usd(100.5))
        );
        // Different rebalance.
        assert_ne!(
            base,
            idempotency_key_for_leg(id2, 0, "local_swap", Some("USDC"), Some("ETH"), usd(100.0))
        );
    }

    #[test]
    fn idempotency_key_handles_missing_symbols() {
        let id = Uuid::new_v4();
        let k = idempotency_key_for_leg(id, 3, "cross_chain_mint", None, None, usd(250.0));
        assert_eq!(k, format!("{id}:3:cross_chain_mint:none>none:25000"));
    }

    // ── Strand decision (which failures leave funds stranded) ─────────────

    #[test]
    fn dependent_swap_waits_for_minted_usdc_on_same_chain() {
        // Mint delivered USDC to Base; the next leg is a USDC→ETH swap on Base.
        let confirmed = vec![make_mint_leg(ChainKey::Base, 40.0)];
        let mut dep = make_swap_leg("USDC", "ETH"); // Base→Base, amount 600
        dep.depends_on = vec![1];
        assert_eq!(
            pending_funding_dependency(&dep, &confirmed),
            Some((ChainKey::Base, 600.0))
        );
    }

    #[test]
    fn no_funding_wait_without_prior_mint_on_that_chain() {
        // A mint to Arc doesn't gate a Base swap.
        let confirmed = vec![make_mint_leg(ChainKey::Arc, 40.0)];
        let dep = make_swap_leg("USDC", "ETH"); // Base
        assert_eq!(pending_funding_dependency(&dep, &confirmed), None);
        // No prior mint at all → no wait.
        assert_eq!(pending_funding_dependency(&dep, &[]), None);
    }

    #[test]
    fn sell_leg_does_not_wait_for_funding() {
        // A sell spends the non-USDC asset, not bridged USDC → no funding wait.
        let confirmed = vec![make_mint_leg(ChainKey::Base, 40.0)];
        let sell = make_swap_leg("ETH", "USDC");
        assert_eq!(pending_funding_dependency(&sell, &confirmed), None);
    }

    #[test]
    fn independent_same_chain_swap_does_not_wait_for_unrelated_mint() {
        let confirmed = vec![make_mint_leg(ChainKey::Base, 40.0)];
        let dep = make_swap_leg("USDC", "ETH");

        assert_eq!(pending_funding_dependency(&dep, &confirmed), None);
    }

    #[test]
    fn mint_failure_does_not_strand_even_after_burn() {
        // A failed mint means the destination USDC never landed (still in CCTP
        // transit, re-mintable) — a source-burn confirmation does NOT imply idle
        // cash, so the mint leg must not be marked stranded.
        let prior = vec![make_leg(LegKind::CrossChainBurn, 500.0)];
        let mint = make_mint_leg(ChainKey::Base, 500.0);
        assert!(!leg_strands_funds_on_failure(
            LegKind::CrossChainMint,
            &mint,
            &prior
        ));
    }

    #[test]
    fn local_swap_failure_after_mint_strands_idle_usdc() {
        // burn → mint (confirmed on Base) → local swap on Base fails: the bridged
        // USDC is now idle cash on Base, so the swap leg strands for the replan.
        let confirmed = vec![make_mint_leg(ChainKey::Base, 600.0)];
        let mut swap = make_swap_leg("USDC", "ETH"); // Base → Base
        swap.depends_on = vec![1];
        assert!(leg_strands_funds_on_failure(
            LegKind::LocalSwap,
            &swap,
            &confirmed
        ));
    }

    #[test]
    fn independent_same_chain_swap_failure_after_unrelated_mint_does_not_strand() {
        let confirmed = vec![make_mint_leg(ChainKey::Base, 600.0)];
        let swap = make_swap_leg("USDC", "ETH");

        assert!(!leg_strands_funds_on_failure(
            LegKind::LocalSwap,
            &swap,
            &confirmed
        ));
    }

    #[test]
    fn local_swap_failure_without_prior_mint_does_not_strand() {
        // A same-chain swap with no preceding bridge reverts atomically — the
        // USDC returns to the wallet, nothing stranded.
        let swap = make_swap_leg("USDC", "ETH");
        assert!(!leg_strands_funds_on_failure(
            LegKind::LocalSwap,
            &swap,
            &[]
        ));
    }

    #[test]
    fn burn_park_fx_failures_do_not_strand() {
        // A burn failure leaves source USDC in place; park / FX revert atomically
        // — none leave idle cash, even when a prior mint confirmed.
        let confirmed = vec![make_mint_leg(ChainKey::Base, 600.0)];
        for kind in [
            LegKind::CrossChainBurn,
            LegKind::ParkUsyc,
            LegKind::RedeemUsyc,
            LegKind::FxStablefx,
        ] {
            assert!(
                !leg_strands_funds_on_failure(kind, &make_leg(kind, 500.0), &confirmed),
                "{kind:?} must not strand on failure"
            );
        }
    }

    // ── Recovery: remaining-delta replan ──────────────────────────────────

    #[test]
    fn remaining_delta_re_buys_each_stranded_asset() {
        let stranded = vec![strand("ETH", 300.0), strand("BTC", 200.0)];
        let remaining = remaining_delta_after_strand(&stranded);
        // Sorted by symbol for determinism: BTC then ETH.
        assert_eq!(
            remaining,
            vec![
                RemainingDelta {
                    dest_symbol: "BTC".into(),
                    amount_usdc: 200.0
                },
                RemainingDelta {
                    dest_symbol: "ETH".into(),
                    amount_usdc: 300.0
                },
            ]
        );
    }

    #[test]
    fn remaining_delta_sums_same_symbol_strands() {
        // A split buy that stranded on two chains replans as one delta.
        let stranded = vec![strand("ETH", 120.0), strand("ETH", 80.0)];
        let remaining = remaining_delta_after_strand(&stranded);
        assert_eq!(
            remaining,
            vec![RemainingDelta {
                dest_symbol: "ETH".into(),
                amount_usdc: 200.0
            }]
        );
    }

    #[test]
    fn remaining_delta_drops_usdc_and_nonpositive() {
        // USDC strands are already cash (nothing to re-buy); zero/negative
        // amounts are noise.
        let stranded = vec![
            strand("USDC", 500.0),
            strand("usdc", 100.0),
            strand("ETH", 0.0),
            strand("BTC", -10.0),
            strand("", 50.0),
            strand("SOL", 75.0),
        ];
        let remaining = remaining_delta_after_strand(&stranded);
        assert_eq!(
            remaining,
            vec![RemainingDelta {
                dest_symbol: "SOL".into(),
                amount_usdc: 75.0
            }]
        );
    }

    #[test]
    fn remaining_delta_empty_when_nothing_stranded() {
        assert!(remaining_delta_after_strand(&[]).is_empty());
    }
}
