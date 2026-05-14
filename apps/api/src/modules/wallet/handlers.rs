use axum::{extract::State, http::StatusCode, Extension, Json};

use super::models::{
    LoginPasskeyRequest, OtpStartRequest, OtpStartResponse, OtpVerifyRequest,
    RegisterPasskeyRequest, WalletAuthResponse, WalletUserPublic,
};
use super::provider::{CircleProvider, MockProvider};
use super::service::WalletService;
use crate::middleware::auth::Claims;
use crate::router::AppState;

pub async fn create_passkey(
    State(state): State<AppState>,
    Json(body): Json<RegisterPasskeyRequest>,
) -> crate::error::Result<(StatusCode, Json<WalletAuthResponse>)> {
    let resp = if state.config.circle_mock {
        let p = MockProvider;
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.create_with_passkey(&body.email, &body.passkey_attestation)
            .await?
    } else {
        let p = CircleProvider::new(&state.http, &state.config);
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.create_with_passkey(&body.email, &body.passkey_attestation)
            .await?
    };
    Ok((StatusCode::CREATED, Json(resp)))
}

pub async fn login_passkey(
    State(state): State<AppState>,
    Json(body): Json<LoginPasskeyRequest>,
) -> crate::error::Result<Json<WalletAuthResponse>> {
    let resp = if state.config.circle_mock {
        let p = MockProvider;
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.login_with_passkey(&body.email, &body.passkey_assertion)
            .await?
    } else {
        let p = CircleProvider::new(&state.http, &state.config);
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.login_with_passkey(&body.email, &body.passkey_assertion)
            .await?
    };
    Ok(Json(resp))
}

pub async fn start_otp(
    State(state): State<AppState>,
    Json(body): Json<OtpStartRequest>,
) -> crate::error::Result<Json<OtpStartResponse>> {
    let resp = if state.config.circle_mock {
        let p = MockProvider;
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.start_otp(&body.email).await?
    } else {
        let p = CircleProvider::new(&state.http, &state.config);
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.start_otp(&body.email).await?
    };
    Ok(Json(resp))
}

pub async fn verify_otp(
    State(state): State<AppState>,
    Json(body): Json<OtpVerifyRequest>,
) -> crate::error::Result<Json<WalletAuthResponse>> {
    let resp = if state.config.circle_mock {
        let p = MockProvider;
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.verify_otp(&body.email, &body.code).await?
    } else {
        let p = CircleProvider::new(&state.http, &state.config);
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.verify_otp(&body.email, &body.code).await?
    };
    Ok(Json(resp))
}

pub async fn me(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> crate::error::Result<Json<WalletUserPublic>> {
    let user = sqlx::query_as::<_, super::models::WalletUser>(
        "SELECT id, email, risk_tolerance, investment_horizon_months,
                wallet_id, arc_address, base_address, created_at
         FROM users WHERE id = $1",
    )
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| crate::error::AppError::Unauthorized("unknown user".into()))?;
    Ok(Json(WalletUserPublic {
        id: user.id,
        email: user.email,
        risk_tolerance: user.risk_tolerance,
    }))
}
