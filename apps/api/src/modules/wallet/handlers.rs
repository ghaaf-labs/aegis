use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::{extract::State, Extension, Json};

use super::models::{
    ResendEmailAuthRequest, StartEmailAuthRequest, VerifyEmailAuthRequest, WalletAuthCodeResponse,
    WalletAuthResponse, WalletSessionResponse,
};
use super::provider::{CircleProvider, MockProvider};
use super::service::WalletService;
use crate::config::Config;
use crate::middleware::auth::{claims_from_session, Claims};
use crate::router::AppState;

/// Build the `Set-Cookie` header for the opaque session id. HttpOnly, Lax, optionally
/// Secure (env-driven). Browsers send this on every same-site request, so
/// `fetch(..., { credentials: "include" })` and `EventSource(..., { withCredentials })`
/// both authenticate without exposing the session to JS.
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
    headers.insert(
        header::SET_COOKIE,
        session_cookie(config, &resp.session_token),
    );
    (status, headers, Json(resp))
}

async fn issue_code(
    state: &AppState,
    email: &str,
    referrer_handle: Option<&str>,
) -> crate::error::Result<Json<WalletAuthCodeResponse>> {
    let p = MockProvider;
    let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
    let issue = svc.request_auth_code(email, referrer_handle).await?;
    deliver_code_issue(state, issue).await
}

async fn resend_code(
    state: &AppState,
    challenge_id: uuid::Uuid,
) -> crate::error::Result<Json<WalletAuthCodeResponse>> {
    let p = MockProvider;
    let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
    let issue = svc.resend_auth_code(challenge_id).await?;
    deliver_code_issue(state, issue).await
}

async fn deliver_code_issue(
    state: &AppState,
    issue: super::service::WalletAuthCodeIssue,
) -> crate::error::Result<Json<WalletAuthCodeResponse>> {
    let mut response = issue.response;

    match code_delivery_mode(&state.config) {
        CodeDeliveryMode::DevCode => {
            response.dev_code = Some(issue.code);
        }
        CodeDeliveryMode::Email => {
            if let Err(e) = send_auth_code_email(state, &response.email, &issue.code).await {
                tracing::error!(error=%e, "wallet auth email delivery failed after code issuance");
            }
        }
        CodeDeliveryMode::Unavailable => {
            tracing::error!(
                email_domain = sender_domain(&response.email).as_deref().unwrap_or("unknown"),
                "wallet auth email delivery is not configured; code challenge was issued but no email was sent"
            );
        }
    }

    Ok(Json(response))
}

/// Unified email auth start. The response is identical for known and unknown
/// emails so the entry screen does not need separate signup/login branches.
pub async fn email_start(
    State(state): State<AppState>,
    Json(body): Json<StartEmailAuthRequest>,
) -> crate::error::Result<Json<WalletAuthCodeResponse>> {
    issue_code(&state, &body.email, body.referrer_handle.as_deref()).await
}

/// Challenge-scoped resend. Keeps the same challenge id, rotates the code,
/// and enforces the cooldown from the original send/resend timestamp.
pub async fn email_resend(
    State(state): State<AppState>,
    Json(body): Json<ResendEmailAuthRequest>,
) -> crate::error::Result<Json<WalletAuthCodeResponse>> {
    resend_code(&state, body.challenge_id).await
}

/// Unified email auth verification. After the code is valid, the server
/// decides whether to restore an existing account or create/resume setup.
pub async fn email_verify(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<VerifyEmailAuthRequest>,
) -> crate::error::Result<axum::response::Response> {
    let verifier = MockProvider;
    let verifier_svc = WalletService::new(&state.db, &verifier, &state.config, &state.sse);
    let referrer_from_code = match verifier_svc
        .verify_auth_code(&body.email, body.challenge_id, &body.code)
        .await
    {
        Ok(referrer) => referrer,
        Err(crate::error::AppError::BadRequest(message)) if message == "code_used" => {
            if let Some(resp) = idempotent_verify_response(&state, &headers, &body.email).await? {
                return Ok(auth_response(&state.config, StatusCode::OK, resp).into_response());
            }
            return Err(crate::error::AppError::BadRequest("code_used".into()));
        }
        Err(e) => return Err(e),
    };

    let resp = if state.config.circle_mock {
        let p = MockProvider;
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.init_continue(&body.email, body.consent.as_ref())
            .await?
    } else {
        let p = CircleProvider::new(&state.http, &state.config);
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.init_continue(&body.email, body.consent.as_ref())
            .await?
    };
    let referrer = body
        .referrer_handle
        .as_deref()
        .or(referrer_from_code.as_deref());
    maybe_credit_referral(&state, referrer, &resp).await;
    Ok(auth_response(&state.config, StatusCode::OK, resp).into_response())
}

async fn idempotent_verify_response(
    state: &AppState,
    headers: &HeaderMap,
    email: &str,
) -> crate::error::Result<Option<WalletAuthResponse>> {
    let Some(token) = token_from_headers(headers, &state.config.session_cookie_name) else {
        return Ok(None);
    };
    let Ok(session_id) = uuid::Uuid::parse_str(&token) else {
        return Ok(None);
    };
    let claims = match claims_from_session(state, session_id).await {
        Ok(claims) => claims,
        Err(crate::error::AppError::Unauthorized(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    if !claims.email.eq_ignore_ascii_case(email.trim()) {
        return Ok(None);
    }

    let session = if state.config.circle_mock {
        let p = MockProvider;
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.session(claims.sub).await?
    } else {
        let p = CircleProvider::new(&state.http, &state.config);
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.session(claims.sub).await?
    };
    let status = if session.wallet.is_some() {
        "active"
    } else {
        "provisioning"
    };

    Ok(Some(WalletAuthResponse {
        session_token: token,
        status: status.into(),
        user: session.user,
        wallet: session.wallet,
        is_new_user: false,
    }))
}

pub async fn session(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> crate::error::Result<Json<WalletSessionResponse>> {
    let resp = if state.config.circle_mock {
        let p = MockProvider;
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.session(claims.sub).await?
    } else {
        let p = CircleProvider::new(&state.http, &state.config);
        let svc = WalletService::new(&state.db, &p, &state.config, &state.sse);
        svc.session(claims.sub).await?
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

/// Clears the session cookie.
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> crate::error::Result<axum::response::Response> {
    if let Some(token) = token_from_headers(&headers, &state.config.session_cookie_name) {
        if let Ok(session_id) = uuid::Uuid::parse_str(&token) {
            sqlx::query(
                "UPDATE auth_sessions
                 SET revoked_at = COALESCE(revoked_at, NOW())
                 WHERE id = $1
                   AND revoked_at IS NULL",
            )
            .bind(session_id)
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
    let cookie_header = headers.get(header::COOKIE).and_then(|v| v.to_str().ok())?;
    for piece in cookie_header.split(';') {
        let trimmed = piece.trim();
        if let Some(value) = trimmed.strip_prefix(&format!("{cookie_name}=")) {
            return Some(value.to_string());
        }
    }
    None
}

async fn send_auth_code_email(state: &AppState, email: &str, code: &str) -> anyhow::Result<()> {
    let payload = serde_json::json!({
        "from": state.config.digest_from,
        "to": [email],
        "subject": "Aegis verification code",
        "html": format!(
            "<p>Your Aegis code is <strong>{}</strong>.</p><p>Use it within 10 minutes to continue. If you did not request this, you can ignore this email.</p>",
            code
        ),
    });
    let resp = state
        .http
        .post("https://api.resend.com/emails")
        .bearer_auth(&state.config.resend_api_key)
        .json(&payload)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        tracing::error!(
            status = %status,
            response = %text,
            "wallet auth email provider rejected the message"
        );
        anyhow::bail!("wallet auth email provider rejected the message with {status}");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodeDeliveryMode {
    DevCode,
    Email,
    Unavailable,
}

fn code_delivery_mode(config: &Config) -> CodeDeliveryMode {
    code_delivery_mode_for(
        config.circle_mock,
        &config.public_base_url,
        &config.api_base_url,
        &config.cors_allow_origin,
        &config.resend_api_key,
        &config.digest_from,
    )
}

fn code_delivery_mode_for(
    circle_mock: bool,
    public_base_url: &str,
    api_base_url: &str,
    cors_allow_origin: &str,
    resend_api_key: &str,
    digest_from: &str,
) -> CodeDeliveryMode {
    if dev_codes_allowed_for(
        circle_mock,
        public_base_url,
        api_base_url,
        cors_allow_origin,
    ) {
        return CodeDeliveryMode::DevCode;
    }
    if wallet_auth_email_delivery_configured_for(resend_api_key, digest_from) {
        return CodeDeliveryMode::Email;
    }
    CodeDeliveryMode::Unavailable
}

fn wallet_auth_email_delivery_configured_for(resend_api_key: &str, digest_from: &str) -> bool {
    !resend_api_key.trim().is_empty()
        && sender_domain(digest_from).is_some_and(|domain| !is_local_sender_domain(&domain))
}

fn sender_domain(from: &str) -> Option<String> {
    let trimmed = from.trim().trim_matches(['"', '\'']);
    let address = trimmed
        .rsplit_once('<')
        .and_then(|(_, rest)| rest.split_once('>').map(|(email, _)| email))
        .unwrap_or(trimmed)
        .trim()
        .trim_matches(['"', '\'']);
    let (_, domain) = address.rsplit_once('@')?;
    let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    (!domain.is_empty()).then_some(domain)
}

fn is_local_sender_domain(domain: &str) -> bool {
    matches!(domain, "localhost" | "local")
        || domain.ends_with(".local")
        || domain.ends_with(".localhost")
        || domain.ends_with(".invalid")
        || domain.ends_with(".example")
        || domain.ends_with(".test")
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
    use super::CodeDeliveryMode;

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

    #[test]
    fn sender_domain_rejects_local_sender_addresses() {
        assert_eq!(
            super::sender_domain("Aegis <auth@aegis.local>").as_deref(),
            Some("aegis.local"),
        );
        assert!(super::is_local_sender_domain("aegis.local"));
        assert!(super::is_local_sender_domain("mail.localhost"));
        assert!(!super::is_local_sender_domain("aegis.finance"));
    }

    #[test]
    fn delivery_mode_prefers_dev_code_only_for_local_mock_runs() {
        assert_eq!(
            super::code_delivery_mode_for(
                true,
                "http://localhost:3000",
                "http://localhost:8080",
                "http://localhost:3000",
                "",
                "Aegis <auth@aegis.local>",
            ),
            CodeDeliveryMode::DevCode
        );
        assert_eq!(
            super::code_delivery_mode_for(
                false,
                "http://localhost:3000",
                "http://localhost:8080",
                "http://localhost:3000",
                "resend-key",
                "Aegis <auth@aegis.finance>",
            ),
            CodeDeliveryMode::Email
        );
        assert_eq!(
            super::code_delivery_mode_for(
                false,
                "http://localhost:3000",
                "http://localhost:8080",
                "http://localhost:3000",
                "",
                "Aegis <auth@aegis.local>",
            ),
            CodeDeliveryMode::Unavailable
        );
    }
}
