use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

use crate::config::Config;
use crate::error::{AppError, Result};
use crate::modules::rebalance::{
    models::PlannedLeg,
    planner::plan_legs,
    registry::{capabilities::RuntimeCapabilities, route, route::RouteLeg},
    snapshot::{routability_changed, RoutableSnapshot},
};
use crate::router::AppState;

use super::{plan_input::build_plan_input, LegView, RebalanceView};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalSafety {
    pub approvable: bool,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing_capabilities: Option<Vec<MissingCapability>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingCapability {
    pub code: String,
    pub label: String,
    pub detail: String,
}

pub(super) async fn approval_safety(
    state: &AppState,
    rebalance_id: Uuid,
) -> Result<ApprovalSafety> {
    let plan: RebalanceView = sqlx::query_as(
        "SELECT id, portfolio_id, decision_id, status, total_legs, completed_legs,
                total_gas_usdc, failure_reason, approved_at, completed_at,
                created_at, updated_at, NULL::text as protocol_fee_settlement_tx
         FROM rebalances WHERE id = $1",
    )
    .bind(rebalance_id)
    .fetch_one(&state.db)
    .await?;

    if plan.status != "planned" {
        return Ok(ApprovalSafety {
            approvable: false,
            code: "NOT_PLANNED".into(),
            message: format!(
                "This rebalance is already in '{}' state and cannot be approved again.",
                plan.status
            ),
            missing_capabilities: None,
        });
    }

    let newer_planned_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1
            FROM rebalances newer
            WHERE newer.portfolio_id = $1
              AND newer.status = 'planned'
              AND newer.created_at > $2
        )",
    )
    .bind(plan.portfolio_id)
    .bind(plan.created_at)
    .fetch_one(&state.db)
    .await?;
    if newer_planned_exists {
        return Ok(ApprovalSafety {
            approvable: false,
            code: "SUPERSEDED".into(),
            message: "A newer rebalance review exists. Open the latest review or build a fresh one before approving.".into(),
            missing_capabilities: None,
        });
    }

    let (model_slug, reasoning): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT model_slug, reasoning FROM agent_decisions WHERE id = $1")
            .bind(plan.decision_id)
            .fetch_one(&state.db)
            .await?;
    let real_mode = !state.config.execution_mock && !state.config.circle_mock;
    let mock_like_decision = model_slug.as_deref() != Some("aegis/rebalance-planner-v1")
        || reasoning
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("mock");
    if real_mode && mock_like_decision {
        return Ok(ApprovalSafety {
            approvable: false,
            code: "MOCK_OR_LEGACY_PLAN".into(),
            message: "This approval plan was created by a mock or legacy planner. Build a fresh real execution review before approving.".into(),
            missing_capabilities: None,
        });
    }

    let stored_legs: Vec<LegView> = sqlx::query_as(
        "SELECT id, rebalance_id, leg_index, kind, src_chain, dest_chain,
                src_symbol, dest_symbol, amount_usdc, status, tx_hash,
                failure_reason, submitted_at, confirmed_at
         FROM rebalance_legs WHERE rebalance_id = $1
         ORDER BY leg_index ASC",
    )
    .bind(rebalance_id)
    .fetch_all(&state.db)
    .await?;

    if stored_legs.is_empty() {
        return Ok(ApprovalSafety {
            approvable: false,
            code: "NO_OP".into(),
            message: "This plan has no executable legs. No approval is needed.".into(),
            missing_capabilities: None,
        });
    }

    let current_input = match build_plan_input(state, plan.portfolio_id).await {
        Ok((input, _deferred)) => input,
        Err(AppError::Conflict(message)) => {
            return Ok(ApprovalSafety {
                approvable: false,
                code: "BALANCE_UNAVAILABLE".into(),
                message,
                missing_capabilities: None,
            });
        }
        Err(e) => return Err(e),
    };
    let current_legs = plan_legs(&current_input);
    if !legs_match_current(&stored_legs, &current_legs) {
        return Ok(ApprovalSafety {
            approvable: false,
            code: "STALE_PLAN".into(),
            message: "Portfolio holdings or Gateway cash changed after this plan was created. Build a fresh review before approving real execution.".into(),
            missing_capabilities: None,
        });
    }

    // INV-6: the plan is bound to the routability it was built against. If a
    // rail flipped Ready⇄track-only since then, the approved legs may no longer
    // settle — refuse so the user rebuilds against the live routable set.
    let stored_snapshot_hash: Option<String> =
        sqlx::query_scalar("SELECT routable_snapshot_hash FROM rebalances WHERE id = $1")
            .bind(rebalance_id)
            .fetch_one(&state.db)
            .await?;
    let current_caps = RuntimeCapabilities::from_config(&state.config);
    let current_snapshot = RoutableSnapshot::capture(&current_caps, &state.config);
    if routability_changed(stored_snapshot_hash.as_deref(), current_snapshot.hash()) {
        return Ok(ApprovalSafety {
            approvable: false,
            code: "ROUTABILITY_CHANGED".into(),
            message: "A token's route went live or track-only since this plan was built. Build a fresh review so the agent only targets what can settle now.".into(),
            missing_capabilities: None,
        });
    }

    if real_mode {
        let missing_capabilities = route_blockers(&state.config, &stored_legs);
        if !missing_capabilities.is_empty() {
            let message = execution_blocked_message(&missing_capabilities);
            return Ok(ApprovalSafety {
                approvable: false,
                code: "EXECUTION_UNAVAILABLE".into(),
                message,
                missing_capabilities: Some(missing_capabilities),
            });
        }
    }

    Ok(ApprovalSafety {
        approvable: true,
        code: "APPROVABLE".into(),
        message: "Plan matches current holdings and Gateway cash.".into(),
        missing_capabilities: None,
    })
}

pub(super) async fn history_approval_safety(
    state: &AppState,
    plan: &RebalanceView,
    latest_review_id: Option<Uuid>,
) -> Result<ApprovalSafety> {
    if plan.status != "planned" {
        return Ok(ApprovalSafety {
            approvable: false,
            code: "NOT_PLANNED".into(),
            message: format!(
                "This rebalance is already in '{}' state and cannot be approved again.",
                plan.status
            ),
            missing_capabilities: None,
        });
    }

    if Some(plan.id) != latest_review_id {
        return Ok(ApprovalSafety {
            approvable: false,
            code: "SUPERSEDED".into(),
            message: "A newer rebalance review exists. Open the latest review or build a fresh one before approving.".into(),
            missing_capabilities: None,
        });
    }

    approval_safety(state, plan.id).await
}

pub(super) fn legs_match_current(stored: &[LegView], current: &[PlannedLeg]) -> bool {
    if stored.len() != current.len() {
        return false;
    }
    stored.iter().zip(current.iter()).all(|(a, b)| {
        a.leg_index == b.leg_index
            && a.kind == b.kind.as_str()
            && a.src_chain.as_deref() == b.src_chain.map(|c| c.as_str())
            && a.dest_chain.as_deref() == b.dest_chain.map(|c| c.as_str())
            && a.src_symbol.as_deref() == b.src_symbol.as_deref()
            && a.dest_symbol.as_deref() == b.dest_symbol.as_deref()
            && amount_matches(a.amount_usdc, b.amount_usdc)
    })
}

/// Map the route registry's blockers onto the wire `MissingCapability` shape
/// the frontend approval modal already renders. Single source of truth: the
/// same `route::validate_legs` consulted by the executor and the agent.
fn route_blockers(cfg: &Config, legs: &[LegView]) -> Vec<MissingCapability> {
    let caps = RuntimeCapabilities::from_config(cfg);
    let route_legs: Vec<RouteLeg> = legs
        .iter()
        .filter_map(|l| {
            RouteLeg::from_parts(
                &l.kind,
                l.src_chain.clone(),
                l.dest_chain.clone(),
                l.src_symbol.clone(),
                l.dest_symbol.clone(),
                l.amount_usdc.to_f64().unwrap_or(0.0),
            )
        })
        .collect();
    route::validate_legs(&caps, cfg, &route_legs)
        .into_iter()
        .map(|b| MissingCapability {
            code: b.code.wire_code().to_string(),
            label: b.code.label().to_string(),
            detail: b.detail,
        })
        .collect()
}

fn execution_blocked_message(missing: &[MissingCapability]) -> String {
    let labels = missing
        .iter()
        .map(|cap| cap.label.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let noun = if missing.len() == 1 {
        "capability"
    } else {
        "capabilities"
    };
    format!(
        "This review is saved as a draft because {noun} must be ready before money can move: {labels}. Change the target and build a fresh executable review before approving."
    )
}

fn amount_matches(stored: Decimal, current: f64) -> bool {
    let stored = stored.to_f64().unwrap_or(0.0);
    let tolerance = (current.abs() * 0.005).max(0.01);
    (stored - current).abs() <= tolerance
}
