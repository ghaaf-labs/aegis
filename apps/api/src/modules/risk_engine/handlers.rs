//! Peg-defense CRUD handlers.
//!
//! Every route is scoped to the authenticated user — peg rules are private,
//! and a JWT for user A can never read, mutate, or pause a rule belonging to
//! user B. The PEG_DEFENSE_ENABLED flag is enforced at the route level so a
//! production build with the flag off returns 404 for the whole namespace
//! without leaking the route's existence.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::middleware::auth::Claims;
use crate::router::AppState;

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
    ensure_enabled(&state)?;
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
    ensure_enabled(&state)?;
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
    ensure_enabled(&state)?;
    own_rule_or_404(&state, claims.sub, rule_id).await?;

    if let Some(p) = body.threshold_price {
        if !p.is_finite() || p <= 0.0 {
            return Err(AppError::BadRequest(
                "thresholdPrice must be positive".into(),
            ));
        }
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
    ensure_enabled(&state)?;
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
    ensure_enabled(&state)?;
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
    ensure_enabled(&state)?;
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

// ── Helpers ───────────────────────────────────────────────────────────────

fn ensure_enabled(state: &AppState) -> Result<()> {
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
    if !body.threshold_price.is_finite() || body.threshold_price <= 0.0 {
        return Err(AppError::BadRequest(
            "thresholdPrice must be positive".into(),
        ));
    }
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
    if let Some(ref ta) = body.target_asset {
        let ta = ta.to_uppercase();
        if !ALLOWED_ASSETS.contains(&ta.as_str()) {
            return Err(AppError::BadRequest(format!(
                "targetAsset must be one of {ALLOWED_ASSETS:?}"
            )));
        }
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
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM peg_rules WHERE id = $1 AND user_id = $2)",
    )
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
        let mut b = body("EURC", 0.97, "auto_execute");
        b.target_asset = Some("USYC".into());
        assert!(validate_create(&b).is_ok());
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
    fn validate_rejects_unknown_action_kind() {
        assert!(validate_create(&body("USDC", 0.995, "yolo")).is_err());
    }

    #[test]
    fn validate_rejects_unknown_target_asset() {
        let mut b = body("USDC", 0.995, "propose_rebalance");
        b.target_asset = Some("ETH".into());
        assert!(validate_create(&b).is_err());
    }
}
