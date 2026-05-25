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

/// Tagged on the `status` field so the frontend branches on one discriminator.
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PlanOutcome {
    /// Real legs to review and approve.
    Executable(PlanResponse),
    /// Holdings already match the target within thresholds — calm success.
    OnTargetNoop { message: String },
    /// Approved target is a USDC reserve — cash is already in the target asset.
    ReserveFallback { message: String },
    /// Wallet has no confirmed positions and no deployable USDC — actionable.
    Unfunded { message: String },
    /// Only sub-dust USDC is idle — below the minimum move size.
    DustOnly { message: String },
}

impl PlanOutcome {
    /// Classify an empty-legs plan into its non-executable outcome. Never an error.
    pub fn from_noop(input: &PlanInput) -> Self {
        let message = noop_plan_message(input);
        match classify_noop(input) {
            NoopReason::Unfunded => Self::Unfunded { message },
            NoopReason::UsdcReserve => Self::ReserveFallback { message },
            NoopReason::DustOnly => Self::DustOnly { message },
            NoopReason::OnTarget => Self::OnTargetNoop { message },
        }
    }
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

    #[test]
    fn unfunded_wallet_is_a_typed_outcome_not_an_error() {
        let out = PlanOutcome::from_noop(&input(0.0, 0.0));
        assert!(matches!(out, PlanOutcome::Unfunded { .. }));
        assert_eq!(status_of(&out), "unfunded");
    }

    #[test]
    fn usdc_reserve_target_is_reserve_fallback_not_red_409() {
        // The exact screenshot scenario: real value + idle USDC, target is USDC-only.
        let mut i = input(100.0, 21.0);
        i.target_weights.insert("USDC".into(), 1.0);
        let out = PlanOutcome::from_noop(&i);
        assert!(matches!(out, PlanOutcome::ReserveFallback { .. }));
        assert_eq!(status_of(&out), "reserve_fallback");
    }

    #[test]
    fn dust_surplus_is_dust_only() {
        let out = PlanOutcome::from_noop(&input(100.0, 3.0));
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
        let out = PlanOutcome::from_noop(&i);
        assert!(matches!(out, PlanOutcome::OnTargetNoop { .. }));
        assert_eq!(status_of(&out), "on_target_noop");
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
