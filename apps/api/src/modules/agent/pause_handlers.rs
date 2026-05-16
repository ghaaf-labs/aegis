use axum::{extract::State, Extension, Json};
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::error::Result;
use crate::middleware::auth::Claims;
use crate::router::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPauseStatus {
    pub paused_at: Option<DateTime<Utc>>,
}

pub async fn status(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<AgentPauseStatus>> {
    let paused_at: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT agent_paused_at FROM users WHERE id = $1")
            .bind(claims.sub)
            .fetch_optional(&state.db)
            .await?
            .flatten();
    Ok(Json(AgentPauseStatus { paused_at }))
}

pub async fn pause(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<AgentPauseStatus>> {
    let paused_at: DateTime<Utc> = sqlx::query_scalar(
        "UPDATE users SET agent_paused_at = COALESCE(agent_paused_at, NOW()) \
         WHERE id = $1 RETURNING agent_paused_at",
    )
    .bind(claims.sub)
    .fetch_one(&state.db)
    .await?;
    tracing::info!(user_id = %claims.sub, "agent paused");
    Ok(Json(AgentPauseStatus {
        paused_at: Some(paused_at),
    }))
}

pub async fn resume(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<AgentPauseStatus>> {
    sqlx::query("UPDATE users SET agent_paused_at = NULL WHERE id = $1")
        .bind(claims.sub)
        .execute(&state.db)
        .await?;
    tracing::info!(user_id = %claims.sub, "agent resumed");
    Ok(Json(AgentPauseStatus { paused_at: None }))
}
