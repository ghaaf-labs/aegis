//! Wallet service — orchestrates email-code auth, Circle wallet persistence,
//! and opaque session minting.

use chrono::Utc;
use hmac::{Hmac, Mac};
use rand::Rng;
use serde_json::json;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use super::models::{
    EmailAuthConsent, WalletAuthCodeResponse, WalletAuthResponse, WalletInfo, WalletNetwork,
    WalletSessionResponse, WalletUser, WalletUserPublic,
};
use super::provider::WalletProvider;
use crate::config::Config;
use crate::db::Db;
use crate::error::AppError;
use crate::modules::sse::{SseEvent, SseSender};

const AUTH_CODE_EXPIRY_MINUTES: i64 = 10;
const AUTH_CODE_RESEND_COOLDOWN_SECONDS: u64 = 30;
const AUTH_IP_RATE_WINDOW_SECONDS: i64 = 10 * 60;
const AUTH_IP_RATE_LIMIT: i32 = 20;

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
    email: String,
    code_hash: String,
    referrer_handle: Option<String>,
}

pub struct VerifiedAuthCode {
    pub email: String,
    pub referrer_handle: Option<String>,
}

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
        challenge_id: Uuid,
        code: &str,
        consent: Option<&EmailAuthConsent>,
    ) -> crate::error::Result<VerifiedAuthCode> {
        let checked = self.check_auth_code(challenge_id, code).await?;

        if self.find_user_by_email(&checked.email).await?.is_none()
            && !has_required_consent(consent)
        {
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

    async fn check_auth_code(
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

        let email = normalize_email(&row.try_get::<String, _>("email")?);
        validate_email(&email)?;

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
        let mut response = if existing.is_some() {
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
        let mut wallet = self.wallet_from_network_routes(&user).await?;
        let routes_synced = wallet
            .as_ref()
            .is_some_and(|wallet| !self.network_routes_need_provider_sync(wallet));
        if !routes_synced {
            self.set_account_status(user.id, "pending_wallet").await?;
            user.account_status = "pending_wallet".into();
        }
        if !routes_synced {
            wallet = match self.refresh_wallet(user.id).await {
                Ok(wallet) => wallet,
                Err(e) => {
                    tracing::warn!(error=%e, user_id = %user.id, "wallet provisioning failed");
                    self.set_account_status(user.id, "pending_wallet").await?;
                    None
                }
            };
        }
        if wallet.is_some() {
            user = self.find_user_by_id(user.id).await?.unwrap_or(user);
        }
        let session_token = mint_session_token(&user, self.config, self.db).await?;
        if was_inserted {
            crate::modules::analytics::service::emit(
                self.db,
                Some(user.id),
                "auth.signup_created",
                json!({
                    "accountStatus": user.account_status,
                    "walletReady": wallet.is_some(),
                }),
            )
            .await;
        }
        crate::modules::analytics::service::emit(
            self.db,
            Some(user.id),
            "auth.session_rotated",
            json!({
                "source": "email_verify",
            }),
        )
        .await;

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
        let mut wallet = self.wallet_from_network_routes(&user).await?;
        let routes_synced = wallet
            .as_ref()
            .is_some_and(|wallet| !self.network_routes_need_provider_sync(wallet));
        if !routes_synced {
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
        crate::modules::analytics::service::emit(
            self.db,
            Some(user.id),
            "auth.login_restored",
            json!({
                "accountStatus": user.account_status,
                "walletReady": wallet.is_some(),
            }),
        )
        .await;
        crate::modules::analytics::service::emit(
            self.db,
            Some(user.id),
            "auth.session_rotated",
            json!({
                "source": "email_verify",
            }),
        )
        .await;

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
        let mut wallet = self.wallet_from_network_routes(&user).await?;
        let routes_synced = wallet
            .as_ref()
            .is_some_and(|wallet| !self.network_routes_need_provider_sync(wallet));
        if wallet.is_none() || user.account_status == "pending_wallet" || !routes_synced {
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
            if let Some(w) = self.wallet_from_network_routes(&existing).await? {
                if !self.network_routes_need_provider_sync(&w) {
                    if existing.account_status != "active" {
                        self.set_account_status(user_id, "active").await?;
                    }
                    return Ok(Some(w));
                }
            }
        }
        crate::modules::analytics::service::emit(
            self.db,
            Some(user_id),
            "auth.provision_started",
            json!({}),
        )
        .await;
        let info = match self.provider.provision_wallet(user_id).await {
            Ok(Some(info)) => info,
            Ok(None) => {
                self.set_account_status(user_id, "pending_wallet").await?;
                crate::modules::analytics::service::emit(
                    self.db,
                    Some(user_id),
                    "auth.provision_failed",
                    json!({
                        "reason": "provider_returned_empty",
                    }),
                )
                .await;
                return Ok(None);
            }
            Err(e) => {
                crate::modules::analytics::service::emit(
                    self.db,
                    Some(user_id),
                    "auth.provision_failed",
                    json!({
                        "reason": "provider_error",
                    }),
                )
                .await;
                return Err(e);
            }
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
        crate::modules::analytics::service::emit(
            self.db,
            Some(user_id),
            "auth.provision_succeeded",
            json!({
                "persisted": just_persisted,
            }),
        )
        .await;
        Ok(Some(info))
    }

    // ── DB helpers ────────────────────────────────────────────────────────

    async fn wallet_from_network_routes(
        &self,
        user: &WalletUser,
    ) -> crate::error::Result<Option<WalletInfo>> {
        let networks = sqlx::query_as::<_, WalletNetwork>(
            "SELECT blockchain,
                    circle_wallet_id AS wallet_id,
                    address,
                    account_type,
                    state
             FROM user_wallet_networks
             WHERE user_id = $1
             ORDER BY blockchain",
        )
        .bind(user.id)
        .fetch_all(self.db)
        .await?;
        Ok(wallet_from_networks(user.created_at, networks))
    }

    fn network_routes_need_provider_sync(&self, wallet: &WalletInfo) -> bool {
        if self.config.circle_mock {
            return false;
        }
        network_routes_need_provider_sync(wallet, &self.config.circle_wallet_set_id)
    }

    async fn find_user_by_email(&self, email: &str) -> crate::error::Result<Option<WalletUser>> {
        let user = sqlx::query_as::<_, WalletUser>(
            "SELECT id, email, risk_tolerance, investment_horizon_months,
                    account_status, custody_model, wallet_set_id, created_at
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
                    account_status, custody_model, wallet_set_id, created_at
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
                         account_status, custody_model, wallet_set_id, created_at,
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
            wallet_set_id: row.try_get("wallet_set_id")?,
            created_at: row.try_get("created_at")?,
        };
        Ok((user, was_inserted))
    }

    /// Persist Circle's wallet info. Returns `true` when this call actually
    /// wrote (so the caller can fire the `wallet.created` SSE once). Two
    /// concurrent status polls used to both fire SSE because the write path is
    /// idempotent. Check for a complete network-route wallet before writing so
    /// only the actual provisioning caller emits the event.
    async fn persist_wallet(&self, user_id: Uuid, info: &WalletInfo) -> crate::error::Result<bool> {
        let already_had_synced_routes = if let Some(user) = self.find_user_by_id(user_id).await? {
            self.wallet_from_network_routes(&user)
                .await?
                .as_ref()
                .is_some_and(|wallet| !self.network_routes_need_provider_sync(wallet))
        } else {
            false
        };

        let mut tx = self.db.begin().await?;

        for network in &info.networks {
            sqlx::query(
                "INSERT INTO user_wallet_networks (
                    user_id, blockchain, circle_wallet_id, address,
                    account_type, wallet_set_id, state
                 )
                 VALUES ($1, $2, $3, $4, $5, NULLIF($6, ''), $7)
                 ON CONFLICT (user_id, blockchain) DO UPDATE
                    SET circle_wallet_id = EXCLUDED.circle_wallet_id,
                        address = EXCLUDED.address,
                        account_type = EXCLUDED.account_type,
                        wallet_set_id = EXCLUDED.wallet_set_id,
                        state = EXCLUDED.state",
            )
            .bind(user_id)
            .bind(&network.blockchain)
            .bind(&network.wallet_id)
            .bind(&network.address)
            .bind(&network.account_type)
            .bind(&self.config.circle_wallet_set_id)
            .bind(&network.state)
            .execute(&mut *tx)
            .await?;
        }

        let result = sqlx::query(
            "UPDATE users
                SET wallet_set_id = NULLIF($2, ''),
                    custody_model = 'circle_developer',
                    account_status = 'active'
              WHERE id = $1",
        )
        .bind(user_id)
        .bind(&self.config.circle_wallet_set_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(!already_had_synced_routes && result.rows_affected() > 0)
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

fn wallet_from_networks(
    created_at: chrono::DateTime<Utc>,
    networks: Vec<WalletNetwork>,
) -> Option<WalletInfo> {
    let arc = networks
        .iter()
        .find(|network| network.blockchain == "ARC-TESTNET" || network.blockchain == "ARC")?;
    let base = networks
        .iter()
        .find(|network| network.blockchain == "BASE-SEPOLIA" || network.blockchain == "BASE")?;
    Some(WalletInfo {
        wallet_id: arc.wallet_id.clone(),
        arc_address: arc.address.clone(),
        base_address: base.address.clone(),
        networks,
        created_at,
    })
}

fn network_routes_need_provider_sync(wallet: &WalletInfo, expected_wallet_set_id: &str) -> bool {
    if expected_wallet_set_id.trim().is_empty() || wallet.networks.len() < 2 {
        return true;
    }
    let has_arc = wallet
        .networks
        .iter()
        .any(|network| network.blockchain == "ARC-TESTNET");
    let has_base = wallet
        .networks
        .iter()
        .any(|network| network.blockchain == "BASE-SEPOLIA");
    if !has_arc || !has_base {
        return true;
    }
    let mut ids = std::collections::HashSet::new();
    for network in &wallet.networks {
        if network.wallet_id.starts_with("mock_wallet_")
            || network.address.starts_with("0xARC")
            || network.address.starts_with("0xBASE")
            || network.account_type != "SCA"
            || network.state != "LIVE"
            || !ids.insert(network.wallet_id.clone())
        {
            return true;
        }
    }
    false
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
                networks: vec![
                    WalletNetwork {
                        blockchain: "ARC-TESTNET".into(),
                        wallet_id: "wallet-live".into(),
                        address: "0x1111111111111111111111111111111111111111".into(),
                        account_type: "SCA".into(),
                        state: "LIVE".into(),
                    },
                    WalletNetwork {
                        blockchain: "BASE-SEPOLIA".into(),
                        wallet_id: "wallet-live".into(),
                        address: "0x2222222222222222222222222222222222222222".into(),
                        account_type: "SCA".into(),
                        state: "LIVE".into(),
                    },
                ],
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
