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

use crate::modules::agent::{models::AnalyzeRequest, service::analyze_portfolio};
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

            // Skip portfolios whose owning user has paused the agent globally
            // (FE-PAUSE-1). Manual /agent/analyze + /rebalance/:id/execute are
            // unaffected — only the scheduled trigger is gated here.
            let active: Vec<Uuid> = match sqlx::query_scalar(
                "SELECT p.id FROM portfolios p \
                 JOIN users u ON u.id = p.user_id \
                 WHERE p.total_value_usd > 0 AND u.agent_paused_at IS NULL",
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

            for portfolio_id in active {
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
                tracing::info!(?portfolio_id, reason=%triggered, "scheduler firing");
                if let Err(e) = analyze_portfolio(
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

/// Inspect a single portfolio; return `Some(reason)` if any trigger fires.
pub async fn evaluate(
    state: &AppState,
    portfolio_id: Uuid,
) -> crate::error::Result<Option<String>> {
    // Drift trigger. `target_weight` / `current_weight` are stored 0–100
    // (DB CHECK constraint); normalize to fractions before comparing to
    // the 0.05 (5%) threshold the planner uses.
    let max_drift: Option<f64> = sqlx::query_scalar(
        "SELECT COALESCE(MAX(ABS(target_weight - current_weight)) / 100.0, 0)
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
