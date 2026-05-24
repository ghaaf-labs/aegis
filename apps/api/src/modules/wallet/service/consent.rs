use serde_json::json;
use uuid::Uuid;

use super::{WalletService, CURRENT_PRIVACY_VERSION, CURRENT_TOS_VERSION};
use crate::error::AppError;
use crate::modules::wallet::models::EmailAuthConsent;

impl<'a> WalletService<'a> {
    pub async fn init_continue(
        &self,
        email: &str,
        consent: Option<&EmailAuthConsent>,
    ) -> crate::error::Result<crate::modules::wallet::models::WalletAuthResponse> {
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
            self.update_consent_preferences(user.id, consent).await?;
        }
        let mut response = if existing.is_some() {
            self.init_login(&email).await?
        } else {
            self.init_signup_with_consent(&email, consent).await?
        };

        if response.wallet.is_none() {
            if let Ok(wallet) = self.refresh_wallet(response.user.id).await {
                response.wallet = wallet;
                response.status = super::provisioning::auth_response_status(&response.wallet);
                response.user.account_status =
                    super::provisioning::account_status_for_wallet(&response.wallet);
            }
        }

        Ok(response)
    }

    pub(super) async fn init_signup_with_consent(
        &self,
        email: &str,
        consent: Option<&EmailAuthConsent>,
    ) -> crate::error::Result<crate::modules::wallet::models::WalletAuthResponse> {
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
        let session_token =
            super::provisioning::mint_session_token(&user, self.config, self.db).await?;
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

        Ok(crate::modules::wallet::models::WalletAuthResponse {
            session_token,
            status: super::provisioning::auth_response_status(&wallet),
            user: super::provisioning::public(&user),
            wallet,
            is_new_user: was_inserted,
        })
    }

    pub(super) async fn init_login(
        &self,
        email: &str,
    ) -> crate::error::Result<crate::modules::wallet::models::WalletAuthResponse> {
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
        let session_token =
            super::provisioning::mint_session_token(&user, self.config, self.db).await?;
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

        Ok(crate::modules::wallet::models::WalletAuthResponse {
            session_token,
            status: super::provisioning::auth_response_status(&wallet),
            user: super::provisioning::public(&user),
            wallet,
            is_new_user: false,
        })
    }

    pub(super) async fn user_needs_current_consent(
        &self,
        user_id: Uuid,
    ) -> crate::error::Result<bool> {
        use chrono::Utc;
        use sqlx::Row;

        let row = sqlx::query(
            "SELECT tos_version, privacy_version, consented_at
             FROM users
             WHERE id = $1",
        )
        .bind(user_id)
        .fetch_optional(self.db)
        .await?;

        let Some(row) = row else {
            return Ok(true);
        };
        let tos_version: Option<String> = row.try_get("tos_version")?;
        let privacy_version: Option<String> = row.try_get("privacy_version")?;
        let consented_at: Option<chrono::DateTime<Utc>> = row.try_get("consented_at")?;
        Ok(consented_at.is_none()
            || tos_version.as_deref() != Some(CURRENT_TOS_VERSION)
            || privacy_version.as_deref() != Some(CURRENT_PRIVACY_VERSION))
    }

    pub(super) async fn update_consent_preferences(
        &self,
        user_id: Uuid,
        consent: Option<&EmailAuthConsent>,
    ) -> crate::error::Result<()> {
        let marketing_opt_in = consent.and_then(|c| c.marketing_opt_in);
        let has_current_consent = has_required_consent(consent);
        sqlx::query(
            "UPDATE users
             SET marketing_opt_in = COALESCE($2, marketing_opt_in),
                 tos_version = CASE WHEN $3 THEN $4 ELSE tos_version END,
                 privacy_version = CASE WHEN $3 THEN $5 ELSE privacy_version END,
                 consented_at = CASE WHEN $3 THEN NOW() ELSE consented_at END
             WHERE id = $1",
        )
        .bind(user_id)
        .bind(marketing_opt_in)
        .bind(has_current_consent)
        .bind(CURRENT_TOS_VERSION)
        .bind(CURRENT_PRIVACY_VERSION)
        .execute(self.db)
        .await?;
        Ok(())
    }
}

pub(super) fn has_required_consent(consent: Option<&EmailAuthConsent>) -> bool {
    consent.is_some_and(|c| {
        c.tos
            && c.privacy
            && c.tos_version.as_deref() == Some(CURRENT_TOS_VERSION)
            && c.privacy_version.as_deref() == Some(CURRENT_PRIVACY_VERSION)
    })
}

pub(super) fn validate_email(email: &str) -> crate::error::Result<()> {
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

pub(super) fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
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
}
