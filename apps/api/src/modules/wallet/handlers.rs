use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::{extract::State, Extension, Json};

use super::models::{
    InitWalletRequest, WalletAuthResponse, WalletStatusResponse, WalletUserPublic,
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

/// Signup — body: `{ email, referrerHandle? }`. Returns the W3S
/// `UserTokenBundle` the browser SDK needs to complete the PIN ceremony, plus
/// a JWT session cookie bound to the new user. Wallet addresses arrive
/// asynchronously via polling `GET /auth/wallet/status`.
pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<InitWalletRequest>,
) -> crate::error::Result<axum::response::Response> {
    let resp = if state.config.circle_mock {
        let p = MockProvider;
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.init_signup(&body.email).await?
    } else {
        let p = CircleProvider::new(&state.http, &state.config);
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.init_signup(&body.email).await?
    };
    maybe_credit_referral(&state, body.referrer_handle.as_deref(), &resp).await;
    Ok(auth_response(&state.config, StatusCode::CREATED, resp).into_response())
}

/// Login — body: `{ email }`. Returning users get a fresh `UserTokenBundle`
/// (no challenge_id; PIN was set on signup) and a refreshed JWT cookie.
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<InitWalletRequest>,
) -> crate::error::Result<axum::response::Response> {
    let resp = if state.config.circle_mock {
        let p = MockProvider;
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.init_login(&body.email).await?
    } else {
        let p = CircleProvider::new(&state.http, &state.config);
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.init_login(&body.email).await?
    };
    Ok(auth_response(&state.config, StatusCode::OK, resp).into_response())
}

/// Polled by the browser after the SDK completes the challenge. Returns the
/// wallet info once Circle has provisioned both chains, otherwise
/// `{ wallet: null }` so the browser keeps polling.
pub async fn status(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> crate::error::Result<Json<WalletStatusResponse>> {
    let resp = if state.config.circle_mock {
        let p = MockProvider;
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.fetch_wallet_status(claims.sub).await?
    } else {
        let p = CircleProvider::new(&state.http, &state.config);
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.fetch_wallet_status(claims.sub).await?
    };
    Ok(Json(resp))
}

async fn maybe_credit_referral(
    state: &AppState,
    referrer_handle: Option<&str>,
    resp: &WalletAuthResponse,
) {
    let Some(handle) = referrer_handle else {
        return;
    };
    let handle = handle.trim().to_ascii_lowercase();
    if handle.is_empty() || !resp.is_new_user {
        return;
    }
    if let Err(e) = crate::modules::billing::service::record_referral(
        &state.db,
        &state.config,
        &state.sse,
        &handle,
        resp.user.id,
    )
    .await
    {
        tracing::warn!(error=%e, "referral attribution failed");
    }
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

/// Clears the session cookie.
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
