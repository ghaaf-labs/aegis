//! Trustability HTTP handlers.
//!
//! - `GET /trustability/me` — per-user aggregate; powers the dashboard hero.
//! - `GET /leaderboard` — top-50 by `trustability_delta`. Public route (the
//!   `handle` field is a non-reversible hash of `user_id`, so anonymity is
//!   preserved). Consumed by `/leaderboard` page.

use axum::{
    extract::{Query, State},
    Extension, Json,
};
use serde::{Deserialize, Serialize};

use super::service::{self, TrustabilityProgress, TrustabilityRow};
use crate::error::Result;
use crate::middleware::auth::Claims;
use crate::router::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustabilityResponse {
    pub row: Option<TrustabilityRow>,
    pub progress: TrustabilityProgress,
    /// `null` when there's no row yet (new user, no decisions in 7 days).
    pub label: Option<&'static str>,
}

pub async fn me(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<TrustabilityResponse>> {
    let row = service::for_user(&state.db, claims.sub).await?;
    let progress = service::progress_for_user(&state.db, claims.sub).await?;
    let label = row
        .as_ref()
        .map(|r| service::label_for_delta(r.trustability_delta));
    Ok(Json(TrustabilityResponse {
        row,
        progress,
        label,
    }))
}

#[derive(Debug, Deserialize, Default)]
pub struct LeaderboardQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardEntry {
    #[serde(flatten)]
    pub row: TrustabilityRow,
    pub label: &'static str,
}

pub async fn leaderboard(
    State(state): State<AppState>,
    Query(q): Query<LeaderboardQuery>,
) -> Result<Json<Vec<LeaderboardEntry>>> {
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let rows = service::leaderboard(&state.db, limit).await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| {
                let label = service::label_for_delta(row.trustability_delta);
                LeaderboardEntry { row, label }
            })
            .collect(),
    ))
}
