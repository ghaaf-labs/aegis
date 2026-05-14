use axum::{
    extract::{Path, State},
    Extension, Json,
};
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::middleware::auth::Claims;
use crate::router::AppState;

use super::models::HarvestableLoss;
use super::service::harvestable_losses;

pub async fn harvestable(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(portfolio_id): Path<Uuid>,
) -> Result<Json<Vec<HarvestableLoss>>> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM portfolios WHERE id = $1 AND user_id = $2)",
    )
    .bind(portfolio_id)
    .bind(claims.sub)
    .fetch_one(&state.db)
    .await?;
    if !exists {
        return Err(AppError::NotFound(format!("portfolio {portfolio_id}")));
    }
    Ok(Json(
        harvestable_losses(&state, claims.sub, portfolio_id).await?,
    ))
}
