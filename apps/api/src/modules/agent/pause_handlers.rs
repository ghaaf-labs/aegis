use axum::{extract::State, Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::middleware::auth::Claims;
use crate::router::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPauseStatus {
    pub paused_at: Option<DateTime<Utc>>,
    /// When true, the scheduler executes the agent's rebalances on its own
    /// (within the safety clamps) instead of only surfacing a review.
    pub auto_pilot_enabled: bool,
}

pub async fn status(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<AgentPauseStatus>> {
    let row: Option<(Option<DateTime<Utc>>, bool)> =
        sqlx::query_as("SELECT agent_paused_at, auto_pilot_enabled FROM users WHERE id = $1")
            .bind(claims.sub)
            .fetch_optional(&state.db)
            .await?;
    let (paused_at, auto_pilot_enabled) = row.unwrap_or((None, false));
    Ok(Json(AgentPauseStatus {
        paused_at,
        auto_pilot_enabled,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAutoPilotBody {
    pub enabled: bool,
}

/// Read the auto-pilot flag on its own (the dashboard toggle polls this).
pub async fn auto_pilot_status(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<AgentPauseStatus>> {
    status(State(state), Extension(claims)).await
}

/// Flip auto-pilot on/off. ON → the agent executes deployments + rebalances on
/// its own within the guardrails (clamp, stable floor, $5 dust min, route
/// registry fail-closed, constitution). OFF → one per-move approval screen.
pub async fn set_auto_pilot(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<SetAutoPilotBody>,
) -> Result<Json<AgentPauseStatus>> {
    let row: (Option<DateTime<Utc>>, bool) = sqlx::query_as(
        "UPDATE users SET auto_pilot_enabled = $2 WHERE id = $1
         RETURNING agent_paused_at, auto_pilot_enabled",
    )
    .bind(claims.sub)
    .bind(body.enabled)
    .fetch_one(&state.db)
    .await?;
    tracing::info!(user_id = %claims.sub, enabled = body.enabled, "agent auto-pilot toggled");
    Ok(Json(AgentPauseStatus {
        paused_at: row.0,
        auto_pilot_enabled: row.1,
    }))
}

pub async fn pause(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<AgentPauseStatus>> {
    let row: (Option<DateTime<Utc>>, bool) = sqlx::query_as(
        "UPDATE users SET agent_paused_at = COALESCE(agent_paused_at, NOW()) \
         WHERE id = $1 RETURNING agent_paused_at, auto_pilot_enabled",
    )
    .bind(claims.sub)
    .fetch_one(&state.db)
    .await?;
    tracing::info!(user_id = %claims.sub, "agent paused");
    Ok(Json(AgentPauseStatus {
        paused_at: row.0,
        auto_pilot_enabled: row.1,
    }))
}

pub async fn resume(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<AgentPauseStatus>> {
    let auto_pilot_enabled: bool = sqlx::query_scalar(
        "UPDATE users SET agent_paused_at = NULL WHERE id = $1 RETURNING auto_pilot_enabled",
    )
    .bind(claims.sub)
    .fetch_one(&state.db)
    .await?;
    tracing::info!(user_id = %claims.sub, "agent resumed");
    Ok(Json(AgentPauseStatus {
        paused_at: None,
        auto_pilot_enabled,
    }))
}
