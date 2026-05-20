//! Circle Wallets W3S User-Controlled provider.
//!
//! Three-call server side flow:
//!
//! 1. `ensure_user(user_id)` — POST `/v1/w3s/users` to create the Circle user
//!    record under our internal UUID (idempotent — "user already exists"
//!    responses are treated as success).
//! 2. `issue_user_token(user_id, with_initialize_challenge)` — POST
//!    `/v1/w3s/users/token` for `userToken` + `encryptionKey`, then (only on
//!    signup) POST `/v1/w3s/user/initialize` for a `challengeId` the browser
//!    SDK uses to complete the PIN ceremony and provision wallets.
//! 3. `fetch_user_wallets(user_id)` — GET `/v1/w3s/wallets?userId=` to read
//!    out the wallet IDs and addresses once the browser SDK has finished.
//!    Returns empty Vec while still pending so the browser can poll.
//!
//! F-WALLET-1 (closed 2026-05-17): live `GET /v1/w3s/{config/entity,users,wallets}`
//! probes against `https://api.circle.com` returned 200 with the existing
//! `TEST_API_KEY:cd732deb...` key. The 401s observed earlier were from the
//! dead `api-sandbox.circle.com` host. Per Circle docs sandbox vs prod is
//! keyed by `TEST_API_KEY:` / `LIVE_API_KEY:` prefix, not by separate hosts.

use async_trait::async_trait;
use chrono::Utc;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::models::{UserTokenBundle, WalletInfo};
use crate::config::Config;
use crate::error::AppError;

/// Circle blockchain identifiers (the strings their API expects). Centralised
/// so the wallet, gateway and CCTP modules can share one source of truth.
const ARC_BLOCKCHAIN: &str = "ARC-TESTNET";
const BASE_BLOCKCHAIN: &str = "BASE-SEPOLIA";

#[async_trait]
pub trait WalletProvider: Send + Sync {
    async fn ensure_user(&self, user_id: Uuid) -> crate::error::Result<()>;

    async fn issue_user_token(
        &self,
        user_id: Uuid,
        with_initialize_challenge: bool,
    ) -> crate::error::Result<UserTokenBundle>;

    async fn fetch_user_wallets(&self, user_id: Uuid) -> crate::error::Result<Option<WalletInfo>>;
}

// ── Live (Circle W3S) ──────────────────────────────────────────────────────

pub struct CircleProvider<'a> {
    pub http: &'a Client,
    pub config: &'a Config,
}

impl<'a> CircleProvider<'a> {
    pub fn new(http: &'a Client, config: &'a Config) -> Self {
        Self { http, config }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/v1{}", self.config.circle_base_url, path)
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.config.circle_api_key)
    }
}

#[async_trait]
impl WalletProvider for CircleProvider<'_> {
    async fn ensure_user(&self, user_id: Uuid) -> crate::error::Result<()> {
        #[derive(Serialize)]
        struct Body<'a> {
            #[serde(rename = "userId")]
            user_id: &'a str,
        }
        let id_str = user_id.to_string();
        let resp = self
            .http
            .post(self.endpoint("/w3s/users"))
            .header("Authorization", self.auth_header())
            .json(&Body { user_id: &id_str })
            .send()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle network: {e}")))?;

        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        // Circle returns 409 when the user already exists. Decode the
        // structured `code` field for any 4xx so a wording change in
        // Circle's `message` field doesn't silently break idempotency.
        // 155101 = "Entity (User) already exists" — treat as success.
        if status == StatusCode::CONFLICT {
            return Ok(());
        }
        let body = resp.text().await.unwrap_or_default();
        if status.is_client_error() {
            #[derive(serde::Deserialize)]
            struct CircleError {
                code: Option<i64>,
            }
            if let Ok(parsed) = serde_json::from_str::<CircleError>(&body) {
                if parsed.code == Some(155101) {
                    return Ok(());
                }
            }
        }
        Err(AppError::Internal(anyhow::anyhow!(
            "circle ensure_user {status}: {}",
            body.chars().take(200).collect::<String>()
        )))
    }

    async fn issue_user_token(
        &self,
        user_id: Uuid,
        with_initialize_challenge: bool,
    ) -> crate::error::Result<UserTokenBundle> {
        #[derive(Serialize)]
        struct TokenReq<'a> {
            #[serde(rename = "userId")]
            user_id: &'a str,
        }
        #[derive(Deserialize)]
        struct TokenEnvelope {
            data: TokenData,
        }
        #[derive(Deserialize)]
        struct TokenData {
            #[serde(rename = "userToken")]
            user_token: String,
            #[serde(rename = "encryptionKey")]
            encryption_key: String,
        }

        let id_str = user_id.to_string();
        let token_envelope: TokenEnvelope = self
            .http
            .post(self.endpoint("/w3s/users/token"))
            .header("Authorization", self.auth_header())
            .json(&TokenReq { user_id: &id_str })
            .send()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle network: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle token: {e}")))?
            .json()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle token decode: {e}")))?;

        let mut challenge_id = None;
        if with_initialize_challenge {
            #[derive(Serialize)]
            struct InitReq<'a> {
                #[serde(rename = "idempotencyKey")]
                idempotency_key: String,
                blockchains: &'a [&'a str],
                #[serde(rename = "accountType")]
                account_type: &'a str,
            }
            #[derive(Deserialize)]
            struct InitEnvelope {
                data: InitData,
            }
            #[derive(Deserialize)]
            struct InitData {
                #[serde(rename = "challengeId")]
                challenge_id: String,
            }

            let init_envelope: InitEnvelope = self
                .http
                .post(self.endpoint("/w3s/user/initialize"))
                .header("Authorization", self.auth_header())
                .header("X-User-Token", &token_envelope.data.user_token)
                .json(&InitReq {
                    idempotency_key: Uuid::new_v4().to_string(),
                    blockchains: &[ARC_BLOCKCHAIN, BASE_BLOCKCHAIN],
                    // SCA is required — Circle Paymaster gas abstraction
                    // and CCTP V2 Hook execution need a smart contract
                    // account. EOA wallets can't sponsor gas, can't run
                    // hooks, and Circle doesn't migrate them later.
                    account_type: "SCA",
                })
                .send()
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("circle network: {e}")))?
                .error_for_status()
                .map_err(|e| AppError::Internal(anyhow::anyhow!("circle initialize: {e}")))?
                .json()
                .await
                .map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("circle initialize decode: {e}"))
                })?;
            challenge_id = Some(init_envelope.data.challenge_id);
        }

        Ok(UserTokenBundle {
            user_token: token_envelope.data.user_token,
            encryption_key: token_envelope.data.encryption_key,
            app_id: self.config.circle_app_id.clone(),
            challenge_id,
        })
    }

    async fn fetch_user_wallets(&self, user_id: Uuid) -> crate::error::Result<Option<WalletInfo>> {
        #[derive(Deserialize)]
        struct WalletsEnvelope {
            data: WalletsData,
        }
        #[derive(Deserialize)]
        struct WalletsData {
            wallets: Vec<CircleWallet>,
        }
        #[derive(Deserialize)]
        struct CircleWallet {
            id: String,
            address: String,
            blockchain: String,
        }

        let url = self.endpoint("/w3s/wallets");
        let envelope: WalletsEnvelope = self
            .http
            .get(&url)
            .query(&[("userId", user_id.to_string())])
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle network: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle fetch_wallets: {e}")))?
            .json()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle fetch_wallets decode: {e}")))?;

        let rows: Vec<(String, String, String)> = envelope
            .data
            .wallets
            .into_iter()
            .map(|w| (w.id, w.address, w.blockchain))
            .collect();
        Ok(materialize_wallet(&rows))
    }
}

/// Pair Circle's per-chain wallet rows into one `WalletInfo`. Returns `None`
/// until both ARC and BASE rows are present, so the browser keeps polling
/// `/auth/wallet/status` while Circle is still provisioning.
fn materialize_wallet(rows: &[(String, String, String)]) -> Option<WalletInfo> {
    let mut arc = None;
    let mut base = None;
    let mut wallet_id = None;
    for (id, address, blockchain) in rows {
        match blockchain.as_str() {
            ARC_BLOCKCHAIN => {
                wallet_id.get_or_insert_with(|| id.clone());
                arc = Some(address.clone());
            }
            BASE_BLOCKCHAIN => {
                wallet_id.get_or_insert_with(|| id.clone());
                base = Some(address.clone());
            }
            _ => {}
        }
    }
    Some(WalletInfo {
        wallet_id: wallet_id?,
        arc_address: arc?,
        base_address: base?,
        created_at: Utc::now(),
    })
}

// ── Mock (offline dev) ─────────────────────────────────────────────────────

pub struct MockProvider;

impl Default for MockProvider {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl WalletProvider for MockProvider {
    async fn ensure_user(&self, _user_id: Uuid) -> crate::error::Result<()> {
        Ok(())
    }

    async fn issue_user_token(
        &self,
        user_id: Uuid,
        _with_initialize_challenge: bool,
    ) -> crate::error::Result<UserTokenBundle> {
        Ok(UserTokenBundle {
            user_token: format!("mock-token-{user_id}"),
            encryption_key: format!("mock-key-{user_id}"),
            app_id: "mock-app-id".into(),
            challenge_id: None,
        })
    }

    async fn fetch_user_wallets(&self, user_id: Uuid) -> crate::error::Result<Option<WalletInfo>> {
        let seed = stable_hash(&user_id.to_string());
        Ok(Some(WalletInfo {
            wallet_id: format!("mock_wallet_{seed:016x}"),
            arc_address: format!("0xARC{seed:040x}"),
            base_address: format!("0xBASE{seed:040x}"),
            created_at: Utc::now(),
        }))
    }
}

fn stable_hash(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_issue_token_deterministic_per_user() {
        let p = MockProvider;
        let id = Uuid::new_v4();
        let a = p.issue_user_token(id, true).await.unwrap();
        let b = p.issue_user_token(id, true).await.unwrap();
        assert_eq!(a.user_token, b.user_token);
        assert_eq!(a.encryption_key, b.encryption_key);
        assert!(a.challenge_id.is_none()); // mock skips SDK ceremony
    }

    #[tokio::test]
    async fn mock_issue_token_skips_challenge_when_not_requested() {
        let p = MockProvider;
        let id = Uuid::new_v4();
        let bundle = p.issue_user_token(id, false).await.unwrap();
        assert!(bundle.challenge_id.is_none());
    }

    #[tokio::test]
    async fn mock_fetch_user_wallets_returns_addresses() {
        let p = MockProvider;
        let id = Uuid::new_v4();
        let w = p.fetch_user_wallets(id).await.unwrap().unwrap();
        assert!(w.wallet_id.starts_with("mock_wallet_"));
        assert!(w.arc_address.starts_with("0xARC"));
        assert!(w.base_address.starts_with("0xBASE"));
    }

    #[test]
    fn materialize_wallet_pairs_arc_and_base_rows() {
        let rows = vec![
            ("circle-w-1".into(), "0xARC1".into(), ARC_BLOCKCHAIN.into()),
            (
                "circle-w-2".into(),
                "0xBASE2".into(),
                BASE_BLOCKCHAIN.into(),
            ),
        ];
        let w = materialize_wallet(&rows).unwrap();
        assert_eq!(w.arc_address, "0xARC1");
        assert_eq!(w.base_address, "0xBASE2");
        assert_eq!(w.wallet_id, "circle-w-1");
    }

    #[test]
    fn materialize_wallet_returns_none_when_missing_a_chain() {
        let rows: Vec<(String, String, String)> =
            vec![("circle-w-1".into(), "0xARC1".into(), ARC_BLOCKCHAIN.into())];
        assert!(materialize_wallet(&rows).is_none());
    }
}
