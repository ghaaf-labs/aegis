use axum::{extract::State, Extension, Json};
use axum::http::StatusCode;

use crate::{middleware::auth::Claims, router::AppState};
use super::{models::{LoginRequest, RegisterRequest, UserPublic}, service};

pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> crate::error::Result<(StatusCode, Json<super::models::AuthResponse>)> {
    let resp = service::register(&state.db, body, &state.config).await?;
    Ok((StatusCode::CREATED, Json(resp)))
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> crate::error::Result<Json<super::models::AuthResponse>> {
    let resp = service::login(&state.db, body, &state.config).await?;
    Ok(Json(resp))
}

pub async fn me(
    Extension(claims): Extension<Claims>,
) -> Json<UserPublic> {
    Json(UserPublic {
        id: claims.sub,
        email: claims.email,
        risk_tolerance: "moderate".into(),
    })
}
