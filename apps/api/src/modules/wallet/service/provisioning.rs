use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use super::{WalletService, CURRENT_PRIVACY_VERSION, CURRENT_TOS_VERSION};
use crate::config::Config;
use crate::db::Db;
use crate::error::AppError;
use crate::modules::sse::SseEvent;
use crate::modules::wallet::models::{
    EmailAuthConsent, WalletInfo, WalletNetwork, WalletSessionResponse, WalletUser,
    WalletUserPublic,
};
use crate::modules::wallet_routes::{ARC_TESTNET, BASE_SEPOLIA, SUPPORTED_WALLET_BLOCKCHAINS};

const WALLET_PROVISION_RETRY_BASE_SECONDS: i32 = 30;
const WALLET_PROVISION_RETRY_MAX_SECONDS: i32 = 15 * 60;

impl<'a> WalletService<'a> {
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

    pub async fn reconcile_pending_wallets(&self, limit: i64) -> crate::error::Result<usize> {
        let user_ids = self.pending_wallet_user_ids(limit).await?;
        let mut healed = 0;

        for user_id in user_ids {
            match self.session(user_id).await {
                Ok(session) if session.wallet.is_some() => healed += 1,
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error=%e, %user_id, "wallet provisioning reconciler user retry failed");
                }
            }
        }

        Ok(healed)
    }

    pub(super) async fn refresh_wallet(
        &self,
        user_id: Uuid,
    ) -> crate::error::Result<Option<WalletInfo>> {
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
                self.schedule_wallet_retry(user_id, "provider_returned_empty")
                    .await?;
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
                self.schedule_wallet_retry(user_id, "provider_error")
                    .await?;
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
                .send(SseEvent::WalletCreated(super::super::sse::WalletCreatedPayload {
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

    pub(super) async fn pending_wallet_user_ids(
        &self,
        limit: i64,
    ) -> crate::error::Result<Vec<Uuid>> {
        let user_ids = sqlx::query_scalar(
            r#"SELECT u.id
               FROM users u
               WHERE u.deletion_requested_at IS NULL
                 AND u.anonymized_at IS NULL
                 AND u.custody_model = 'circle_developer'
                 AND COALESCE(u.wallet_provision_next_retry_at, NOW()) <= NOW()
                 AND (
                   u.account_status = 'pending_wallet'
                   OR NOT EXISTS (
                     SELECT 1
                     FROM user_wallet_networks n
                     WHERE n.user_id = u.id
                       AND n.blockchain = 'ARC-TESTNET'
                       AND n.account_type = 'SCA'
                       AND n.state = 'LIVE'
                       AND (NULLIF($1, '') IS NULL OR n.wallet_set_id = NULLIF($1, ''))
                   )
                   OR NOT EXISTS (
                     SELECT 1
                     FROM user_wallet_networks n
                     WHERE n.user_id = u.id
                       AND n.blockchain = 'BASE-SEPOLIA'
                       AND n.account_type = 'SCA'
                       AND n.state = 'LIVE'
                       AND (NULLIF($1, '') IS NULL OR n.wallet_set_id = NULLIF($1, ''))
                   )
                   OR NOT EXISTS (
                     SELECT 1
                     FROM user_wallet_networks n
                     WHERE n.user_id = u.id
                       AND n.blockchain = 'ETH-SEPOLIA'
                       AND n.account_type = 'SCA'
                       AND n.state = 'LIVE'
                       AND (NULLIF($1, '') IS NULL OR n.wallet_set_id = NULLIF($1, ''))
                   )
                   OR NOT EXISTS (
                     SELECT 1
                     FROM user_wallet_networks n
                     WHERE n.user_id = u.id
                       AND n.blockchain = 'ARB-SEPOLIA'
                       AND n.account_type = 'SCA'
                       AND n.state = 'LIVE'
                       AND (NULLIF($1, '') IS NULL OR n.wallet_set_id = NULLIF($1, ''))
                   )
                   OR NOT EXISTS (
                     SELECT 1
                     FROM user_wallet_networks n
                     WHERE n.user_id = u.id
                       AND n.blockchain = 'AVAX-FUJI'
                       AND n.account_type = 'SCA'
                       AND n.state = 'LIVE'
                       AND (NULLIF($1, '') IS NULL OR n.wallet_set_id = NULLIF($1, ''))
                   )
                 )
               ORDER BY COALESCE(u.wallet_provision_next_retry_at, u.updated_at), u.updated_at
               LIMIT $2"#,
        )
        .bind(self.config.circle_wallet_set_id.trim())
        .bind(limit.max(1))
        .fetch_all(self.db)
        .await?;
        Ok(user_ids)
    }

    pub(super) async fn wallet_from_network_routes(
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

    pub(super) fn network_routes_need_provider_sync(&self, wallet: &WalletInfo) -> bool {
        if self.config.circle_mock {
            return false;
        }
        network_routes_need_provider_sync(wallet, &self.config.circle_wallet_set_id)
    }

    pub(super) async fn find_user_by_email(
        &self,
        email: &str,
    ) -> crate::error::Result<Option<WalletUser>> {
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

    pub(super) async fn email_has_deletion_request(
        &self,
        email: &str,
    ) -> crate::error::Result<bool> {
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

    pub(super) async fn find_user_by_id(
        &self,
        id: Uuid,
    ) -> crate::error::Result<Option<WalletUser>> {
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
    pub(super) async fn upsert_user_record(
        &self,
        email: &str,
        consent: Option<&EmailAuthConsent>,
    ) -> crate::error::Result<(WalletUser, bool)> {
        use sqlx::Row;
        let tos_version = consent
            .and_then(|c| c.tos_version.as_deref())
            .filter(|version| *version == CURRENT_TOS_VERSION);
        let privacy_version = consent
            .and_then(|c| c.privacy_version.as_deref())
            .filter(|version| *version == CURRENT_PRIVACY_VERSION);
        let consented = super::consent::has_required_consent(consent);
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
    pub(super) async fn persist_wallet(
        &self,
        user_id: Uuid,
        info: &WalletInfo,
    ) -> crate::error::Result<bool> {
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
                    account_status = 'active',
                    wallet_provision_attempts = 0,
                    wallet_provision_next_retry_at = NULL,
                    wallet_provision_last_error = NULL
              WHERE id = $1",
        )
        .bind(user_id)
        .bind(&self.config.circle_wallet_set_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(!already_had_synced_routes && result.rows_affected() > 0)
    }

    pub(super) async fn set_account_status(
        &self,
        user_id: Uuid,
        status: &str,
    ) -> crate::error::Result<()> {
        if status == "active" {
            sqlx::query(
                "UPDATE users
                 SET account_status = 'active',
                     wallet_provision_attempts = 0,
                     wallet_provision_next_retry_at = NULL,
                     wallet_provision_last_error = NULL
                 WHERE id = $1",
            )
            .bind(user_id)
            .execute(self.db)
            .await?;
        } else {
            sqlx::query("UPDATE users SET account_status = $2 WHERE id = $1")
                .bind(user_id)
                .bind(status)
                .execute(self.db)
                .await?;
        }
        Ok(())
    }

    pub(super) async fn schedule_wallet_retry(
        &self,
        user_id: Uuid,
        reason: &str,
    ) -> crate::error::Result<()> {
        sqlx::query(
            "UPDATE users
             SET account_status = 'pending_wallet',
                 wallet_provision_attempts = wallet_provision_attempts + 1,
                 wallet_provision_next_retry_at =
                    NOW() + make_interval(secs => LEAST($2, $3 * CAST(POWER(2, LEAST(wallet_provision_attempts, 5)) AS INTEGER))),
                 wallet_provision_last_error = LEFT($4, 160)
             WHERE id = $1",
        )
        .bind(user_id)
        .bind(WALLET_PROVISION_RETRY_MAX_SECONDS)
        .bind(WALLET_PROVISION_RETRY_BASE_SECONDS)
        .bind(reason)
        .execute(self.db)
        .await?;
        Ok(())
    }
}

pub(super) fn wallet_from_networks(
    created_at: chrono::DateTime<Utc>,
    networks: Vec<WalletNetwork>,
) -> Option<WalletInfo> {
    let arc = networks
        .iter()
        .find(|network| network.blockchain == ARC_TESTNET || network.blockchain == "ARC")?;
    let base = networks
        .iter()
        .find(|network| network.blockchain == BASE_SEPOLIA || network.blockchain == "BASE")?;
    Some(WalletInfo {
        wallet_id: arc.wallet_id.clone(),
        arc_address: arc.address.clone(),
        base_address: base.address.clone(),
        networks,
        created_at,
    })
}

pub(super) fn network_routes_need_provider_sync(
    wallet: &WalletInfo,
    expected_wallet_set_id: &str,
) -> bool {
    if expected_wallet_set_id.trim().is_empty()
        || wallet.networks.len() < SUPPORTED_WALLET_BLOCKCHAINS.len()
    {
        return true;
    }
    let missing_supported_route = SUPPORTED_WALLET_BLOCKCHAINS.iter().any(|blockchain| {
        !wallet
            .networks
            .iter()
            .any(|network| network.blockchain == *blockchain)
    });
    if missing_supported_route {
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

pub(super) fn public(u: &WalletUser) -> WalletUserPublic {
    WalletUserPublic {
        id: u.id,
        email: u.email.clone(),
        risk_tolerance: u.risk_tolerance.clone(),
        account_status: u.account_status.clone(),
    }
}

pub(super) fn auth_response_status(wallet: &Option<WalletInfo>) -> String {
    if wallet.is_some() {
        "active".into()
    } else {
        "provisioning".into()
    }
}

pub(super) fn account_status_for_wallet(wallet: &Option<WalletInfo>) -> String {
    if wallet.is_some() {
        "active".into()
    } else {
        "pending_wallet".into()
    }
}

pub(super) async fn mint_session_token(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::wallet::models::WalletAuthResponse;

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
    fn provider_sync_requires_every_supported_wallet_route() {
        let wallet = wallet_with_routes(&[ARC_TESTNET, BASE_SEPOLIA]);
        assert!(network_routes_need_provider_sync(&wallet, "wallet-set"));

        let wallet = wallet_with_routes(&SUPPORTED_WALLET_BLOCKCHAINS);
        assert!(!network_routes_need_provider_sync(&wallet, "wallet-set"));
    }

    #[test]
    fn provider_sync_rejects_mock_or_placeholder_supported_routes() {
        let mut wallet = wallet_with_routes(&SUPPORTED_WALLET_BLOCKCHAINS);
        wallet.networks[2].wallet_id = "mock_wallet_1".into();
        assert!(network_routes_need_provider_sync(&wallet, "wallet-set"));

        let mut wallet = wallet_with_routes(&SUPPORTED_WALLET_BLOCKCHAINS);
        wallet.networks[3].state = "PENDING".into();
        assert!(network_routes_need_provider_sync(&wallet, "wallet-set"));
    }

    fn wallet_with_routes(blockchains: &[&str]) -> WalletInfo {
        let networks: Vec<WalletNetwork> = blockchains
            .iter()
            .enumerate()
            .map(|(index, blockchain)| WalletNetwork {
                blockchain: (*blockchain).into(),
                wallet_id: format!("circle-wallet-{index}"),
                address: format!("0x{index:040x}"),
                account_type: "SCA".into(),
                state: "LIVE".into(),
            })
            .collect();
        WalletInfo {
            wallet_id: networks
                .first()
                .map(|network| network.wallet_id.clone())
                .unwrap_or_default(),
            arc_address: "0x0000000000000000000000000000000000000000".into(),
            base_address: "0x0000000000000000000000000000000000000001".into(),
            networks,
            created_at: Utc::now(),
        }
    }
}
