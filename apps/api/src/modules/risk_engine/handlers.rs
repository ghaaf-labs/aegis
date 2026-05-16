//! F-REG-4 — Regime-backtest HTTP handlers.
//!
//! - `GET  /admin/regime/evaluations` — latest 10 evaluation rows. Auth
//!   required (admin-gating arrives later); also gated by
//!   `REGIME_BACKTEST_ENABLED`.
//! - `POST /admin/regime/backtest` — kicks off an async run, returns the
//!   new `evalRunId`. Same gates.
//! - `GET  /about/regime/latest` — public read-only alias for the model
//!   card page; reads the single newest persisted row. Not gated, so the
//!   model card keeps working even with the flag off.

use std::time::Duration;

use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::regime_backtest::{
    list_latest, run_backtest, ModelEvaluationRow, OpenRouterRegimeClassifier,
};
use crate::error::{AppError, Result};
use crate::modules::ai::OpenRouterClient;
use crate::router::AppState;

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationsQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationsResponse {
    pub evaluations: Vec<ModelEvaluationRow>,
}

pub async fn list_evaluations(
    State(state): State<AppState>,
    Query(q): Query<EvaluationsQuery>,
) -> Result<Json<EvaluationsResponse>> {
    if !state.config.regime_backtest_enabled {
        return Err(AppError::NotFound(
            "regime backtest endpoints are disabled".into(),
        ));
    }
    let limit = q.limit.unwrap_or(10).clamp(1, 100);
    let evaluations = list_latest(&state.db, limit)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(EvaluationsResponse { evaluations }))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BacktestRequest {
    /// How many years of price_history to walk. Defaults to 5.
    pub years: Option<u32>,
    /// Override the OpenRouter slug. Defaults to `config.model_regime`.
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestResponse {
    pub eval_run_id: Uuid,
    pub model_slug: String,
    pub samples_count: usize,
    pub accuracy: f64,
    pub precision_macro: f64,
    pub recall_macro: f64,
    pub f1_macro: f64,
    pub brier_score: f64,
}

pub async fn kick_off_backtest(
    State(state): State<AppState>,
    Json(req): Json<BacktestRequest>,
) -> Result<Json<BacktestResponse>> {
    if !state.config.regime_backtest_enabled {
        return Err(AppError::NotFound(
            "regime backtest endpoints are disabled".into(),
        ));
    }
    let years = req.years.unwrap_or(5).clamp(1, 10);
    let mut cfg = state.config.clone();
    if let Some(m) = req.model {
        cfg.model_regime = m;
    }

    let ai = OpenRouterClient::new(&state.http, &cfg);
    let classifier = OpenRouterRegimeClassifier {
        ai,
        prompts: state.prompts.as_ref(),
        min_delay: Duration::from_millis(250),
        max_retries: 5,
    };

    let run = run_backtest(&state.db, &cfg.model_regime, years, &classifier)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(BacktestResponse {
        eval_run_id: run.eval_run_id,
        model_slug: run.model_slug,
        samples_count: run.samples_count,
        accuracy: run.accuracy,
        precision_macro: run.precision_macro,
        recall_macro: run.recall_macro,
        f1_macro: run.f1_macro,
        brier_score: run.brier_score,
    }))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestEvaluationResponse {
    pub evaluation: Option<ModelEvaluationRow>,
}

/// Public read-only alias powering the `/about/regime` model card. Always
/// safe to call: returns `{ "evaluation": null }` when no run has been
/// persisted yet.
pub async fn latest_public(
    State(state): State<AppState>,
) -> Result<Json<LatestEvaluationResponse>> {
    let rows = list_latest(&state.db, 1)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(LatestEvaluationResponse {
        evaluation: rows.into_iter().next(),
    }))
}
