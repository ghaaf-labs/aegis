use axum::{extract::State, Json};

use super::service::{usdc_eurc_basis, UsdcEurcBasis};
use crate::router::AppState;

pub async fn basis(State(state): State<AppState>) -> crate::error::Result<Json<UsdcEurcBasis>> {
    let basis = usdc_eurc_basis(&state.http, &state.config).await?;
    Ok(Json(basis))
}
