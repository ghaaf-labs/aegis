//! F-CONF-3 — nightly calibration trainer.
//!
//! Two tasks fit on the same schedule:
//!
//!   1. **regime_classifier** — reads the most recent `model_evaluations` row
//!      for `task='regime_classifier'`, joins its samples, and trains a
//!      3-class histogram-bin calibrator. Persists one row in `calibrations`.
//!
//!   2. **strategist_confidence** — reads agent_memory outcome rows joined to
//!      their decision. Treats `outcome_24h.pnlPct > 0` as a positive
//!      realization ("the strategist was right"). Trains a single-class
//!      ("right") histogram-bin calibrator and persists a second row.
//!
//! Both fits are gated by `CALIBRATED_CONF_ENABLED`. The task spawns at
//! startup, runs once immediately (so demos see a row right away), then
//! every 24h. Errors are logged but do not crash the worker.
//!
//! The `agent_memory.outcome_24h` JSONB blob shape is documented in
//! `apps/api/src/modules/agent/memory.rs`: `{ "pnlPct": <f64>, ... }`.
//! "Right" is defined here as pnlPct strictly positive — a clean binary
//! signal that lets the histogram-bin map the strategist's *flat*
//! confidence into the same probability space.

use std::collections::HashMap;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::db::Db;
use crate::modules::agent::calibration::{self, Calibration, CalibrationSample, REGIME_CLASSES};
use crate::router::AppState;

/// Minimum sample count before we'll fit the strategist calibrator.
/// Lower than the plan's "≥50" guidance so demo databases can show *some*
/// calibration; bumping is a one-liner once we're past pilot.
pub const STRATEGIST_MIN_SAMPLES: usize = 20;

/// 24h tick. Exposed so tests can override.
const TICK_INTERVAL_SECS: u64 = 24 * 60 * 60;

pub const TASK_REGIME: &str = "regime_classifier";
pub const TASK_STRATEGIST: &str = "strategist_confidence";

/// Spawn the nightly trainer in the background. No-op when the feature flag
/// is off (the spawn itself still happens to keep startup symmetric; the
/// task exits immediately after logging the gate).
pub fn spawn(state: AppState, cancel: CancellationToken) {
    tokio::spawn(async move {
        if !state.config.calibrated_conf_enabled {
            info!("calibration trainer: CALIBRATED_CONF_ENABLED=false; trainer disabled");
            return;
        }
        info!("calibration trainer: starting (interval {TICK_INTERVAL_SECS}s)");

        // Run once at startup so demos see a row immediately.
        if let Err(e) = tick(&state.db).await {
            warn!("calibration trainer initial tick failed: {e:#}");
        }

        let mut interval = tokio::time::interval(Duration::from_secs(TICK_INTERVAL_SECS));
        // First tick fires immediately; skip it because we already ran.
        interval.tick().await;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("calibration trainer: shutting down");
                    return;
                }
                _ = interval.tick() => {
                    if let Err(e) = tick(&state.db).await {
                        error!("calibration trainer tick failed: {e:#}");
                    }
                }
            }
        }
    });
}

/// One training pass over both tasks. Public so the integration test in
/// F-CONF-8 can drive it without spinning up a worker.
pub async fn tick(db: &Db) -> anyhow::Result<()> {
    if let Err(e) = fit_regime(db).await {
        warn!("calibration trainer: regime fit failed: {e:#}");
    }
    if let Err(e) = fit_strategist(db).await {
        warn!("calibration trainer: strategist fit failed: {e:#}");
    }
    Ok(())
}

/// Fit a regime calibrator from the most recent backtest run's samples.
pub async fn fit_regime(db: &Db) -> anyhow::Result<Option<Uuid>> {
    let latest: Option<(Uuid, String)> = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT eval_run_id, model_slug
        FROM model_evaluations
        WHERE task = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(TASK_REGIME)
    .fetch_optional(db)
    .await?;

    let Some((eval_run_id, model_slug)) = latest else {
        info!("calibration trainer: no model_evaluations row for {TASK_REGIME}; skipping");
        return Ok(None);
    };

    let rows: Vec<(serde_json::Value, String)> = sqlx::query_as::<_, (serde_json::Value, String)>(
        r#"
        SELECT predicted_proba, realized_label
        FROM model_evaluation_samples
        WHERE eval_run_id = $1
        "#,
    )
    .bind(eval_run_id)
    .fetch_all(db)
    .await?;

    if rows.is_empty() {
        info!("calibration trainer: regime eval_run_id={eval_run_id} has zero samples");
        return Ok(None);
    }

    let samples: Vec<CalibrationSample> = rows
        .into_iter()
        .map(|(p, realized)| CalibrationSample {
            predicted_proba: deserialize_proba(&p),
            realized_label: realized,
        })
        .collect();

    let cal = calibration::fit(&samples);
    let id = persist(db, &model_slug, TASK_REGIME, Some(eval_run_id), &cal).await?;
    info!(
        "calibration trainer: regime fit n={} brier {:.4}→{:.4} id={id}",
        samples.len(),
        cal.brier_before,
        cal.brier_after
    );
    Ok(Some(id))
}

/// Fit the strategist confidence calibrator from agent_memory outcomes.
///
/// Each row is one (raw_confidence, was_right) pair. Histogram-bin treats this
/// as a single-class problem with class label "right"; the calibrator's
/// `apply_scalar` is used at inference time.
pub async fn fit_strategist(db: &Db) -> anyhow::Result<Option<Uuid>> {
    let rows: Vec<(f64, Option<serde_json::Value>, Option<String>)> =
        sqlx::query_as::<_, (f64, Option<serde_json::Value>, Option<String>)>(
            r#"
        SELECT d.confidence,
               m.outcome_24h,
               d.model_slug
        FROM agent_memory m
        JOIN agent_decisions d ON d.id = m.decision_id
        WHERE m.outcome_24h IS NOT NULL
        ORDER BY m.recorded_at DESC
        LIMIT 5000
        "#,
        )
        .fetch_all(db)
        .await?;

    if rows.len() < STRATEGIST_MIN_SAMPLES {
        info!(
            "calibration trainer: strategist outcomes n={} below threshold {STRATEGIST_MIN_SAMPLES}; skipping",
            rows.len()
        );
        return Ok(None);
    }

    let mut model_count: HashMap<String, usize> = HashMap::new();
    let samples: Vec<CalibrationSample> = rows
        .iter()
        .map(|(conf, outcome, slug)| {
            if let Some(s) = slug {
                *model_count.entry(s.clone()).or_default() += 1;
            }
            let pnl_pct = outcome
                .as_ref()
                .and_then(|v| v.get("pnlPct"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let realized_label = if pnl_pct > 0.0 { "right" } else { "wrong" }.to_string();
            let mut proba = HashMap::with_capacity(2);
            proba.insert("right".to_string(), clamp01(*conf));
            proba.insert("wrong".to_string(), clamp01(1.0 - *conf));
            CalibrationSample {
                predicted_proba: proba,
                realized_label,
            }
        })
        .collect();

    // Fit with a 2-class hist-bin. The shared `fit` is hard-coded to the 3-class
    // regime layout, so build the calibrator inline to keep the trainer self-contained.
    let cal = fit_binary(&samples);
    let model_slug = model_count
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(s, _)| s)
        .unwrap_or_else(|| "mixed".to_string());

    let id = persist(db, &model_slug, TASK_STRATEGIST, None, &cal).await?;
    info!(
        "calibration trainer: strategist fit n={} brier {:.4}→{:.4} id={id}",
        samples.len(),
        cal.brier_before,
        cal.brier_after
    );
    Ok(Some(id))
}

fn fit_binary(samples: &[CalibrationSample]) -> Calibration {
    let classes = vec!["right".to_string(), "wrong".to_string()];
    let n_bins = calibration::N_BINS;
    let mut bins: Vec<calibration::Bin> = (0..n_bins)
        .map(|i| calibration::Bin {
            lo: i as f64 / n_bins as f64,
            hi: (i + 1) as f64 / n_bins as f64,
            empirical: HashMap::new(),
            n: 0,
        })
        .collect();

    let mut per_bin: Vec<HashMap<String, (usize, usize)>> = vec![HashMap::new(); n_bins];

    for s in samples {
        for class in &classes {
            let raw = clamp01(*s.predicted_proba.get(class).unwrap_or(&0.0));
            let idx = bin_index(raw, n_bins);
            let entry = per_bin[idx].entry(class.clone()).or_insert((0, 0));
            entry.1 += 1;
            if &s.realized_label == class {
                entry.0 += 1;
            }
            bins[idx].n += 1;
        }
    }

    for (i, classes_in_bin) in per_bin.into_iter().enumerate() {
        for (class, (hits, total)) in classes_in_bin {
            if total > 0 {
                bins[i].empirical.insert(class, hits as f64 / total as f64);
            }
        }
    }

    let brier_before = calibration::brier(samples);
    // Compute Brier after by applying the calibrator we just fit.
    let brier_after = {
        let calibrated: Vec<CalibrationSample> = samples
            .iter()
            .map(|s| {
                let mut out: HashMap<String, f64> = HashMap::new();
                for class in &classes {
                    let raw = clamp01(*s.predicted_proba.get(class).unwrap_or(&0.0));
                    let idx = bin_index(raw, n_bins);
                    let v = bins[idx].empirical.get(class).copied().unwrap_or(raw);
                    out.insert(class.clone(), v);
                }
                let sum: f64 = out.values().copied().sum();
                if sum > f64::EPSILON {
                    for v in out.values_mut() {
                        *v /= sum;
                    }
                }
                CalibrationSample {
                    predicted_proba: out,
                    realized_label: s.realized_label.clone(),
                }
            })
            .collect();
        calibration::brier(&calibrated)
    };

    Calibration {
        classes,
        bins,
        fit_samples_count: samples.len(),
        brier_before,
        brier_after,
    }
}

async fn persist(
    db: &Db,
    model_slug: &str,
    task: &str,
    source_eval_run_id: Option<Uuid>,
    cal: &Calibration,
) -> anyhow::Result<Uuid> {
    let params = serde_json::to_value(cal)?;
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO calibrations
            (id, model_slug, task, source_eval_run_id, method, params_jsonb,
             fit_samples_count, brier_before, brier_after)
        VALUES ($1, $2, $3, $4, 'brier_bin', $5, $6, $7, $8)
        "#,
    )
    .bind(id)
    .bind(model_slug)
    .bind(task)
    .bind(source_eval_run_id)
    .bind(&params)
    .bind(cal.fit_samples_count as i32)
    .bind(cal.brier_before)
    .bind(cal.brier_after)
    .execute(db)
    .await?;
    Ok(id)
}

/// Row shape from `calibrations` used by `latest_for`. Lifted out so we don't
/// trip clippy::type_complexity on the tuple inference.
#[derive(sqlx::FromRow)]
struct LatestRow {
    id: Uuid,
    model_slug: String,
    params_jsonb: serde_json::Value,
    fit_samples_count: i32,
    brier_before: f64,
    brier_after: f64,
    source_eval_run_id: Option<Uuid>,
}

/// Fetch the most recent calibration for a task, deserialized.
pub async fn latest_for(db: &Db, task: &str) -> anyhow::Result<Option<LatestCalibration>> {
    let row: Option<LatestRow> = sqlx::query_as::<_, LatestRow>(
        r#"
        SELECT id, model_slug, params_jsonb, fit_samples_count,
               brier_before, brier_after, source_eval_run_id
        FROM calibrations
        WHERE task = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(task)
    .fetch_optional(db)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let cal: Calibration = serde_json::from_value(row.params_jsonb)?;
    Ok(Some(LatestCalibration {
        id: row.id,
        model_slug: row.model_slug,
        calibration: cal,
        fit_samples_count: row.fit_samples_count,
        brier_before: row.brier_before,
        brier_after: row.brier_after,
        source_eval_run_id: row.source_eval_run_id,
    }))
}

#[derive(Debug, Clone)]
pub struct LatestCalibration {
    pub id: Uuid,
    pub model_slug: String,
    pub calibration: Calibration,
    pub fit_samples_count: i32,
    pub brier_before: f64,
    pub brier_after: f64,
    pub source_eval_run_id: Option<Uuid>,
}

fn deserialize_proba(v: &serde_json::Value) -> HashMap<String, f64> {
    let mut out = HashMap::with_capacity(REGIME_CLASSES.len());
    if let Some(obj) = v.as_object() {
        for k in REGIME_CLASSES {
            let p = obj.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
            out.insert(k.to_string(), p);
        }
    }
    out
}

fn bin_index(p: f64, n_bins: usize) -> usize {
    let p = clamp01(p);
    let i = (p * n_bins as f64).floor() as usize;
    if i >= n_bins {
        n_bins - 1
    } else {
        i
    }
}

fn clamp01(p: f64) -> f64 {
    if p.is_nan() {
        return 0.0;
    }
    p.clamp(0.0, 1.0)
}
