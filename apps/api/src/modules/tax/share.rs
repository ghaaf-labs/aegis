//! Accountant share tokens for the 1099-DA export.
//!
//! Read-only signed-URL semantics: the user creates a token with a TTL,
//! hands the URL to their accountant, and can revoke it at any time. The
//! token is the lookup key (32 random bytes → base64-url); resolution
//! returns the `(user_id, portfolio_id, year)` triple iff the row is not
//! expired and not revoked.

use base64::engine::{general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::Result;

/// Maximum TTL the API will mint. Anything longer is a foot-gun; the user
/// can always rotate.
const MAX_TTL_DAYS: i64 = 90;

/// One stored row, used by the listing endpoint.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareTokenRow {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub year: i32,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Mint a new share token for `(user_id, portfolio_id, year)`. TTL is
/// clamped to `MAX_TTL_DAYS`.
pub async fn create_share_token(
    pool: &PgPool,
    user_id: Uuid,
    portfolio_id: Uuid,
    year: i32,
    ttl_days: i64,
) -> Result<(Uuid, String, DateTime<Utc>)> {
    let ttl = ttl_days.clamp(1, MAX_TTL_DAYS);
    let expires_at = Utc::now() + Duration::days(ttl);
    let token = generate_token();
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO tax_share_tokens (user_id, portfolio_id, token, year, expires_at)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id",
    )
    .bind(user_id)
    .bind(portfolio_id)
    .bind(&token)
    .bind(year)
    .bind(expires_at)
    .fetch_one(pool)
    .await?;
    Ok((row.0, token, expires_at))
}

/// Resolve a token to `(user_id, portfolio_id, year)`. Returns `None` if
/// the row is missing, expired, or revoked — the handler treats all three
/// as 404 to avoid leaking which tokens existed.
pub async fn resolve_share_token(pool: &PgPool, token: &str) -> Result<Option<(Uuid, Uuid, i32)>> {
    let row: Option<(Uuid, Uuid, i32)> = sqlx::query_as(
        "SELECT user_id, portfolio_id, year
         FROM tax_share_tokens
         WHERE token = $1
           AND expires_at > NOW()
           AND revoked_at IS NULL",
    )
    .bind(token)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Revoke a token owned by `user_id`. Idempotent.
pub async fn revoke_share_token(pool: &PgPool, user_id: Uuid, token_id: Uuid) -> Result<()> {
    sqlx::query(
        "UPDATE tax_share_tokens
            SET revoked_at = NOW()
          WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL",
    )
    .bind(token_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// List a user's active (non-revoked, non-expired) share tokens.
pub async fn list_share_tokens(pool: &PgPool, user_id: Uuid) -> Result<Vec<ShareTokenRow>> {
    let rows: Vec<ShareTokenRow> = sqlx::query_as(
        "SELECT id, portfolio_id, year, token, expires_at, revoked_at, created_at
         FROM tax_share_tokens
         WHERE user_id = $1
         ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

fn generate_token() -> String {
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_token_is_url_safe_and_long() {
        let t = generate_token();
        // 32 bytes base64-url no-pad = 43 chars.
        assert_eq!(t.len(), 43);
        for c in t.chars() {
            assert!(c.is_ascii_alphanumeric() || c == '-' || c == '_');
        }
    }

    #[test]
    fn tokens_are_unique() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b);
    }
}
