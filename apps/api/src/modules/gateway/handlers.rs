use axum::{extract::State, Extension, Json};

use super::service::{broadcast, fetch_balance, GatewayBalance};
use crate::middleware::auth::Claims;
use crate::router::AppState;

pub async fn balance(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> crate::error::Result<Json<GatewayBalance>> {
    let balance = fetch_balance(&state.http, &state.config, claims.sub).await?;
    broadcast(&state.sse, claims.sub, &balance);
    Ok(Json(balance))
}
