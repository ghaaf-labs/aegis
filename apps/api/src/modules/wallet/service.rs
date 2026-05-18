//! Wallet service — orchestrates Circle W3S User-Controlled provider +
//! persistence + JWT minting.

use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use uuid::Uuid;

use super::models::{
    UserTokenBundle, WalletAuthResponse, WalletInfo, WalletStatusResponse, WalletUser,
    WalletUserPublic,
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

    /// New-user signup. Idempotent if called twice with the same email — the
    /// second call returns a fresh bundle bound to the existing user row.
    /// A user row that exists but has no wallet (W3S ceremony was aborted
    /// before Circle returned addresses) is treated as a fresh signup so the
    /// browser SDK gets a real challenge_id and the user can recover instead
    /// of polling forever.
    pub async fn init_signup(&self, email: &str) -> crate::error::Result<WalletAuthResponse> {
        validate_email(email)?;

        let (user, was_inserted) = self.upsert_user_record(email).await?;
        let needs_challenge = was_inserted || user.wallet_id.is_none();
        self.provider.ensure_user(user.id).await?;
        let bundle = self
            .provider
            .issue_user_token(user.id, needs_challenge)
            .await?;
        let token = mint_token(&user, self.config)?;
        let wallet = wallet_from_user(&user);

        Ok(WalletAuthResponse {
            token,
            user: public(&user),
            wallet,
            bundle,
            is_new_user: was_inserted,
        })
    }

    /// Returning-user signin. Mirrors `init_signup` but never requests an
    /// initialize challenge — the PIN was set on first signup.
    pub async fn init_login(&self, email: &str) -> crate::error::Result<WalletAuthResponse> {
        validate_email(email)?;
        let user = self
            .find_user_by_email(email)
            .await?
            .ok_or_else(|| AppError::Unauthorized("no account for this email".into()))?;
        self.provider.ensure_user(user.id).await?;
        let bundle = self.provider.issue_user_token(user.id, false).await?;
        let token = mint_token(&user, self.config)?;
        let wallet = wallet_from_user(&user);

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
            if let Some(w) = wallet_from_user(&existing) {
                return Ok(WalletStatusResponse { wallet: Some(w) });
            }
        }
        let Some(info) = self.provider.fetch_user_wallets(user_id).await? else {
            return Ok(WalletStatusResponse { wallet: None });
        };
        self.persist_wallet(user_id, &info).await?;
        let _ = self
            .sse
            .send(SseEvent::WalletCreated(super::sse::WalletCreatedPayload {
                user_id,
                wallet_id: info.wallet_id.clone(),
                arc_address: info.arc_address.clone(),
                base_address: info.base_address.clone(),
                created_at: info.created_at,
            }));
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
    async fn upsert_user_record(&self, email: &str) -> crate::error::Result<(WalletUser, bool)> {
        if let Some(existing) = self.find_user_by_email(email).await? {
            return Ok((existing, false));
        }
        let user = sqlx::query_as::<_, WalletUser>(
            r#"INSERT INTO users (id, email)
               VALUES ($1, $2)
               RETURNING id, email, risk_tolerance, investment_horizon_months,
                         wallet_id, arc_address, base_address, created_at"#,
        )
        .bind(Uuid::new_v4())
        .bind(email)
        .fetch_one(self.db)
        .await?;
        Ok((user, true))
    }

    async fn persist_wallet(&self, user_id: Uuid, info: &WalletInfo) -> crate::error::Result<()> {
        sqlx::query(
            "UPDATE users
                SET wallet_id    = COALESCE(wallet_id, $2),
                    arc_address  = COALESCE(arc_address, $3),
                    base_address = COALESCE(base_address, $4)
              WHERE id = $1",
        )
        .bind(user_id)
        .bind(&info.wallet_id)
        .bind(&info.arc_address)
        .bind(&info.base_address)
        .execute(self.db)
        .await?;
        Ok(())
    }
}

fn wallet_from_user(u: &WalletUser) -> Option<WalletInfo> {
    Some(WalletInfo {
        wallet_id: u.wallet_id.clone()?,
        arc_address: u.arc_address.clone()?,
        base_address: u.base_address.clone()?,
        created_at: u.created_at,
    })
}

fn public(u: &WalletUser) -> WalletUserPublic {
    WalletUserPublic {
        id: u.id,
        email: u.email.clone(),
        risk_tolerance: u.risk_tolerance.clone(),
    }
}

fn mint_token(user: &WalletUser, cfg: &Config) -> crate::error::Result<String> {
    let now = Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user.id,
        email: user.email.clone(),
        wallet_id: user.wallet_id.clone(),
        iat: now,
        exp: now + (cfg.jwt_expiry_hours as usize * 3600),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(cfg.jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))
}

fn validate_email(email: &str) -> crate::error::Result<()> {
    if !email.contains('@') || email.len() > 254 || email.contains(char::is_whitespace) {
        return Err(AppError::BadRequest("invalid email".into()));
    }
    Ok(())
}

/// Borrow-only marker so the compiler stops complaining when the bundle
/// isn't read by every code path in tests/build configurations.
#[allow(dead_code)]
fn _ensure_user_token_bundle_is_used(_: &UserTokenBundle) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_email_basics() {
        assert!(validate_email("a@b.co").is_ok());
        assert!(validate_email("invalid").is_err());
        assert!(validate_email("has space@x.com").is_err());
        assert!(validate_email(&"a".repeat(260)).is_err());
    }
}
