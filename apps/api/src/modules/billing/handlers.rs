//! Billing HTTP endpoints.
//!
//! GET /billing/referrals — list the caller's referrals + reward totals.
//! Used by the dashboard "Referrals" widget so users see who they brought in
//! and how much they've earned.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Extension, Json,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::aum_stream::{self, TickReport};
use crate::error::{AppError, Result};
use crate::middleware::auth::Claims;
use crate::router::AppState;

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ReferralRow {
    pub id: Uuid,
    pub new_user_id: Uuid,
    pub reward_usdc: f64,
    pub paid_at: Option<DateTime<Utc>>,
    pub tx_hash: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferralsResponse {
    pub rows: Vec<ReferralRow>,
    pub total_paid_usdc: f64,
    pub total_pending_usdc: f64,
}

pub async fn list_referrals(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<ReferralsResponse>> {
    let rows: Vec<ReferralRow> = sqlx::query_as(
        "SELECT id, new_user_id, reward_usdc, paid_at, tx_hash, created_at
         FROM referrals WHERE referrer_user_id = $1
         ORDER BY created_at DESC LIMIT 100",
    )
    .bind(claims.sub)
    .fetch_all(&state.db)
    .await?;

    let total_paid_usdc = rows
        .iter()
        .filter(|r| r.paid_at.is_some())
        .map(|r| r.reward_usdc)
        .sum::<f64>();
    let total_pending_usdc = rows
        .iter()
        .filter(|r| r.paid_at.is_none())
        .map(|r| r.reward_usdc)
        .sum::<f64>();

    Ok(Json(ReferralsResponse {
        rows,
        total_paid_usdc,
        total_pending_usdc,
    }))
}

// ── F-AUM-5 — admin observability ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccrualListQuery {
    pub user_id: Option<Uuid>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AccrualListRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub subscription_id: Uuid,
    pub invoice_id: Option<Uuid>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub aum_snapshot_usd: Decimal,
    pub bps: i32,
    pub accrued_usdc: Decimal,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccrualListResponse {
    pub rows: Vec<AccrualListRow>,
}

fn require_aum_enabled(state: &AppState) -> Result<()> {
    if !state.config.aum_stream_enabled {
        return Err(AppError::NotFound("aum_stream disabled".into()));
    }
    Ok(())
}

pub async fn list_accruals(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Query(q): Query<AccrualListQuery>,
) -> Result<Json<AccrualListResponse>> {
    require_aum_enabled(&state)?;
    let limit = q.limit.unwrap_or(100).clamp(1, 1000);
    let rows: Vec<AccrualListRow> = match q.user_id {
        Some(uid) => {
            sqlx::query_as(
                "SELECT id, user_id, subscription_id, invoice_id, period_start, period_end,
                    aum_snapshot_usd, bps, accrued_usdc, created_at
             FROM aum_accruals WHERE user_id = $1
             ORDER BY period_end DESC LIMIT $2",
            )
            .bind(uid)
            .bind(limit)
            .fetch_all(&state.db)
            .await?
        }
        None => {
            sqlx::query_as(
                "SELECT id, user_id, subscription_id, invoice_id, period_start, period_end,
                    aum_snapshot_usd, bps, accrued_usdc, created_at
             FROM aum_accruals ORDER BY period_end DESC LIMIT $1",
            )
            .bind(limit)
            .fetch_all(&state.db)
            .await?
        }
    };
    Ok(Json(AccrualListResponse { rows }))
}

pub async fn run_accruals_once(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
) -> Result<(StatusCode, Json<TickReport>)> {
    require_aum_enabled(&state)?;
    let report = aum_stream::run_once(&state.db, &state.config)
        .await
        .map_err(AppError::Internal)?;
    Ok((StatusCode::OK, Json(report)))
}
