use axum::{extract::State, Extension, Json};

use super::service::{broadcast, fetch_balance, GatewayBalance};
use crate::error::AppError;
use crate::middleware::auth::Claims;
use crate::router::AppState;

pub async fn balance(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> crate::error::Result<Json<GatewayBalance>> {
    let wallet_id = claims
        .wallet_id
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("user has no wallet".into()))?;
    let balance = fetch_balance(&state.http, &state.config, wallet_id).await?;
    broadcast(&state.sse, &balance);
    Ok(Json(balance))
}
