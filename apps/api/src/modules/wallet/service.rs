//! Wallet service — orchestrates Circle W3S User-Controlled provider +
//! persistence + JWT minting.

use chrono::{TimeZone, Utc};
use hmac::{Hmac, Mac};
use jsonwebtoken::{encode, EncodingKey, Header};
use rand::Rng;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use super::models::{
    WalletAuthCodeResponse, WalletAuthIntent, WalletAuthResponse, WalletInfo, WalletStatusResponse,
    WalletUser, WalletUserPublic,
};
use super::provider::WalletProvider;
use crate::config::Config;
use crate::db::Db;
use crate::error::AppError;
use crate::middleware::auth::Claims;
use crate::modules::sse::{SseEvent, SseSender};

pub struct WalletService<'a> {
    pub db: &'a Db,
    pub provider: &'a dyn WalletProvider,
    pub config: &'a Config,
    pub sse: &'a SseSender,
}

pub struct WalletAuthCodeIssue {
    pub response: WalletAuthCodeResponse,
    pub code: String,
}

impl<'a> WalletService<'a> {
    pub fn new(
        db: &'a Db,
        provider: &'a dyn WalletProvider,
        config: &'a Config,
        sse: &'a SseSender,
    ) -> Self {
        Self {
            db,
            provider,
            config,
            sse,
        }
    }

    pub async fn request_auth_code(
        &self,
        email: &str,
        intent: WalletAuthIntent,
        referrer_handle: Option<&str>,
    ) -> crate::error::Result<WalletAuthCodeIssue> {
        let normalized = normalize_email(email);
        validate_email(&normalized)?;
        let challenge_id = Uuid::new_v4();
        let code = format!("{:06}", rand::thread_rng().gen_range(0..1_000_000));
        let code_hash = code_hash(self.config, &normalized, challenge_id, &code);
        let expires_at = Utc::now() + chrono::Duration::minutes(10);
        let referrer_handle = referrer_handle
            .map(str::trim)
            .filter(|h| !h.is_empty())
            .map(|h| h.to_ascii_lowercase());

        self.validate_auth_intent(&normalized, intent).await?;

        let recent_10m: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM wallet_auth_codes
             WHERE email = $1
               AND intent = $2
               AND created_at > NOW() - INTERVAL '10 minutes'",
        )
        .bind(&normalized)
        .bind(intent.as_str())
        .fetch_one(self.db)
        .await?;
        if recent_10m >= 3 {
            return Err(AppError::TooManyRequests(
                "too many verification code requests".into(),
            ));
        }

        let recent_hour: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM wallet_auth_codes
             WHERE email = $1
               AND intent = $2
               AND created_at > NOW() - INTERVAL '1 hour'",
        )
        .bind(&normalized)
        .bind(intent.as_str())
        .fetch_one(self.db)
        .await?;
        if recent_hour >= 10 {
            return Err(AppError::TooManyRequests(
                "too many verification code requests".into(),
            ));
        }

        let mut tx = self.db.begin().await?;
        sqlx::query(
            "UPDATE wallet_auth_codes
             SET consumed_at = NOW()
             WHERE email = $1
               AND intent = $2
               AND consumed_at IS NULL",
        )
        .bind(&normalized)
        .bind(intent.as_str())
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO wallet_auth_codes
                (id, email, intent, code_hash, referrer_handle, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(challenge_id)
        .bind(&normalized)
        .bind(intent.as_str())
        .bind(&code_hash)
        .bind(referrer_handle)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(WalletAuthCodeIssue {
            response: WalletAuthCodeResponse {
                challenge_id,
                email: normalized,
                expires_at,
                dev_code: None,
            },
            code,
        })
    }

    async fn validate_auth_intent(
        &self,
        email: &str,
        intent: WalletAuthIntent,
    ) -> crate::error::Result<()> {
        let user = self.find_user_by_email(email).await?;
        validate_auth_intent_for_user(intent, user.as_ref(), self.config.circle_mock)
    }

    pub async fn verify_auth_code(
        &self,
        email: &str,
        challenge_id: Uuid,
        code: &str,
        intent: WalletAuthIntent,
    ) -> crate::error::Result<Option<String>> {
        use sqlx::Row;

        let normalized = normalize_email(email);
        validate_email(&normalized)?;
        let trimmed_code = code.trim();
        if trimmed_code.len() != 6 || !trimmed_code.chars().all(|c| c.is_ascii_digit()) {
            return Err(AppError::Unauthorized("invalid verification code".into()));
        }

        let row = sqlx::query(
            "SELECT code_hash, attempts, expires_at, referrer_handle
             FROM wallet_auth_codes
             WHERE id = $1
               AND email = $2
               AND intent = $3
               AND consumed_at IS NULL",
        )
        .bind(challenge_id)
        .bind(&normalized)
        .bind(intent.as_str())
        .fetch_optional(self.db)
        .await?;

        let Some(row) = row else {
            return Err(AppError::Unauthorized("verification code not found".into()));
        };

        let attempts: i32 = row.try_get("attempts")?;
        if attempts >= 5 {
            return Err(AppError::Unauthorized(
                "too many verification attempts".into(),
            ));
        }
        let expires_at: chrono::DateTime<Utc> = row.try_get("expires_at")?;
        if expires_at <= Utc::now() {
            return Err(AppError::Unauthorized("verification code expired".into()));
        }

        let actual_hash: String = row.try_get("code_hash")?;
        let expected_hash = code_hash(self.config, &normalized, challenge_id, trimmed_code);
        if actual_hash
            .as_bytes()
            .ct_eq(expected_hash.as_bytes())
            .unwrap_u8()
            != 1
        {
            sqlx::query("UPDATE wallet_auth_codes SET attempts = attempts + 1 WHERE id = $1")
                .bind(challenge_id)
                .execute(self.db)
                .await?;
            return Err(AppError::Unauthorized("invalid verification code".into()));
        }

        let consumed = sqlx::query(
            "UPDATE wallet_auth_codes
             SET consumed_at = NOW()
             WHERE id = $1 AND consumed_at IS NULL",
        )
        .bind(challenge_id)
        .execute(self.db)
        .await?;
        if consumed.rows_affected() != 1 {
            return Err(AppError::Unauthorized(
                "verification code already used".into(),
            ));
        }

        Ok(row.try_get("referrer_handle")?)
    }

    /// New-user signup. A user row that exists but has no wallet (W3S
    /// ceremony was aborted before Circle returned addresses) is treated as a
    /// fresh signup so the browser SDK gets a real challenge_id and the user
    /// can recover instead of polling forever. Completed wallets must use
    /// login; signup should not silently become a session restore.
    pub async fn init_signup(&self, email: &str) -> crate::error::Result<WalletAuthResponse> {
        let email = normalize_email(email);
        validate_email(&email)?;

        let (user, was_inserted) = self.upsert_user_record(&email).await?;
        let has_wallet = user_has_wallet(&user, self.config.circle_mock);
        if !was_inserted && has_wallet {
            return Err(AppError::Conflict(
                "wallet already exists for this email; sign in instead".into(),
            ));
        }
        let needs_challenge = was_inserted || !has_wallet;
        self.provider.ensure_user(user.id).await?;
        let bundle = self
            .provider
            .issue_user_token(user.id, needs_challenge)
            .await?;
        let token = mint_token(&user, self.config, self.db).await?;
        let wallet = wallet_from_user(&user, self.config.circle_mock);

        Ok(WalletAuthResponse {
            token,
            user: public(&user),
            wallet,
            bundle: Some(bundle),
            is_new_user: was_inserted,
        })
    }

    /// Returning-user signin. Provisioned wallets skip the initialize
    /// challenge. Users who abandoned signup before wallet creation get a
    /// fresh challenge here so login can recover the account instead of
    /// polling forever.
    pub async fn init_login(&self, email: &str) -> crate::error::Result<WalletAuthResponse> {
        let email = normalize_email(email);
        validate_email(&email)?;
        let user = self
            .find_user_by_email(&email)
            .await?
            .ok_or_else(|| AppError::Unauthorized("no account for this email".into()))?;
        let has_wallet = user_has_wallet(&user, self.config.circle_mock);
        let bundle = if has_wallet {
            None
        } else {
            self.provider.ensure_user(user.id).await?;
            Some(self.provider.issue_user_token(user.id, true).await?)
        };
        let token = mint_token(&user, self.config, self.db).await?;
        let wallet = wallet_from_user(&user, self.config.circle_mock);

        Ok(WalletAuthResponse {
            token,
            user: public(&user),
            wallet,
            bundle,
            is_new_user: false,
        })
    }

    /// Polled by the browser after the SDK completes its PIN ceremony. Fetches
    /// the wallet from Circle and writes it back to the users row the first
    /// time it appears. Returns the wallet info (or `None` while pending).
    pub async fn fetch_wallet_status(
        &self,
        user_id: Uuid,
    ) -> crate::error::Result<WalletStatusResponse> {
        if let Some(existing) = self.find_user_by_id(user_id).await? {
            if let Some(w) = wallet_from_user(&existing, self.config.circle_mock) {
                return Ok(WalletStatusResponse { wallet: Some(w) });
            }
        }
        let Some(info) = self.provider.fetch_user_wallets(user_id).await? else {
            return Ok(WalletStatusResponse { wallet: None });
        };
        // Only the writer that actually persisted the wallet emits SSE —
        // concurrent status polls used to fire `wallet.created` twice,
        // causing a double redirect in the frontend.
        let just_persisted = self.persist_wallet(user_id, &info).await?;
        if just_persisted {
            let _ = self
                .sse
                .send(SseEvent::WalletCreated(super::sse::WalletCreatedPayload {
                    user_id,
                    wallet_id: info.wallet_id.clone(),
                    arc_address: info.arc_address.clone(),
                    base_address: info.base_address.clone(),
                    created_at: info.created_at,
                }));
        }
        Ok(WalletStatusResponse { wallet: Some(info) })
    }

    // ── DB helpers ────────────────────────────────────────────────────────

    async fn find_user_by_email(&self, email: &str) -> crate::error::Result<Option<WalletUser>> {
        let user = sqlx::query_as::<_, WalletUser>(
            "SELECT id, email, risk_tolerance, investment_horizon_months,
                    wallet_id, arc_address, base_address, created_at
             FROM users WHERE email = $1",
        )
        .bind(email)
        .fetch_optional(self.db)
        .await?;
        Ok(user)
    }

    async fn find_user_by_id(&self, id: Uuid) -> crate::error::Result<Option<WalletUser>> {
        let user = sqlx::query_as::<_, WalletUser>(
            "SELECT id, email, risk_tolerance, investment_horizon_months,
                    wallet_id, arc_address, base_address, created_at
             FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(self.db)
        .await?;
        Ok(user)
    }

    /// Create-or-find a user row by email. Returns `(user, is_new)` where
    /// `is_new` is true when this call inserted the row.
    ///
    /// Atomic upsert via `INSERT … ON CONFLICT (email) DO UPDATE`. The
    /// earlier "find then insert" version had a TOCTOU race: two concurrent
    /// signups with the same email both passed the find check and the
    /// second INSERT raised a UNIQUE violation. The `xmax = 0` trick
    /// distinguishes the INSERT row (xmax = 0) from the UPDATE-on-conflict
    /// row (xmax != 0) so we still know whether this caller's INSERT or
    /// another's won.
    async fn upsert_user_record(&self, email: &str) -> crate::error::Result<(WalletUser, bool)> {
        use sqlx::Row;
        let row = sqlx::query(
            r#"INSERT INTO users (id, email)
               VALUES ($1, $2)
               ON CONFLICT (email) DO UPDATE
                 SET email = EXCLUDED.email
               RETURNING id, email, risk_tolerance, investment_horizon_months,
                         wallet_id, arc_address, base_address, created_at,
                         (xmax = 0) AS was_inserted"#,
        )
        .bind(Uuid::new_v4())
        .bind(email)
        .fetch_one(self.db)
        .await?;
        let was_inserted: bool = row.try_get("was_inserted")?;
        let user = WalletUser {
            id: row.try_get("id")?,
            email: row.try_get("email")?,
            risk_tolerance: row.try_get("risk_tolerance")?,
            investment_horizon_months: row.try_get("investment_horizon_months")?,
            wallet_id: row.try_get("wallet_id")?,
            arc_address: row.try_get("arc_address")?,
            base_address: row.try_get("base_address")?,
            created_at: row.try_get("created_at")?,
        };
        Ok((user, was_inserted))
    }

    /// Persist Circle's wallet info. Returns `true` when this call actually
    /// wrote (so the caller can fire the `wallet.created` SSE once). Two
    /// concurrent status polls used to both fire SSE because the UPDATE
    /// is idempotent and reports rows_affected even on no-op writes. The
    /// `wallet_id IS NULL` predicate makes the second writer a no-op so
    /// only the actual provisioning caller emits the event.
    async fn persist_wallet(&self, user_id: Uuid, info: &WalletInfo) -> crate::error::Result<bool> {
        let result = sqlx::query(
            "UPDATE users
                SET wallet_id    = $2,
                    arc_address  = $3,
                    base_address = $4
              WHERE id = $1
                AND (
                  wallet_id IS NULL
                  OR wallet_id LIKE 'mock_wallet_%'
                  OR arc_address LIKE '0xARC%'
                  OR base_address LIKE '0xBASE%'
                )",
        )
        .bind(user_id)
        .bind(&info.wallet_id)
        .bind(&info.arc_address)
        .bind(&info.base_address)
        .execute(self.db)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}

fn wallet_from_user(u: &WalletUser, allow_mock_wallet: bool) -> Option<WalletInfo> {
    if !allow_mock_wallet && is_mock_wallet_fields(u) {
        return None;
    }
    Some(WalletInfo {
        wallet_id: u.wallet_id.clone()?,
        arc_address: u.arc_address.clone()?,
        base_address: u.base_address.clone()?,
        created_at: u.created_at,
    })
}

fn user_has_wallet(u: &WalletUser, allow_mock_wallet: bool) -> bool {
    wallet_from_user(u, allow_mock_wallet).is_some()
}

fn validate_auth_intent_for_user(
    intent: WalletAuthIntent,
    user: Option<&WalletUser>,
    allow_mock_wallet: bool,
) -> crate::error::Result<()> {
    match (intent, user) {
        (WalletAuthIntent::Login, None) => {
            Err(AppError::Unauthorized("no account for this email".into()))
        }
        (WalletAuthIntent::Signup, Some(user)) if user_has_wallet(user, allow_mock_wallet) => Err(
            AppError::Conflict("wallet already exists for this email; sign in instead".into()),
        ),
        _ => Ok(()),
    }
}

fn is_mock_wallet_fields(u: &WalletUser) -> bool {
    u.wallet_id
        .as_deref()
        .is_some_and(|wallet_id| wallet_id.starts_with("mock_wallet_"))
        || u.arc_address
            .as_deref()
            .is_some_and(|address| address.starts_with("0xARC"))
        || u.base_address
            .as_deref()
            .is_some_and(|address| address.starts_with("0xBASE"))
}

fn public(u: &WalletUser) -> WalletUserPublic {
    WalletUserPublic {
        id: u.id,
        email: u.email.clone(),
        risk_tolerance: u.risk_tolerance.clone(),
    }
}

async fn mint_token(user: &WalletUser, cfg: &Config, db: &Db) -> crate::error::Result<String> {
    let now = Utc::now().timestamp() as usize;
    let session_id = Uuid::new_v4();
    let exp = now + (cfg.jwt_expiry_hours as usize * 3600);
    let claims = Claims {
        sub: user.id,
        email: user.email.clone(),
        jti: session_id,
        wallet_id: user.wallet_id.clone(),
        iat: now,
        exp,
    };
    sqlx::query(
        "INSERT INTO auth_sessions (id, user_id, expires_at)
         VALUES ($1, $2, $3)",
    )
    .bind(session_id)
    .bind(user.id)
    .bind(
        Utc.timestamp_opt(exp as i64, 0)
            .single()
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("invalid session expiry")))?,
    )
    .execute(db)
    .await?;

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(cfg.jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))
}

fn validate_email(email: &str) -> crate::error::Result<()> {
    let Some((local, domain)) = email.split_once('@') else {
        return Err(AppError::BadRequest("invalid email".into()));
    };
    if local.is_empty()
        || domain.is_empty()
        || email.len() > 254
        || email.contains(char::is_whitespace)
        || domain.starts_with('.')
        || domain.ends_with('.')
        || !domain.contains('.')
        || email.matches('@').count() != 1
    {
        return Err(AppError::BadRequest("invalid email".into()));
    }
    Ok(())
}

fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

fn code_hash(cfg: &Config, email: &str, challenge_id: Uuid, code: &str) -> String {
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

    #[test]
    fn validate_email_basics() {
        assert!(validate_email("a@b.co").is_ok());
        assert!(validate_email("invalid").is_err());
        assert!(validate_email("a@b").is_err());
        assert!(validate_email("a@@b.co").is_err());
        assert!(validate_email("has space@x.com").is_err());
        assert!(validate_email(&"a".repeat(260)).is_err());
    }

    #[test]
    fn wallet_auth_response_does_not_serialize_app_jwt() {
        let user_id = Uuid::new_v4();
        let response = WalletAuthResponse {
            token: "app.jwt.must.stay.cookie-only".into(),
            user: WalletUserPublic {
                id: user_id,
                email: "user@example.com".into(),
                risk_tolerance: "moderate".into(),
            },
            wallet: None,
            bundle: Some(crate::modules::wallet::models::UserTokenBundle {
                user_token: "circle-user-token".into(),
                encryption_key: "circle-encryption-key".into(),
                app_id: "circle-app".into(),
                challenge_id: None,
            }),
            is_new_user: false,
        };

        let json = serde_json::to_value(response).unwrap();
        assert!(json.get("token").is_none());
        assert_eq!(json["user"]["id"], user_id.to_string());
        assert_eq!(json["bundle"]["userToken"], "circle-user-token");
    }

    #[test]
    fn wallet_auth_response_omits_circle_bundle_for_returning_wallet() {
        let user_id = Uuid::new_v4();
        let response = WalletAuthResponse {
            token: "app.jwt.must.stay.cookie-only".into(),
            user: WalletUserPublic {
                id: user_id,
                email: "returning@example.com".into(),
                risk_tolerance: "moderate".into(),
            },
            wallet: Some(WalletInfo {
                wallet_id: "wallet-live".into(),
                arc_address: "0x1111111111111111111111111111111111111111".into(),
                base_address: "0x2222222222222222222222222222222222222222".into(),
                created_at: Utc::now(),
            }),
            bundle: None,
            is_new_user: false,
        };

        let json = serde_json::to_value(response).unwrap();
        assert!(json.get("token").is_none());
        assert!(json.get("bundle").is_none());
        assert_eq!(json["wallet"]["walletId"], "wallet-live");
    }

    #[test]
    fn real_mode_ignores_legacy_mock_wallet_rows() {
        let user = WalletUser {
            id: Uuid::new_v4(),
            email: "legacy@example.com".into(),
            risk_tolerance: "moderate".into(),
            investment_horizon_months: 12,
            wallet_id: Some("mock_wallet_deadbeef".into()),
            arc_address: Some("0xARC0000000000000000000000000000000000000000".into()),
            base_address: Some("0xBASE0000000000000000000000000000000000000000".into()),
            created_at: Utc::now(),
        };

        assert!(wallet_from_user(&user, true).is_some());
        assert!(wallet_from_user(&user, false).is_none());
        assert!(!user_has_wallet(&user, false));
    }

    #[test]
    fn auth_code_intent_validation_rejects_wrong_entry_points() {
        let live_wallet = WalletUser {
            id: Uuid::new_v4(),
            email: "live@example.com".into(),
            risk_tolerance: "moderate".into(),
            investment_horizon_months: 12,
            wallet_id: Some("wallet-live".into()),
            arc_address: Some("0x1111111111111111111111111111111111111111".into()),
            base_address: Some("0x2222222222222222222222222222222222222222".into()),
            created_at: Utc::now(),
        };
        let abandoned_signup = WalletUser {
            wallet_id: None,
            arc_address: None,
            base_address: None,
            ..live_wallet.clone()
        };

        assert!(validate_auth_intent_for_user(WalletAuthIntent::Login, None, false).is_err());
        assert!(
            validate_auth_intent_for_user(WalletAuthIntent::Signup, Some(&live_wallet), false)
                .is_err()
        );
        assert!(
            validate_auth_intent_for_user(WalletAuthIntent::Login, Some(&live_wallet), false)
                .is_ok()
        );
        assert!(validate_auth_intent_for_user(
            WalletAuthIntent::Signup,
            Some(&abandoned_signup),
            false
        )
        .is_ok());
    }

    #[test]
    fn real_signup_can_recover_legacy_mock_wallet_rows() {
        let legacy_mock_wallet = WalletUser {
            id: Uuid::new_v4(),
            email: "legacy@example.com".into(),
            risk_tolerance: "moderate".into(),
            investment_horizon_months: 12,
            wallet_id: Some("mock_wallet_deadbeef".into()),
            arc_address: Some("0xARC0000000000000000000000000000000000000000".into()),
            base_address: Some("0xBASE0000000000000000000000000000000000000000".into()),
            created_at: Utc::now(),
        };

        assert!(validate_auth_intent_for_user(
            WalletAuthIntent::Signup,
            Some(&legacy_mock_wallet),
            false
        )
        .is_ok());
        assert!(validate_auth_intent_for_user(
            WalletAuthIntent::Signup,
            Some(&legacy_mock_wallet),
            true
        )
        .is_err());
    }
}
