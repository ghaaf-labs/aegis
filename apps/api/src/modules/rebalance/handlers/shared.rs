use std::collections::HashMap;

use rust_decimal::prelude::ToPrimitive;
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::modules::rebalance::models::PlannedLeg;
use crate::modules::wallet_routes;
use crate::router::AppState;

use super::{approval::legs_match_current, LegView, PlanLegView, PlanResponse, RebalanceView};

pub(super) async fn own_portfolio_or_404(
    state: &AppState,
    user_id: Uuid,
    portfolio_id: Uuid,
) -> Result<()> {
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

pub(super) async fn own_rebalance_or_404(
    state: &AppState,
    user_id: Uuid,
    rebalance_id: Uuid,
) -> Result<()> {
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

pub(super) async fn ensure_no_active_execution(state: &AppState, portfolio_id: Uuid) -> Result<()> {
    let active: Option<(Uuid, i32, i32)> = sqlx::query_as(
        "SELECT id, completed_legs, total_legs
         FROM rebalances
         WHERE portfolio_id = $1 AND status = 'executing'
         ORDER BY approved_at DESC NULLS LAST, created_at DESC
         LIMIT 1",
    )
    .bind(portfolio_id)
    .fetch_optional(&state.db)
    .await?;

    if let Some((id, completed, total)) = active {
        return Err(AppError::Conflict(format!(
            "Rebalance {id} is already executing ({completed}/{total} legs confirmed). Open the trace and wait for it to finish before building another review."
        )));
    }

    Ok(())
}

pub(super) async fn reusable_planned_rebalance(
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

pub(super) async fn decision_can_be_reused(state: &AppState, decision_id: Uuid) -> Result<bool> {
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

pub(super) async fn rebalance_totals_by_id(
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

pub(super) fn execution_mode(state: &AppState) -> &'static str {
    if state.config.execution_mock || state.config.circle_mock {
        "mock"
    } else {
        "real"
    }
}

/// Why a plan produced zero executable legs. The single classifier consumed by
/// both the human message (`noop_plan_message`) and the typed HTTP outcome
/// (`PlanOutcome::from_noop`) so the two can never drift. None of these are
/// errors — a no-op is a legitimate 200 result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NoopReason {
    /// No confirmed positions and no deployable USDC — wallet needs funding.
    Unfunded,
    /// Approved target is a USDC reserve — wallet cash is already in target.
    UsdcReserve,
    /// Only sub-dust USDC is idle — below the minimum move size.
    DustOnly,
    /// Holdings already match the target within the execution thresholds.
    OnTarget,
}

pub(super) fn classify_noop(input: &crate::modules::rebalance::models::PlanInput) -> NoopReason {
    let idle_usdc: f64 = input.usdc_per_chain.values().copied().sum();
    if input.portfolio_value_usd <= input.dust_threshold_usd
        && idle_usdc <= input.dust_threshold_usd
    {
        return NoopReason::Unfunded;
    }
    let non_usdc_target = input.target_weights.iter().any(|(symbol, weight)| {
        symbol != "USDC" && weight * input.portfolio_value_usd > input.dust_threshold_usd
    });
    if idle_usdc > input.dust_threshold_usd && !input.target_weights.is_empty() && !non_usdc_target
    {
        return NoopReason::UsdcReserve;
    }
    if idle_usdc > 0.0 && idle_usdc <= input.dust_threshold_usd {
        return NoopReason::DustOnly;
    }
    NoopReason::OnTarget
}

pub(super) fn noop_plan_message(input: &crate::modules::rebalance::models::PlanInput) -> String {
    match classify_noop(input) {
        NoopReason::Unfunded => "No rebalance plan was created because this portfolio has no confirmed positions and no deployable USDC above the $5 dust threshold. Fund the wallet first, then review deployment.".into(),
        NoopReason::UsdcReserve => "No rebalance plan was created because the approved target is a USDC reserve, so wallet cash is already in the target asset and no market move is required.".into(),
        NoopReason::DustOnly => {
            let idle_usdc: f64 = input.usdc_per_chain.values().copied().sum();
            format!(
                "No rebalance plan was created because only ${idle_usdc:.2} USDC is idle, below the ${:.2} dust threshold.",
                input.dust_threshold_usd
            )
        }
        NoopReason::OnTarget => "No rebalance plan was created because current weights, target weights, and idle USDC are already within the execution thresholds.".into(),
    }
}

pub(super) async fn ensure_rebalance_wallet_ready(state: &AppState, user_id: Uuid) -> Result<()> {
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

pub(super) fn plan_leg_view(leg: &PlannedLeg) -> PlanLegView {
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

pub(super) fn plan_leg_view_from_row(leg: &LegView) -> PlanLegView {
    PlanLegView {
        leg_index: leg.leg_index,
        kind: leg.kind.clone(),
        src_chain: leg.src_chain.clone(),
        dest_chain: leg.dest_chain.clone(),
        src_symbol: leg.src_symbol.clone(),
        dest_symbol: leg.dest_symbol.clone(),
        amount_usdc: leg.amount_usdc.to_f64().unwrap_or(0.0),
    }
}
