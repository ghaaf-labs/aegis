use chrono::Utc;
use hmac::{Hmac, Mac};
use rand::Rng;
use serde_json::json;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use super::{AuthCodeCheck, VerifiedAuthCode, WalletAuthCodeIssue, WalletService};
use crate::db::Db;
use crate::error::AppError;
use crate::modules::wallet::models::WalletAuthCodeResponse;

const AUTH_CODE_EXPIRY_MINUTES: i64 = 10;
pub(super) const AUTH_CODE_RESEND_COOLDOWN_SECONDS: u64 = 30;
const AUTH_IP_RATE_WINDOW_SECONDS: i64 = 10 * 60;
const AUTH_IP_RATE_LIMIT: i32 = 20;

pub async fn enforce_auth_ip_rate_limit(
    db: &Db,
    action: &str,
    client_key: &str,
) -> crate::error::Result<()> {
    use sqlx::Row;

    let now = Utc::now();
    let reset_at = now + chrono::Duration::seconds(AUTH_IP_RATE_WINDOW_SECONDS);
    let bucket = auth_rate_bucket(action, client_key);
    let row = sqlx::query(
        "INSERT INTO auth_rate_limits (id, hits, reset_at, updated_at)
         VALUES ($1, 1, $2, $3)
         ON CONFLICT (id) DO UPDATE
           SET hits = CASE
                 WHEN auth_rate_limits.reset_at <= $3 THEN 1
                 ELSE auth_rate_limits.hits + 1
               END,
               reset_at = CASE
                 WHEN auth_rate_limits.reset_at <= $3 THEN $2
                 ELSE auth_rate_limits.reset_at
               END,
               updated_at = $3
         RETURNING hits, reset_at",
    )
    .bind(bucket)
    .bind(reset_at)
    .bind(now)
    .fetch_one(db)
    .await?;

    let hits: i32 = row.try_get("hits")?;
    if hits > AUTH_IP_RATE_LIMIT {
        let reset_at: chrono::DateTime<Utc> = row.try_get("reset_at")?;
        let retry_after = (reset_at - now).num_seconds().max(1);
        return Err(AppError::TooManyRequests(format!(
            "rate_limited:{retry_after}"
        )));
    }

    Ok(())
}

impl<'a> WalletService<'a> {
    pub async fn request_auth_code(
        &self,
        email: &str,
        referrer_handle: Option<&str>,
    ) -> crate::error::Result<WalletAuthCodeIssue> {
        let normalized = super::consent::normalize_email(email);
        super::consent::validate_email(&normalized)?;
        let challenge_id = Uuid::new_v4();
        let code = format!("{:06}", rand::thread_rng().gen_range(0..1_000_000));
        let code_hash = code_hash(self.config, &normalized, challenge_id, &code);
        let expires_at = Utc::now() + chrono::Duration::minutes(AUTH_CODE_EXPIRY_MINUTES);
        let referrer_handle = referrer_handle
            .map(str::trim)
            .filter(|h| !h.is_empty())
            .map(|h| h.to_ascii_lowercase());
        let has_referrer = referrer_handle.is_some();

        let recent_10m: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM wallet_auth_codes
             WHERE email = $1
               AND created_at > NOW() - INTERVAL '10 minutes'",
        )
        .bind(&normalized)
        .fetch_one(self.db)
        .await?;
        if recent_10m >= 3 {
            return Err(AppError::TooManyRequests("rate_limited".into()));
        }

        let recent_hour: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM wallet_auth_codes
             WHERE email = $1
               AND created_at > NOW() - INTERVAL '1 hour'",
        )
        .bind(&normalized)
        .fetch_one(self.db)
        .await?;
        if recent_hour >= 10 {
            return Err(AppError::TooManyRequests("rate_limited".into()));
        }

        let mut tx = self.db.begin().await?;
        sqlx::query(
            "UPDATE wallet_auth_codes
             SET consumed_at = NOW()
             WHERE email = $1
               AND consumed_at IS NULL",
        )
        .bind(&normalized)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO wallet_auth_codes
                (id, email, code_hash, referrer_handle, expires_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(challenge_id)
        .bind(&normalized)
        .bind(&code_hash)
        .bind(referrer_handle)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        crate::modules::analytics::service::emit(
            self.db,
            None,
            "auth.code_requested",
            json!({
                "hasReferrer": has_referrer,
            }),
        )
        .await;

        Ok(WalletAuthCodeIssue {
            response: WalletAuthCodeResponse {
                challenge_id,
                email: normalized,
                expires_at,
                resend_in_seconds: AUTH_CODE_RESEND_COOLDOWN_SECONDS,
                dev_code: None,
            },
            code,
        })
    }

    pub async fn resend_auth_code(
        &self,
        challenge_id: Uuid,
    ) -> crate::error::Result<WalletAuthCodeIssue> {
        use sqlx::Row;

        let row = sqlx::query(
            "SELECT email, referrer_handle, created_at
             FROM wallet_auth_codes
             WHERE id = $1
               AND consumed_at IS NULL
               AND expires_at > NOW()",
        )
        .bind(challenge_id)
        .fetch_optional(self.db)
        .await?
        .ok_or_else(|| AppError::BadRequest("code_expired".into()))?;

        let email: String = row.try_get("email")?;
        super::consent::validate_email(&email)?;
        let created_at: chrono::DateTime<Utc> = row.try_get("created_at")?;
        let elapsed = (Utc::now() - created_at).num_seconds().max(0) as u64;
        if elapsed < AUTH_CODE_RESEND_COOLDOWN_SECONDS {
            return Err(AppError::TooManyRequests(format!(
                "resend_cooldown:{}",
                AUTH_CODE_RESEND_COOLDOWN_SECONDS - elapsed
            )));
        }

        let code = format!("{:06}", rand::thread_rng().gen_range(0..1_000_000));
        let code_hash = code_hash(self.config, &email, challenge_id, &code);
        let expires_at = Utc::now() + chrono::Duration::minutes(AUTH_CODE_EXPIRY_MINUTES);
        let referrer_handle: Option<String> = row.try_get("referrer_handle")?;

        sqlx::query(
            "UPDATE wallet_auth_codes
             SET code_hash = $2,
                 referrer_handle = $3,
                 attempts = 0,
                 expires_at = $4,
                 created_at = NOW()
             WHERE id = $1
               AND consumed_at IS NULL",
        )
        .bind(challenge_id)
        .bind(&code_hash)
        .bind(referrer_handle)
        .bind(expires_at)
        .execute(self.db)
        .await?;

        Ok(WalletAuthCodeIssue {
            response: WalletAuthCodeResponse {
                challenge_id,
                email,
                expires_at,
                resend_in_seconds: AUTH_CODE_RESEND_COOLDOWN_SECONDS,
                dev_code: None,
            },
            code,
        })
    }

    pub async fn verify_auth_code(
        &self,
        challenge_id: Uuid,
        code: &str,
        consent: Option<&crate::modules::wallet::models::EmailAuthConsent>,
    ) -> crate::error::Result<VerifiedAuthCode> {
        let checked = self.check_auth_code(challenge_id, code).await?;
        let existing = self.find_user_by_email(&checked.email).await?;
        let needs_consent = match existing.as_ref() {
            Some(user) => self.user_needs_current_consent(user.id).await?,
            None => true,
        };

        if needs_consent && !super::consent::has_required_consent(consent) {
            return Err(AppError::BadRequest("consent_required".into()));
        }

        self.consume_auth_code(challenge_id, &checked.code_hash)
            .await?;
        crate::modules::analytics::service::emit(self.db, None, "auth.code_verified", json!({}))
            .await;
        Ok(VerifiedAuthCode {
            email: checked.email,
            referrer_handle: checked.referrer_handle,
        })
    }

    pub async fn verify_auth_code_for_email_update(
        &self,
        current_user_id: Uuid,
        challenge_id: Uuid,
        code: &str,
    ) -> crate::error::Result<VerifiedAuthCode> {
        let checked = self.check_auth_code(challenge_id, code).await?;
        if let Some(owner) = self.find_user_by_email(&checked.email).await? {
            if owner.id != current_user_id {
                return Err(AppError::Conflict("email_in_use".into()));
            }
        }

        self.consume_auth_code(challenge_id, &checked.code_hash)
            .await?;
        crate::modules::analytics::service::emit(
            self.db,
            Some(current_user_id),
            "account.email_verified",
            json!({}),
        )
        .await;
        Ok(VerifiedAuthCode {
            email: checked.email,
            referrer_handle: checked.referrer_handle,
        })
    }

    pub async fn auth_code_email(
        &self,
        challenge_id: Uuid,
    ) -> crate::error::Result<Option<String>> {
        let email = sqlx::query_scalar::<_, String>(
            "SELECT email
             FROM wallet_auth_codes
             WHERE id = $1",
        )
        .bind(challenge_id)
        .fetch_optional(self.db)
        .await?;

        Ok(email)
    }

    pub(super) async fn check_auth_code(
        &self,
        challenge_id: Uuid,
        code: &str,
    ) -> crate::error::Result<AuthCodeCheck> {
        use sqlx::Row;

        let trimmed_code = code.trim();
        if trimmed_code.len() != 6 || !trimmed_code.chars().all(|c| c.is_ascii_digit()) {
            return Err(AppError::BadRequest("code_invalid".into()));
        }

        let row = sqlx::query(
            "SELECT email, code_hash, attempts, expires_at, referrer_handle, consumed_at
             FROM wallet_auth_codes
             WHERE id = $1",
        )
        .bind(challenge_id)
        .fetch_optional(self.db)
        .await?;

        let Some(row) = row else {
            return Err(AppError::BadRequest("code_invalid".into()));
        };

        let email = super::consent::normalize_email(&row.try_get::<String, _>("email")?);
        super::consent::validate_email(&email)?;

        let attempts: i32 = row.try_get("attempts")?;
        let consumed_at: Option<chrono::DateTime<Utc>> = row.try_get("consumed_at")?;
        let expires_at: chrono::DateTime<Utc> = row.try_get("expires_at")?;
        if let Some(error) = auth_code_state_error(attempts, consumed_at.is_some(), expires_at) {
            return Err(error);
        }

        let actual_hash: String = row.try_get("code_hash")?;
        let expected_hash = code_hash(self.config, &email, challenge_id, trimmed_code);
        if actual_hash
            .as_bytes()
            .ct_eq(expected_hash.as_bytes())
            .unwrap_u8()
            != 1
        {
            let failed_attempts = attempts + 1;
            sqlx::query(
                "UPDATE wallet_auth_codes
                 SET attempts = attempts + 1,
                     consumed_at = CASE
                         WHEN attempts + 1 >= 3 THEN NOW()
                         ELSE consumed_at
                     END
                 WHERE id = $1",
            )
            .bind(challenge_id)
            .execute(self.db)
            .await?;
            if failed_attempts >= 3 {
                return Err(AppError::TooManyRequests("too_many_attempts".into()));
            }
            return Err(AppError::BadRequest("code_invalid".into()));
        }

        Ok(AuthCodeCheck {
            email,
            code_hash: expected_hash,
            referrer_handle: row.try_get("referrer_handle")?,
        })
    }

    pub(super) async fn consume_auth_code(
        &self,
        challenge_id: Uuid,
        code_hash: &str,
    ) -> crate::error::Result<()> {
        let consumed = sqlx::query(
            "UPDATE wallet_auth_codes
             SET consumed_at = NOW()
             WHERE id = $1
               AND code_hash = $2
               AND consumed_at IS NULL",
        )
        .bind(challenge_id)
        .bind(code_hash)
        .execute(self.db)
        .await?;
        if consumed.rows_affected() != 1 {
            return Err(AppError::BadRequest("code_used".into()));
        }

        Ok(())
    }
}

fn auth_rate_bucket(action: &str, client_key: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(action.as_bytes());
    hash.update(b":");
    hash.update(client_key.as_bytes());
    format!("auth:{action}:{}", hex::encode(hash.finalize()))
}

fn auth_code_state_error(
    attempts: i32,
    consumed: bool,
    expires_at: chrono::DateTime<Utc>,
) -> Option<AppError> {
    if attempts >= 3 {
        return Some(AppError::TooManyRequests("too_many_attempts".into()));
    }
    if consumed {
        return Some(AppError::BadRequest("code_used".into()));
    }
    if expires_at <= Utc::now() {
        return Some(AppError::BadRequest("code_expired".into()));
    }
    None
}

fn code_hash(cfg: &crate::config::Config, email: &str, challenge_id: Uuid, code: &str) -> String {
    type HmacSha = Hmac<Sha256>;
    let mut mac =
        HmacSha::new_from_slice(cfg.jwt_secret.as_bytes()).expect("HMAC accepts any key size");
    mac.update(email.as_bytes());
    mac.update(challenge_id.as_bytes());
    mac.update(code.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::wallet::models::WalletAuthCodeResponse;

    #[test]
    fn auth_code_response_serializes_resend_cooldown() {
        let response = WalletAuthCodeResponse {
            challenge_id: uuid::Uuid::new_v4(),
            email: "user@example.com".into(),
            expires_at: Utc::now(),
            resend_in_seconds: AUTH_CODE_RESEND_COOLDOWN_SECONDS,
            dev_code: None,
        };

        let json = serde_json::to_value(response).unwrap();
        assert_eq!(json["resendInSeconds"], AUTH_CODE_RESEND_COOLDOWN_SECONDS);
    }

    #[test]
    fn auth_rate_bucket_does_not_store_raw_client_key() {
        let bucket = auth_rate_bucket("start", "203.0.113.10");
        assert!(bucket.starts_with("auth:start:"));
        assert!(!bucket.contains("203.0.113.10"));
    }

    #[test]
    fn exhausted_auth_code_reports_attempt_limit_before_used() {
        let err = auth_code_state_error(3, true, Utc::now() + chrono::Duration::minutes(5))
            .expect("attempt limit should reject");

        assert!(matches!(
            err,
            AppError::TooManyRequests(message) if message == "too_many_attempts"
        ));
    }

    #[test]
    fn consumed_auth_code_reports_used_when_not_attempt_exhausted() {
        let err = auth_code_state_error(1, true, Utc::now() + chrono::Duration::minutes(5))
            .expect("consumed code should reject");

        assert!(matches!(
            err,
            AppError::BadRequest(message) if message == "code_used"
        ));
    }

    #[test]
    fn expired_auth_code_reports_expired_when_live_and_not_exhausted() {
        let err = auth_code_state_error(0, false, Utc::now() - chrono::Duration::seconds(1))
            .expect("expired code should reject");

        assert!(matches!(
            err,
            AppError::BadRequest(message) if message == "code_expired"
        ));
    }
}
