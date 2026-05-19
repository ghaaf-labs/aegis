//! Wallet service — orchestrates Circle W3S User-Controlled provider +
//! persistence + JWT minting.

use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use uuid::Uuid;

use super::models::{
    WalletAuthResponse, WalletInfo, WalletStatusResponse, WalletUser, WalletUserPublic,
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
        // Skip ensure_user when the wallet is already provisioned — the
        // Circle user record exists at that point and the extra POST is
        // a guaranteed 409 round-trip on the hot login path.
        if user.wallet_id.is_none() {
            self.provider.ensure_user(user.id).await?;
        }
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
              WHERE id = $1 AND wallet_id IS NULL",
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
