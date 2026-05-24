//! HTTP handlers for rebalance plan / execute / poll / history.
//!
//! Every route is scoped to the authenticated user — ownership of the
//! portfolio is enforced on each lookup so session A can never read
//! or execute a rebalance belonging to user B.

mod approval;
mod autonomous;
mod plan_input;
mod shared;

pub use autonomous::{prepare_autonomous_plan, AutonomousPlan};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::middleware::auth::Claims;
use crate::modules::agent::{models::AnalyzeRequest, service::analyze_portfolio};
use crate::modules::rebalance::{
    executor::{approve_and_execute, create_plan},
    planner::plan_legs,
};
use crate::router::AppState;

use approval::{approval_safety, history_approval_safety, ApprovalSafety};
use plan_input::build_plan_input;
use shared::{
    ensure_rebalance_wallet_ready, execution_mode, noop_plan_message, own_portfolio_or_404,
    own_rebalance_or_404, plan_leg_view, rebalance_totals_by_id, reusable_planned_rebalance,
};

use autonomous::{mock_agent_decision, planner_agent_decision};

/// Original trigger endpoint kept for back-compat with Sprint 2 callers.
/// New code should hit `POST /portfolios/:id/rebalance/plan` instead.
pub async fn trigger(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(portfolio_id): Path<Uuid>,
) -> Result<Json<crate::modules::agent::models::AgentDecision>> {
    own_portfolio_or_404(&state, claims.sub, portfolio_id).await?;
    let decision = analyze_portfolio(
        &state,
        AnalyzeRequest {
            portfolio_id,
            triggered_by: Some("user_request".into()),
        },
    )
    .await?;
    Ok(Json(decision))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanResponse {
    pub rebalance_id: Uuid,
    pub decision_id: Uuid,
    pub execution_mode: String,
    pub legs: Vec<PlanLegView>,
    pub total_legs: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanLegView {
    pub leg_index: i32,
    pub kind: String,
    pub src_chain: Option<String>,
    pub dest_chain: Option<String>,
    pub src_symbol: Option<String>,
    pub dest_symbol: Option<String>,
    pub amount_usdc: f64,
}

/// Build an agent decision *and* a concrete rebalance plan for that
/// decision. Returns the plan immediately — the user reviews and approves
/// via `POST /rebalance/:id/execute`.
pub async fn create(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(portfolio_id): Path<Uuid>,
) -> Result<Json<PlanResponse>> {
    own_portfolio_or_404(&state, claims.sub, portfolio_id).await?;
    ensure_rebalance_wallet_ready(&state, claims.sub).await?;
    let input = build_plan_input(&state, portfolio_id).await?;
    let legs = plan_legs(&input);
    if legs.is_empty() {
        return Err(AppError::Conflict(noop_plan_message(&input)));
    }
    if let Some(existing) = reusable_planned_rebalance(&state, portfolio_id, &legs).await? {
        return Ok(Json(existing));
    }
    // Plan creation is an execution-control path, not a model-chat path. It
    // must stay fast in real mode so users can reach the approval screen even
    // when OpenRouter is slow. The separate /agent/analyze endpoint still runs
    // strategist + critic commentary; this route records a deterministic
    // planner decision tied to the concrete legs the executor will review.
    let decision = if state.config.execution_mock || state.config.circle_mock {
        mock_agent_decision(&state, portfolio_id).await?
    } else {
        planner_agent_decision(&state, portfolio_id, &input, &legs).await?
    };
    let rebalance_id = create_plan(&state, portfolio_id, decision.id, &legs).await?;

    Ok(Json(PlanResponse {
        rebalance_id,
        decision_id: decision.id,
        execution_mode: execution_mode(&state).to_string(),
        total_legs: legs.len() as i32,
        legs: legs.iter().map(plan_leg_view).collect(),
    }))
}

#[derive(Debug, Default, Deserialize)]
pub struct ExecuteBody {
    /// Optional user-provided slippage tolerance override in bps.
    #[allow(dead_code)]
    #[serde(default)]
    pub max_slippage_bps: Option<u32>,
}

pub async fn execute(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(rebalance_id): Path<Uuid>,
    body: Option<Json<ExecuteBody>>,
) -> Result<StatusCode> {
    let _ = body; // body fields are reserved; accept missing/empty body gracefully
    own_rebalance_or_404(&state, claims.sub, rebalance_id).await?;
    ensure_rebalance_wallet_ready(&state, claims.sub).await?;
    let safety = approval_safety(&state, rebalance_id).await?;
    if !safety.approvable {
        return Err(AppError::Conflict(safety.message));
    }
    approve_and_execute(state, rebalance_id).await?;
    Ok(StatusCode::ACCEPTED)
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RebalanceView {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub decision_id: Uuid,
    pub status: String,
    pub total_legs: i32,
    pub completed_legs: i32,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub total_gas_usdc: Option<Decimal>,
    pub failure_reason: Option<String>,
    pub approved_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Nanopayments 25bps protocol fee settlement tx (if recorded).
    /// Enables showing the real x402 tx in the execution trace UI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_fee_settlement_tx: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct LegView {
    pub id: Uuid,
    pub rebalance_id: Uuid,
    pub leg_index: i32,
    pub kind: String,
    pub src_chain: Option<String>,
    pub dest_chain: Option<String>,
    pub src_symbol: Option<String>,
    pub dest_symbol: Option<String>,
    #[serde(with = "rust_decimal::serde::float")]
    pub amount_usdc: Decimal,
    pub status: String,
    pub tx_hash: Option<String>,
    pub failure_reason: Option<String>,
    pub submitted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub confirmed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RebalanceDetail {
    #[serde(flatten)]
    pub plan: RebalanceView,
    pub execution_mode: String,
    pub approval_safety: ApprovalSafety,
    pub legs: Vec<LegView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RebalanceHistoryView {
    #[serde(flatten)]
    pub plan: RebalanceView,
    pub execution_mode: String,
    pub approval_safety: ApprovalSafety,
    pub total_amount_usdc: f64,
}

pub async fn get(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(rebalance_id): Path<Uuid>,
) -> Result<Json<RebalanceDetail>> {
    own_rebalance_or_404(&state, claims.sub, rebalance_id).await?;

    let mut plan: RebalanceView = sqlx::query_as(
        "SELECT id, portfolio_id, decision_id, status, total_legs, completed_legs,
                total_gas_usdc, failure_reason, approved_at, completed_at,
                created_at, updated_at, NULL::text as protocol_fee_settlement_tx
         FROM rebalances WHERE id = $1",
    )
    .bind(rebalance_id)
    .fetch_one(&state.db)
    .await?;

    // Load the first protocol fee settlement tx (Nanopayments x402) if present.
    if let Some(tx) = sqlx::query_scalar::<_, Option<String>>(
        "SELECT settlement_tx_hash FROM rebalance_fees
         WHERE rebalance_id = $1 AND fee_type = 'protocol' AND settlement_tx_hash IS NOT NULL
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(rebalance_id)
    .fetch_optional(&state.db)
    .await?
    {
        plan.protocol_fee_settlement_tx = tx;
    }

    let legs: Vec<LegView> = sqlx::query_as(
        "SELECT id, rebalance_id, leg_index, kind, src_chain, dest_chain,
                src_symbol, dest_symbol, amount_usdc, status, tx_hash,
                failure_reason, submitted_at, confirmed_at
         FROM rebalance_legs WHERE rebalance_id = $1
         ORDER BY leg_index ASC",
    )
    .bind(rebalance_id)
    .fetch_all(&state.db)
    .await?;

    let approval_safety = approval_safety(&state, rebalance_id).await?;

    Ok(Json(RebalanceDetail {
        plan,
        execution_mode: execution_mode(&state).to_string(),
        approval_safety,
        legs,
    }))
}

pub async fn history(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(portfolio_id): Path<Uuid>,
) -> Result<Json<Vec<RebalanceHistoryView>>> {
    own_portfolio_or_404(&state, claims.sub, portfolio_id).await?;
    let rows: Vec<RebalanceView> = sqlx::query_as(
        "SELECT id, portfolio_id, decision_id, status, total_legs, completed_legs,
                total_gas_usdc, failure_reason, approved_at, completed_at,
                created_at, updated_at, NULL::text as protocol_fee_settlement_tx
         FROM rebalances WHERE portfolio_id = $1
         ORDER BY created_at DESC LIMIT 50",
    )
    .bind(portfolio_id)
    .fetch_all(&state.db)
    .await?;
    let rebalance_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let totals_by_id = rebalance_totals_by_id(&state, &rebalance_ids).await?;
    let latest_review_id = rows.first().map(|row| row.id);
    let mut history = Vec::with_capacity(rows.len());
    for row in rows {
        let total_amount_usdc = totals_by_id.get(&row.id).copied().unwrap_or(0.0);
        let approval_safety = history_approval_safety(&state, &row, latest_review_id).await?;
        history.push(RebalanceHistoryView {
            plan: row,
            execution_mode: execution_mode(&state).to_string(),
            approval_safety,
            total_amount_usdc,
        });
    }
    Ok(Json(history))
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::noop_plan_message;
    use std::collections::HashMap;

    use crate::modules::rebalance::models::PlanInput;

    #[test]
    fn noop_message_distinguishes_empty_wallet_from_on_target_portfolio() {
        let empty = PlanInput {
            portfolio_value_usd: 0.0,
            current_weights: HashMap::new(),
            target_weights: HashMap::new(),
            usdc_per_chain: HashMap::new(),
            drift_threshold: 0.05,
            dust_threshold_usd: 5.0,
            prices: HashMap::new(),
            regime: None,
        };
        assert!(noop_plan_message(&empty).contains("no confirmed positions"));

        let mut current_weights = HashMap::new();
        current_weights.insert("BTC".to_string(), 0.6);
        current_weights.insert("ETH".to_string(), 0.4);
        let on_target = PlanInput {
            portfolio_value_usd: 100.0,
            target_weights: current_weights.clone(),
            current_weights,
            usdc_per_chain: HashMap::new(),
            drift_threshold: 0.05,
            dust_threshold_usd: 5.0,
            prices: HashMap::new(),
            regime: None,
        };
        assert!(noop_plan_message(&on_target).contains("already within"));
    }
}
