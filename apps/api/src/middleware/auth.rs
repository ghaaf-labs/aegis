use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{error::AppError, router::AppState};

#[derive(Debug, Clone)]
pub struct Claims {
    pub sub: Uuid,
    pub email: String,
    pub jti: Uuid,
    pub exp: usize,
    pub iat: usize,
}

/// Auth middleware. Looks for the opaque session id only in the HttpOnly cookie
/// named per `Config::session_cookie_name`.
///
/// The cookie value is not a portable JWT. It is only a random session id backed
/// by `auth_sessions`, so logout/re-login can revoke it immediately.
pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = extract_session_token(&req, &state.config.session_cookie_name)
        .ok_or_else(|| AppError::Unauthorized("missing session".into()))?;
    let session_id = parse_session_id(&token)?;

    let claims = claims_from_session(&state, session_id).await?;

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

pub async fn claims_from_session(state: &AppState, session_id: Uuid) -> Result<Claims, AppError> {
    let idle_cutoff =
        Utc::now() - chrono::Duration::minutes(state.config.session_idle_timeout_minutes as i64);

    sqlx::query(
        "UPDATE auth_sessions
         SET revoked_at = COALESCE(revoked_at, NOW())
         WHERE id = $1
           AND revoked_at IS NULL
           AND (expires_at <= NOW() OR last_seen_at <= $2)",
    )
    .bind(session_id)
    .bind(idle_cutoff)
    .execute(&state.db)
    .await?;

    let row = sqlx::query_as::<_, SessionClaimsRow>(
        "SELECT s.id AS session_id,
                s.user_id,
                s.expires_at,
                s.created_at,
                u.email
         FROM auth_sessions s
         JOIN users u ON u.id = s.user_id
         WHERE s.id = $1
           AND s.revoked_at IS NULL
           AND s.expires_at > NOW()
           AND s.last_seen_at > $2
           AND u.deletion_requested_at IS NULL
           AND u.anonymized_at IS NULL",
    )
    .bind(session_id)
    .bind(idle_cutoff)
    .fetch_optional(&state.db)
    .await?;

    let Some(row) = row else {
        return Err(AppError::Unauthorized("session expired or revoked".into()));
    };

    sqlx::query("UPDATE auth_sessions SET last_seen_at = NOW() WHERE id = $1")
        .bind(row.session_id)
        .execute(&state.db)
        .await?;

    Ok(Claims {
        sub: row.user_id,
        email: row.email,
        jti: row.session_id,
        exp: row.expires_at.timestamp().max(0) as usize,
        iat: row.created_at.timestamp().max(0) as usize,
    })
}

#[derive(sqlx::FromRow)]
struct SessionClaimsRow {
    session_id: Uuid,
    user_id: Uuid,
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
    email: String,
}

fn parse_session_id(token: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(token).map_err(|_| AppError::Unauthorized("invalid session".into()))
}

pub fn extract_session_token(req: &Request, cookie_name: &str) -> Option<String> {
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
    fn extract_session_token_ignores_authorization_header() {
        let id = Uuid::new_v4().to_string();
        let r = req_with(&[("Authorization", &format!("Bearer {id}"))]);
        assert_eq!(extract_session_token(&r, "aegis_session"), None);
    }

    #[test]
    fn extract_session_token_reads_cookie() {
        let id = Uuid::new_v4().to_string();
        let cookie = format!("other=foo; aegis_session={id}; trailing=bar");
        let r = req_with(&[("Cookie", &cookie)]);
        assert_eq!(extract_session_token(&r, "aegis_session"), Some(id));
    }

    #[test]
    fn extract_session_token_missing_returns_none() {
        let r = req_with(&[]);
        assert!(extract_session_token(&r, "aegis_session").is_none());
    }

    #[test]
    fn parse_session_id_rejects_non_uuid_tokens() {
        assert!(parse_session_id("not.jwt.or.uuid").is_err());
    }
}
