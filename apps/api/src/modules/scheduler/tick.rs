//! Per-portfolio drift/regime/harvest watcher.
//!
//! Triggers:
//! 1. Any allocation's `target_weight - current_weight` exceeds the drift
//!    threshold (default 5%).
//! 2. A `regime.flip` was classified more recently than the last decision.
//! 3. Total harvestable losses on the portfolio exceed `HARVEST_THRESHOLD_USD`.
//!
//! Cooldown: 30 minutes per portfolio, in-memory. Restart clears the
//! cooldowns; that's acceptable since the drift/regime triggers are
//! self-recovering — the next tick will fire again if conditions persist.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::modules::agent::models::{AgentDecision, AnalyzeRequest, ProposeAllocationRequest};
use crate::modules::agent::service::{
    analyze_portfolio, apply_allocation, propose_allocation, CriticOutput,
};
use crate::modules::rebalance::executor::approve_and_execute;
use crate::modules::rebalance::handlers::{prepare_autonomous_plan, AutonomousPlan};
use crate::router::AppState;

/// Last-decision-emitted instant per portfolio.
#[derive(Default)]
pub struct CooldownMap {
    inner: DashMap<Uuid, Instant>,
}

impl CooldownMap {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: DashMap::new(),
        })
    }
    pub fn touch(&self, portfolio_id: Uuid) {
        self.inner.insert(portfolio_id, Instant::now());
    }
    pub fn within(&self, portfolio_id: Uuid, window: Duration) -> bool {
        match self.inner.get(&portfolio_id) {
            Some(t) => t.elapsed() < window,
            None => false,
        }
    }
}

/// Spawn the long-running per-portfolio watcher.
pub fn spawn_portfolio_scheduler(state: AppState, cancel: CancellationToken) -> Arc<CooldownMap> {
    let cooldowns = CooldownMap::new();
    let st = state.clone();
    let cd = cooldowns.clone();
    tokio::spawn(async move {
        let tick = Duration::from_secs(st.config.scheduler_tick_secs);
        let window = Duration::from_secs(st.config.scheduler_cooldown_secs);
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("scheduler shutting down");
                    return;
                }
                _ = tokio::time::sleep(tick) => {}
            }

            // Skip portfolios whose owning user has paused the agent for all of
            // that user's portfolios (FE-PAUSE-1). Manual /agent/analyze +
            // /rebalance/:id/execute are unaffected — only the scheduled trigger
            // is gated here. Auto-pilot portfolios are scanned even at $0 invested
            // value so a first deployment of idle Gateway cash can fire.
            let active: Vec<(Uuid, Uuid, bool)> = match sqlx::query_as(
                "SELECT p.id, p.user_id, u.auto_pilot_enabled \
                 FROM portfolios p \
                 JOIN users u ON u.id = p.user_id \
                 WHERE (p.total_value_usd > 0 OR u.auto_pilot_enabled) \
                   AND u.agent_paused_at IS NULL",
            )
            .fetch_all(&st.db)
            .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(error=%e, "scheduler: portfolio fetch failed");
                    continue;
                }
            };

            for (portfolio_id, user_id, auto_pilot) in active {
                if cd.within(portfolio_id, window) {
                    continue;
                }
                let triggered = match evaluate(&st, portfolio_id).await {
                    Ok(Some(reason)) => reason,
                    Ok(None) => continue,
                    Err(e) => {
                        tracing::warn!(?portfolio_id, error=%e, "scheduler eval failed");
                        continue;
                    }
                };
                tracing::info!(?portfolio_id, reason=%triggered, auto_pilot, "scheduler firing");

                if auto_pilot {
                    // Auto-pilot ON: the agent acts on its own within the
                    // guardrails. On any failure it leaves a review behind
                    // (the proposal / planned rebalance), exactly the OFF path.
                    if let Err(e) = run_autopilot(&st, portfolio_id, user_id, &triggered).await {
                        tracing::warn!(?portfolio_id, error=%e, "auto-pilot run failed; left as review");
                    }
                } else if let Err(e) = analyze_portfolio(
                    &st,
                    AnalyzeRequest {
                        portfolio_id,
                        triggered_by: Some(triggered),
                    },
                )
                .await
                {
                    tracing::warn!(?portfolio_id, error=%e, "analyze_portfolio failed");
                    continue;
                }
                cd.touch(portfolio_id);
            }
        }
    });
    cooldowns
}

/// Autonomous (auto-pilot) handling for one triggered portfolio.
///
/// The agent proposes a fresh target, adopts it, builds a real rebalance plan,
/// and — only when every guardrail clears — executes it on the *exact same*
/// path the manual approval endpoint uses. Any guardrail miss falls back to
/// leaving a review (the proposal + any planned rebalance) for the user, which
/// is the auto-pilot-OFF behavior.
///
/// Fail-safes that downgrade to review-only (never auto-execute):
/// - the constitution flags the clamped allocation,
/// - a depeg is active for this user (defer to peg defense),
/// - approval-safety is not `approvable` (non-executable route, stale plan,
///   balance unavailable, superseded, mock/legacy decision),
/// - nothing to move (on-target or sub-$5 dust).
async fn run_autopilot(
    state: &AppState,
    portfolio_id: Uuid,
    user_id: Uuid,
    triggered_by: &str,
) -> crate::error::Result<()> {
    // Defer to peg defense during an active depeg rather than rebalancing into
    // a destabilized market.
    if peg_defense_active(state, user_id).await {
        tracing::info!(
            ?portfolio_id,
            "auto-pilot: depeg active; deferring to peg defense"
        );
        return Ok(());
    }

    // 1. Propose a fresh target. This runs the allocator, the deterministic
    //    clamp (single-asset cap, stable floor, executable-only), and the
    //    constitution check — all surfaced over SSE for the activity feed.
    let proposal = propose_allocation(
        state,
        ProposeAllocationRequest {
            portfolio_id,
            triggered_by: Some(format!("autopilot:{triggered_by}")),
            risk_override: None,
        },
    )
    .await?;

    // 2. Constitution gate (C4): never auto-adopt/execute an allocation the
    //    constitution flags. The proposal stays as a Gate-1 review for the user.
    if !proposal_constitution_clean(&proposal) {
        tracing::info!(?portfolio_id, decision_id=?proposal.id, "auto-pilot: constitution flagged proposal; left as review");
        return Ok(());
    }

    // 3. Adopt the target (idempotent; the user's own approval would do the same).
    apply_allocation(state, proposal.id, user_id).await?;

    // 4. Build the real rebalance plan toward the new target + its safety verdict.
    let prepared = prepare_autonomous_plan(state, portfolio_id).await?;
    let (rebalance_id, safety) = match prepared {
        AutonomousPlan::NoOp => {
            tracing::info!(
                ?portfolio_id,
                "auto-pilot: target adopted, nothing to move (on-target / dust)"
            );
            return Ok(());
        }
        AutonomousPlan::Prepared {
            rebalance_id,
            safety,
        } => (rebalance_id, safety),
    };

    // 5. Approval-safety gate: only execute a plan the manual flow would accept.
    if !safety.approvable {
        tracing::info!(
            ?portfolio_id,
            ?rebalance_id,
            code = %safety.code,
            "auto-pilot: plan not approvable; left as review"
        );
        return Ok(());
    }

    // 6. Execute via the exact path the approval endpoint uses.
    approve_and_execute(state.clone(), rebalance_id).await?;
    tracing::info!(
        ?portfolio_id,
        ?rebalance_id,
        "auto-pilot: executing rebalance autonomously"
    );
    Ok(())
}

/// True when the proposal's constitution verdict cites no violated clauses.
///
/// The verdict is a serialized [`CriticOutput`], which renders camelCase
/// (`clauseIds`) and omits the field entirely when empty. We deserialize the
/// canonical type rather than poking at a raw JSON key so the gate can never
/// silently drift from the producer's field names again (a prior version read
/// `clause_ids` and so always passed). A verdict that is absent reads as clean
/// (nothing flagged); a verdict that is present but fails to parse fails closed
/// (treated as flagged) on this autonomous execution path.
fn proposal_constitution_clean(decision: &AgentDecision) -> bool {
    let Some(verdict) = decision.critic_verdict.as_ref() else {
        return true;
    };
    match serde_json::from_value::<CriticOutput>(verdict.clone()) {
        Ok(critic) => critic.constitution_clean(),
        Err(_) => false,
    }
}

/// Whether a peg-defense rule fired for this user within the fire cooldown — a
/// proxy for "a depeg is active right now". Fails **closed** on the autonomous
/// path: a query error is treated as "depeg active" so auto-pilot defers rather
/// than rebalancing into a possibly-destabilized market on incomplete data.
async fn peg_defense_active(state: &AppState, user_id: Uuid) -> bool {
    if !state.config.peg_defense_enabled {
        return false;
    }
    let cooldown_secs = state.config.peg_fire_cooldown_secs.max(60);
    match sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(
            SELECT 1 FROM peg_events e
            JOIN peg_rules r ON r.id = e.rule_id
            WHERE r.user_id = $1
              AND e.action_taken IN ('propose_rebalance', 'auto_execute')
              AND e.observed_at > NOW() - ($2 || ' seconds')::interval
         )",
    )
    .bind(user_id)
    .bind(cooldown_secs.to_string())
    .fetch_one(&state.db)
    .await
    {
        Ok(active) => active,
        Err(e) => {
            tracing::warn!(
                ?user_id,
                error = %e,
                "auto-pilot: peg-status query failed; treating depeg as active (fail-closed)"
            );
            true
        }
    }
}

/// Inspect a single portfolio; return `Some(reason)` if any trigger fires.
pub async fn evaluate(
    state: &AppState,
    portfolio_id: Uuid,
) -> crate::error::Result<Option<String>> {
    // Drift trigger. `target_weight` / `current_weight` are stored 0–100
    // (DB CHECK constraint); normalize to fractions before comparing to
    // the 0.05 (5%) threshold the planner uses.
    let max_drift: Option<f64> = sqlx::query_scalar(
        "SELECT COALESCE(MAX(ABS(target_weight - current_weight)) / 100.0, 0)::DOUBLE PRECISION
         FROM allocations WHERE portfolio_id = $1",
    )
    .bind(portfolio_id)
    .fetch_one(&state.db)
    .await?;
    if max_drift.unwrap_or(0.0) >= 0.05 {
        return Ok(Some("drift_threshold".into()));
    }

    // Regime-flip trigger (M1). If the latest decision's regime is older than
    // the freshest market_snapshots entry's classified regime, fire. Cheap
    // heuristic: if there has been no decision in the last hour at all, fire
    // unconditionally — the regime might have shifted.
    let stale: Option<bool> = sqlx::query_scalar(
        "SELECT NOT EXISTS(
            SELECT 1 FROM agent_decisions
            WHERE portfolio_id = $1 AND created_at > NOW() - INTERVAL '1 hour'
         )",
    )
    .bind(portfolio_id)
    .fetch_optional(&state.db)
    .await?;
    if stale.unwrap_or(false) {
        return Ok(Some("regime_flip".into()));
    }

    // Harvest trigger.
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM portfolios WHERE id = $1")
        .bind(portfolio_id)
        .fetch_optional(&state.db)
        .await?;
    if let Some(uid) = owner {
        let total =
            crate::modules::tax::service::total_harvestable_usd(state, uid, portfolio_id).await?;
        if total >= state.config.harvest_threshold_usd {
            return Ok(Some("harvest_threshold".into()));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    fn decision_with_verdict(verdict: Option<serde_json::Value>) -> AgentDecision {
        AgentDecision {
            id: Uuid::new_v4(),
            portfolio_id: Uuid::new_v4(),
            reasoning: String::new(),
            recommendation: json!({}),
            confidence: 1.0,
            triggered_by: "test".into(),
            created_at: Utc::now(),
            model_slug: None,
            regime: None,
            prompt_tokens: None,
            completion_tokens: None,
            latency_ms: None,
            critic_verdict: verdict,
            snapshot: json!({}),
            raw_confidence: None,
            calibrated_confidence: None,
            counterfactual: None,
            kind: Some("allocation_proposal".into()),
            recommended_allocation: None,
            allocation_applied_at: None,
            status: Some("ready".into()),
            error: None,
            route_states: None,
        }
    }

    #[test]
    fn flagged_clause_ids_block_autonomous_adoption() {
        // The producer serializes CriticOutput as camelCase `clauseIds`. A
        // populated list means the constitution flagged the allocation, so the
        // gate must report "not clean" (auto-pilot leaves it as a review). This
        // is the regression: the prior gate read `clause_ids` and so always
        // passed even with `{"clauseIds":["RISK-1"]}`.
        let decision = decision_with_verdict(Some(json!({
            "demandsRevision": false,
            "notes": "Constitution advisories after clamp: RISK-1",
            "confidence": 1.0,
            "clauseIds": ["RISK-1"],
            "verdict": "advise",
        })));
        assert!(!proposal_constitution_clean(&decision));
    }

    #[test]
    fn clean_verdict_permits_autonomous_adoption() {
        // A clean verdict omits `clauseIds` entirely (skip_serializing_if).
        let decision = decision_with_verdict(Some(json!({
            "demandsRevision": false,
            "notes": "Constitution clean (allocation clamped to policy).",
            "confidence": 1.0,
            "verdict": "approve",
        })));
        assert!(proposal_constitution_clean(&decision));
        // Empty list is equivalent to absent.
        let empty = decision_with_verdict(Some(json!({ "clauseIds": [] })));
        assert!(proposal_constitution_clean(&empty));
    }

    #[test]
    fn absent_verdict_is_clean_but_unparsable_fails_closed() {
        // No verdict recorded → nothing flagged → clean.
        assert!(proposal_constitution_clean(&decision_with_verdict(None)));
        // A present-but-malformed verdict (not a CriticOutput object) fails
        // closed on the autonomous path.
        assert!(!proposal_constitution_clean(&decision_with_verdict(Some(
            json!("not an object")
        ))));
    }
}
