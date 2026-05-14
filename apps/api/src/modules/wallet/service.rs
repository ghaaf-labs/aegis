//! Wallet service — orchestrates Circle WaaS provider + persistence + JWT.

use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use uuid::Uuid;

use super::models::{
    OtpStartResponse, WalletAuthResponse, WalletInfo, WalletUser, WalletUserPublic,
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

    /// Create a wallet via WebAuthn passkey. If a user with this email
    /// already exists *and* has a wallet, returns the existing wallet
    /// (idempotent). Otherwise creates wallet via Circle and persists.
    pub async fn create_with_passkey(
        &self,
        email: &str,
        passkey_attestation: &serde_json::Value,
    ) -> crate::error::Result<WalletAuthResponse> {
        validate_email(email)?;

        if let Some(existing) = self.find_user_by_email(email).await? {
            if let Some(wallet) = wallet_from_user(&existing) {
                let token = mint_token(&existing, self.config)?;
                return Ok(WalletAuthResponse {
                    token,
                    wallet,
                    user: public(&existing),
                });
            }
            // user exists but has no wallet — fall through and bind one
        }

        let info = self
            .provider
            .create_with_passkey(email, passkey_attestation)
            .await?;

        let user = self.upsert_user_with_wallet(email, &info).await?;
        let token = mint_token(&user, self.config)?;

        let _ = self
            .sse
            .send(SseEvent::WalletCreated(super::sse::WalletCreatedPayload {
                wallet_id: info.wallet_id.clone(),
                arc_address: info.arc_address.clone(),
                base_address: info.base_address.clone(),
                created_at: info.created_at,
            }));

        Ok(WalletAuthResponse {
            token,
            wallet: info,
            user: public(&user),
        })
    }

    pub async fn login_with_passkey(
        &self,
        email: &str,
        passkey_assertion: &serde_json::Value,
    ) -> crate::error::Result<WalletAuthResponse> {
        validate_email(email)?;

        let user = self
            .find_user_by_email(email)
            .await?
            .ok_or_else(|| AppError::Unauthorized("no wallet for this email".into()))?;

        let _info = self
            .provider
            .authenticate_with_passkey(email, passkey_assertion)
            .await?;

        let wallet = wallet_from_user(&user)
            .ok_or_else(|| AppError::Unauthorized("user has no wallet bound".into()))?;
        let token = mint_token(&user, self.config)?;

        Ok(WalletAuthResponse {
            token,
            wallet,
            user: public(&user),
        })
    }

    pub async fn start_otp(&self, email: &str) -> crate::error::Result<OtpStartResponse> {
        validate_email(email)?;
        self.provider.start_otp(email).await
    }

    pub async fn verify_otp(
        &self,
        email: &str,
        code: &str,
    ) -> crate::error::Result<WalletAuthResponse> {
        validate_email(email)?;
        if code.len() < 4 || code.len() > 10 {
            return Err(AppError::BadRequest("invalid OTP code length".into()));
        }

        let info = self.provider.verify_otp(email, code).await?;

        let already_existed = self.find_user_by_email(email).await?.is_some();
        let user = self.upsert_user_with_wallet(email, &info).await?;
        let token = mint_token(&user, self.config)?;

        if !already_existed {
            let _ = self
                .sse
                .send(SseEvent::WalletCreated(super::sse::WalletCreatedPayload {
                    wallet_id: info.wallet_id.clone(),
                    arc_address: info.arc_address.clone(),
                    base_address: info.base_address.clone(),
                    created_at: info.created_at,
                }));
        }

        Ok(WalletAuthResponse {
            token,
            wallet: info,
            user: public(&user),
        })
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

    async fn upsert_user_with_wallet(
        &self,
        email: &str,
        info: &WalletInfo,
    ) -> crate::error::Result<WalletUser> {
        let user = sqlx::query_as::<_, WalletUser>(
            r#"INSERT INTO users (id, email, wallet_id, arc_address, base_address)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (email) DO UPDATE
                 SET wallet_id    = COALESCE(users.wallet_id, EXCLUDED.wallet_id),
                     arc_address  = COALESCE(users.arc_address, EXCLUDED.arc_address),
                     base_address = COALESCE(users.base_address, EXCLUDED.base_address)
               RETURNING id, email, risk_tolerance, investment_horizon_months,
                         wallet_id, arc_address, base_address, created_at"#,
        )
        .bind(Uuid::new_v4())
        .bind(email)
        .bind(&info.wallet_id)
        .bind(&info.arc_address)
        .bind(&info.base_address)
        .fetch_one(self.db)
        .await?;
        Ok(user)
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
    // Boundary check — full RFC compliance is overkill; we just want to reject
    // obviously malformed input before forwarding to Circle.
    if !email.contains('@') || email.len() > 254 || email.contains(char::is_whitespace) {
        return Err(AppError::BadRequest("invalid email".into()));
    }
    Ok(())
}

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
