use axum::{
    extract::{Request, State},
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
    pub jti: Uuid,
    /// Set once the user has a Circle Wallet (Sprint 2+).
    #[serde(default)]
    pub wallet_id: Option<String>,
    pub exp: usize,
    pub iat: usize,
}

/// Auth middleware. Looks for the JWT in this order:
///
/// 1. `Authorization: Bearer …` header (API clients, fetch).
/// 2. HttpOnly cookie named per `Config::session_cookie_name` (default
///    `aegis_jwt`) — set by the wallet handlers on successful login.
///
/// First match wins. Missing token → 401.
pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = extract_token(&req, &state.config.session_cookie_name)
        .ok_or_else(|| AppError::Unauthorized("missing token".into()))?;

    let claims = decode_claims(&state, &token)?;
    ensure_session_active(&state, &claims).await?;

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

pub fn decode_claims(state: &AppState, token: &str) -> Result<Claims, AppError> {
    Ok(decode::<Claims>(
        token,
        &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| AppError::Unauthorized(e.to_string()))?
    .claims)
}

async fn ensure_session_active(state: &AppState, claims: &Claims) -> Result<(), AppError> {
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1
            FROM auth_sessions
            WHERE id = $1
              AND user_id = $2
              AND revoked_at IS NULL
              AND expires_at > NOW()
        )",
    )
    .bind(claims.jti)
    .bind(claims.sub)
    .fetch_one(&state.db)
    .await?;

    if !active {
        return Err(AppError::Unauthorized("session expired or revoked".into()));
    }

    sqlx::query("UPDATE auth_sessions SET last_seen_at = NOW() WHERE id = $1")
        .bind(claims.jti)
        .execute(&state.db)
        .await?;
    Ok(())
}

pub fn extract_token(req: &Request, cookie_name: &str) -> Option<String> {
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
