use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::{extract::State, Extension, Json};

use super::models::{
    LoginPasskeyRequest, OtpStartRequest, OtpStartResponse, OtpVerifyRequest,
    RegisterPasskeyRequest, WalletAuthResponse, WalletUserPublic,
};
use super::provider::{CircleProvider, MockProvider};
use super::service::WalletService;
use crate::config::Config;
use crate::middleware::auth::Claims;
use crate::router::AppState;

/// Build the `Set-Cookie` header for the JWT. HttpOnly, Lax, optionally
/// Secure (env-driven). Browsers send this on every same-site request, so
/// `fetch(..., { credentials: "include" })` and `EventSource(..., { withCredentials })`
/// both authenticate without exposing the token to JS.
fn session_cookie(config: &Config, token: &str) -> HeaderValue {
    let secure = if config.session_cookie_secure {
        "; Secure"
    } else {
        ""
    };
    let max_age = config.jwt_expiry_hours.saturating_mul(3600);
    let value = format!(
        "{name}={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age={max_age}{secure}",
        name = config.session_cookie_name,
    );
    HeaderValue::from_str(&value).expect("session cookie value is ASCII")
}

fn auth_response(
    config: &Config,
    status: StatusCode,
    resp: WalletAuthResponse,
) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, session_cookie(config, &resp.token));
    (status, headers, Json(resp))
}

pub async fn create_passkey(
    State(state): State<AppState>,
    Json(body): Json<RegisterPasskeyRequest>,
) -> crate::error::Result<axum::response::Response> {
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
    Ok(auth_response(&state.config, StatusCode::CREATED, resp).into_response())
}

pub async fn login_passkey(
    State(state): State<AppState>,
    Json(body): Json<LoginPasskeyRequest>,
) -> crate::error::Result<axum::response::Response> {
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
    Ok(auth_response(&state.config, StatusCode::OK, resp).into_response())
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
) -> crate::error::Result<axum::response::Response> {
    let resp = if state.config.circle_mock {
        let p = MockProvider;
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.verify_otp(&body.email, &body.code).await?
    } else {
        let p = CircleProvider::new(&state.http, &state.config);
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.verify_otp(&body.email, &body.code).await?
    };
    Ok(auth_response(&state.config, StatusCode::OK, resp).into_response())
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

/// Clears the session cookie. Frontend can also just forget the localStorage
/// fallback after calling this.
pub async fn logout(State(state): State<AppState>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    let cleared = format!(
        "{name}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0",
        name = state.config.session_cookie_name
    );
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cleared).expect("ASCII"),
    );
    (StatusCode::NO_CONTENT, headers)
}
