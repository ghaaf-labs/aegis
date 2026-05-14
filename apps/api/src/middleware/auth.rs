use axum::{
    extract::{Query, Request, State},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::AppError, router::AppState};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid,
    pub email: String,
    /// Set once the user has a Circle Wallet (Sprint 2+).
    #[serde(default)]
    pub wallet_id: Option<String>,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Debug, Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

/// Auth middleware. Looks for the JWT in this order:
///
/// 1. `Authorization: Bearer …` header (API clients, fetch).
/// 2. HttpOnly cookie named per `Config::session_cookie_name` (default
///    `aegis_jwt`) — set by the wallet handlers on successful login.
/// 3. `?token=` query string — only used by `EventSource`, which can't send
///    custom headers and doesn't always have credentials cookies attached.
///
/// First match wins. Missing token → 401.
pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = extract_token(&req, &state.config.session_cookie_name)
        .or_else(|| {
            // Read `?token=` from query string. We can't use the Axum
            // `Query` extractor mid-middleware because the request body has
            // already been consumed by other extractors downstream; parse
            // the URI directly.
            req.uri().query().and_then(|q| {
                serde_urlencoded::from_str::<TokenQuery>(q)
                    .ok()
                    .and_then(|t| t.token)
            })
        })
        .ok_or_else(|| AppError::Unauthorized("missing token".into()))?;
    // Silence the unused-trait import — `Query` is brought in for callers
    // that want to extract it; this middleware reads the URI directly.
    let _ = std::marker::PhantomData::<Query<TokenQuery>>;

    let claims = decode::<Claims>(
        &token,
        &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| AppError::Unauthorized(e.to_string()))?
    .claims;

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

fn extract_token(req: &Request, cookie_name: &str) -> Option<String> {
    if let Some(bearer) = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        return Some(bearer.to_string());
    }
    let cookie_header = req.headers().get("Cookie").and_then(|v| v.to_str().ok())?;
    for piece in cookie_header.split(';') {
        let trimmed = piece.trim();
        if let Some(value) = trimmed.strip_prefix(&format!("{cookie_name}=")) {
            return Some(value.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;

    fn req_with(headers: &[(&str, &str)]) -> Request {
        let mut b = HttpRequest::builder().uri("/x");
        for (k, v) in headers {
            b = b.header(*k, *v);
        }
        b.body(Body::empty()).unwrap()
    }

    #[test]
    fn extract_token_prefers_authorization_header() {
        let r = req_with(&[
            ("Authorization", "Bearer abc.def"),
            ("Cookie", "aegis_jwt=xxx"),
        ]);
        assert_eq!(extract_token(&r, "aegis_jwt"), Some("abc.def".into()));
    }

    #[test]
    fn extract_token_falls_back_to_cookie() {
        let r = req_with(&[("Cookie", "other=foo; aegis_jwt=eyJabc; trailing=bar")]);
        assert_eq!(extract_token(&r, "aegis_jwt"), Some("eyJabc".into()));
    }

    #[test]
    fn extract_token_missing_returns_none() {
        let r = req_with(&[]);
        assert!(extract_token(&r, "aegis_jwt").is_none());
    }
}
