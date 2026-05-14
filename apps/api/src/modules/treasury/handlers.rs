use axum::{extract::State, Json};

use super::service::{rate, UsycRate};
use crate::router::AppState;

pub async fn usyc_rate(State(state): State<AppState>) -> crate::error::Result<Json<UsycRate>> {
    let r = rate(&state.http, &state.config).await?;
    Ok(Json(r))
}
