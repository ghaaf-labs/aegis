//! HTTP handlers for rebalance plan / execute / poll / history.
//!
//! Every route is scoped to the authenticated user — ownership of the
//! portfolio is enforced on each lookup so a JWT for user A can never read
//! or execute a rebalance belonging to user B.

use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::middleware::auth::Claims;
use crate::modules::agent::{models::AnalyzeRequest, service::analyze_portfolio};
use crate::modules::rebalance::{
    executor::{approve_and_execute, create_plan},
    models::{ChainKey, PlanInput, PlannedLeg},
    planner::plan_legs,
};
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
    let input = build_plan_input(&state, portfolio_id).await?;
    let legs = plan_legs(&input);
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalSafety {
    pub approvable: bool,
    pub code: String,
    pub message: String,
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
) -> Result<Json<Vec<RebalanceView>>> {
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
    Ok(Json(rows))
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
            "Aegis built this execution review from live portfolio values (${:.2}) and idle Gateway USDC (${:.2}). The plan is deterministic and remains gated by user approval before any Arc/Base transaction is submitted.",
            input.portfolio_value_usd, idle_usdc
        )
    };
    let rec = json!({
        "summary": summary,
        "trades": trades,
        "expectedImpact": { "riskDelta": 0.0, "diversificationScore": 0.5 }
    });
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
        });
    }

    let current_input = build_plan_input(state, plan.portfolio_id).await?;
    let current_legs = plan_legs(&current_input);
    if !legs_match_current(&stored_legs, &current_legs) {
        return Ok(ApprovalSafety {
            approvable: false,
            code: "STALE_PLAN".into(),
            message: "Portfolio holdings or Gateway cash changed after this plan was created. Build a fresh review before approving real execution.".into(),
        });
    }

    Ok(ApprovalSafety {
        approvable: true,
        code: "APPROVABLE".into(),
        message: "Plan matches current holdings and Gateway cash.".into(),
    })
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

    let allocations: Vec<(String, f64, f64)> = sqlx::query_as(
        "SELECT asset_symbol, current_weight::DOUBLE PRECISION, value_usd::DOUBLE PRECISION
         FROM allocations WHERE portfolio_id = $1",
    )
    .bind(portfolio_id)
    .fetch_all(&state.db)
    .await?;

    let allocation_value_sum: f64 = allocations.iter().map(|(_, _, v)| v.max(0.0)).sum();
    let invested_value_usd = if allocation_value_sum > 0.0 {
        allocation_value_sum
    } else {
        portfolio_value_usd
    };
    let mut invested_weights = HashMap::new();
    if invested_value_usd > 0.0 {
        for (sym, weight, value_usd) in allocations {
            let confirmed_value = if allocation_value_sum > 0.0 {
                value_usd.max(0.0)
            } else {
                (weight / 100.0) * portfolio_value_usd
            };
            invested_weights.insert(sym, confirmed_value / invested_value_usd);
        }
    }

    let mut target_weights = HashMap::new();
    if let Some(target_obj) = goal.get("targetAllocation").and_then(|v| v.as_object()) {
        for (k, v) in target_obj {
            if let Some(n) = v.as_f64() {
                target_weights.insert(k.clone(), n / 100.0);
            }
        }
    }
    if target_weights.is_empty() {
        // Portfolios without a goal fall back to "stay where you are".
        target_weights = invested_weights.clone();
    }

    let usdc_per_chain = load_gateway_pool(state, user_id).await;
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

    // Populate recent prices from price_history (dense table from Phase 1).
    // This lets the planner compute sensible min_out values for cross-chain
    // hook legs and local swaps when EXECUTION_MOCK=false.
    let relevant_symbols: Vec<String> = current_weights
        .keys()
        .chain(target_weights.keys())
        .cloned()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let prices = if relevant_symbols.is_empty() {
        HashMap::new()
    } else {
        crate::modules::market_data::service::get_historical_prices(
            &state.db,
            &relevant_symbols,
            chrono::Utc::now(),
        )
        .await
        .unwrap_or_default()
    };

    Ok(PlanInput {
        portfolio_value_usd: plan_value_usd,
        current_weights,
        target_weights,
        usdc_per_chain,
        drift_threshold: 0.05,
        dust_threshold_usd: 5.0,
        prices,
    })
}

/// Best-effort lookup of unified USDC by chain from Circle Gateway.
/// Returns a zero pool on any failure so the planner still degrades to
/// single-chain rebalances rather than 5xx-ing the user.
async fn load_gateway_pool(state: &AppState, user_id: Uuid) -> HashMap<ChainKey, f64> {
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
            tracing::warn!(error=%e, ?user_id, "gateway balance fetch failed; planner will use zero pool");
        }
    }
    pool
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

fn execution_mode(state: &AppState) -> &'static str {
    if state.config.execution_mock || state.config.circle_mock {
        "mock"
    } else {
        "real"
    }
}
