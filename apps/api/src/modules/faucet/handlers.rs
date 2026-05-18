use axum::{extract::State, Extension, Json};

use super::service::{claim, FaucetClaimResult};
use crate::middleware::auth::Claims;
use crate::router::AppState;

pub async fn claim_usdc(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> crate::error::Result<Json<FaucetClaimResult>> {
    let arc_address: Option<String> =
        sqlx::query_scalar("SELECT arc_address FROM users WHERE id = $1")
            .bind(claims.sub)
            .fetch_optional(&state.db)
            .await?
            .flatten();

    let arc_address = arc_address
        .ok_or_else(|| crate::error::AppError::BadRequest("user has no wallet".into()))?;

    let result = claim(&state.db, &state.config, claims.sub, &arc_address).await?;
    Ok(Json(result))
}
