//! Risk-engine HTTP handlers — two co-located feature surfaces:
//!
//! **F-REG-4 — Regime-backtest admin + public model card** (A7):
//! - `GET  /admin/regime/evaluations` — latest evaluation rows. Auth
//!   required; gated by `REGIME_BACKTEST_ENABLED`.
//! - `POST /admin/regime/backtest` — kicks off an async run, returns
//!   the new `evalRunId`. Same gates.
//! - `GET  /about/regime/latest` — public read-only alias for the model
//!   card; reads the newest persisted row. Not gated.
//!
//! **F-PEG-4 — Peg-defense CRUD** (A6):
//! - `GET/POST/PATCH/DELETE /peg/rules[/:id]` — user-scoped rules.
//! - `POST /peg/rules/:id/pause` and `/unpause`.
//! - Every route is scoped to the authenticated user — session A
//!   can never read or mutate user B's rule. The `PEG_DEFENSE_ENABLED`
//!   flag is enforced at the handler level so a production build with
//!   the flag off returns 404 across the whole namespace without
//!   leaking route existence.

use std::time::Duration;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::regime_backtest::{
    list_latest, run_backtest, ModelEvaluationRow, OpenRouterRegimeClassifier,
};
use crate::error::{AppError, Result};
use crate::middleware::auth::Claims;
use crate::modules::ai::OpenRouterClient;
use crate::router::AppState;

// ═════════════════════════════════════════════════════════════════════════
// Regime backtest (F-REG-4)
// ═════════════════════════════════════════════════════════════════════════

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
    Extension(claims): Extension<Claims>,
    Json(req): Json<BacktestRequest>,
) -> Result<Json<BacktestResponse>> {
    if !state.config.admin_user_ids.contains(&claims.sub) {
        return Err(AppError::NotFound("not found".into()));
    }
    if !state.config.regime_backtest_enabled {
        return Err(AppError::NotFound(
            "regime backtest endpoints are disabled".into(),
        ));
    }
    let years = req.years.unwrap_or(5).clamp(1, 10);
    let cfg = state.config.clone();
    // Caller-supplied model overrides are intentionally ignored to prevent
    // billing abuse via expensive model injection.
    let _ = req.model;

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

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct BacktestSample {
    pub observed_at: DateTime<Utc>,
    pub predicted_label: String,
    pub realized_label: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestSamplesResponse {
    pub eval_run_id: Option<Uuid>,
    pub model_slug: Option<String>,
    pub samples: Vec<BacktestSample>,
}

/// FF-1 — public timeseries for the regime backtest UI. Returns up to
/// `limit` samples (default 200, max 2000) from the most recent eval run,
/// ordered by `observed_at`. No auth required; flag-gated so a fresh
/// deploy without backtest data 404s cleanly.
pub async fn backtest_samples_public(
    State(state): State<AppState>,
    Query(q): Query<EvaluationsQuery>,
) -> Result<Json<BacktestSamplesResponse>> {
    if !state.config.regime_backtest_enabled {
        return Err(AppError::NotFound(
            "regime backtest endpoints are disabled".into(),
        ));
    }
    let limit = q.limit.unwrap_or(200).clamp(1, 2000);

    let latest = list_latest(&state.db, 1)
        .await
        .map_err(AppError::Internal)?
        .into_iter()
        .next();
    let Some(run) = latest else {
        return Ok(Json(BacktestSamplesResponse {
            eval_run_id: None,
            model_slug: None,
            samples: Vec::new(),
        }));
    };

    let samples: Vec<BacktestSample> = sqlx::query_as(
        "SELECT observed_at, predicted_label, realized_label \
         FROM model_evaluation_samples \
         WHERE eval_run_id = $1 \
         ORDER BY observed_at ASC \
         LIMIT $2",
    )
    .bind(run.eval_run_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(BacktestSamplesResponse {
        eval_run_id: Some(run.eval_run_id),
        model_slug: Some(run.model_slug),
        samples,
    }))
}

// ═════════════════════════════════════════════════════════════════════════
// Peg defense (F-PEG-4)
// ═════════════════════════════════════════════════════════════════════════

const ALLOWED_ASSETS: &[&str] = &["USDC", "EURC", "USYC"];
const ALLOWED_ACTION_KINDS: &[&str] = &["alert", "propose_rebalance", "auto_execute"];

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PegRuleView {
    pub id: Uuid,
    pub user_id: Uuid,
    pub portfolio_id: Option<Uuid>,
    pub asset: String,
    pub threshold_price: f64,
    pub window_seconds: i32,
    pub action_kind: String,
    pub target_asset: Option<String>,
    pub enabled: bool,
    pub paused_at: Option<DateTime<Utc>>,
    pub last_fired_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRuleBody {
    pub portfolio_id: Option<Uuid>,
    pub asset: String,
    pub threshold_price: f64,
    #[serde(default = "default_window")]
    pub window_seconds: i32,
    pub action_kind: String,
    pub target_asset: Option<String>,
}

fn default_window() -> i32 {
    300
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PatchRuleBody {
    pub enabled: Option<bool>,
    pub paused: Option<bool>,
    pub threshold_price: Option<f64>,
    pub window_seconds: Option<i32>,
    pub action_kind: Option<String>,
    pub target_asset: Option<Option<String>>,
}

pub async fn list(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<PegRuleView>>> {
    ensure_peg_enabled(&state)?;
    let rows: Vec<PegRuleView> = sqlx::query_as(
        "SELECT id, user_id, portfolio_id, asset, threshold_price, window_seconds,
                action_kind, target_asset, enabled, paused_at, last_fired_at,
                created_at, updated_at
         FROM peg_rules WHERE user_id = $1
         ORDER BY created_at DESC LIMIT 200",
    )
    .bind(claims.sub)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

pub async fn create(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRuleBody>,
) -> Result<(StatusCode, Json<PegRuleView>)> {
    ensure_peg_enabled(&state)?;
    validate_create(&body)?;

    if let Some(pid) = body.portfolio_id {
        own_portfolio_or_404(&state, claims.sub, pid).await?;
    }

    let row: PegRuleView = sqlx::query_as(
        "INSERT INTO peg_rules
            (user_id, portfolio_id, asset, threshold_price, window_seconds,
             action_kind, target_asset)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id, user_id, portfolio_id, asset, threshold_price, window_seconds,
                   action_kind, target_asset, enabled, paused_at, last_fired_at,
                   created_at, updated_at",
    )
    .bind(claims.sub)
    .bind(body.portfolio_id)
    .bind(body.asset.to_uppercase())
    .bind(body.threshold_price)
    .bind(body.window_seconds)
    .bind(body.action_kind)
    .bind(body.target_asset.map(|s| s.to_uppercase()))
    .fetch_one(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn patch(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(rule_id): Path<Uuid>,
    Json(body): Json<PatchRuleBody>,
) -> Result<Json<PegRuleView>> {
    ensure_peg_enabled(&state)?;
    own_rule_or_404(&state, claims.sub, rule_id).await?;
    let current_asset: String =
        sqlx::query_scalar("SELECT asset FROM peg_rules WHERE id = $1 AND user_id = $2")
            .bind(rule_id)
            .bind(claims.sub)
            .fetch_one(&state.db)
            .await?;

    if let Some(p) = body.threshold_price {
        validate_threshold_price(p)?;
    }
    if let Some(w) = body.window_seconds {
        if w < 0 {
            return Err(AppError::BadRequest(
                "windowSeconds must be non-negative".into(),
            ));
        }
    }
    if let Some(ref a) = body.action_kind {
        if !ALLOWED_ACTION_KINDS.contains(&a.as_str()) {
            return Err(AppError::BadRequest(format!(
                "actionKind must be one of {ALLOWED_ACTION_KINDS:?}"
            )));
        }
        if a == "auto_execute" {
            return Err(AppError::PaymentRequired(
                "auto_execute peg-defense rules are not available yet; choose alert or propose_rebalance"
                    .into(),
            ));
        }
    }
    if let Some(Some(ref ta)) = body.target_asset {
        validate_target_asset(ta, Some(&current_asset))?;
    }

    let row: PegRuleView = sqlx::query_as(
        "UPDATE peg_rules
            SET enabled         = COALESCE($2, enabled),
                paused_at       = CASE
                                    WHEN $3::boolean IS NULL THEN paused_at
                                    WHEN $3 = TRUE THEN NOW()
                                    ELSE NULL
                                  END,
                threshold_price = COALESCE($4, threshold_price),
                window_seconds  = COALESCE($5, window_seconds),
                action_kind     = COALESCE($6, action_kind),
                target_asset    = CASE
                                    WHEN $7::boolean IS TRUE THEN $8
                                    ELSE target_asset
                                  END
          WHERE id = $1
         RETURNING id, user_id, portfolio_id, asset, threshold_price, window_seconds,
                   action_kind, target_asset, enabled, paused_at, last_fired_at,
                   created_at, updated_at",
    )
    .bind(rule_id)
    .bind(body.enabled)
    .bind(body.paused)
    .bind(body.threshold_price)
    .bind(body.window_seconds)
    .bind(body.action_kind)
    .bind(body.target_asset.is_some())
    .bind(body.target_asset.flatten().map(|s| s.to_uppercase()))
    .fetch_one(&state.db)
    .await?;
    Ok(Json(row))
}

pub async fn delete(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(rule_id): Path<Uuid>,
) -> Result<StatusCode> {
    ensure_peg_enabled(&state)?;
    own_rule_or_404(&state, claims.sub, rule_id).await?;
    sqlx::query("DELETE FROM peg_rules WHERE id = $1")
        .bind(rule_id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn pause(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(rule_id): Path<Uuid>,
) -> Result<Json<PegRuleView>> {
    ensure_peg_enabled(&state)?;
    own_rule_or_404(&state, claims.sub, rule_id).await?;
    let row: PegRuleView = sqlx::query_as(
        "UPDATE peg_rules SET paused_at = NOW() WHERE id = $1
         RETURNING id, user_id, portfolio_id, asset, threshold_price, window_seconds,
                   action_kind, target_asset, enabled, paused_at, last_fired_at,
                   created_at, updated_at",
    )
    .bind(rule_id)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(row))
}

pub async fn unpause(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(rule_id): Path<Uuid>,
) -> Result<Json<PegRuleView>> {
    ensure_peg_enabled(&state)?;
    own_rule_or_404(&state, claims.sub, rule_id).await?;
    let row: PegRuleView = sqlx::query_as(
        "UPDATE peg_rules SET paused_at = NULL WHERE id = $1
         RETURNING id, user_id, portfolio_id, asset, threshold_price, window_seconds,
                   action_kind, target_asset, enabled, paused_at, last_fired_at,
                   created_at, updated_at",
    )
    .bind(rule_id)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(row))
}

// ── Peg helpers ──────────────────────────────────────────────────────────

fn ensure_peg_enabled(state: &AppState) -> Result<()> {
    if !state.config.peg_defense_enabled {
        return Err(AppError::NotFound("peg defense disabled".into()));
    }
    Ok(())
}

fn validate_create(body: &CreateRuleBody) -> Result<()> {
    let asset = body.asset.to_uppercase();
    if !ALLOWED_ASSETS.contains(&asset.as_str()) {
        return Err(AppError::BadRequest(format!(
            "asset must be one of {ALLOWED_ASSETS:?}"
        )));
    }
    validate_threshold_price(body.threshold_price)?;
    if body.window_seconds < 0 {
        return Err(AppError::BadRequest(
            "windowSeconds must be non-negative".into(),
        ));
    }
    if !ALLOWED_ACTION_KINDS.contains(&body.action_kind.as_str()) {
        return Err(AppError::BadRequest(format!(
            "actionKind must be one of {ALLOWED_ACTION_KINDS:?}"
        )));
    }
    if body.action_kind == "auto_execute" {
        return Err(AppError::PaymentRequired(
            "auto_execute peg-defense rules are not available yet; choose alert or propose_rebalance"
                .into(),
        ));
    }
    validate_target_asset(body.target_asset.as_deref().unwrap_or(""), Some(&asset))?;
    Ok(())
}

fn validate_threshold_price(price: f64) -> Result<()> {
    if !price.is_finite() || price <= 0.0 || price > 1.0 {
        return Err(AppError::BadRequest(
            "thresholdPrice must be greater than 0 and at or below 1.0".into(),
        ));
    }
    Ok(())
}

fn validate_target_asset(target_asset: &str, source_asset: Option<&str>) -> Result<()> {
    if target_asset.trim().is_empty() {
        return Ok(());
    }
    let target_asset = target_asset.to_uppercase();
    if !ALLOWED_ASSETS.contains(&target_asset.as_str()) {
        return Err(AppError::BadRequest(format!(
            "targetAsset must be one of {ALLOWED_ASSETS:?}"
        )));
    }
    if source_asset.is_some_and(|source| source == target_asset) {
        return Err(AppError::BadRequest(
            "targetAsset must differ from asset".into(),
        ));
    }
    Ok(())
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

async fn own_rule_or_404(state: &AppState, user_id: Uuid, rule_id: Uuid) -> Result<()> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM peg_rules WHERE id = $1 AND user_id = $2)")
            .bind(rule_id)
            .bind(user_id)
            .fetch_one(&state.db)
            .await?;
    if !exists {
        return Err(AppError::NotFound(format!("peg rule {rule_id}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(asset: &str, threshold: f64, action: &str) -> CreateRuleBody {
        CreateRuleBody {
            portfolio_id: None,
            asset: asset.into(),
            threshold_price: threshold,
            window_seconds: 300,
            action_kind: action.into(),
            target_asset: None,
        }
    }

    #[test]
    fn validate_accepts_canonical_inputs() {
        assert!(validate_create(&body("USDC", 0.995, "alert")).is_ok());
        assert!(validate_create(&body("usdc", 0.99, "propose_rebalance")).is_ok());
    }

    #[test]
    fn validate_rejects_unknown_asset() {
        let err = validate_create(&body("BTC", 0.5, "alert")).unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn validate_rejects_non_positive_threshold() {
        assert!(validate_create(&body("USDC", 0.0, "alert")).is_err());
        assert!(validate_create(&body("USDC", -0.5, "alert")).is_err());
        assert!(validate_create(&body("USDC", f64::NAN, "alert")).is_err());
    }

    #[test]
    fn validate_rejects_always_firing_threshold() {
        let err = validate_create(&body("USDC", 1.0001, "alert")).unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn validate_rejects_unknown_action_kind() {
        assert!(validate_create(&body("USDC", 0.995, "yolo")).is_err());
    }

    #[test]
    fn validate_rejects_auto_execute_until_gate_lands() {
        let err = validate_create(&body("EURC", 0.97, "auto_execute")).unwrap_err();
        assert!(matches!(err, AppError::PaymentRequired(_)));
    }

    #[test]
    fn validate_rejects_unknown_target_asset() {
        let mut b = body("USDC", 0.995, "propose_rebalance");
        b.target_asset = Some("ETH".into());
        assert!(validate_create(&b).is_err());
    }

    #[test]
    fn validate_rejects_target_matching_source_asset() {
        let mut b = body("USDC", 0.995, "propose_rebalance");
        b.target_asset = Some("USDC".into());
        assert!(validate_create(&b).is_err());
    }
}
