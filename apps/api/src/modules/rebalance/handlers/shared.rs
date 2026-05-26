use std::collections::{HashMap, HashSet};

use rust_decimal::prelude::ToPrimitive;
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::modules::rebalance::models::{PlanInput, PlannedLeg, SellSources};
use crate::modules::rebalance::routing::EngineDeferred;
use crate::modules::wallet_routes;
use crate::router::AppState;

use super::{
    approval::legs_match_current, outcome::DeferredTarget, LegView, PlanLegView, PlanResponse,
    RebalanceView,
};

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
        "SELECT id, rebalance_id, leg_index, depends_on, kind, src_chain, dest_chain,
                src_symbol, dest_symbol, amount_usdc, min_out, status, leg_state, tx_hash,
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

/// Bind a freshly-created plan to the routability and quote-price buckets it
/// was built against (INV-6).
/// Both the manual handler and the auto-pilot path call this immediately after
/// `replace_planned_review`, so a rail flip
/// Ready⇄track-only or material price move after planning is caught at approval
/// for *either* path — never only manual reviews. Stamp only newly-created
/// plans: re-stamping a reused plan with the current snapshot would erase the
/// binding it must keep.
pub(super) async fn stamp_routable_snapshot(
    state: &AppState,
    rebalance_id: Uuid,
    prices: &HashMap<String, f64>,
    legs: &[PlannedLeg],
) -> Result<()> {
    let caps = crate::modules::rebalance::registry::RuntimeCapabilities::from_config(&state.config);
    let snapshot = crate::modules::rebalance::snapshot::RoutableSnapshot::capture_for_plan(
        &caps,
        &state.config,
        prices,
        legs,
    );
    sqlx::query("UPDATE rebalances SET routable_snapshot_hash = $1 WHERE id = $2")
        .bind(snapshot.hash())
        .bind(rebalance_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

pub(super) fn execution_mode(state: &AppState) -> &'static str {
    if state.config.execution_mock || state.config.circle_mock {
        "mock"
    } else {
        "real"
    }
}

pub(super) struct RouteShapedPlan {
    pub input: PlanInput,
    pub legs: Vec<PlannedLeg>,
    pub route_deferred: Vec<DeferredTarget>,
}

/// Build the deterministic plan, then iteratively freeze any leg whose live
/// route is unsafe and re-plan, so the safe legs still execute while only the
/// blocked sleeves are deferred to a USDC reserve. One un-executable sleeve must
/// never discard the rest of the rebalance. Each pass freezes at least one
/// blocked route (a blocked buy is pinned to its current weight; a blocked sell
/// source is frozen), so it converges in at most one pass per distinct
/// target/sell source.
pub(super) async fn route_shaped_plan(
    state: &AppState,
    user_id: Uuid,
    mut input: PlanInput,
) -> Result<RouteShapedPlan> {
    let mut frozen_deferred: Vec<DeferredTarget> = Vec::new();
    let max_passes = input.target_weights.len() + input.sell_sources.len() + 1;
    let mut plan = crate::modules::rebalance::routing::engine_plan(&state.config, &input);

    for _ in 0..max_passes {
        let blocks = crate::modules::rebalance::route_assessment::live_route_blocks(
            state, user_id, &input, &plan.legs,
        )
        .await?;
        if blocks.is_empty() {
            // Every remaining leg passed live assessment — ship them.
            return Ok(shaped_plan(input, plan, frozen_deferred));
        }
        let (adjusted, deferred) = freeze_blocked_routes(input, &blocks);
        frozen_deferred.extend(deferred);
        input = adjusted;
        plan = crate::modules::rebalance::routing::engine_plan(&state.config, &input);
    }

    // Unreachable in practice: each pass eliminates at least one block, and
    // `max_passes` exceeds the number of distinct routable nodes. If we somehow
    // never converge, ship zero legs (defer the whole intent) rather than risk
    // an unassessed, unsafe leg.
    tracing::warn!(%user_id, "route shaping did not converge; deferring all legs");
    plan.legs.clear();
    Ok(shaped_plan(input, plan, frozen_deferred))
}

/// Assemble the final shaped plan: safe legs + the frozen sleeves and the
/// engine's residual deferrals, de-duplicated so each symbol is listed once.
fn shaped_plan(
    input: PlanInput,
    plan: crate::modules::rebalance::routing::EnginePlan,
    mut deferred: Vec<DeferredTarget>,
) -> RouteShapedPlan {
    deferred.extend(engine_deferred_targets(&input, plan.deferred));
    dedup_deferred(&mut deferred);
    RouteShapedPlan {
        input,
        legs: plan.legs,
        route_deferred: deferred,
    }
}

/// Keep one deferral per symbol. The freeze loop and the engine's own residual
/// deferrals can name the same sleeve, which would otherwise list it twice.
fn dedup_deferred(deferred: &mut Vec<DeferredTarget>) {
    let mut seen: HashSet<String> = HashSet::new();
    deferred.retain(|d| seen.insert(d.symbol.to_ascii_lowercase()));
}

fn engine_deferred_targets(
    input: &PlanInput,
    deferred: Vec<EngineDeferred>,
) -> Vec<DeferredTarget> {
    deferred
        .into_iter()
        .map(|d| {
            let fallback_weight = d.amount_usd / input.portfolio_value_usd.max(1.0);
            let target_weight = match d.side {
                crate::modules::rebalance::routing::DeferredSide::Buy => {
                    input.target_weights.get(&d.symbol)
                }
                crate::modules::rebalance::routing::DeferredSide::Sell => {
                    input.current_weights.get(&d.symbol)
                }
            }
            .copied()
            .unwrap_or(fallback_weight);
            DeferredTarget {
                target_weight,
                symbol: d.symbol,
                reason: d.reason,
            }
        })
        .collect()
}

fn freeze_blocked_routes(
    mut input: PlanInput,
    blocks: &[crate::modules::rebalance::route_assessment::RouteBlock],
) -> (PlanInput, Vec<DeferredTarget>) {
    let mut route_deferred = Vec::new();
    for block in blocks {
        let original_target = input.target_weights.get(&block.symbol).copied();
        let current_weight = input
            .current_weights
            .get(&block.symbol)
            .copied()
            .unwrap_or(0.0);
        match block.side {
            crate::modules::rebalance::route_assessment::RouteBlockSide::Sell => {
                freeze_blocked_sell_source(&mut input, block);
            }
            crate::modules::rebalance::route_assessment::RouteBlockSide::Buy => {
                input
                    .target_weights
                    .insert(block.symbol.clone(), current_weight);
                input
                    .current_weights
                    .insert(block.symbol.clone(), current_weight);
            }
        }
        route_deferred.push(DeferredTarget {
            symbol: block.symbol.clone(),
            target_weight: original_target.unwrap_or(current_weight),
            reason: block.message.clone(),
        });
    }
    (input, route_deferred)
}

fn freeze_blocked_sell_source(
    input: &mut PlanInput,
    block: &crate::modules::rebalance::route_assessment::RouteBlock,
) {
    let Some(sources) = input.sell_sources.get_mut(&block.symbol) else {
        let current_weight = input
            .current_weights
            .get(&block.symbol)
            .copied()
            .unwrap_or(0.0);
        let target_weight = input
            .target_weights
            .get(&block.symbol)
            .copied()
            .unwrap_or(current_weight);
        input
            .target_weights
            .insert(block.symbol.clone(), target_weight);
        input
            .current_weights
            .insert(block.symbol.clone(), target_weight);
        input
            .sell_sources
            .insert(block.symbol.clone(), SellSources::Frozen);
        return;
    };
    match sources {
        SellSources::ByChain(by_chain) => {
            if let Some(chain) = block.chain {
                by_chain.remove(&chain);
            }
            if by_chain.is_empty() {
                *sources = SellSources::Frozen;
            }
        }
        SellSources::CanonicalFallback | SellSources::Frozen => {
            *sources = SellSources::Frozen;
        }
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
        NoopReason::UsdcReserve => "Your portfolio is on target — wallet cash is already in USDC, the reserve asset, so no market move is needed right now.".into(),
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
        deps: leg.deps.clone(),
        kind: leg.kind.as_str().to_string(),
        src_chain: leg.src_chain.map(|c| c.as_str().to_string()),
        dest_chain: leg.dest_chain.map(|c| c.as_str().to_string()),
        src_symbol: leg.src_symbol.clone(),
        dest_symbol: leg.dest_symbol.clone(),
        amount_usdc: leg.amount_usdc.to_f64().unwrap_or(0.0),
        min_out: leg.min_out.and_then(|m| m.to_f64()),
    }
}

pub(super) fn plan_leg_view_from_row(leg: &LegView) -> PlanLegView {
    PlanLegView {
        leg_index: leg.leg_index,
        deps: leg.depends_on.clone(),
        kind: leg.kind.clone(),
        src_chain: leg.src_chain.clone(),
        dest_chain: leg.dest_chain.clone(),
        src_symbol: leg.src_symbol.clone(),
        dest_symbol: leg.dest_symbol.clone(),
        amount_usdc: leg.amount_usdc.to_f64().unwrap_or(0.0),
        min_out: leg.min_out.and_then(|m| m.to_f64()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::modules::rebalance::{
        models::{ChainKey, PlanInput, SellSources},
        route_assessment::{RouteBlock, RouteBlockSide},
    };

    use super::super::outcome::DeferredTarget;
    use super::{dedup_deferred, freeze_blocked_routes};

    fn deferred(symbol: &str, weight: f64) -> DeferredTarget {
        DeferredTarget {
            symbol: symbol.into(),
            target_weight: weight,
            reason: format!("no route for {symbol}"),
        }
    }

    #[test]
    fn dedup_deferred_keeps_one_entry_per_symbol() {
        // The freeze loop and the engine's residual deferrals can both name the
        // same blocked sleeve; the user must not see it listed twice (the
        // "ETH, ETH" duplicate). Case-insensitive, first occurrence wins.
        let mut list = vec![
            deferred("cbBTC", 0.4),
            deferred("ETH", 0.3),
            deferred("eth", 0.3),
            deferred("ETH", 0.3),
        ];

        dedup_deferred(&mut list);

        let symbols: Vec<&str> = list.iter().map(|d| d.symbol.as_str()).collect();
        assert_eq!(symbols, vec!["cbBTC", "ETH"]);
    }

    fn input() -> PlanInput {
        let mut current_weights = HashMap::new();
        current_weights.insert("ETH".into(), 1.0);
        let mut target_weights = HashMap::new();
        target_weights.insert("ETH".into(), 0.2);
        target_weights.insert("cbBTC".into(), 0.8);
        let mut usdc_per_chain = HashMap::new();
        usdc_per_chain.insert(ChainKey::Base, 50.0);
        PlanInput {
            portfolio_value_usd: 1_000.0,
            current_weights,
            sell_sources: HashMap::new(),
            target_weights,
            usdc_per_chain,
            drift_threshold: 0.05,
            dust_threshold_usd: 5.0,
            prices: HashMap::new(),
            regime: None,
        }
    }

    #[test]
    fn freeze_blocked_sells_removes_only_the_unsafe_trim_delta() {
        let block = RouteBlock {
            leg_index: 0,
            side: RouteBlockSide::Sell,
            symbol: "ETH".into(),
            chain: Some(ChainKey::Base),
            amount_usd: 800.0,
            message: "bad ETH route".into(),
        };

        let (adjusted, deferred) = freeze_blocked_routes(input(), &[block]);

        assert_eq!(
            adjusted.current_weights["ETH"],
            adjusted.target_weights["ETH"]
        );
        assert_eq!(adjusted.target_weights["cbBTC"], 0.8);
        assert_eq!(adjusted.usdc_per_chain[&ChainKey::Base], 50.0);
        assert_eq!(deferred.len(), 1);
        assert_eq!(deferred[0].symbol, "ETH");
        assert!(deferred[0].reason.contains("bad ETH route"));
    }

    #[test]
    fn freeze_blocked_buys_removes_only_the_unsafe_acquire_delta() {
        let block = RouteBlock {
            leg_index: 1,
            side: RouteBlockSide::Buy,
            symbol: "cbBTC".into(),
            chain: Some(ChainKey::Base),
            amount_usd: 800.0,
            message: "bad cbBTC route".into(),
        };

        let (adjusted, deferred) = freeze_blocked_routes(input(), &[block]);

        assert_eq!(adjusted.target_weights["cbBTC"], 0.0);
        assert_eq!(adjusted.current_weights["cbBTC"], 0.0);
        assert_eq!(adjusted.target_weights["ETH"], 0.2);
        assert_eq!(adjusted.current_weights["ETH"], 1.0);
        assert_eq!(deferred.len(), 1);
        assert_eq!(deferred[0].symbol, "cbBTC");
        assert_eq!(deferred[0].target_weight, 0.8);
        assert!(deferred[0].reason.contains("bad cbBTC route"));
    }

    #[test]
    fn freeze_blocked_sell_removes_only_the_blocked_chain_slice() {
        let mut input = input();
        input.sell_sources.insert(
            "ETH".into(),
            SellSources::ByChain(HashMap::from([
                (ChainKey::Base, 500.0),
                (ChainKey::ArbSepolia, 500.0),
            ])),
        );
        let block = RouteBlock {
            leg_index: 0,
            side: RouteBlockSide::Sell,
            symbol: "ETH".into(),
            chain: Some(ChainKey::Base),
            amount_usd: 100.0,
            message: "bad Base ETH route".into(),
        };

        let (adjusted, deferred) = freeze_blocked_routes(input, &[block]);

        let SellSources::ByChain(by_chain) = &adjusted.sell_sources["ETH"] else {
            panic!("expected remaining Arb source");
        };
        assert!(!by_chain.contains_key(&ChainKey::Base));
        assert_eq!(by_chain[&ChainKey::ArbSepolia], 500.0);
        assert_eq!(adjusted.current_weights["ETH"], 1.0);
        assert_eq!(adjusted.target_weights["ETH"], 0.2);
        assert_eq!(deferred.len(), 1);
    }

    #[test]
    fn freeze_blocked_sell_keeps_empty_source_marker_when_last_chain_is_removed() {
        let mut input = input();
        input.sell_sources.insert(
            "ETH".into(),
            SellSources::ByChain(HashMap::from([(ChainKey::Base, 500.0)])),
        );
        let block = RouteBlock {
            leg_index: 0,
            side: RouteBlockSide::Sell,
            symbol: "ETH".into(),
            chain: Some(ChainKey::Base),
            amount_usd: 500.0,
            message: "bad Base ETH route".into(),
        };

        let (adjusted, deferred) = freeze_blocked_routes(input, &[block]);

        assert!(
            matches!(adjusted.sell_sources.get("ETH"), Some(SellSources::Frozen)),
            "explicit Frozen source prevents canonical-chain sell fallback"
        );
        assert_eq!(deferred.len(), 1);
    }
}
