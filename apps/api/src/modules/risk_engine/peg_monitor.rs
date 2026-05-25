//! Peg-defense monitor.
//!
//! Background task: every `PEG_MONITOR_TICK_SECS` seconds it samples the
//! current stablecoin prices (USDC, EURC, USYC), feeds them into a per-rule
//! rolling buffer, and fires when every sample in the rule's `window_seconds`
//! sits at or below `threshold_price`.
//!
//! Firing semantics:
//!
//! - `alert` — insert `peg_events` row + broadcast `peg.alert` SSE.
//! - `propose_rebalance` — build a defensive plan via the rebalance planner,
//!   persist it as a `planned` rebalance, surface the alert with the plan id.
//!   The user still has to hit `POST /rebalance/:id/execute` to approve.
//! - `auto_execute` — (Pro/Business only) propose the plan and immediately
//!   call the executor. If the `subscriptions` table is missing or the user
//!   is on a Free tier, downgrade to `propose_rebalance` and emit a warn log.
//!
//! After firing, `last_fired_at` is set so the same rule is throttled for
//! `peg_fire_cooldown_secs` seconds — prevents an alert storm when a peg
//! stays under threshold for hours.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::Db;
use crate::modules::rebalance::executor::create_plan;
use crate::modules::rebalance::models::{ChainKey, PlanInput, PlannedLeg};
use crate::modules::rebalance::registry::RuntimeCapabilities;
use crate::modules::rebalance::snapshot::RoutableSnapshot;
use crate::modules::sse::{PegAlertPayload, SseEvent};
use crate::router::AppState;

/// Symbols the monitor will sample on every tick.
pub const PEG_ASSETS: &[&str] = &["USDC", "EURC", "USYC"];

/// One (timestamp, price) sample in a rule's rolling window buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PegSample {
    pub observed_at: DateTime<Utc>,
    pub price: f64,
}

/// Decide whether a rule's buffer triggers a fire.
///
/// The rule fires iff:
///
/// - the buffer is non-empty,
/// - the most recent sample is at/below `threshold_price`,
/// - **every** sample within `window_seconds` of the most recent sample is
///   at/below `threshold_price`,
/// - the window's span covers at least `window_seconds` of data
///   (so a single under-threshold sample doesn't fire a 5-minute rule).
///
/// Pure function — no DB, no async, no time-of-day side effect — so the
/// decision logic is unit-testable on its own.
pub fn should_fire(buffer: &[PegSample], threshold_price: f64, window_seconds: i64) -> bool {
    if buffer.is_empty() {
        return false;
    }
    let last = buffer.last().expect("non-empty checked above");
    if last.price > threshold_price {
        return false;
    }
    let window_start = last.observed_at - chrono::Duration::seconds(window_seconds);
    let window_samples: Vec<&PegSample> = buffer
        .iter()
        .filter(|s| s.observed_at >= window_start)
        .collect();
    if window_samples.is_empty() {
        return false;
    }
    // Earliest sample in the window must be at least `window_seconds` old.
    let span = last.observed_at - window_samples[0].observed_at;
    if span < chrono::Duration::seconds(window_seconds) {
        return false;
    }
    window_samples.iter().all(|s| s.price <= threshold_price)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PegRuleRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub portfolio_id: Option<Uuid>,
    pub asset: String,
    pub threshold_price: Decimal,
    pub window_seconds: i32,
    pub action_kind: String,
    pub target_asset: Option<String>,
    pub last_fired_at: Option<DateTime<Utc>>,
}

/// In-memory buffer of recent samples per (rule_id). Keeps at most enough
/// samples to cover the largest possible `window_seconds`; older entries are
/// dropped on every push.
#[derive(Default)]
pub struct PegMonitor {
    buffers: Mutex<HashMap<Uuid, VecDeque<PegSample>>>,
}

impl PegMonitor {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Append a sample for `rule_id`, trimming the buffer to the rule's
    /// `window_seconds` (plus a 1-tick safety margin).
    pub async fn push_sample(&self, rule_id: Uuid, window_seconds: i32, sample: PegSample) {
        let mut buffers = self.buffers.lock().await;
        let buf = buffers.entry(rule_id).or_default();
        buf.push_back(sample);
        let cutoff = sample.observed_at - chrono::Duration::seconds(window_seconds as i64 + 60);
        while let Some(front) = buf.front() {
            if front.observed_at < cutoff {
                buf.pop_front();
            } else {
                break;
            }
        }
    }

    pub async fn snapshot(&self, rule_id: Uuid) -> Vec<PegSample> {
        let buffers = self.buffers.lock().await;
        buffers
            .get(&rule_id)
            .map(|b| b.iter().copied().collect())
            .unwrap_or_default()
    }
}

/// Spawn the long-running peg monitor task.
///
/// Default-off: returns immediately when `PEG_DEFENSE_ENABLED=false`.
pub fn spawn_peg_monitor(state: AppState, cancel: CancellationToken) -> Arc<PegMonitor> {
    let monitor = PegMonitor::new();
    if !state.config.peg_defense_enabled {
        return monitor;
    }
    let mon = monitor.clone();
    let st = state;
    tokio::spawn(async move {
        let tick = Duration::from_secs(st.config.peg_monitor_tick_secs.max(1));
        info!(
            tick_secs = st.config.peg_monitor_tick_secs,
            "peg monitor started"
        );
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("peg monitor shutting down");
                    return;
                }
                _ = tokio::time::sleep(tick) => {}
            }

            if let Err(e) = tick_once(&st, &mon).await {
                warn!(error=%e, "peg monitor tick failed");
            }
        }
    });
    monitor
}

async fn tick_once(state: &AppState, monitor: &PegMonitor) -> anyhow::Result<()> {
    let prices = sample_stable_prices(state).await;
    // Skip peg rules whose owning user has paused the agent globally
    // (FE-PAUSE-1). Per-rule pause + global pause both gate; either one true
    // means the rule sits dormant.
    let rules: Vec<PegRuleRow> = sqlx::query_as(
        "SELECT r.id, r.user_id, r.portfolio_id, r.asset, r.threshold_price,
                r.window_seconds, r.action_kind, r.target_asset, r.last_fired_at
         FROM peg_rules r
         JOIN users u ON u.id = r.user_id
         WHERE r.enabled = TRUE AND r.paused_at IS NULL AND u.agent_paused_at IS NULL",
    )
    .fetch_all(&state.db)
    .await?;

    let now = Utc::now();
    for rule in rules {
        let Some(&price) = prices.get(rule.asset.as_str()) else {
            continue;
        };
        let sample = PegSample {
            observed_at: now,
            price,
        };
        monitor
            .push_sample(rule.id, rule.window_seconds, sample)
            .await;

        if within_cooldown(&rule, now, state.config.peg_fire_cooldown_secs) {
            continue;
        }
        let buf = monitor.snapshot(rule.id).await;
        if !should_fire(
            &buf,
            rule.threshold_price.to_f64().unwrap_or(0.0),
            rule.window_seconds as i64,
        ) {
            continue;
        }

        if let Err(e) = handle_fire(state, &rule, &sample).await {
            warn!(rule_id=%rule.id, error=%e, "peg fire handler failed");
        }
    }
    Ok(())
}

fn within_cooldown(rule: &PegRuleRow, now: DateTime<Utc>, cooldown_secs: i64) -> bool {
    match rule.last_fired_at {
        Some(t) => (now - t) < chrono::Duration::seconds(cooldown_secs),
        None => false,
    }
}

async fn handle_fire(
    state: &AppState,
    rule: &PegRuleRow,
    sample: &PegSample,
) -> anyhow::Result<()> {
    let action_taken = resolve_action(state, rule).await;
    let rebalance_id = match action_taken.as_str() {
        "propose_rebalance" | "auto_execute" => {
            propose_defensive_plan(state, rule).await.unwrap_or(None)
        }
        _ => None,
    };

    sqlx::query(
        "INSERT INTO peg_events
            (rule_id, asset, observed_price, observed_at, action_taken, rebalance_id)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(rule.id)
    .bind(&rule.asset)
    .bind(sample.price)
    .bind(sample.observed_at)
    .bind(&action_taken)
    .bind(rebalance_id)
    .execute(&state.db)
    .await?;

    sqlx::query("UPDATE peg_rules SET last_fired_at = NOW() WHERE id = $1")
        .bind(rule.id)
        .execute(&state.db)
        .await?;

    let threshold_f64 = rule.threshold_price.to_f64().unwrap_or(0.0);
    let payload = PegAlertPayload {
        user_id: rule.user_id,
        rule_id: rule.id,
        asset: rule.asset.clone(),
        observed_price: sample.price,
        threshold_price: threshold_f64,
        observed_at: sample.observed_at,
        action_taken: action_taken.clone(),
        rebalance_id,
    };
    let _ = state.sse.send(SseEvent::PegAlert(payload));
    info!(
        rule_id=%rule.id,
        asset=%rule.asset,
        price=sample.price,
        threshold=threshold_f64,
        action=%action_taken,
        "peg rule fired"
    );
    Ok(())
}

/// `auto_execute` is Pro/Business-tier only. The A3 agent owns the gate; until
/// the `subscriptions` table exists, we tolerate the missing schema with a
/// warn-log and downgrade to `propose_rebalance` so a defensive plan is still
/// surfaced for user approval.
async fn resolve_action(state: &AppState, rule: &PegRuleRow) -> String {
    if rule.action_kind != "auto_execute" {
        return rule.action_kind.clone();
    }
    match user_tier(&state.db, rule.user_id).await {
        Ok(tier) if matches!(tier.as_str(), "pro" | "business") => "auto_execute".into(),
        Ok(other) => {
            warn!(
                user_id=%rule.user_id, tier=%other,
                "auto_execute requested on non-Pro tier; downgrading to propose_rebalance"
            );
            "propose_rebalance".into()
        }
        Err(_) => {
            warn!(
                user_id=%rule.user_id,
                "subscriptions table unavailable; downgrading auto_execute to propose_rebalance \
                 until A3 lands"
            );
            "propose_rebalance".into()
        }
    }
}

async fn user_tier(db: &Db, user_id: Uuid) -> anyhow::Result<String> {
    let tier: Option<String> =
        sqlx::query_scalar("SELECT tier FROM subscriptions WHERE user_id = $1 LIMIT 1")
            .bind(user_id)
            .fetch_optional(db)
            .await?;
    Ok(tier.unwrap_or_else(|| "free".into()))
}

/// Build a defensive rebalance plan that shifts the depegged asset's full
/// weight into the rule's `target_asset`. Uses the same graph-backed routing
/// engine as manual reviews, so peg-defense plans get explicit DAG dependencies,
/// N-chain USDC source selection, and the same typed route model.
///
/// Returns the rebalance plan id when legs were produced; `Ok(None)` when
/// the rule has no resolvable portfolio, the depegged asset's weight is
/// negligible, or the planner deemed the move dust.
///
/// Does NOT auto-execute. The caller fires `peg.alert` SSE; the user
/// approves via the existing `POST /rebalance/:id/execute` endpoint.
/// Auto-execute for Pro/Business tiers is deferred until tier-gate
/// integration testing on real CCTP lands (see F-PEG-8 follow-up).
async fn propose_defensive_plan(
    state: &AppState,
    rule: &PegRuleRow,
) -> anyhow::Result<Option<Uuid>> {
    let Some((portfolio_id, total_value_usd, current_weights)) =
        load_defensive_portfolio(state, rule).await?
    else {
        return Ok(None);
    };

    let depegged_asset = rule.asset.to_uppercase();
    // Repoint the defensive sleeve to a target that can actually execute today.
    // A rule's configured target is honored only if it is an executable stable
    // that isn't the depegged asset; otherwise (e.g. the legacy "USYC" default,
    // which is allowlist-gated and disabled) it falls back to the best
    // executable stable. Without this, peg defense emits a plan that the route
    // registry blocks at approval/execute — never actionable under auto-pilot.
    let target_asset = match rule.target_asset.clone().map(|t| t.to_uppercase()) {
        Some(t)
            if !t.eq_ignore_ascii_case(&depegged_asset)
                && is_executable_stable(&state.config, &t) =>
        {
            t
        }
        _ => default_defensive_target(&state.config, &depegged_asset),
    };

    let Some((target_weights, depegged_weight)) =
        build_defensive_target(&current_weights, &depegged_asset, &target_asset)
    else {
        debug!(rule_id=%rule.id, %depegged_asset,
            "depegged asset weight < 1%; no defensive plan needed");
        return Ok(None);
    };

    let prices = recent_defensive_prices(state, &current_weights, &target_weights).await;

    let Some(usdc_per_chain) = fetch_usdc_pool(state, rule).await else {
        return Ok(None);
    };

    let input = PlanInput {
        portfolio_value_usd: total_value_usd,
        current_weights,
        token_values_by_chain: HashMap::new(),
        target_weights,
        usdc_per_chain,
        // Peg defense bypasses the usual drift threshold — a depeg is the
        // signal, not a 5% drift.
        drift_threshold: 0.0,
        dust_threshold_usd: 5.0,
        prices,
        // A depeg is a risk-off event; tag the plan accordingly (moot at
        // drift_threshold 0, but keeps the signal honest).
        regime: Some("risk_off".to_string()),
    };

    let legs = defensive_plan_legs(&state.config, &input);
    if legs.is_empty() {
        debug!(rule_id=%rule.id, "planner produced no legs (dust or zero portfolio); no plan persisted");
        return Ok(None);
    }

    let rebalance_id = persist_defensive_plan(
        state,
        portfolio_id,
        rule,
        &target_asset,
        depegged_weight,
        &legs,
    )
    .await?;
    info!(
        rule_id=%rule.id,
        %rebalance_id,
        legs_count = legs.len(),
        action = %rule.action_kind,
        "peg-defense plan persisted"
    );

    Ok(Some(rebalance_id))
}

fn defensive_plan_legs(cfg: &crate::config::Config, input: &PlanInput) -> Vec<PlannedLeg> {
    crate::modules::rebalance::routing::engine_plan_legs(cfg, input)
}

/// Resolve the portfolio a peg rule defends and load its current allocation.
/// Prefers the rule's pinned `portfolio_id`, else the user's most-recently
/// updated portfolio (multi-portfolio fan-out is a later follow-up). Returns
/// `None` when the user has no portfolio. DB weights are 0–100; the planner
/// wants 0–1 fractions, so they're normalized here.
async fn load_defensive_portfolio(
    state: &AppState,
    rule: &PegRuleRow,
) -> anyhow::Result<Option<(Uuid, f64, HashMap<String, f64>)>> {
    let portfolio_id: Option<Uuid> = match rule.portfolio_id {
        Some(pid) => Some(pid),
        None => {
            sqlx::query_scalar(
                "SELECT id FROM portfolios WHERE user_id = $1 ORDER BY updated_at DESC LIMIT 1",
            )
            .bind(rule.user_id)
            .fetch_optional(&state.db)
            .await?
        }
    };
    let Some(portfolio_id) = portfolio_id else {
        warn!(rule_id=%rule.id, user_id=%rule.user_id, "peg rule has no portfolio; skipping defensive plan");
        return Ok(None);
    };

    let total_value_usd: f64 = sqlx::query_scalar(
        "SELECT total_value_usd::DOUBLE PRECISION FROM portfolios WHERE id = $1",
    )
    .bind(portfolio_id)
    .fetch_one(&state.db)
    .await?;

    let allocations: Vec<(String, f64)> = sqlx::query_as(
        "SELECT asset_symbol, current_weight::DOUBLE PRECISION FROM allocations WHERE portfolio_id = $1",
    )
    .bind(portfolio_id)
    .fetch_all(&state.db)
    .await?;

    let current_weights = allocations
        .into_iter()
        .map(|(sym, w)| (sym, w / 100.0))
        .collect();
    Ok(Some((portfolio_id, total_value_usd, current_weights)))
}

/// Best-effort recent USD prices for every symbol involved in the move (the
/// planner uses them for `min_out`). An empty symbol set or a failed lookup
/// yields an empty map, and the planner falls back to its internal defaults.
async fn recent_defensive_prices(
    state: &AppState,
    current_weights: &HashMap<String, f64>,
    target_weights: &HashMap<String, f64>,
) -> HashMap<String, f64> {
    let relevant_symbols: Vec<String> = current_weights
        .keys()
        .chain(target_weights.keys())
        .cloned()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    if relevant_symbols.is_empty() {
        return HashMap::new();
    }
    crate::modules::market_data::service::get_historical_prices(
        &state.db,
        &relevant_symbols,
        chrono::Utc::now(),
    )
    .await
    .unwrap_or_default()
}

/// The user's real Gateway USDC per chain (Arc + Base seeded to 0). A defensive
/// move converts the depegged USDC into the target stable, so the buy leg must
/// fund from the USDC actually in the wallet — a missing balance means we can't
/// build a safe plan, so this returns `None` and the caller downgrades to an
/// alert with no plan rather than guessing a pool.
async fn fetch_usdc_pool(state: &AppState, rule: &PegRuleRow) -> Option<HashMap<ChainKey, f64>> {
    match crate::modules::gateway::service::fetch_balance_for_user(
        &state.db,
        &state.http,
        &state.config,
        rule.user_id,
    )
    .await
    {
        Ok(balance) => {
            let mut pool: HashMap<ChainKey, f64> = HashMap::new();
            pool.insert(ChainKey::Arc, 0.0);
            pool.insert(ChainKey::Base, 0.0);
            for (chain, amount) in balance.per_chain {
                if let Some(key) = ChainKey::parse(chain.to_lowercase().as_str()) {
                    pool.insert(key, amount);
                }
            }
            Some(pool)
        }
        Err(e) => {
            warn!(rule_id=%rule.id, error=%e, "peg defense: gateway balance unavailable; no defensive plan");
            None
        }
    }
}

/// Persist a defensive plan: a synthetic `peg_alert` agent_decisions row anchors
/// the rebalance (rebalances.decision_id is NOT NULL), then the planned rebalance
/// itself. `triggered_by='peg_alert'` is accepted by the shared TS type via its
/// `(string & {})` union. Returns the new rebalance id.
async fn persist_defensive_plan(
    state: &AppState,
    portfolio_id: Uuid,
    rule: &PegRuleRow,
    target_asset: &str,
    depegged_weight: f64,
    legs: &[PlannedLeg],
) -> anyhow::Result<Uuid> {
    let reasoning = format!(
        "Peg-defense: {asset} observed at or below {threshold:.4} for the configured window; \
         shifting {pct}% of portfolio from {asset} into {target}.",
        asset = rule.asset.to_uppercase(),
        threshold = rule.threshold_price.to_f64().unwrap_or(0.0),
        pct = (depegged_weight * 100.0).round() as i64,
        target = target_asset,
    );
    let decision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO agent_decisions
            (portfolio_id, reasoning, recommendation, confidence, triggered_by,
             model_slug, regime, prompt_tokens, completion_tokens, latency_ms,
             critic_verdict, snapshot, raw_confidence, counterfactual)
         VALUES ($1, $2, $3, 0.9, 'peg_alert',
                 'aegis/rebalance-planner-v1', 'risk_off', 0, 0, 0,
                 $4, $5, 0.9, $6)
         RETURNING id",
    )
    .bind(portfolio_id)
    .bind(&reasoning)
    .bind(serde_json::json!({
        "summary": "Peg-defense rebalance",
        "trades": [],
        "expectedImpact": { "riskDelta": -1.0, "diversificationScore": 0.5 }
    }))
    .bind(serde_json::json!({
        "verdict": "approved",
        "notes": "Deterministic peg-defense planner built an approval-gated route from live Gateway balances.",
        "confidence": 0.90
    }))
    .bind(serde_json::json!({
        "planner": "deterministic",
        "trigger": "peg_alert",
        "legs": legs.len(),
        "targetAsset": target_asset,
    }))
    .bind("If the peg recovers or route readiness changes, rebuild the review before approving.".to_string())
    .fetch_one(&state.db)
    .await?;

    let rebalance_id = create_plan(state, portfolio_id, decision_id, legs).await?;
    let caps = RuntimeCapabilities::from_config(&state.config);
    let snapshot = RoutableSnapshot::capture(&caps, &state.config);
    sqlx::query("UPDATE rebalances SET routable_snapshot_hash = $1 WHERE id = $2")
        .bind(snapshot.hash())
        .bind(rebalance_id)
        .execute(&state.db)
        .await?;
    Ok(rebalance_id)
}

/// Sample the current stablecoin prices via the platform price provider.
/// USDC/EURC come straight from the provider; USYC defaults to 1.00 because
/// Hashnote hasn't surfaced a public oracle yet. EURC's USD price is
/// converted through the live FX module's mid rate so the threshold
/// semantics stay "EURC vs 1 EURC" — the old hardcoded 1.085 baked in
/// 2024-era ECB pricing and would have raised false depegs once EUR/USD
/// moved more than a percent or two.
///
/// Failures fall back to "1.00" for every symbol so an upstream outage never
/// triggers a false depeg.
async fn sample_stable_prices(state: &AppState) -> HashMap<String, f64> {
    let mut out: HashMap<String, f64> =
        PEG_ASSETS.iter().map(|s| ((*s).to_string(), 1.0)).collect();

    let symbols: Vec<&_> = ["USDC", "EURC"]
        .iter()
        .filter_map(|t| crate::modules::prices::lookup_symbol(t))
        .collect();
    let eur_usd_mid = match crate::modules::fx::prices::fetch_quote(state.prices.as_ref()).await {
        Ok(q) if q.eurc_usd > 0.0 => q.eurc_usd,
        _ => 1.085,
    };
    match state.prices.fetch_spot(&symbols).await {
        Ok(quotes) => {
            for q in quotes {
                match q.ticker {
                    "USDC" => {
                        out.insert("USDC".into(), q.price_usd);
                    }
                    "EURC" => {
                        out.insert("EURC".into(), q.price_usd / eur_usd_mid);
                    }
                    _ => {}
                }
            }
        }
        Err(e) => debug!(error=%e, "peg monitor: price provider fetch failed"),
    }

    out
}

/// Public hook so an HTTP handler / test can inject a sample manually
/// (e.g. simulating a 0.994 USDC quote in dev). The handler ignores
/// `peg_defense_enabled` because the test owns lifecycle.
#[allow(dead_code)]
pub async fn record_sample_for_test(
    monitor: &PegMonitor,
    rule_id: Uuid,
    window_seconds: i32,
    sample: PegSample,
) {
    monitor.push_sample(rule_id, window_seconds, sample).await;
}

/// The set of stable-class symbols peg defense may rotate into: the settlement
/// stable, the yield sleeve, and the FX sleeve. (Volatiles are never a peg
/// hedge.) Used to keep a defensive target within the "stable" universe.
const DEFENSIVE_STABLE_SYMBOLS: &[&str] = &["USDC", "EURC", "USYC"];

/// Whether `symbol` is a stable that can actually execute right now (the route
/// registry's executable set ∩ the defensive-stable universe).
fn is_executable_stable(cfg: &crate::config::Config, symbol: &str) -> bool {
    use crate::modules::rebalance::registry::{
        capabilities::RuntimeCapabilities, executable_token_symbols,
    };
    if !DEFENSIVE_STABLE_SYMBOLS
        .iter()
        .any(|s| s.eq_ignore_ascii_case(symbol))
    {
        return false;
    }
    let caps = RuntimeCapabilities::from_config(cfg);
    executable_token_symbols(&caps, cfg)
        .iter()
        .any(|s| s.eq_ignore_ascii_case(symbol))
}

/// Pick the best executable stable to rotate a depegged asset into. Prefers the
/// yield sleeve (USYC), then the FX sleeve (EURC), then plain USDC — skipping
/// the depegged asset itself. Falls back to USDC when nothing else is
/// executable: `build_defensive_target` then yields no net move (track-only),
/// the honest outcome when there is no executable hedge.
fn default_defensive_target(cfg: &crate::config::Config, depegged_asset: &str) -> String {
    const PREFERENCE: &[&str] = &["USYC", "EURC", "USDC"];
    for candidate in PREFERENCE {
        if candidate.eq_ignore_ascii_case(depegged_asset) {
            continue;
        }
        if is_executable_stable(cfg, candidate) {
            return (*candidate).to_string();
        }
    }
    "USDC".to_string()
}

/// Pure helper extracted for unit testability. Returns `Some((target_weights,
/// depegged_weight))` when a defensive rebalance is warranted (depegged asset
/// has ≥1% weight), `None` otherwise. The target map zeroes the depegged
/// asset and piles its weight onto `target_asset`, creating the target_asset
/// entry if it doesn't exist in the current allocation.
fn build_defensive_target(
    current_weights: &HashMap<String, f64>,
    depegged_asset: &str,
    target_asset: &str,
) -> Option<(HashMap<String, f64>, f64)> {
    let depegged_weight = *current_weights.get(depegged_asset).unwrap_or(&0.0);
    if depegged_weight < 0.01 {
        return None;
    }
    let mut target_weights = current_weights.clone();
    target_weights.insert(depegged_asset.to_string(), 0.0);
    *target_weights
        .entry(target_asset.to_string())
        .or_insert(0.0) += depegged_weight;
    Some((target_weights, depegged_weight))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn peg_rule_proposes_usdc_to_usyc() {
        // 80% USDC / 20% BTC. USDC depegs → defensive target should move
        // 80% USDC into USYC (a brand-new entry), zero USDC, keep BTC at 20%.
        let mut current = HashMap::new();
        current.insert("USDC".to_string(), 0.80);
        current.insert("BTC".to_string(), 0.20);

        let (target, moved) =
            build_defensive_target(&current, "USDC", "USYC").expect("80% > 1% → defensive plan");
        assert!((moved - 0.80).abs() < 1e-9, "moved 80% of weight");
        assert_eq!(target.get("USDC"), Some(&0.0));
        assert_eq!(target.get("USYC"), Some(&0.80));
        assert_eq!(target.get("BTC"), Some(&0.20));
        let total: f64 = target.values().sum();
        assert!((total - 1.0).abs() < 1e-9, "weights still sum to 1.0");
    }

    #[test]
    fn default_defensive_target_avoids_disabled_usyc() {
        let mut cfg = crate::config::test_config();
        cfg.execution_mock = false;
        cfg.circle_mock = false;
        use crate::modules::rebalance::models::ChainKey;
        cfg.chains[ChainKey::Arc.index()].private_key = "0xaa".into();
        cfg.chains[ChainKey::Base.index()].private_key = "0xbb".into();
        cfg.chains[ChainKey::Base.index()].usdc =
            "0x036CbD53842c5426634e7929541eC2318f3dCF7e".into();
        cfg.set_token_address(
            "EURC",
            ChainKey::Base,
            "0x60a3E35Cc302bFA44Cb288Bc5a4F316Fdb1adb42",
        );
        // The Base swap venue must be wired for EURC to count as executable.
        cfg.chains[ChainKey::Base.index()].swap_router =
            "0x1111111111111111111111111111111111111111".into();
        cfg.chains[ChainKey::Base.index()].swap_quoter =
            "0x2222222222222222222222222222222222222222".into();
        // ...and EURC must be in the chain's liquid-venue allowlist: the default
        // curates Base execution to ETH only (EURC/cbBTC have no usable Base
        // Sepolia pool), so this test opts EURC in to prove the wired-sleeve path.
        cfg.swap_liquid_tokens
            .insert(ChainKey::Base, vec!["ETH".into(), "EURC".into()]);

        // USYC is disabled by default → never an executable stable, never chosen.
        assert!(!is_executable_stable(&cfg, "USYC"));
        assert_ne!(default_defensive_target(&cfg, "USDC"), "USYC");
        assert_ne!(default_defensive_target(&cfg, "EURC"), "USYC");

        // USDC is always executable and is the safe fallback for an EURC depeg.
        assert!(is_executable_stable(&cfg, "USDC"));
        assert_eq!(default_defensive_target(&cfg, "EURC"), "USDC");

        // With the swap rail compiled + EURC wired, a USDC depeg routes to the
        // executable EURC sleeve rather than the disabled USYC one.
        #[cfg(feature = "real-swap")]
        {
            assert!(is_executable_stable(&cfg, "EURC"));
            assert_eq!(default_defensive_target(&cfg, "USDC"), "EURC");
        }
    }

    #[test]
    fn defensive_plan_legs_use_routing_engine_dag() {
        let sentinel = "0x1111111111111111111111111111111111111111";
        let mut cfg = crate::config::test_config();
        cfg.chains[ChainKey::Arc.index()].usdc = sentinel.into();
        cfg.chains[ChainKey::Base.index()].usdc = sentinel.into();
        cfg.set_token_address("ETH", ChainKey::Base, sentinel);

        let mut current_weights = HashMap::new();
        current_weights.insert("USDC".to_string(), 1.0);
        let mut target_weights = HashMap::new();
        target_weights.insert("USDC".to_string(), 0.0);
        target_weights.insert("ETH".to_string(), 1.0);
        let mut usdc_per_chain = HashMap::new();
        usdc_per_chain.insert(ChainKey::Arc, 1_000.0);
        usdc_per_chain.insert(ChainKey::Base, 0.0);

        let input = PlanInput {
            portfolio_value_usd: 1_000.0,
            current_weights,
            token_values_by_chain: HashMap::new(),
            target_weights,
            usdc_per_chain,
            drift_threshold: 0.0,
            dust_threshold_usd: 5.0,
            prices: HashMap::new(),
            regime: Some("risk_off".to_string()),
        };

        let legs = defensive_plan_legs(&cfg, &input);

        let burn = legs
            .iter()
            .find(|l| l.kind == crate::modules::rebalance::models::LegKind::CrossChainBurn)
            .expect("Arc-funded Base buy must bridge first");
        let mint = legs
            .iter()
            .find(|l| l.kind == crate::modules::rebalance::models::LegKind::CrossChainMint)
            .expect("bridge must include mint leg");
        let swap = legs
            .iter()
            .find(|l| {
                l.kind == crate::modules::rebalance::models::LegKind::LocalSwap
                    && l.dest_symbol.as_deref() == Some("ETH")
            })
            .expect("destination token must be acquired after mint");

        assert!(mint.deps.contains(&burn.leg_index));
        assert!(swap.deps.contains(&mint.leg_index));
    }

    #[test]
    fn peg_rule_no_op_when_depegged_asset_absent() {
        // Portfolio holds only BTC. USDC depegs but the user isn't holding any
        // USDC → no defensive plan needed.
        let mut current = HashMap::new();
        current.insert("BTC".to_string(), 1.00);
        assert!(build_defensive_target(&current, "USDC", "USYC").is_none());
    }

    #[test]
    fn peg_rule_no_op_when_depegged_weight_below_one_percent() {
        // 0.5% USDC dust → not worth a defensive trade (planner's dust
        // threshold would drop it anyway, but we short-circuit earlier).
        let mut current = HashMap::new();
        current.insert("USDC".to_string(), 0.005);
        current.insert("BTC".to_string(), 0.995);
        assert!(build_defensive_target(&current, "USDC", "USYC").is_none());
    }

    #[test]
    fn peg_rule_appends_to_existing_target_asset_weight() {
        // 50% USDC / 30% USYC / 20% BTC. USDC depegs → target_weights should
        // be 0% USDC, 80% USYC (30% original + 50% moved), 20% BTC.
        let mut current = HashMap::new();
        current.insert("USDC".to_string(), 0.50);
        current.insert("USYC".to_string(), 0.30);
        current.insert("BTC".to_string(), 0.20);
        let (target, moved) = build_defensive_target(&current, "USDC", "USYC").expect("50% > 1%");
        assert!((moved - 0.50).abs() < 1e-9);
        assert_eq!(target.get("USDC"), Some(&0.0));
        assert_eq!(target.get("USYC"), Some(&0.80));
        assert_eq!(target.get("BTC"), Some(&0.20));
    }

    fn at(secs_after_epoch: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs_after_epoch, 0).unwrap()
    }

    #[test]
    fn empty_buffer_never_fires() {
        assert!(!should_fire(&[], 0.995, 300));
    }

    #[test]
    fn single_under_threshold_sample_does_not_fire_300s_rule() {
        let buf = [PegSample {
            observed_at: at(1_000),
            price: 0.994,
        }];
        assert!(
            !should_fire(&buf, 0.995, 300),
            "a single under-threshold sample is not a 5-minute depeg"
        );
    }

    #[test]
    fn under_threshold_for_full_window_fires() {
        let buf = vec![
            PegSample {
                observed_at: at(1_000),
                price: 0.993,
            },
            PegSample {
                observed_at: at(1_120),
                price: 0.992,
            },
            PegSample {
                observed_at: at(1_240),
                price: 0.994,
            },
            PegSample {
                observed_at: at(1_300),
                price: 0.994,
            },
        ];
        // span = 300s, every sample <= 0.995 → fire.
        assert!(should_fire(&buf, 0.995, 300));
    }

    #[test]
    fn one_over_threshold_sample_in_window_blocks_fire() {
        let buf = vec![
            PegSample {
                observed_at: at(1_000),
                price: 0.993,
            },
            PegSample {
                observed_at: at(1_150),
                price: 1.001,
            },
            PegSample {
                observed_at: at(1_300),
                price: 0.994,
            },
        ];
        assert!(!should_fire(&buf, 0.995, 300));
    }

    #[test]
    fn current_price_above_threshold_blocks_fire_even_if_history_was_under() {
        let buf = vec![
            PegSample {
                observed_at: at(1_000),
                price: 0.990,
            },
            PegSample {
                observed_at: at(1_150),
                price: 0.991,
            },
            PegSample {
                observed_at: at(1_300),
                price: 1.001,
            },
        ];
        assert!(!should_fire(&buf, 0.995, 300));
    }

    #[test]
    fn window_zero_fires_on_a_single_under_threshold_sample() {
        // window_seconds = 0 means "fire on any sample under threshold".
        let buf = [PegSample {
            observed_at: at(1_000),
            price: 0.994,
        }];
        assert!(should_fire(&buf, 0.995, 0));
    }

    #[test]
    fn within_cooldown_blocks_repeated_fires() {
        let now = Utc.with_ymd_and_hms(2026, 5, 16, 12, 0, 0).unwrap();
        let mut rule = sample_rule();
        rule.last_fired_at = Some(now - chrono::Duration::seconds(600));
        // 30-minute cooldown — last fire was 10 minutes ago.
        assert!(within_cooldown(&rule, now, 1800));
        // 30-minute cooldown — last fire was 31 minutes ago.
        rule.last_fired_at = Some(now - chrono::Duration::seconds(31 * 60));
        assert!(!within_cooldown(&rule, now, 1800));
    }

    #[test]
    fn missing_last_fired_at_never_in_cooldown() {
        let mut rule = sample_rule();
        rule.last_fired_at = None;
        assert!(!within_cooldown(&rule, Utc::now(), 1800));
    }

    #[tokio::test]
    async fn push_sample_trims_old_entries() {
        let monitor = PegMonitor::new();
        let rule_id = Uuid::new_v4();
        let window = 60_i32;
        // Insert a sample 10 minutes ago and one at "now".
        let stale = PegSample {
            observed_at: at(1_000),
            price: 0.994,
        };
        let fresh = PegSample {
            observed_at: at(1_000 + 10_000),
            price: 0.994,
        };
        monitor.push_sample(rule_id, window, stale).await;
        monitor.push_sample(rule_id, window, fresh).await;
        let snap = monitor.snapshot(rule_id).await;
        assert_eq!(snap.len(), 1, "stale sample should be evicted");
        assert_eq!(snap[0], fresh);
    }

    fn sample_rule() -> PegRuleRow {
        use rust_decimal_macros::dec;
        PegRuleRow {
            id: Uuid::nil(),
            user_id: Uuid::nil(),
            portfolio_id: None,
            asset: "USDC".into(),
            threshold_price: dec!(0.995),
            window_seconds: 300,
            action_kind: "alert".into(),
            target_asset: None,
            last_fired_at: None,
        }
    }
}
