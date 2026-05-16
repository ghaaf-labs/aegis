//! F-REG-2 — Regime classifier backtest harness.
//!
//! Walks daily windows of `price_history` for the past N years, recomputing the
//! same statistical features the live classifier consumes (BTC 30d realized
//! vol, 90d cross-asset correlation, 30d max drawdown), invokes the LLM
//! classifier on each window, and labels the *realized* regime from a
//! deterministic forward-return rule (30d BTC return ≷ ±10%). Persists per-
//! sample predictions and run-level metrics for the public `/about/regime`
//! model card and the downstream A8 Brier calibrator.
//!
//! The classifier is abstracted behind the `RegimeClassifier` trait so the
//! unit tests can plug in a deterministic mock — CI never hits OpenRouter.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;
use async_trait::async_trait;
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::types::Json as SqlxJson;
use uuid::Uuid;

use crate::config::ModelRoute;
use crate::db::Db;
use crate::modules::ai::{Message, OpenRouterClient, PromptKey, PromptRegistry};

use super::regime::MarketRegime;

const TASK: &str = "regime_classifier";
const REGIMES: [MarketRegime; 3] = [
    MarketRegime::RiskOn,
    MarketRegime::Neutral,
    MarketRegime::RiskOff,
];

/// What the classifier returns for one sample. `probabilities` is the
/// distribution over the three regimes; sums to ~1.0.
#[derive(Debug, Clone)]
pub struct ClassifierOutput {
    pub label: MarketRegime,
    pub probabilities: HashMap<MarketRegime, f64>,
}

/// Pluggable classifier — production wraps OpenRouter; tests use a closure.
#[async_trait]
pub trait RegimeClassifier: Send + Sync {
    async fn classify(&self, features: &Features) -> anyhow::Result<ClassifierOutput>;
}

/// Daily-resampled features computed at a fixed `as_of` date, looking
/// strictly backward.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Features {
    pub as_of: NaiveDate,
    pub btc_vol_30d: f64,
    pub corr_90d: f64,
    pub max_drawdown_30d: f64,
}

/// The OpenRouter-backed classifier used by the live CLI. CI tests don't
/// instantiate this — they use the mock in `tests`.
pub struct OpenRouterRegimeClassifier<'a> {
    pub ai: OpenRouterClient<'a>,
    pub prompts: &'a PromptRegistry,
    /// Minimum delay between calls (rate-limit throttle).
    pub min_delay: Duration,
    /// Max retries on a 429 / transient error.
    pub max_retries: u32,
}

#[async_trait]
impl<'a> RegimeClassifier for OpenRouterRegimeClassifier<'a> {
    async fn classify(&self, features: &Features) -> anyhow::Result<ClassifierOutput> {
        let features_json = json!({
            "btc_vol_30d": features.btc_vol_30d,
            "corr_90d": features.corr_90d,
            "max_drawdown": features.max_drawdown_30d,
            // The live prompt also references fear_greed / btc_dominance.
            // We don't have historical values for these, so we use neutral
            // defaults — the model card documents this caveat.
            "fear_greed": 50u8,
            "btc_dominance": 50.0f64,
        });
        let mut ctx = HashMap::new();
        ctx.insert(
            "features_json",
            serde_json::to_string_pretty(&features_json)?,
        );
        let prompt = self.prompts.render(PromptKey::Regime, &ctx);

        let mut attempt = 0u32;
        loop {
            tokio::time::sleep(self.min_delay).await;
            let res = self
                .ai
                .chat(
                    ModelRoute::RegimeClassify,
                    vec![
                        Message::system(prompt.clone()),
                        Message::user("Label the regime.".to_string()),
                    ],
                )
                .await;
            match res {
                Ok(resp) => return parse_label(&resp.content),
                Err(e) => {
                    let msg = e.to_string();
                    let retriable = msg.contains("429")
                        || msg.contains("rate limit")
                        || msg.contains("timeout");
                    if !retriable || attempt >= self.max_retries {
                        return Err(e);
                    }
                    attempt += 1;
                    let backoff = Duration::from_millis(250 * (1u64 << attempt.min(5)));
                    tracing::warn!(
                        "regime classifier retriable error (attempt {attempt}): {msg}; backing off {backoff:?}"
                    );
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }
}

fn parse_label(raw: &str) -> anyhow::Result<ClassifierOutput> {
    let stripped = crate::modules::ai::strip_json_fences(raw);
    let v: Value = serde_json::from_str(stripped)
        .map_err(|e| anyhow::anyhow!("regime classifier: invalid JSON ({e}): {raw}"))?;
    let label_str = v
        .get("regime")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing regime field: {raw}"))?;
    let label = parse_regime(label_str)?;
    let confidence = v
        .get("confidence")
        .and_then(|x| x.as_f64())
        .unwrap_or(0.5)
        .clamp(0.0, 1.0);
    // The live prompt doesn't emit a full per-class distribution, only a
    // top-1 label + scalar confidence. Spread the residual mass uniformly
    // across the other two regimes so we still have a usable proba vector
    // for Brier-score calibration downstream.
    let mut probs = HashMap::with_capacity(3);
    let leftover = ((1.0 - confidence) / 2.0).max(0.0);
    for r in REGIMES {
        probs.insert(r, if r == label { confidence } else { leftover });
    }
    Ok(ClassifierOutput {
        label,
        probabilities: probs,
    })
}

fn parse_regime(s: &str) -> anyhow::Result<MarketRegime> {
    match s {
        "risk_on" => Ok(MarketRegime::RiskOn),
        "neutral" => Ok(MarketRegime::Neutral),
        "risk_off" => Ok(MarketRegime::RiskOff),
        other => Err(anyhow::anyhow!("unknown regime label: {other}")),
    }
}

/// One daily price point used by the feature computer.
#[derive(Debug, Clone)]
struct DailyPrice {
    date: NaiveDate,
    price: f64,
}

/// Aggregate metrics + sample list for one backtest run.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalRun {
    pub eval_run_id: Uuid,
    pub model_slug: String,
    pub task: String,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub samples_count: usize,
    pub accuracy: f64,
    pub precision_macro: f64,
    pub recall_macro: f64,
    pub f1_macro: f64,
    pub brier_score: f64,
    /// 3x3 row-major confusion: rows = predicted, cols = realized,
    /// order risk_on / neutral / risk_off.
    pub confusion: [[u32; 3]; 3],
    pub per_regime: HashMap<MarketRegime, RegimeMetrics>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegimeMetrics {
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
    pub support: u32,
}

/// A single (predicted, realized) row to persist.
#[derive(Debug, Clone)]
pub struct EvalSample {
    pub observed_at: DateTime<Utc>,
    pub predicted_label: MarketRegime,
    pub predicted_proba: HashMap<MarketRegime, f64>,
    pub realized_label: MarketRegime,
    pub features: Features,
}

/// Entry point — runs a full backtest for the past `years` years and
/// persists the result to Postgres. Returns the aggregate `EvalRun`.
pub async fn run_backtest(
    pool: &Db,
    model_slug: &str,
    years: u32,
    classifier: &dyn RegimeClassifier,
) -> anyhow::Result<EvalRun> {
    let prices = fetch_btc_daily_series(pool, years).await?;
    if prices.len() < 130 {
        anyhow::bail!(
            "regime backtest: need at least 130 daily BTC ticks (got {}); price_history coverage is insufficient. Backfill price_history with historical data before running.",
            prices.len()
        );
    }
    let other_series = fetch_companion_series(pool, years).await;

    let samples = walk_windows(&prices, &other_series, classifier).await?;
    if samples.is_empty() {
        anyhow::bail!("regime backtest: zero usable windows after feature computation");
    }

    let run = compute_metrics(samples.as_slice());
    let eval_run_id = Uuid::new_v4();
    persist(pool, eval_run_id, model_slug, &run, &samples).await?;

    Ok(EvalRun {
        eval_run_id,
        model_slug: model_slug.to_string(),
        ..run
    })
}

async fn fetch_btc_daily_series(pool: &Db, years: u32) -> anyhow::Result<Vec<DailyPrice>> {
    fetch_daily_series(pool, "BTC", years).await
}

async fn fetch_companion_series(pool: &Db, years: u32) -> HashMap<&'static str, Vec<DailyPrice>> {
    let mut out = HashMap::new();
    for sym in ["ETH", "SOL"] {
        if let Ok(s) = fetch_daily_series(pool, sym, years).await {
            if !s.is_empty() {
                out.insert(sym, s);
            }
        }
    }
    out
}

async fn fetch_daily_series(
    pool: &Db,
    symbol: &str,
    years: u32,
) -> anyhow::Result<Vec<DailyPrice>> {
    // Daily resample: last tick per (symbol, date).
    let rows: Vec<(NaiveDate, f64)> = sqlx::query_as(
        r#"
        SELECT DISTINCT ON (d) d::date AS d, price
        FROM (
            SELECT fetched_at AT TIME ZONE 'UTC' AS ts,
                   date_trunc('day', fetched_at AT TIME ZONE 'UTC') AS d,
                   price_usd::float8 AS price
            FROM price_history
            WHERE symbol = $1
              AND fetched_at > NOW() - ($2::int || ' years')::interval
        ) t
        ORDER BY d, ts DESC
        "#,
    )
    .bind(symbol)
    .bind(years as i32)
    .fetch_all(pool)
    .await
    .with_context(|| format!("fetch daily series for {symbol}"))?;

    Ok(rows
        .into_iter()
        .map(|(date, price)| DailyPrice { date, price })
        .collect())
}

async fn walk_windows(
    btc: &[DailyPrice],
    others: &HashMap<&'static str, Vec<DailyPrice>>,
    classifier: &dyn RegimeClassifier,
) -> anyhow::Result<Vec<EvalSample>> {
    let mut samples = Vec::new();

    // We need 90d of history before the window and 30d of forward returns
    // after it; sample weekly to keep cost bounded (~52 calls / year).
    let start = 90usize;
    let end = btc.len().saturating_sub(30);
    let step = 7usize;

    let mut i = start;
    while i < end {
        let f = compute_features(btc, others, i);
        let realized = realized_label(&btc[i..]);
        let out = classifier.classify(&f).await?;
        let observed_at = Utc.from_utc_datetime(&f.as_of.and_hms_opt(0, 0, 0).unwrap_or_default());
        samples.push(EvalSample {
            observed_at,
            predicted_label: out.label,
            predicted_proba: out.probabilities,
            realized_label: realized,
            features: f,
        });
        i += step;
    }
    Ok(samples)
}

fn compute_features(
    btc: &[DailyPrice],
    others: &HashMap<&'static str, Vec<DailyPrice>>,
    i: usize,
) -> Features {
    let win_30 = &btc[i.saturating_sub(30)..i];
    let win_90 = &btc[i.saturating_sub(90)..i];

    let log_returns_30: Vec<f64> = win_30
        .windows(2)
        .map(|w| (w[1].price / w[0].price).ln())
        .collect();

    let btc_vol_30d = annualized_std(&log_returns_30);

    // Average pairwise Pearson correlation across (BTC,ETH), (BTC,SOL), (ETH,SOL).
    let btc_rets_90 = log_returns(win_90);
    let mut pair_corrs = Vec::new();
    for sym in ["ETH", "SOL"] {
        if let Some(series) = others.get(sym) {
            if let Some(aligned) = align_to(win_90, series) {
                let r_other = log_returns(&aligned);
                if let Some(r) = pearson(&btc_rets_90, &r_other) {
                    pair_corrs.push(r);
                }
            }
        }
    }
    if let (Some(eth), Some(sol)) = (others.get("ETH"), others.get("SOL")) {
        if let (Some(eth_a), Some(sol_a)) = (align_to(win_90, eth), align_to(win_90, sol)) {
            let r_eth = log_returns(&eth_a);
            let r_sol = log_returns(&sol_a);
            if let Some(r) = pearson(&r_eth, &r_sol) {
                pair_corrs.push(r);
            }
        }
    }
    let corr_90d = if pair_corrs.is_empty() {
        0.0
    } else {
        pair_corrs.iter().sum::<f64>() / pair_corrs.len() as f64
    };

    let max_drawdown_30d = max_drawdown(win_30);

    Features {
        as_of: btc[i].date,
        btc_vol_30d,
        corr_90d,
        max_drawdown_30d,
    }
}

fn log_returns(s: &[DailyPrice]) -> Vec<f64> {
    s.windows(2)
        .map(|w| (w[1].price / w[0].price).ln())
        .collect()
}

fn annualized_std(rets: &[f64]) -> f64 {
    if rets.len() < 2 {
        return 0.0;
    }
    let n = rets.len() as f64;
    let mean = rets.iter().sum::<f64>() / n;
    let var = rets.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0);
    var.sqrt() * (365.0f64).sqrt()
}

fn pearson(a: &[f64], b: &[f64]) -> Option<f64> {
    let n = a.len().min(b.len());
    if n < 5 {
        return None;
    }
    let ma = a[..n].iter().sum::<f64>() / n as f64;
    let mb = b[..n].iter().sum::<f64>() / n as f64;
    let mut cov = 0.0;
    let mut va = 0.0;
    let mut vb = 0.0;
    for k in 0..n {
        let da = a[k] - ma;
        let db = b[k] - mb;
        cov += da * db;
        va += da * da;
        vb += db * db;
    }
    let denom = (va * vb).sqrt();
    if denom <= 0.0 {
        None
    } else {
        Some(cov / denom)
    }
}

/// Align `other` to the same dates as `window` (1:1 by date).
fn align_to(window: &[DailyPrice], other: &[DailyPrice]) -> Option<Vec<DailyPrice>> {
    let mut by_date: HashMap<NaiveDate, f64> = HashMap::with_capacity(other.len());
    for p in other {
        by_date.insert(p.date, p.price);
    }
    let aligned: Vec<DailyPrice> = window
        .iter()
        .filter_map(|w| {
            by_date.get(&w.date).map(|p| DailyPrice {
                date: w.date,
                price: *p,
            })
        })
        .collect();
    if aligned.len() < window.len() / 2 {
        None
    } else {
        Some(aligned)
    }
}

fn max_drawdown(s: &[DailyPrice]) -> f64 {
    let mut peak = 0.0f64;
    let mut max_dd = 0.0f64;
    for p in s {
        if p.price > peak {
            peak = p.price;
        }
        if peak > 0.0 {
            let dd = (peak - p.price) / peak;
            if dd > max_dd {
                max_dd = dd;
            }
        }
    }
    max_dd
}

/// Realized label = 30d forward BTC return bucketed at ±10%.
/// Caller passes the slice starting at the window's `as_of`; we look at
/// index 0 vs index 30.
fn realized_label(forward: &[DailyPrice]) -> MarketRegime {
    let now = forward[0].price;
    let later = forward.get(30).map(|p| p.price).unwrap_or(now);
    if now <= 0.0 {
        return MarketRegime::Neutral;
    }
    let ret = (later - now) / now;
    if ret < -0.10 {
        MarketRegime::RiskOff
    } else if ret > 0.10 {
        MarketRegime::RiskOn
    } else {
        MarketRegime::Neutral
    }
}

/// Pure-function metric computation — exposed so the unit tests can pin
/// down precision/recall/F1/Brier numerics without a DB or LLM.
pub fn compute_metrics(samples: &[EvalSample]) -> EvalRun {
    let n = samples.len();
    let mut confusion = [[0u32; 3]; 3];
    let mut correct = 0u32;
    let mut brier_sum = 0.0;

    for s in samples {
        let pi = regime_index(s.predicted_label);
        let ri = regime_index(s.realized_label);
        confusion[pi][ri] += 1;
        if pi == ri {
            correct += 1;
        }
        for r in REGIMES {
            let p = *s.predicted_proba.get(&r).unwrap_or(&0.0);
            let y = if r == s.realized_label { 1.0 } else { 0.0 };
            brier_sum += (p - y).powi(2);
        }
    }

    let accuracy = correct as f64 / n.max(1) as f64;
    let brier_score = brier_sum / n.max(1) as f64;

    let mut per_regime = HashMap::with_capacity(3);
    let mut precisions = Vec::new();
    let mut recalls = Vec::new();
    let mut f1s = Vec::new();

    for r in REGIMES {
        let idx = regime_index(r);
        let tp = confusion[idx][idx] as f64;
        let fp = (0..3)
            .filter(|&j| j != idx)
            .map(|j| confusion[idx][j] as f64)
            .sum::<f64>();
        let fn_ = (0..3)
            .filter(|&j| j != idx)
            .map(|j| confusion[j][idx] as f64)
            .sum::<f64>();
        let support = (tp + fn_) as u32;
        let precision = if tp + fp > 0.0 { tp / (tp + fp) } else { 0.0 };
        let recall = if tp + fn_ > 0.0 { tp / (tp + fn_) } else { 0.0 };
        let f1 = if precision + recall > 0.0 {
            2.0 * precision * recall / (precision + recall)
        } else {
            0.0
        };
        precisions.push(precision);
        recalls.push(recall);
        f1s.push(f1);
        per_regime.insert(
            r,
            RegimeMetrics {
                precision,
                recall,
                f1,
                support,
            },
        );
    }

    let period_start = samples
        .iter()
        .map(|s| s.observed_at.date_naive())
        .min()
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).expect("epoch is a valid date"));
    let period_end = samples
        .iter()
        .map(|s| s.observed_at.date_naive())
        .max()
        .unwrap_or(period_start);

    EvalRun {
        eval_run_id: Uuid::nil(),
        model_slug: String::new(),
        task: TASK.to_string(),
        period_start,
        period_end,
        samples_count: n,
        accuracy,
        precision_macro: precisions.iter().sum::<f64>() / 3.0,
        recall_macro: recalls.iter().sum::<f64>() / 3.0,
        f1_macro: f1s.iter().sum::<f64>() / 3.0,
        brier_score,
        confusion,
        per_regime,
    }
}

fn regime_index(r: MarketRegime) -> usize {
    match r {
        MarketRegime::RiskOn => 0,
        MarketRegime::Neutral => 1,
        MarketRegime::RiskOff => 2,
    }
}

async fn persist(
    pool: &Db,
    eval_run_id: Uuid,
    model_slug: &str,
    run: &EvalRun,
    samples: &[EvalSample],
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;

    let confusion_json = json!({
        "rows": run.confusion.iter().map(|row| row.to_vec()).collect::<Vec<_>>(),
        "labels": ["risk_on", "neutral", "risk_off"],
    });
    let per_regime_json = serde_json::to_value(per_regime_keyed_by_label(&run.per_regime))?;

    sqlx::query(
        r#"
        INSERT INTO model_evaluations (
            id, model_slug, eval_run_id, task,
            period_start, period_end, samples_count,
            accuracy, precision_macro, recall_macro, f1_macro, brier_score,
            confusion_jsonb, per_regime_jsonb
        ) VALUES (
            gen_random_uuid(), $1, $2, $3,
            $4, $5, $6,
            $7, $8, $9, $10, $11,
            $12, $13
        )
        "#,
    )
    .bind(model_slug)
    .bind(eval_run_id)
    .bind(&run.task)
    .bind(run.period_start)
    .bind(run.period_end)
    .bind(run.samples_count as i32)
    .bind(run.accuracy)
    .bind(run.precision_macro)
    .bind(run.recall_macro)
    .bind(run.f1_macro)
    .bind(run.brier_score)
    .bind(SqlxJson(&confusion_json))
    .bind(SqlxJson(&per_regime_json))
    .execute(&mut *tx)
    .await?;

    for s in samples {
        let proba_json = serde_json::to_value(proba_keyed_by_label(&s.predicted_proba))?;
        let features_json = serde_json::to_value(&s.features)?;
        sqlx::query(
            r#"
            INSERT INTO model_evaluation_samples (
                id, eval_run_id, observed_at,
                predicted_label, predicted_proba,
                realized_label, features_jsonb
            ) VALUES (
                gen_random_uuid(), $1, $2,
                $3, $4,
                $5, $6
            )
            "#,
        )
        .bind(eval_run_id)
        .bind(s.observed_at)
        .bind(s.predicted_label.as_str())
        .bind(SqlxJson(&proba_json))
        .bind(s.realized_label.as_str())
        .bind(SqlxJson(&features_json))
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

fn proba_keyed_by_label(p: &HashMap<MarketRegime, f64>) -> HashMap<&'static str, f64> {
    let mut out = HashMap::with_capacity(p.len());
    for (k, v) in p {
        out.insert(k.as_str(), *v);
    }
    out
}

fn per_regime_keyed_by_label(
    p: &HashMap<MarketRegime, RegimeMetrics>,
) -> HashMap<&'static str, RegimeMetrics> {
    let mut out = HashMap::with_capacity(p.len());
    for (k, v) in p {
        out.insert(k.as_str(), v.clone());
    }
    out
}

/// Used by handlers — quick deserialization of a row for the public model card.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelEvaluationRow {
    pub id: Uuid,
    pub model_slug: String,
    pub eval_run_id: Uuid,
    pub task: String,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub samples_count: i32,
    pub accuracy: Option<f64>,
    pub precision_macro: Option<f64>,
    pub recall_macro: Option<f64>,
    pub f1_macro: Option<f64>,
    pub brier_score: Option<f64>,
    pub confusion_jsonb: serde_json::Value,
    pub per_regime_jsonb: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Latest N rows for the `regime_classifier` task, newest first.
pub async fn list_latest(pool: &Db, limit: i64) -> anyhow::Result<Vec<ModelEvaluationRow>> {
    let rows = sqlx::query_as::<_, (Uuid, String, Uuid, String, NaiveDate, NaiveDate, i32, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, sqlx::types::Json<Value>, sqlx::types::Json<Value>, DateTime<Utc>)>(
        r#"
        SELECT id, model_slug, eval_run_id, task,
               period_start, period_end, samples_count,
               accuracy::float8, precision_macro::float8, recall_macro::float8, f1_macro::float8, brier_score::float8,
               confusion_jsonb, per_regime_jsonb,
               created_at
        FROM model_evaluations
        WHERE task = $1
        ORDER BY created_at DESC
        LIMIT $2
        "#,
    )
    .bind(TASK)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|t| ModelEvaluationRow {
            id: t.0,
            model_slug: t.1,
            eval_run_id: t.2,
            task: t.3,
            period_start: t.4,
            period_end: t.5,
            samples_count: t.6,
            accuracy: t.7,
            precision_macro: t.8,
            recall_macro: t.9,
            f1_macro: t.10,
            brier_score: t.11,
            confusion_jsonb: t.12 .0,
            per_regime_jsonb: t.13 .0,
            created_at: t.14,
        })
        .collect())
}

/// True iff there's at least one stored eval row for the regime task.
#[allow(dead_code)]
pub async fn has_any(pool: &Db) -> anyhow::Result<bool> {
    let row: (bool,) =
        sqlx::query_as("SELECT EXISTS(SELECT 1 FROM model_evaluations WHERE task = $1)")
            .bind(TASK)
            .fetch_one(pool)
            .await?;
    Ok(row.0)
}

/// Document the per-run-id period (used by the daily-stamp on the model card).
#[allow(dead_code)]
pub fn current_year(at: DateTime<Utc>) -> i32 {
    at.year()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(pred: MarketRegime, realized: MarketRegime, conf: f64) -> EvalSample {
        let mut probs = HashMap::new();
        let leftover = ((1.0 - conf) / 2.0).max(0.0);
        for r in REGIMES {
            probs.insert(r, if r == pred { conf } else { leftover });
        }
        EvalSample {
            observed_at: Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
            predicted_label: pred,
            predicted_proba: probs,
            realized_label: realized,
            features: Features {
                as_of: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                btc_vol_30d: 0.5,
                corr_90d: 0.4,
                max_drawdown_30d: 0.05,
            },
        }
    }

    #[test]
    fn perfect_predictions_give_perfect_metrics() {
        let samples = vec![
            sample(MarketRegime::RiskOn, MarketRegime::RiskOn, 1.0),
            sample(MarketRegime::Neutral, MarketRegime::Neutral, 1.0),
            sample(MarketRegime::RiskOff, MarketRegime::RiskOff, 1.0),
            sample(MarketRegime::RiskOn, MarketRegime::RiskOn, 1.0),
        ];
        let m = compute_metrics(&samples);
        assert!((m.accuracy - 1.0).abs() < 1e-12);
        assert!((m.precision_macro - 1.0).abs() < 1e-12);
        assert!((m.recall_macro - 1.0).abs() < 1e-12);
        assert!((m.f1_macro - 1.0).abs() < 1e-12);
        assert!(m.brier_score.abs() < 1e-12);
    }

    #[test]
    fn fully_wrong_predictions_give_zero_accuracy() {
        let samples = vec![
            sample(MarketRegime::RiskOn, MarketRegime::RiskOff, 1.0),
            sample(MarketRegime::RiskOff, MarketRegime::RiskOn, 1.0),
        ];
        let m = compute_metrics(&samples);
        assert!(m.accuracy.abs() < 1e-12);
        // Brier with one-hot wrong predictions = 2 wrong classes per sample.
        // For each sample: (1-0)^2 on the wrong-predicted class + (0-1)^2 on the
        // realized class + 0 for the third class = 2.0; averaged → 2.0.
        assert!((m.brier_score - 2.0).abs() < 1e-12);
    }

    #[test]
    fn confusion_matrix_indexes_predicted_rows_realized_cols() {
        let samples = vec![
            // Predicted risk_on, realized neutral → confusion[0][1] = 1
            sample(MarketRegime::RiskOn, MarketRegime::Neutral, 0.7),
            // Predicted neutral, realized neutral → confusion[1][1] = 1
            sample(MarketRegime::Neutral, MarketRegime::Neutral, 0.7),
        ];
        let m = compute_metrics(&samples);
        assert_eq!(m.confusion[0][1], 1);
        assert_eq!(m.confusion[1][1], 1);
        assert_eq!(m.confusion[2][2], 0);
        assert!((m.accuracy - 0.5).abs() < 1e-12);
    }

    #[test]
    fn per_regime_precision_recall_known_case() {
        // Two predictions: one TP for RiskOn, one FP for RiskOn (predicted
        // RiskOn but realized Neutral). For RiskOn:
        //   tp=1, fp=1, fn=0 → precision = 0.5, recall = 1.0, f1 = 2/3.
        let samples = vec![
            sample(MarketRegime::RiskOn, MarketRegime::RiskOn, 0.6),
            sample(MarketRegime::RiskOn, MarketRegime::Neutral, 0.6),
        ];
        let m = compute_metrics(&samples);
        let r_on = m.per_regime.get(&MarketRegime::RiskOn).unwrap();
        assert!((r_on.precision - 0.5).abs() < 1e-12);
        assert!((r_on.recall - 1.0).abs() < 1e-12);
        assert!((r_on.f1 - (2.0 / 3.0)).abs() < 1e-12);
        assert_eq!(r_on.support, 1);
    }

    #[test]
    fn brier_for_uncertain_correct_prediction() {
        // confidence 0.6 on the correct class, 0.2 on each other.
        // Brier = (0.6-1)^2 + (0.2-0)^2 + (0.2-0)^2 = 0.16 + 0.04 + 0.04 = 0.24.
        let s = sample(MarketRegime::RiskOn, MarketRegime::RiskOn, 0.6);
        let m = compute_metrics(std::slice::from_ref(&s));
        assert!((m.brier_score - 0.24).abs() < 1e-12);
    }

    #[test]
    fn max_drawdown_finds_peak_to_trough() {
        let series: Vec<DailyPrice> = [100.0, 110.0, 95.0, 105.0, 80.0]
            .into_iter()
            .enumerate()
            .map(|(i, p)| DailyPrice {
                date: NaiveDate::from_ymd_opt(2024, 1, 1 + i as u32).unwrap(),
                price: p,
            })
            .collect();
        // Peak 110, trough 80 → dd = 30/110 ≈ 0.2727.
        let dd = max_drawdown(&series);
        assert!((dd - (30.0 / 110.0)).abs() < 1e-9);
    }

    #[test]
    fn realized_label_buckets_at_ten_percent() {
        let up: Vec<DailyPrice> = (0..=30)
            .map(|i| DailyPrice {
                date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                price: 100.0 + i as f64,
            })
            .collect();
        assert_eq!(realized_label(&up), MarketRegime::RiskOn);

        let flat: Vec<DailyPrice> = (0..=30)
            .map(|_| DailyPrice {
                date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                price: 100.0,
            })
            .collect();
        assert_eq!(realized_label(&flat), MarketRegime::Neutral);

        let dn: Vec<DailyPrice> = (0..=30)
            .map(|i| DailyPrice {
                date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                price: 100.0 - 1.5 * i as f64,
            })
            .collect();
        assert_eq!(realized_label(&dn), MarketRegime::RiskOff);
    }

    #[test]
    fn parse_label_unpacks_top1_into_proba_vector() {
        let raw = r#"{"regime":"risk_off","confidence":0.8}"#;
        let out = parse_label(raw).unwrap();
        assert_eq!(out.label, MarketRegime::RiskOff);
        let p_off = out.probabilities.get(&MarketRegime::RiskOff).unwrap();
        let p_on = out.probabilities.get(&MarketRegime::RiskOn).unwrap();
        assert!((p_off - 0.8).abs() < 1e-12);
        assert!((p_on - 0.1).abs() < 1e-12);
    }

    /// Mock classifier returns whatever a stored function says, no I/O.
    struct StaticClassifier {
        label: MarketRegime,
        confidence: f64,
    }

    #[async_trait]
    impl RegimeClassifier for StaticClassifier {
        async fn classify(&self, _features: &Features) -> anyhow::Result<ClassifierOutput> {
            let mut probs = HashMap::new();
            let leftover = ((1.0 - self.confidence) / 2.0).max(0.0);
            for r in REGIMES {
                probs.insert(
                    r,
                    if r == self.label {
                        self.confidence
                    } else {
                        leftover
                    },
                );
            }
            Ok(ClassifierOutput {
                label: self.label,
                probabilities: probs,
            })
        }
    }

    #[tokio::test]
    async fn walk_windows_emits_samples_with_mock_classifier() {
        // Synthesize a 200-day BTC series so walk_windows has room to walk.
        let btc: Vec<DailyPrice> = (0..200)
            .map(|i| DailyPrice {
                date: NaiveDate::from_ymd_opt(2024, 1, 1)
                    .unwrap()
                    .checked_add_days(chrono::Days::new(i as u64))
                    .unwrap(),
                price: 100.0 + (i as f64).sin() * 5.0,
            })
            .collect();
        let others = HashMap::new();
        let clf = StaticClassifier {
            label: MarketRegime::Neutral,
            confidence: 0.6,
        };
        let samples = walk_windows(&btc, &others, &clf).await.unwrap();
        assert!(!samples.is_empty());
        for s in &samples {
            assert_eq!(s.predicted_label, MarketRegime::Neutral);
        }
    }
}
