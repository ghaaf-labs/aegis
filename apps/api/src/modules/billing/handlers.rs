//! Billing HTTP endpoints.
//!
//! GET /billing/referrals — list the caller's referrals + reward totals.
//! Used by the dashboard "Referrals" widget so users see who they brought in
//! and how much they've earned.

use axum::{extract::State, Extension, Json};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::error::Result;
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
