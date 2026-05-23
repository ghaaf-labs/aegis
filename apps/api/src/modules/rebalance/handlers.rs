//! HTTP handlers for rebalance plan / execute / poll / history.
//!
//! Every route is scoped to the authenticated user — ownership of the
//! portfolio is enforced on each lookup so session A can never read
//! or execute a rebalance belonging to user B.

use std::collections::{HashMap, HashSet};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::Config;
use crate::error::{AppError, Result};
use crate::middleware::auth::Claims;
use crate::modules::agent::{models::AnalyzeRequest, service::analyze_portfolio};
use crate::modules::rebalance::{
    executor::{approve_and_execute, create_plan},
    models::{ChainKey, PlanInput, PlannedLeg, ARC_NATIVE_SYMBOLS, BASE_NATIVE_SYMBOLS},
    planner::plan_legs,
    registry::{capabilities::RuntimeCapabilities, route, route::RouteLeg},
};
use crate::modules::wallet_routes;
use crate::router::AppState;
use serde_json::json;

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
    pub total_gas_usdc: Option<f64>,
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
    pub amount_usdc: f64,
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

// ── Helpers ───────────────────────────────────────────────────────────────

/// Insert a canned agent decision for mock-backed local/demo mode. Lets the
/// rebalance plan endpoint be exercised end-to-end without a live AI call.
async fn mock_agent_decision(
    state: &AppState,
    portfolio_id: Uuid,
) -> Result<crate::modules::agent::models::AgentDecision> {
    let rec = json!({
        "summary": "Hold",
        "trades": [],
        "expectedImpact": { "riskDelta": 0.0, "diversificationScore": 0.5 }
    });
    let decision = sqlx::query_as(
        r#"INSERT INTO agent_decisions
           (id, portfolio_id, reasoning, recommendation, confidence,
            triggered_by, model_slug, regime, prompt_tokens,
            completion_tokens, latency_ms, critic_verdict, snapshot,
            raw_confidence, calibrated_confidence, counterfactual)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
           RETURNING *"#,
    )
    .bind(Uuid::new_v4())
    .bind(portfolio_id)
    .bind("Mock decision — local/demo mock mode")
    .bind(&rec)
    .bind(1.0_f64)
    .bind("user_request")
    .bind("anthropic/claude-3-haiku")
    .bind("neutral")
    .bind(0_i32)
    .bind(0_i32)
    .bind(0_i32)
    .bind(serde_json::Value::Null)
    .bind(json!({}))
    .bind(1.0_f64)
    .bind(None::<f64>)
    .bind(None::<String>)
    .fetch_one(&state.db)
    .await?;
    Ok(decision)
}

async fn planner_agent_decision(
    state: &AppState,
    portfolio_id: Uuid,
    input: &PlanInput,
    legs: &[PlannedLeg],
) -> Result<crate::modules::agent::models::AgentDecision> {
    let trades: Vec<serde_json::Value> = legs
        .iter()
        .filter_map(|leg| planned_trade(input, leg))
        .collect();
    let idle_usdc: f64 = input.usdc_per_chain.values().copied().sum();
    let route_surface = plan_route_surface(legs);
    let summary = if legs.is_empty() {
        "No rebalance needed — portfolio drift and idle USDC are below execution thresholds."
            .to_string()
    } else {
        format!(
            "Build {} review {} from current holdings and Gateway cash.",
            legs.len(),
            if legs.len() == 1 { "leg" } else { "legs" }
        )
    };
    let reasoning = if legs.is_empty() {
        "Aegis compared confirmed position values, target weights, and Circle Gateway USDC. No leg exceeded the drift or dust thresholds, so the approval screen is a no-op review."
            .to_string()
    } else {
        format!(
            "Aegis built this execution review from ${:.2} invested positions plus ${:.2} idle Gateway USDC (${:.2} total plan value). The plan is deterministic and remains gated by user approval before any {route_surface} is submitted.",
            (input.portfolio_value_usd - idle_usdc).max(0.0), idle_usdc, input.portfolio_value_usd
        )
    };
    let rec = json!({
        "summary": summary,
        "trades": trades,
        "expectedImpact": { "riskDelta": 0.0, "diversificationScore": 0.5 }
    });
    let invested_value_usd = (input.portfolio_value_usd - idle_usdc).max(0.0);
    let critic_verdict = json!({
        "verdict": "approved",
        "notes": "Deterministic planner matched confirmed holdings, target weights, and Gateway balances; approval remains the final execution gate.",
        "confidence": 0.92
    });
    let decision = sqlx::query_as(
        r#"INSERT INTO agent_decisions
           (id, portfolio_id, reasoning, recommendation, confidence,
            triggered_by, model_slug, regime, prompt_tokens,
            completion_tokens, latency_ms, critic_verdict, snapshot,
            raw_confidence, calibrated_confidence, counterfactual)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
           RETURNING *"#,
    )
    .bind(Uuid::new_v4())
    .bind(portfolio_id)
    .bind(reasoning)
    .bind(&rec)
    .bind(0.92_f64)
    .bind("user_request")
    .bind("aegis/rebalance-planner-v1")
    .bind("neutral")
    .bind(0_i32)
    .bind(0_i32)
    .bind(0_i32)
    .bind(critic_verdict)
    .bind(json!({
        "planner": "deterministic",
        "legs": legs.len(),
        "planValueUsd": input.portfolio_value_usd,
        "investedValueUsd": invested_value_usd,
        "portfolioValueUsd": input.portfolio_value_usd,
        "idleUsdc": idle_usdc
    }))
    .bind(0.92_f64)
    .bind(None::<f64>)
    .bind(Some("If drift or idle cash changes, the deterministic planner will build a different approval plan.".to_string()))
    .fetch_one(&state.db)
    .await?;
    Ok(decision)
}

fn planned_trade(input: &PlanInput, leg: &PlannedLeg) -> Option<serde_json::Value> {
    let (symbol, action) = match (
        leg.src_symbol.as_deref(),
        leg.dest_symbol.as_deref(),
        leg.kind,
    ) {
        (Some("USDC"), Some(dest), _) if dest != "USDC" => (dest, "buy"),
        (Some(src), Some("USDC"), _) if src != "USDC" => (src, "sell"),
        _ => return None,
    };
    let price = input.prices.get(symbol).copied().unwrap_or(1.0).max(0.01);
    let quantity = leg.amount_usdc / price;
    Some(json!({
        "assetId": symbol,
        "symbol": symbol,
        "action": action,
        "quantity": quantity,
        "valueUsd": leg.amount_usdc,
        "reason": format!("{} on {}", leg.kind.as_str(), leg.dest_chain.or(leg.src_chain).map(|c| c.as_str()).unwrap_or("gateway"))
    }))
}

fn plan_route_surface(legs: &[PlannedLeg]) -> String {
    if legs.iter().any(|leg| {
        matches!(
            leg.kind,
            crate::modules::rebalance::models::LegKind::CrossChainBurn
                | crate::modules::rebalance::models::LegKind::CrossChainMint
        )
    }) {
        return "Arc/Base transaction".to_string();
    }
    let mut chains: Vec<&str> = legs
        .iter()
        .filter_map(|leg| leg.dest_chain.or(leg.src_chain))
        .map(|chain| chain.as_str())
        .collect();
    chains.sort_unstable();
    chains.dedup();
    match chains.as_slice() {
        [] => "execution".to_string(),
        ["arc"] => "Arc transaction".to_string(),
        ["base"] => "Base transaction".to_string(),
        _ => "Arc/Base transaction".to_string(),
    }
}

async fn own_portfolio_or_404(state: &AppState, user_id: Uuid, portfolio_id: Uuid) -> Result<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM portfolios WHERE id = $1 AND user_id = $2)",
    )
    .bind(portfolio_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    if !exists {
        return Err(AppError::NotFound(format!("portfolio {portfolio_id}")));
    }
    Ok(())
}

async fn own_rebalance_or_404(state: &AppState, user_id: Uuid, rebalance_id: Uuid) -> Result<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM rebalances r
            JOIN portfolios p ON p.id = r.portfolio_id
            WHERE r.id = $1 AND p.user_id = $2
        )",
    )
    .bind(rebalance_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    if !exists {
        return Err(AppError::NotFound(format!("rebalance {rebalance_id}")));
    }
    Ok(())
}

async fn approval_safety(state: &AppState, rebalance_id: Uuid) -> Result<ApprovalSafety> {
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
        Ok(input) => input,
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

async fn history_approval_safety(
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

fn legs_match_current(stored: &[LegView], current: &[PlannedLeg]) -> bool {
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
                l.amount_usdc,
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

fn amount_matches(stored: f64, current: f64) -> bool {
    let tolerance = (current.abs() * 0.005).max(0.01);
    (stored - current).abs() <= tolerance
}

async fn build_plan_input(state: &AppState, portfolio_id: Uuid) -> Result<PlanInput> {
    // The planner consumes fractions (0-1), but the persisted
    // `current_weight` can lag behind execution. Use confirmed dollar values
    // when present so a just-executed or partially deployed portfolio cannot
    // plan from stale percentages.
    let portfolio: (Uuid, f64, serde_json::Value) = sqlx::query_as(
        "SELECT user_id, total_value_usd::DOUBLE PRECISION, goal FROM portfolios WHERE id = $1",
    )
    .bind(portfolio_id)
    .fetch_one(&state.db)
    .await?;
    let user_id = portfolio.0;
    let portfolio_value_usd = portfolio.1;
    let goal = portfolio.2;

    let allocations: Vec<(String, f64, f64, f64)> = sqlx::query_as(
        "SELECT asset_symbol, current_weight::DOUBLE PRECISION,
                value_usd::DOUBLE PRECISION, quantity::DOUBLE PRECISION
         FROM allocations WHERE portfolio_id = $1",
    )
    .bind(portfolio_id)
    .fetch_all(&state.db)
    .await?;

    let mut target_weights = HashMap::new();
    if let Some(target_obj) = goal.get("targetAllocation").and_then(|v| v.as_object()) {
        for (k, v) in target_obj {
            if let Some(n) = v.as_f64() {
                target_weights.insert(k.clone(), n / 100.0);
            }
        }
    }
    apply_route_preferences_to_targets(&goal, &mut target_weights);

    let relevant_symbols: Vec<String> = allocations
        .iter()
        .map(|(sym, _, _, _)| sym.clone())
        .chain(target_weights.keys().cloned())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let prices = load_planning_prices(state, &relevant_symbols).await;

    let allocation_values: Vec<(String, f64, f64)> = allocations
        .into_iter()
        .map(|(sym, weight, value_usd, quantity)| {
            let marked = marked_allocation_value(&sym, value_usd, quantity, &prices);
            (sym, weight, marked)
        })
        .collect();

    let allocation_value_sum: f64 = allocation_values.iter().map(|(_, _, v)| v.max(0.0)).sum();
    let invested_value_usd = if allocation_value_sum > 0.0 {
        allocation_value_sum
    } else {
        portfolio_value_usd
    };
    let mut invested_weights = HashMap::new();
    if invested_value_usd > 0.0 {
        for (sym, weight, value_usd) in allocation_values {
            let confirmed_value = if allocation_value_sum > 0.0 {
                value_usd.max(0.0)
            } else {
                (weight / 100.0) * portfolio_value_usd
            };
            invested_weights.insert(sym, confirmed_value / invested_value_usd);
        }
    }

    if target_weights.is_empty() {
        // Portfolios without a goal fall back to "stay where you are".
        target_weights = invested_weights.clone();
    }

    let usdc_per_chain = load_gateway_pool(state, user_id).await?;
    let idle_usdc: f64 = usdc_per_chain.values().copied().sum();
    let plan_value_usd = invested_value_usd + idle_usdc;
    let current_weights = if idle_usdc > 0.0 && plan_value_usd > 0.0 {
        invested_weights
            .into_iter()
            .map(|(sym, weight)| {
                let invested_value = weight * invested_value_usd;
                (sym, invested_value / plan_value_usd)
            })
            .collect()
    } else {
        invested_weights
    };

    // Latest classified regime drives the "let winners run" asymmetric bands.
    let regime: Option<String> = sqlx::query_scalar(
        "SELECT regime FROM agent_decisions
         WHERE portfolio_id = $1 AND regime IS NOT NULL
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(portfolio_id)
    .fetch_optional(&state.db)
    .await?
    .flatten();

    Ok(PlanInput {
        portfolio_value_usd: plan_value_usd,
        current_weights,
        target_weights,
        usdc_per_chain,
        drift_threshold: 0.05,
        dust_threshold_usd: 5.0,
        prices,
        regime,
    })
}

async fn load_planning_prices(state: &AppState, symbols: &[String]) -> HashMap<String, f64> {
    if symbols.is_empty() {
        return HashMap::new();
    }

    let mut prices = crate::modules::market_data::service::fetch_snapshot(state.prices.as_ref())
        .await
        .map(|snapshot| {
            snapshot
                .assets
                .into_iter()
                .map(|asset| (asset.symbol, asset.price_usd))
                .filter(|(_, price)| price.is_finite() && *price > 0.0)
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();

    if prices.len() < symbols.len() {
        if let Ok(history) = crate::modules::market_data::service::get_historical_prices(
            &state.db,
            symbols,
            chrono::Utc::now(),
        )
        .await
        {
            for (symbol, price) in history {
                prices.entry(symbol).or_insert(price);
            }
        }
    }

    prices
}

fn marked_allocation_value(
    symbol: &str,
    stored_value_usd: f64,
    quantity: f64,
    prices: &HashMap<String, f64>,
) -> f64 {
    let price = prices
        .get(symbol)
        .copied()
        .or_else(|| stable_planning_price(symbol));
    if quantity > 0.0 && stored_value_usd > 0.0 {
        if let Some(price) = price.filter(|p| p.is_finite() && *p > 0.0) {
            return quantity * price;
        }
    }
    stored_value_usd
}

fn stable_planning_price(symbol: &str) -> Option<f64> {
    match symbol {
        "USDC" | "USYC" => Some(1.0),
        _ => None,
    }
}

fn apply_route_preferences_to_targets(
    goal: &serde_json::Value,
    target_weights: &mut HashMap<String, f64>,
) {
    let Some(route_preferences) = goal.get("routePreferences") else {
        return;
    };

    let allowed_tokens = route_preference_set(route_preferences, "tokens");
    if !allowed_tokens.is_empty() {
        target_weights.retain(|symbol, _| {
            symbol == "USDC" || allowed_tokens.contains(&symbol.to_ascii_uppercase())
        });
    }

    let selected_networks = route_preference_set(route_preferences, "networks");
    if selected_networks.is_empty() {
        return;
    }

    let arc_allowed =
        selected_networks.contains(wallet_routes::ARC_TESTNET) || selected_networks.contains("ARC");
    let base_allowed = selected_networks.contains(wallet_routes::BASE_SEPOLIA)
        || selected_networks.contains("BASE");
    target_weights.retain(|symbol, _| {
        let symbol = symbol.as_str();
        if ARC_NATIVE_SYMBOLS.contains(&symbol) {
            return arc_allowed;
        }
        if BASE_NATIVE_SYMBOLS.contains(&symbol) {
            return base_allowed;
        }
        true
    });
}

fn route_preference_set(route_preferences: &serde_json::Value, key: &str) -> HashSet<String> {
    let mut values: HashSet<String> = route_preferences
        .get(key)
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .map(|v| v.trim().to_ascii_uppercase())
        .filter(|v| !v.is_empty())
        .collect();
    if values.remove("BTC_ETH_SOL") {
        values.insert("BTC".into());
        values.insert("ETH".into());
        values.insert("SOL".into());
    }
    values
}

/// Lookup unified USDC by chain from Circle Gateway. Real execution fails
/// closed when Gateway is unavailable; mock/demo mode degrades to a zero pool
/// so local review screens can still be exercised.
async fn load_gateway_pool(state: &AppState, user_id: Uuid) -> Result<HashMap<ChainKey, f64>> {
    let mut pool: HashMap<ChainKey, f64> = HashMap::new();
    pool.insert(ChainKey::Arc, 0.0);
    pool.insert(ChainKey::Base, 0.0);

    match crate::modules::gateway::service::fetch_balance_for_user(
        &state.db,
        &state.http,
        &state.config,
        user_id,
    )
    .await
    {
        Ok(b) => {
            for (chain, amount) in b.per_chain {
                if let Some(key) = ChainKey::parse(chain.to_lowercase().as_str()) {
                    pool.insert(key, amount);
                }
            }
        }
        Err(e) => {
            if !state.config.execution_mock && !state.config.circle_mock {
                return Err(AppError::Conflict(
                    "Gateway balance is unavailable, so Aegis cannot build a real rebalance plan safely. Retry after Circle Gateway responds."
                        .into(),
                ));
            }
            tracing::warn!(error=%e, ?user_id, "gateway balance fetch failed; mock planner will use zero pool");
        }
    }
    Ok(pool)
}

fn plan_leg_view(leg: &PlannedLeg) -> PlanLegView {
    PlanLegView {
        leg_index: leg.leg_index,
        kind: leg.kind.as_str().to_string(),
        src_chain: leg.src_chain.map(|c| c.as_str().to_string()),
        dest_chain: leg.dest_chain.map(|c| c.as_str().to_string()),
        src_symbol: leg.src_symbol.clone(),
        dest_symbol: leg.dest_symbol.clone(),
        amount_usdc: leg.amount_usdc,
    }
}

fn plan_leg_view_from_row(leg: &LegView) -> PlanLegView {
    PlanLegView {
        leg_index: leg.leg_index,
        kind: leg.kind.clone(),
        src_chain: leg.src_chain.clone(),
        dest_chain: leg.dest_chain.clone(),
        src_symbol: leg.src_symbol.clone(),
        dest_symbol: leg.dest_symbol.clone(),
        amount_usdc: leg.amount_usdc,
    }
}

async fn reusable_planned_rebalance(
    state: &AppState,
    portfolio_id: Uuid,
    current_legs: &[PlannedLeg],
) -> Result<Option<PlanResponse>> {
    let Some(plan) = sqlx::query_as::<_, RebalanceView>(
        "SELECT id, portfolio_id, decision_id, status, total_legs, completed_legs,
                total_gas_usdc, failure_reason, approved_at, completed_at,
                created_at, updated_at, NULL::text as protocol_fee_settlement_tx
         FROM rebalances
         WHERE portfolio_id = $1 AND status = 'planned'
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(portfolio_id)
    .fetch_optional(&state.db)
    .await?
    else {
        return Ok(None);
    };

    if !decision_can_be_reused(state, plan.decision_id).await? {
        return Ok(None);
    }

    let stored_legs: Vec<LegView> = sqlx::query_as(
        "SELECT id, rebalance_id, leg_index, kind, src_chain, dest_chain,
                src_symbol, dest_symbol, amount_usdc, status, tx_hash,
                failure_reason, submitted_at, confirmed_at
         FROM rebalance_legs WHERE rebalance_id = $1
         ORDER BY leg_index ASC",
    )
    .bind(plan.id)
    .fetch_all(&state.db)
    .await?;

    if !legs_match_current(&stored_legs, current_legs) {
        return Ok(None);
    }

    Ok(Some(PlanResponse {
        rebalance_id: plan.id,
        decision_id: plan.decision_id,
        execution_mode: execution_mode(state).to_string(),
        total_legs: plan.total_legs,
        legs: stored_legs.iter().map(plan_leg_view_from_row).collect(),
    }))
}

async fn decision_can_be_reused(state: &AppState, decision_id: Uuid) -> Result<bool> {
    let model_slug: Option<String> =
        sqlx::query_scalar("SELECT model_slug FROM agent_decisions WHERE id = $1")
            .bind(decision_id)
            .fetch_one(&state.db)
            .await?;
    if !state.config.execution_mock && !state.config.circle_mock {
        return Ok(model_slug.as_deref() == Some("aegis/rebalance-planner-v1"));
    }
    Ok(true)
}

fn execution_mode(state: &AppState) -> &'static str {
    if state.config.execution_mock || state.config.circle_mock {
        "mock"
    } else {
        "real"
    }
}

async fn rebalance_totals_by_id(
    state: &AppState,
    rebalance_ids: &[Uuid],
) -> Result<HashMap<Uuid, f64>> {
    if rebalance_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows: Vec<(Uuid, f64)> = sqlx::query_as(
        "SELECT rebalance_id, COALESCE(SUM(amount_usdc), 0)::DOUBLE PRECISION
         FROM rebalance_legs
         WHERE rebalance_id = ANY($1) AND kind != 'cross_chain_mint'
         GROUP BY rebalance_id",
    )
    .bind(rebalance_ids)
    .fetch_all(&state.db)
    .await?;
    Ok(rows.into_iter().collect())
}

fn noop_plan_message(input: &PlanInput) -> String {
    let idle_usdc: f64 = input.usdc_per_chain.values().copied().sum();
    if input.portfolio_value_usd <= input.dust_threshold_usd
        && idle_usdc <= input.dust_threshold_usd
    {
        return "No rebalance plan was created because this portfolio has no confirmed positions and no deployable USDC above the $5 dust threshold. Fund the wallet first, then review deployment.".into();
    }
    if idle_usdc > 0.0 && idle_usdc <= input.dust_threshold_usd {
        return format!(
            "No rebalance plan was created because only ${idle_usdc:.2} USDC is idle, below the ${:.2} dust threshold.",
            input.dust_threshold_usd
        );
    }
    "No rebalance plan was created because current weights, target weights, and idle USDC are already within the execution thresholds.".into()
}

async fn ensure_rebalance_wallet_ready(state: &AppState, user_id: Uuid) -> Result<()> {
    if state.config.execution_mock || state.config.circle_mock {
        return Ok(());
    }
    let user_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;
    if !user_exists {
        return Err(AppError::Unauthorized("unknown user".into()));
    }
    if wallet_routes::user_has_arc_and_base(&state.db, user_id, &state.config.circle_wallet_set_id)
        .await?
    {
        return Ok(());
    }
    Err(AppError::Conflict(
        "Complete account setup before building a rebalance plan. This account still has no real Arc + Base wallet ready for execution."
            .into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn planning_value_prefers_live_price_and_stable_fallback() {
        let mut prices = HashMap::new();
        prices.insert("BTC".to_string(), 77_000.0);

        assert_eq!(marked_allocation_value("BTC", 840.0, 0.01, &prices), 770.0);
        assert_eq!(
            marked_allocation_value("USYC", 60.0, 60.0, &HashMap::new()),
            60.0
        );
        assert_eq!(
            marked_allocation_value("ETH", 300.0, 0.1, &HashMap::new()),
            300.0
        );
    }

    #[test]
    fn route_preferences_filter_unselected_target_tokens() {
        let goal = json!({
            "targetAllocation": {"USDC": 40, "BTC": 30, "ETH": 20, "USYC": 10},
            "routePreferences": {
                "networks": ["ARC-TESTNET", "BASE-SEPOLIA"],
                "tokens": ["USDC", "USYC"],
                "watchlist": ["BTC_ETH_SOL"]
            }
        });
        let mut targets = HashMap::from([
            ("USDC".to_string(), 0.40),
            ("BTC".to_string(), 0.30),
            ("ETH".to_string(), 0.20),
            ("USYC".to_string(), 0.10),
        ]);

        apply_route_preferences_to_targets(&goal, &mut targets);

        assert!(targets.contains_key("USDC"));
        assert!(targets.contains_key("USYC"));
        assert!(!targets.contains_key("BTC"));
        assert!(!targets.contains_key("ETH"));
    }

    #[test]
    fn route_preferences_filter_targets_by_selected_execution_networks() {
        let goal = json!({
            "routePreferences": {
                "networks": ["ARC-TESTNET"],
                "tokens": ["BTC_ETH_SOL", "USYC", "EURC"]
            }
        });
        let mut targets = HashMap::from([
            ("BTC".to_string(), 0.30),
            ("ETH".to_string(), 0.20),
            ("USYC".to_string(), 0.30),
            ("EURC".to_string(), 0.20),
        ]);

        apply_route_preferences_to_targets(&goal, &mut targets);

        assert_eq!(
            targets.keys().cloned().collect::<HashSet<_>>(),
            HashSet::from(["USYC".to_string(), "EURC".to_string()])
        );
    }

    fn planned_leg(
        kind: crate::modules::rebalance::models::LegKind,
        chain: ChainKey,
    ) -> PlannedLeg {
        PlannedLeg {
            leg_index: 0,
            kind,
            src_chain: Some(chain),
            dest_chain: Some(chain),
            src_symbol: Some("USDC".into()),
            dest_symbol: Some("ETH".into()),
            amount_usdc: 10.0,
            min_out: None,
        }
    }

    #[test]
    fn plan_route_surface_uses_single_chain_from_plan_legs() {
        assert_eq!(
            plan_route_surface(&[planned_leg(
                crate::modules::rebalance::models::LegKind::LocalSwap,
                ChainKey::Base,
            )]),
            "Base transaction"
        );
        assert_eq!(
            plan_route_surface(&[planned_leg(
                crate::modules::rebalance::models::LegKind::LocalSwap,
                ChainKey::Arc,
            )]),
            "Arc transaction"
        );
    }
}
