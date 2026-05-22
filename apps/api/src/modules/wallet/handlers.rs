use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::{extract::State, Extension, Json};

use super::models::{
    InitWalletRequest, RequestWalletAuthCode, WalletAuthCodeResponse, WalletAuthIntent,
    WalletAuthReadinessResponse, WalletAuthResponse, WalletStatusResponse, WalletUserPublic,
};
use super::provider::{CircleProvider, MockProvider};
use super::service::WalletService;
use crate::config::Config;
use crate::middleware::auth::{decode_claims, Claims};
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

/// First step for signup/login. Sends a short-lived verification code to the
/// email address. Fully mocked local wallet runs may return `devCode`; real
/// Circle mode must use real email delivery so auth proof is not shown in the
/// same browser that is trying to sign in.
pub async fn request_code(
    State(state): State<AppState>,
    Json(body): Json<RequestWalletAuthCode>,
) -> crate::error::Result<Json<WalletAuthCodeResponse>> {
    let has_email_delivery = !state.config.resend_api_key.trim().is_empty();
    let can_return_dev_code = mock_dev_auth_codes_allowed(&state.config);
    if !has_email_delivery && !can_return_dev_code {
        return Err(crate::error::AppError::ServiceUnavailable(
            "wallet auth email is disabled; set RESEND_API_KEY for real Circle login or set MOCK_CIRCLE=true for local dev codes"
                .into(),
        ));
    }

    let p = MockProvider;
    let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
    let issue = svc
        .request_auth_code(&body.email, body.intent, body.referrer_handle.as_deref())
        .await?;
    let mut response = issue.response;
    if can_return_dev_code {
        response.dev_code = Some(issue.code);
    } else {
        send_auth_code_email(&state, &response.email, body.intent, &issue.code).await?;
    }
    Ok(Json(response))
}

/// Public readiness probe for the wallet auth form. It exposes only coarse
/// capability state so the UI can fail closed before asking the user to wait
/// for an email that this backend cannot send.
pub async fn readiness(
    State(state): State<AppState>,
) -> crate::error::Result<Json<WalletAuthReadinessResponse>> {
    Ok(Json(WalletAuthReadinessResponse {
        circle_mock: state.config.circle_mock,
        email_delivery_configured: !state.config.resend_api_key.trim().is_empty(),
        dev_codes_enabled: mock_dev_auth_codes_allowed(&state.config),
    }))
}

/// Signup — body: `{ email, referrerHandle? }`. Returns the W3S
/// `UserTokenBundle` the browser SDK needs to complete the PIN ceremony, plus
/// a JWT session cookie bound to the new user. Wallet addresses arrive
/// asynchronously via polling `GET /auth/wallet/status`.
pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<InitWalletRequest>,
) -> crate::error::Result<axum::response::Response> {
    let verifier = MockProvider;
    let verifier_svc = WalletService::new(&state.db, &verifier, &state.config, &state.sse);
    let referrer_from_code = verifier_svc
        .verify_auth_code(
            &body.email,
            body.challenge_id,
            &body.code,
            WalletAuthIntent::Signup,
        )
        .await?;
    let resp = if state.config.circle_mock {
        let p = MockProvider;
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.init_signup(&body.email).await?
    } else {
        let p = CircleProvider::new(&state.http, &state.config);
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.init_signup(&body.email).await?
    };
    let referrer = body
        .referrer_handle
        .as_deref()
        .or(referrer_from_code.as_deref());
    maybe_credit_referral(&state, referrer, &resp).await;
    Ok(auth_response(&state.config, StatusCode::CREATED, resp).into_response())
}

/// Login — body: `{ email }`. Returning users with a provisioned wallet get a
/// refreshed JWT cookie and wallet info. Circle credentials are returned only
/// for abandoned wallet setup that still needs a browser challenge.
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<InitWalletRequest>,
) -> crate::error::Result<axum::response::Response> {
    let verifier = MockProvider;
    let verifier_svc = WalletService::new(&state.db, &verifier, &state.config, &state.sse);
    verifier_svc
        .verify_auth_code(
            &body.email,
            body.challenge_id,
            &body.code,
            WalletAuthIntent::Login,
        )
        .await?;
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
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> crate::error::Result<axum::response::Response> {
    if let Some(token) = token_from_headers(&headers, &state.config.session_cookie_name) {
        if let Ok(claims) = decode_claims(&state, &token) {
            sqlx::query(
                "UPDATE auth_sessions
                 SET revoked_at = COALESCE(revoked_at, NOW())
                 WHERE user_id = $1
                   AND revoked_at IS NULL",
            )
            .bind(claims.sub)
            .execute(&state.db)
            .await?;
        }
    }

    let mut headers = HeaderMap::new();
    let secure = if state.config.session_cookie_secure {
        "; Secure"
    } else {
        ""
    };
    let cleared = format!(
        "{name}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT{secure}",
        name = state.config.session_cookie_name
    );
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cleared).expect("ASCII"),
    );
    Ok((StatusCode::NO_CONTENT, headers).into_response())
}

fn token_from_headers(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    if let Some(bearer) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        return Some(bearer.to_string());
    }

    let cookie_header = headers.get(header::COOKIE).and_then(|v| v.to_str().ok())?;
    for piece in cookie_header.split(';') {
        let trimmed = piece.trim();
        if let Some(value) = trimmed.strip_prefix(&format!("{cookie_name}=")) {
            return Some(value.to_string());
        }
    }
    None
}

async fn send_auth_code_email(
    state: &AppState,
    email: &str,
    intent: WalletAuthIntent,
    code: &str,
) -> crate::error::Result<()> {
    let action = match intent {
        WalletAuthIntent::Signup => "create your Aegis wallet",
        WalletAuthIntent::Login => "restore your Aegis wallet session",
    };
    let payload = serde_json::json!({
        "from": state.config.digest_from,
        "to": [email],
        "subject": "Aegis verification code",
        "html": format!(
            "<p>Your Aegis code is <strong>{}</strong>.</p><p>Use it within 10 minutes to {}. If you did not request this, you can ignore this email.</p>",
            code, action
        ),
    });
    let resp = state
        .http
        .post("https://api.resend.com/emails")
        .bearer_auth(&state.config.resend_api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| crate::error::AppError::Internal(anyhow::anyhow!("resend net: {e}")))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(crate::error::AppError::Internal(anyhow::anyhow!(
            "resend status {status}: {text}"
        )));
    }
    Ok(())
}

fn mock_dev_auth_codes_allowed(config: &Config) -> bool {
    dev_codes_allowed_for(
        config.circle_mock,
        &config.public_base_url,
        &config.api_base_url,
        &config.cors_allow_origin,
    )
}

fn dev_codes_allowed_for(
    circle_mock: bool,
    public_base_url: &str,
    api_base_url: &str,
    cors_allow_origin: &str,
) -> bool {
    if !circle_mock {
        return false;
    }
    let origins = [public_base_url, api_base_url, cors_allow_origin];
    origins
        .iter()
        .any(|v| v.contains("localhost") || v.contains("127.0.0.1") || v.contains("[::1]"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn dev_codes_require_mock_circle_and_local_origin() {
        assert!(super::dev_codes_allowed_for(
            true,
            "http://localhost:3000",
            "http://localhost:8080",
            "http://localhost:3000",
        ));
        assert!(!super::dev_codes_allowed_for(
            false,
            "http://localhost:3000",
            "http://localhost:8080",
            "http://localhost:3000",
        ));
        assert!(!super::dev_codes_allowed_for(
            true,
            "https://app.aegis.example",
            "https://api.aegis.example",
            "https://app.aegis.example",
        ));
    }
}
