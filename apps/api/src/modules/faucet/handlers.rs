use axum::{extract::State, Extension, Json};

use super::service::{claim, FaucetClaimResult};
use crate::middleware::auth::Claims;
use crate::modules::wallet_routes;
use crate::router::AppState;

pub async fn claim_usdc(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> crate::error::Result<Json<FaucetClaimResult>> {
    let arc_address = wallet_routes::arc_address_for_user(
        &state.db,
        claims.sub,
        &state.config.circle_wallet_set_id,
    )
    .await?
    .ok_or_else(|| crate::error::AppError::BadRequest("user has no wallet".into()))?;

    let result = claim(&state.db, &state.config, claims.sub, &arc_address).await?;
    Ok(Json(result))
}
