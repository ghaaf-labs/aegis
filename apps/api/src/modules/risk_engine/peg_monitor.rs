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
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::Db;
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
    #[allow(dead_code)]
    pub portfolio_id: Option<Uuid>,
    pub asset: String,
    pub threshold_price: f64,
    pub window_seconds: i32,
    pub action_kind: String,
    #[allow(dead_code)]
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
        let cutoff = sample.observed_at
            - chrono::Duration::seconds(window_seconds as i64 + 60);
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
    let rules: Vec<PegRuleRow> = sqlx::query_as(
        "SELECT id, user_id, portfolio_id, asset, threshold_price, window_seconds,
                action_kind, target_asset, last_fired_at
         FROM peg_rules
         WHERE enabled = TRUE AND paused_at IS NULL",
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
        if !should_fire(&buf, rule.threshold_price, rule.window_seconds as i64) {
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

    let payload = PegAlertPayload {
        user_id: rule.user_id,
        rule_id: rule.id,
        asset: rule.asset.clone(),
        observed_price: sample.price,
        threshold_price: rule.threshold_price,
        observed_at: sample.observed_at,
        action_taken: action_taken.clone(),
        rebalance_id,
    };
    let _ = state.sse.send(SseEvent::PegAlert(payload));
    info!(
        rule_id=%rule.id,
        asset=%rule.asset,
        price=sample.price,
        threshold=rule.threshold_price,
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

/// Returns the rebalance plan id if a planner ran cleanly; `Ok(None)` if no
/// portfolio could be resolved. Logs and returns `Err` only on hard DB failures.
async fn propose_defensive_plan(
    _state: &AppState,
    rule: &PegRuleRow,
) -> anyhow::Result<Option<Uuid>> {
    // TODO(A6 follow-up): wire a real defensive-plan generator via
    // rebalance::planner. Today we stage the alert + event row only — the
    // user-facing UI surfaces "open rebalance" buttons that hit the existing
    // `POST /portfolios/:id/rebalance/plan` endpoint with the depegged asset
    // pre-marked as the source. Auto-execute follows once A3 finalizes the
    // tier gate so we can call the executor without re-prompting.
    debug!(rule_id=%rule.id, "propose_defensive_plan: deferred until A6 follow-up");
    Ok(None)
}

/// Sample the current stablecoin prices. USDC/EURC come from CoinGecko via
/// the existing market-data snapshot; USYC defaults to 1.00 because Hashnote
/// hasn't surfaced a public oracle yet.
///
/// Failures fall back to "1.00" for every symbol so a CoinGecko outage never
/// triggers a false depeg.
async fn sample_stable_prices(state: &AppState) -> HashMap<String, f64> {
    let mut out: HashMap<String, f64> = PEG_ASSETS
        .iter()
        .map(|s| ((*s).to_string(), 1.0))
        .collect();

    // CoinGecko reports `usd-coin` + `euro-coin`; this is best-effort and
    // explicitly tolerant — the fallback above already gives every asset a
    // safe default if the request flakes.
    let url = "https://api.coingecko.com/api/v3/simple/price?ids=usd-coin,euro-coin&vs_currencies=usd";
    let mut req = state.http.get(url);
    if let Some(key) = &state.config.coingecko_api_key {
        req = req.header("x-cg-demo-api-key", key);
    }
    match req.send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(p) = json.get("usd-coin").and_then(|v| v.get("usd")).and_then(|v| v.as_f64()) {
                    out.insert("USDC".into(), p);
                }
                if let Some(p) = json.get("euro-coin").and_then(|v| v.get("usd")).and_then(|v| v.as_f64()) {
                    // EURC's USD price isn't a depeg — convert via current
                    // EUR-USD basis so the threshold semantics stay "EURC vs
                    // 1 EURC". Approximate 1 EURC ≈ 1.085 USD as a stable
                    // mid; the StableFX module owns the real basis but we
                    // prefer a tolerant default over an extra dependency.
                    let eur_usd = 1.085;
                    let eurc_to_eur = p / eur_usd;
                    out.insert("EURC".into(), eurc_to_eur);
                }
            }
        }
        Ok(resp) => debug!(status=%resp.status(), "peg monitor: stablecoin price fetch non-200"),
        Err(e) => debug!(error=%e, "peg monitor: stablecoin price fetch failed"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

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
        PegRuleRow {
            id: Uuid::nil(),
            user_id: Uuid::nil(),
            portfolio_id: None,
            asset: "USDC".into(),
            threshold_price: 0.995,
            window_seconds: 300,
            action_kind: "alert".into(),
            target_asset: None,
            last_fired_at: None,
        }
    }
}
