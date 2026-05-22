//! Wallet service — orchestrates email-code auth, Circle wallet persistence,
//! and opaque session minting.

use chrono::Utc;
use hmac::{Hmac, Mac};
use rand::Rng;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use super::models::{
    EmailAuthConsent, WalletAuthCodeResponse, WalletAuthResponse, WalletInfo,
    WalletSessionResponse, WalletUser, WalletUserPublic,
};
use super::provider::WalletProvider;
use crate::config::Config;
use crate::db::Db;
use crate::error::AppError;
use crate::modules::sse::{SseEvent, SseSender};

const AUTH_CODE_EXPIRY_MINUTES: i64 = 10;
const AUTH_CODE_RESEND_COOLDOWN_SECONDS: u64 = 30;

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

struct AuthCodeCheck {
    code_hash: String,
    referrer_handle: Option<String>,
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
        referrer_handle: Option<&str>,
    ) -> crate::error::Result<WalletAuthCodeIssue> {
        let normalized = normalize_email(email);
        validate_email(&normalized)?;
        let challenge_id = Uuid::new_v4();
        let code = format!("{:06}", rand::thread_rng().gen_range(0..1_000_000));
        let code_hash = code_hash(self.config, &normalized, challenge_id, &code);
        let expires_at = Utc::now() + chrono::Duration::minutes(AUTH_CODE_EXPIRY_MINUTES);
        let referrer_handle = referrer_handle
            .map(str::trim)
            .filter(|h| !h.is_empty())
            .map(|h| h.to_ascii_lowercase());

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
        validate_email(&email)?;
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
        email: &str,
        challenge_id: Uuid,
        code: &str,
        consent: Option<&EmailAuthConsent>,
    ) -> crate::error::Result<Option<String>> {
        let normalized = normalize_email(email);
        let checked = self
            .check_auth_code(&normalized, challenge_id, code)
            .await?;

        if self.find_user_by_email(&normalized).await?.is_none() && !has_required_consent(consent) {
            return Err(AppError::BadRequest("consent_required".into()));
        }

        self.consume_auth_code(challenge_id, &checked.code_hash)
            .await?;
        Ok(checked.referrer_handle)
    }

    async fn check_auth_code(
        &self,
        normalized: &str,
        challenge_id: Uuid,
        code: &str,
    ) -> crate::error::Result<AuthCodeCheck> {
        use sqlx::Row;

        validate_email(normalized)?;
        let trimmed_code = code.trim();
        if trimmed_code.len() != 6 || !trimmed_code.chars().all(|c| c.is_ascii_digit()) {
            return Err(AppError::BadRequest("code_invalid".into()));
        }

        let row = sqlx::query(
            "SELECT code_hash, attempts, expires_at, referrer_handle, consumed_at
             FROM wallet_auth_codes
             WHERE id = $1
               AND email = $2",
        )
        .bind(challenge_id)
        .bind(normalized)
        .fetch_optional(self.db)
        .await?;

        let Some(row) = row else {
            return Err(AppError::BadRequest("code_invalid".into()));
        };

        let consumed_at: Option<chrono::DateTime<Utc>> = row.try_get("consumed_at")?;
        if consumed_at.is_some() {
            return Err(AppError::BadRequest("code_used".into()));
        }

        let attempts: i32 = row.try_get("attempts")?;
        if attempts >= 3 {
            return Err(AppError::TooManyRequests("too_many_attempts".into()));
        }
        let expires_at: chrono::DateTime<Utc> = row.try_get("expires_at")?;
        if expires_at <= Utc::now() {
            return Err(AppError::BadRequest("code_expired".into()));
        }

        let actual_hash: String = row.try_get("code_hash")?;
        let expected_hash = code_hash(self.config, normalized, challenge_id, trimmed_code);
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
            code_hash: expected_hash,
            referrer_handle: row.try_get("referrer_handle")?,
        })
    }

    async fn consume_auth_code(
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

    pub async fn init_continue(
        &self,
        email: &str,
        consent: Option<&EmailAuthConsent>,
    ) -> crate::error::Result<WalletAuthResponse> {
        let email = normalize_email(email);
        validate_email(&email)?;
        if self.email_has_deletion_request(&email).await? {
            return Err(AppError::Forbidden("account_deletion_requested".into()));
        }
        let existing = self.find_user_by_email(&email).await?;
        if existing.is_none() && !has_required_consent(consent) {
            return Err(AppError::BadRequest("consent_required".into()));
        }
        if let Some(user) = existing.as_ref() {
            if let Some(marketing_opt_in) = consent.and_then(|c| c.marketing_opt_in) {
                self.update_marketing_opt_in(user.id, marketing_opt_in)
                    .await?;
            }
        }
        let mut response = if existing
            .as_ref()
            .is_some_and(|user| user_has_wallet(user, self.config.circle_mock))
        {
            self.init_login(&email).await?
        } else {
            self.init_signup_with_consent(&email, consent).await?
        };

        if response.wallet.is_none() {
            if let Ok(wallet) = self.refresh_wallet(response.user.id).await {
                response.wallet = wallet;
                response.status = auth_response_status(&response.wallet);
                response.user.account_status = account_status_for_wallet(&response.wallet);
            }
        }

        Ok(response)
    }

    async fn init_signup_with_consent(
        &self,
        email: &str,
        consent: Option<&EmailAuthConsent>,
    ) -> crate::error::Result<WalletAuthResponse> {
        let email = normalize_email(email);
        validate_email(&email)?;

        let (mut user, was_inserted) = self.upsert_user_record(&email, consent).await?;
        let has_wallet = user_has_wallet(&user, self.config.circle_mock);
        if !has_wallet {
            self.set_account_status(user.id, "pending_wallet").await?;
            user.account_status = "pending_wallet".into();
        }
        let wallet = if has_wallet {
            wallet_from_user(&user, self.config.circle_mock)
        } else {
            match self.refresh_wallet(user.id).await {
                Ok(wallet) => wallet,
                Err(e) => {
                    tracing::warn!(error=%e, user_id = %user.id, "wallet provisioning failed");
                    self.set_account_status(user.id, "pending_wallet").await?;
                    None
                }
            }
        };
        if wallet.is_some() {
            user = self.find_user_by_id(user.id).await?.unwrap_or(user);
        }
        let session_token = mint_session_token(&user, self.config, self.db).await?;

        Ok(WalletAuthResponse {
            session_token,
            status: auth_response_status(&wallet),
            user: public(&user),
            wallet,
            is_new_user: was_inserted,
        })
    }

    async fn init_login(&self, email: &str) -> crate::error::Result<WalletAuthResponse> {
        let email = normalize_email(email);
        validate_email(&email)?;
        let mut user = self
            .find_user_by_email(&email)
            .await?
            .ok_or_else(|| AppError::Unauthorized("no account for this email".into()))?;
        let has_wallet = user_has_wallet(&user, self.config.circle_mock);
        let mut wallet = wallet_from_user(&user, self.config.circle_mock);
        if !has_wallet {
            self.set_account_status(user.id, "pending_wallet").await?;
            user.account_status = "pending_wallet".into();
            wallet = match self.refresh_wallet(user.id).await {
                Ok(wallet) => wallet,
                Err(e) => {
                    tracing::warn!(error=%e, user_id = %user.id, "wallet provisioning failed");
                    None
                }
            };
            if wallet.is_some() {
                user = self.find_user_by_id(user.id).await?.unwrap_or(user);
            }
        }
        let session_token = mint_session_token(&user, self.config, self.db).await?;

        Ok(WalletAuthResponse {
            session_token,
            status: auth_response_status(&wallet),
            user: public(&user),
            wallet,
            is_new_user: false,
        })
    }

    pub async fn session(&self, user_id: Uuid) -> crate::error::Result<WalletSessionResponse> {
        let user = self
            .find_user_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::Unauthorized("unknown user".into()))?;
        let mut wallet = wallet_from_user(&user, self.config.circle_mock);
        if wallet.is_none() || user.account_status == "pending_wallet" {
            match self.refresh_wallet(user_id).await {
                Ok(next_wallet) => {
                    wallet = next_wallet;
                }
                Err(e) => {
                    tracing::warn!(error=%e, user_id=%user_id, "wallet provisioning retry failed");
                    self.set_account_status(user_id, "pending_wallet").await?;
                    wallet = None;
                }
            }
        }

        let mut user = self.find_user_by_id(user_id).await?.unwrap_or(user);
        let account_status = account_status_for_wallet(&wallet);
        if user.account_status != account_status {
            self.set_account_status(user_id, &account_status).await?;
            user.account_status = account_status.clone();
        }

        Ok(WalletSessionResponse {
            user: public(&user),
            wallet,
            account_status,
        })
    }

    async fn refresh_wallet(&self, user_id: Uuid) -> crate::error::Result<Option<WalletInfo>> {
        if let Some(existing) = self.find_user_by_id(user_id).await? {
            if let Some(w) = wallet_from_user(&existing, self.config.circle_mock) {
                if existing.account_status != "active" {
                    self.set_account_status(user_id, "active").await?;
                }
                return Ok(Some(w));
            }
        }
        let Some(info) = self.provider.provision_wallet(user_id).await? else {
            self.set_account_status(user_id, "pending_wallet").await?;
            return Ok(None);
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
        Ok(Some(info))
    }

    // ── DB helpers ────────────────────────────────────────────────────────

    async fn find_user_by_email(&self, email: &str) -> crate::error::Result<Option<WalletUser>> {
        let user = sqlx::query_as::<_, WalletUser>(
            "SELECT id, email, risk_tolerance, investment_horizon_months,
                    account_status, custody_model,
                    wallet_id, arc_address, base_address, created_at
             FROM users
             WHERE email = $1
               AND deletion_requested_at IS NULL
               AND anonymized_at IS NULL",
        )
        .bind(email)
        .fetch_optional(self.db)
        .await?;
        Ok(user)
    }

    async fn email_has_deletion_request(&self, email: &str) -> crate::error::Result<bool> {
        let has_deletion_request = sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1
                FROM users
                WHERE email = $1
                  AND (deletion_requested_at IS NOT NULL OR anonymized_at IS NOT NULL)
             )",
        )
        .bind(email)
        .fetch_one(self.db)
        .await?;
        Ok(has_deletion_request)
    }

    async fn find_user_by_id(&self, id: Uuid) -> crate::error::Result<Option<WalletUser>> {
        let user = sqlx::query_as::<_, WalletUser>(
            "SELECT id, email, risk_tolerance, investment_horizon_months,
                    account_status, custody_model,
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
    async fn upsert_user_record(
        &self,
        email: &str,
        consent: Option<&EmailAuthConsent>,
    ) -> crate::error::Result<(WalletUser, bool)> {
        use sqlx::Row;
        let tos_version = consent.and_then(|c| c.tos_version.as_deref());
        let privacy_version = consent.and_then(|c| c.privacy_version.as_deref());
        let consented = has_required_consent(consent);
        let marketing_opt_in = consent.and_then(|c| c.marketing_opt_in);
        let row = sqlx::query(
            r#"INSERT INTO users (
                    id, email, account_status, custody_model,
                    tos_version, privacy_version, consented_at, marketing_opt_in
               )
               VALUES (
                    $1, $2, 'pending_wallet', 'circle_developer',
                    $3, $4,
                    CASE WHEN $5 THEN NOW() ELSE NULL END,
                    COALESCE($6, FALSE)
               )
               ON CONFLICT (email) DO UPDATE
                 SET email = EXCLUDED.email,
                     marketing_opt_in = COALESCE($6, users.marketing_opt_in)
               RETURNING id, email, risk_tolerance, investment_horizon_months,
                         account_status, custody_model,
                         wallet_id, arc_address, base_address, created_at,
                         (xmax = 0) AS was_inserted"#,
        )
        .bind(Uuid::new_v4())
        .bind(email)
        .bind(tos_version)
        .bind(privacy_version)
        .bind(consented)
        .bind(marketing_opt_in)
        .fetch_one(self.db)
        .await?;
        let was_inserted: bool = row.try_get("was_inserted")?;
        let user = WalletUser {
            id: row.try_get("id")?,
            email: row.try_get("email")?,
            risk_tolerance: row.try_get("risk_tolerance")?,
            investment_horizon_months: row.try_get("investment_horizon_months")?,
            account_status: row.try_get("account_status")?,
            custody_model: row.try_get("custody_model")?,
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
                    base_address = $4,
                    wallet_set_id = NULLIF($5, ''),
                    custody_model = 'circle_developer',
                    account_status = 'active'
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
        .bind(&self.config.circle_wallet_set_id)
        .execute(self.db)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn set_account_status(&self, user_id: Uuid, status: &str) -> crate::error::Result<()> {
        sqlx::query("UPDATE users SET account_status = $2 WHERE id = $1")
            .bind(user_id)
            .bind(status)
            .execute(self.db)
            .await?;
        Ok(())
    }

    async fn update_marketing_opt_in(
        &self,
        user_id: Uuid,
        marketing_opt_in: bool,
    ) -> crate::error::Result<()> {
        sqlx::query("UPDATE users SET marketing_opt_in = $2 WHERE id = $1")
            .bind(user_id)
            .bind(marketing_opt_in)
            .execute(self.db)
            .await?;
        Ok(())
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
        account_status: u.account_status.clone(),
    }
}

fn auth_response_status(wallet: &Option<WalletInfo>) -> String {
    if wallet.is_some() {
        "active".into()
    } else {
        "provisioning".into()
    }
}

fn account_status_for_wallet(wallet: &Option<WalletInfo>) -> String {
    if wallet.is_some() {
        "active".into()
    } else {
        "pending_wallet".into()
    }
}

fn has_required_consent(consent: Option<&EmailAuthConsent>) -> bool {
    consent.is_some_and(|c| {
        c.tos
            && c.privacy
            && non_empty_opt(c.tos_version.as_deref())
            && non_empty_opt(c.privacy_version.as_deref())
    })
}

fn non_empty_opt(value: Option<&str>) -> bool {
    value.is_some_and(|v| !v.trim().is_empty())
}

async fn mint_session_token(
    user: &WalletUser,
    cfg: &Config,
    db: &Db,
) -> crate::error::Result<String> {
    let session_id = Uuid::new_v4();
    let expires_at = Utc::now() + chrono::Duration::hours(cfg.jwt_expiry_hours as i64);

    sqlx::query(
        "UPDATE auth_sessions
         SET revoked_at = COALESCE(revoked_at, NOW())
         WHERE user_id = $1
           AND revoked_at IS NULL",
    )
    .bind(user.id)
    .execute(db)
    .await?;

    sqlx::query(
        "INSERT INTO auth_sessions (id, user_id, expires_at)
         VALUES ($1, $2, $3)",
    )
    .bind(session_id)
    .bind(user.id)
    .bind(expires_at)
    .execute(db)
    .await?;

    Ok(session_id.to_string())
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
    fn consent_requires_terms_privacy_and_versions() {
        assert!(!has_required_consent(None));

        let mut consent = EmailAuthConsent {
            tos: true,
            privacy: true,
            tos_version: Some("2026-05".into()),
            privacy_version: Some("2026-05".into()),
            marketing_opt_in: Some(false),
        };
        assert!(has_required_consent(Some(&consent)));

        consent.tos_version = Some(" ".into());
        assert!(!has_required_consent(Some(&consent)));

        consent.tos_version = Some("2026-05".into());
        consent.privacy = false;
        assert!(!has_required_consent(Some(&consent)));
    }

    #[test]
    fn wallet_auth_response_does_not_serialize_session_token() {
        let user_id = Uuid::new_v4();
        let response = WalletAuthResponse {
            session_token: "opaque-session-id".into(),
            status: "provisioning".into(),
            user: WalletUserPublic {
                id: user_id,
                email: "user@example.com".into(),
                risk_tolerance: "moderate".into(),
                account_status: "pending_wallet".into(),
            },
            wallet: None,
            is_new_user: false,
        };

        let json = serde_json::to_value(response).unwrap();
        assert!(json.get("sessionToken").is_none());
        assert!(json.get("isNewUser").is_none());
        assert_eq!(json["user"]["id"], user_id.to_string());
        assert!(json.get("bundle").is_none());
    }

    #[test]
    fn wallet_auth_response_serializes_active_wallet() {
        let user_id = Uuid::new_v4();
        let response = WalletAuthResponse {
            session_token: "opaque-session-id".into(),
            status: "active".into(),
            user: WalletUserPublic {
                id: user_id,
                email: "returning@example.com".into(),
                risk_tolerance: "moderate".into(),
                account_status: "active".into(),
            },
            wallet: Some(WalletInfo {
                wallet_id: "wallet-live".into(),
                arc_address: "0x1111111111111111111111111111111111111111".into(),
                base_address: "0x2222222222222222222222222222222222222222".into(),
                created_at: Utc::now(),
            }),
            is_new_user: false,
        };

        let json = serde_json::to_value(response).unwrap();
        assert!(json.get("sessionToken").is_none());
        assert!(json.get("isNewUser").is_none());
        assert!(json.get("bundle").is_none());
        assert_eq!(json["wallet"]["walletId"], "wallet-live");
    }

    #[test]
    fn auth_code_response_serializes_resend_cooldown() {
        let response = WalletAuthCodeResponse {
            challenge_id: Uuid::new_v4(),
            email: "user@example.com".into(),
            expires_at: Utc::now(),
            resend_in_seconds: AUTH_CODE_RESEND_COOLDOWN_SECONDS,
            dev_code: None,
        };

        let json = serde_json::to_value(response).unwrap();
        assert_eq!(json["resendInSeconds"], AUTH_CODE_RESEND_COOLDOWN_SECONDS);
    }

    #[test]
    fn real_mode_ignores_legacy_mock_wallet_rows() {
        let user = WalletUser {
            id: Uuid::new_v4(),
            email: "legacy@example.com".into(),
            risk_tolerance: "moderate".into(),
            investment_horizon_months: 12,
            account_status: "active".into(),
            custody_model: "circle_developer".into(),
            wallet_id: Some("mock_wallet_deadbeef".into()),
            arc_address: Some("0xARC0000000000000000000000000000000000000000".into()),
            base_address: Some("0xBASE0000000000000000000000000000000000000000".into()),
            created_at: Utc::now(),
        };

        assert!(wallet_from_user(&user, true).is_some());
        assert!(wallet_from_user(&user, false).is_none());
        assert!(!user_has_wallet(&user, false));
    }
}
