//! The typed result of `POST .../rebalance/plan`.
//!
//! A plan request has five honest outcomes — only one is "go execute". The
//! other four are *not errors*: an unfunded wallet, an on-target portfolio, a
//! USDC-reserve target, and a sub-dust surplus are all legitimate 200 results
//! the UI renders calmly or actionably. This replaces the old
//! `Err(AppError::Conflict(noop_plan_message))` 409 that rendered every no-op
//! as a red error (the "USDC reserve" dead-end in the screenshots).
//!
//! The variant is chosen by the single `classify_noop` predicate in `shared`,
//! so the typed tag and the human message can never drift.

use serde::Serialize;

use crate::modules::rebalance::models::PlanInput;

use super::shared::{classify_noop, noop_plan_message, NoopReason};
use super::PlanResponse;

/// A target sleeve the agent wanted but could not route now (spec §11/§12:
/// "deferred targets shown as intent"). Its weight was held as USDC reserve
/// rather than silently dropped, so the UI can show the *intended* allocation
/// alongside what actually executed.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeferredTarget {
    pub symbol: String,
    pub target_weight: f64,
    pub reason: String,
}

/// Tagged on the `status` field so the frontend branches on one discriminator.
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PlanOutcome {
    /// Real legs to review and approve.
    Executable(PlanResponse),
    /// Real legs to review, *plus* targets that couldn't be routed now and were
    /// held as USDC reserve — surfaced as intent, not silently folded.
    PartialDeferred {
        #[serde(flatten)]
        plan: PlanResponse,
        deferred: Vec<DeferredTarget>,
    },
    /// Holdings already match the target within thresholds — calm success.
    OnTargetNoop { message: String },
    /// Approved target is a USDC reserve — cash is already in the target asset.
    ReserveFallback { message: String },
    /// The desired non-USDC sleeves have no live route, so deployable cash is
    /// held as USDC until one opens. Nothing executed, but not a no-op — the
    /// intended (deferred) targets are surfaced so the user sees why.
    Blocked {
        message: String,
        deferred: Vec<DeferredTarget>,
    },
    /// Wallet has no confirmed positions and no deployable USDC — actionable.
    Unfunded { message: String },
    /// Only sub-dust USDC is idle — below the minimum move size.
    DustOnly { message: String },
    /// Circle Gateway balance could not be read just now (transient). Not a
    /// no-op and not a dead-end 409 — a retryable 200 the UI renders in place.
    BalanceUnavailable { message: String },
}

impl PlanOutcome {
    /// A plan with real legs: `Executable` when everything routed, else
    /// `PartialDeferred` carrying the sleeves held back as USDC reserve.
    pub fn executable(plan: PlanResponse, deferred: Vec<DeferredTarget>) -> Self {
        if deferred.is_empty() {
            Self::Executable(plan)
        } else {
            Self::PartialDeferred { plan, deferred }
        }
    }

    /// Classify an empty-legs plan into its non-executable outcome. Never an error.
    /// When deployable cash is idle *because* every non-USDC target was deferred
    /// (no live route), that is `Blocked` — distinct from a genuine USDC reserve.
    pub fn from_noop(input: &PlanInput, deferred: &[DeferredTarget]) -> Self {
        let reason = classify_noop(input);
        if !deferred.is_empty() {
            return Self::Blocked {
                message: blocked_message(deferred),
                deferred: deferred.to_vec(),
            };
        }
        let message = noop_plan_message(input);
        match reason {
            NoopReason::Unfunded => Self::Unfunded { message },
            NoopReason::UsdcReserve => Self::ReserveFallback { message },
            NoopReason::DustOnly => Self::DustOnly { message },
            NoopReason::OnTarget => Self::OnTargetNoop { message },
        }
    }
}

/// `Blocked` only ever carries *genuine* routing blockers: tracked-by-design
/// volatile sleeves are folded silently upstream (see
/// `fold_nonexecutable_targets_into_usdc`), so the deferred set here is a mix of
/// no-route, rail-not-ready, unsupported-sleeve (e.g. EURC), *and* route-shaping
/// blocks where a route exists but is unsafe right now (a stale/wide live quote
/// or a price-safety rejection). The headline stays broad enough to be honest
/// for all of them — the per-sleeve specifics ride along in `deferred[].reason`.
fn blocked_message(deferred: &[DeferredTarget]) -> String {
    let names = deferred
        .iter()
        .map(|d| d.symbol.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Not tradeable right now on this network: {names}. There's no safe execution route for them at the moment, so their intended allocation is held as USDC reserve until one is available — your USDC capital is still managed normally."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::modules::rebalance::models::ChainKey;

    fn input(portfolio_value_usd: f64, idle_usdc: f64) -> PlanInput {
        let mut usdc_per_chain = HashMap::new();
        if idle_usdc != 0.0 {
            usdc_per_chain.insert(ChainKey::Base, idle_usdc);
        }
        PlanInput {
            portfolio_value_usd,
            current_weights: HashMap::new(),
            sell_sources: HashMap::new(),
            target_weights: HashMap::new(),
            usdc_per_chain,
            drift_threshold: 0.05,
            dust_threshold_usd: 5.0,
            prices: HashMap::new(),
            regime: None,
        }
    }

    /// The serialized `status` tag is the frontend contract — pin it.
    fn status_of(outcome: &PlanOutcome) -> String {
        serde_json::to_value(outcome).unwrap()["status"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn deferred(symbol: &str, weight: f64) -> DeferredTarget {
        DeferredTarget {
            symbol: symbol.into(),
            target_weight: weight,
            reason: "no live route".into(),
        }
    }

    #[test]
    fn unfunded_wallet_is_a_typed_outcome_not_an_error() {
        let out = PlanOutcome::from_noop(&input(0.0, 0.0), &[]);
        assert!(matches!(out, PlanOutcome::Unfunded { .. }));
        assert_eq!(status_of(&out), "unfunded");
    }

    #[test]
    fn usdc_reserve_target_is_reserve_fallback_not_red_409() {
        // The exact screenshot scenario: real value + idle USDC, target is USDC-only.
        let mut i = input(100.0, 21.0);
        i.target_weights.insert("USDC".into(), 1.0);
        let out = PlanOutcome::from_noop(&i, &[]);
        assert!(matches!(out, PlanOutcome::ReserveFallback { .. }));
        assert_eq!(status_of(&out), "reserve_fallback");
    }

    #[test]
    fn deferred_targets_turn_a_reserve_noop_into_blocked() {
        // Same idle-cash shape as the reserve case, but the cash is idle *because*
        // the desired sleeve has no route — that is Blocked, not a calm reserve.
        let mut i = input(100.0, 21.0);
        i.target_weights.insert("USDC".into(), 1.0);
        let out = PlanOutcome::from_noop(&i, &[deferred("EURC", 0.5)]);
        assert_eq!(status_of(&out), "blocked");
        match out {
            PlanOutcome::Blocked { deferred, message } => {
                assert_eq!(deferred.len(), 1);
                assert!(message.contains("EURC"));
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn deferred_sell_noop_is_blocked_even_when_classifier_would_say_on_target() {
        let mut i = input(100.0, 0.0);
        i.target_weights.insert("ETH".into(), 0.2);
        i.current_weights.insert("ETH".into(), 1.0);
        let out = PlanOutcome::from_noop(&i, &[deferred("ETH", 0.2)]);
        assert_eq!(status_of(&out), "blocked");
        match out {
            PlanOutcome::Blocked { message, deferred } => {
                assert!(message.contains("ETH"));
                assert_eq!(deferred.len(), 1);
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn dust_surplus_is_dust_only() {
        let out = PlanOutcome::from_noop(&input(100.0, 3.0), &[]);
        assert!(matches!(out, PlanOutcome::DustOnly { .. }));
        assert_eq!(status_of(&out), "dust_only");
    }

    #[test]
    fn on_target_holdings_are_calm_success() {
        let mut i = input(100.0, 0.0);
        i.target_weights.insert("BTC".into(), 0.6);
        i.target_weights.insert("ETH".into(), 0.4);
        i.current_weights.insert("BTC".into(), 0.6);
        i.current_weights.insert("ETH".into(), 0.4);
        let out = PlanOutcome::from_noop(&i, &[]);
        assert!(matches!(out, PlanOutcome::OnTargetNoop { .. }));
        assert_eq!(status_of(&out), "on_target_noop");
    }

    #[test]
    fn partial_deferred_flattens_plan_and_carries_deferred() {
        let out = PlanOutcome::PartialDeferred {
            plan: PlanResponse {
                rebalance_id: uuid::Uuid::nil(),
                decision_id: uuid::Uuid::nil(),
                execution_mode: "real".into(),
                legs: vec![],
                total_legs: 1,
            },
            deferred: vec![deferred("cbBTC", 0.05)],
        };
        let v = serde_json::to_value(&out).unwrap();
        assert_eq!(v["status"], "partial_deferred");
        assert!(v.get("rebalanceId").is_some(), "plan fields flatten");
        assert_eq!(v["deferred"][0]["symbol"], "cbBTC");
    }

    #[test]
    fn balance_unavailable_is_a_typed_retry_not_a_409() {
        // Gateway unreadable must surface as a calm, retryable 200 outcome —
        // never the red 409 the FE throws on for any non-2xx response.
        let out = PlanOutcome::BalanceUnavailable {
            message: "Circle Gateway balance is unavailable. Retry in a moment.".into(),
        };
        assert_eq!(status_of(&out), "balance_unavailable");
    }

    #[test]
    fn executable_variant_flattens_plan_response_with_tag() {
        let out = PlanOutcome::Executable(PlanResponse {
            rebalance_id: uuid::Uuid::nil(),
            decision_id: uuid::Uuid::nil(),
            execution_mode: "mock".into(),
            legs: vec![],
            total_legs: 0,
        });
        let v = serde_json::to_value(&out).unwrap();
        assert_eq!(v["status"], "executable");
        assert!(
            v.get("rebalanceId").is_some(),
            "PlanResponse fields flatten alongside the tag"
        );
    }
}
