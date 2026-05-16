//! Public model-card handler — exposes the loaded constitution as JSON so
//! the `/about/constitution` page can render the version, effective date,
//! and the full list of clauses. Read-only, not gated by the feature flag:
//! the constitution is always loaded; the flag only controls whether the
//! critic *applies* it.

use axum::{extract::State, Json};

use super::constitution::{self, Constitution};
use crate::router::AppState;

pub async fn document(State(_state): State<AppState>) -> crate::error::Result<Json<Constitution>> {
    let c = constitution::load()
        .map_err(|e| crate::error::AppError::Internal(anyhow::anyhow!("constitution load: {e}")))?;
    Ok(Json(c.clone()))
}
