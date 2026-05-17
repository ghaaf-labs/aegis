use axum::{
    extract::{Path, State},
    Extension, Json,
};
use serde::Serialize;
use uuid::Uuid;

use super::models::StrategyPublic;
use super::service;
use crate::error::{AppError, Result};
use crate::middleware::auth::Claims;
use crate::router::AppState;

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<StrategyPublic>>> {
    let rows = service::list_public(&state.db)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    Ok(Json(rows))
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<StrategyPublic>> {
    let row = service::get(&state.db, id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .ok_or_else(|| AppError::NotFound(format!("strategy {id}")))?;
    Ok(Json(row))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptResponse {
    pub portfolio_id: Uuid,
}

pub async fn adopt(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<Json<AdoptResponse>> {
    let strategy = service::get(&state.db, id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .ok_or_else(|| AppError::NotFound(format!("strategy {id}")))?;
    let portfolio_id = service::adopt(&state.db, claims.sub, &strategy)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    Ok(Json(AdoptResponse { portfolio_id }))
}
