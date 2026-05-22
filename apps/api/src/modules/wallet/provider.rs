//! Circle Wallets provider.
//!
//! Auth now treats the verified email as the user action and provisions the
//! wallet server-side. The live provider therefore uses Circle's
//! developer-controlled wallet API directly: create/fetch an SCA wallet pair
//! for Arc Testnet + Base Sepolia under our wallet set, keyed by the Aegis
//! `users.id` as Circle `refId`.

use async_trait::async_trait;
use base64::engine::{general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use chrono::Utc;
use rand::rngs::OsRng;
use reqwest::Client;
use rsa::{pkcs8::DecodePublicKey, Oaep, RsaPublicKey};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use super::models::WalletInfo;
use crate::config::Config;
use crate::error::AppError;

/// Circle blockchain identifiers (the strings their API expects). Centralised
/// so the wallet, gateway and CCTP modules can share one source of truth.
const ARC_BLOCKCHAIN: &str = "ARC-TESTNET";
const BASE_BLOCKCHAIN: &str = "BASE-SEPOLIA";

#[async_trait]
pub trait WalletProvider: Send + Sync {
    async fn provision_wallet(&self, user_id: Uuid) -> crate::error::Result<Option<WalletInfo>>;
}

// ── Live (Circle W3S developer-controlled) ────────────────────────────────

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

    fn require_dev_wallet_config(&self) -> crate::error::Result<()> {
        if self.config.circle_wallet_set_id.trim().is_empty() {
            return Err(AppError::ServiceUnavailable(
                "circle developer wallet set is not configured".into(),
            ));
        }
        if self.config.circle_entity_secret.trim().is_empty() {
            return Err(AppError::ServiceUnavailable(
                "circle entity secret is not configured".into(),
            ));
        }
        Ok(())
    }

    async fn fetch_user_wallets(&self, user_id: Uuid) -> crate::error::Result<Option<WalletInfo>> {
        self.require_dev_wallet_config()?;

        #[derive(Deserialize)]
        struct WalletsEnvelope {
            data: WalletsData,
        }
        #[derive(Deserialize)]
        struct WalletsData {
            wallets: Vec<CircleWallet>,
        }

        let url = self.endpoint("/w3s/wallets");
        let ref_id = user_ref_id(user_id);
        let envelope: WalletsEnvelope = self
            .http
            .get(&url)
            .query(&[
                ("walletSetId", self.config.circle_wallet_set_id.as_str()),
                ("refId", ref_id.as_str()),
                ("pageSize", "50"),
            ])
            .header("Authorization", self.auth_header())
            .header("X-Request-Id", Uuid::new_v4().to_string())
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
            .filter(|w| w.state.as_deref().is_none_or(|state| state == "LIVE"))
            .map(|w| (w.id, w.address, w.blockchain))
            .collect();
        Ok(materialize_wallet(&rows))
    }

    async fn create_user_wallets(&self, user_id: Uuid) -> crate::error::Result<Option<WalletInfo>> {
        self.require_dev_wallet_config()?;

        #[derive(Serialize)]
        struct CreateWalletReq {
            #[serde(rename = "idempotencyKey")]
            idempotency_key: String,
            blockchains: [&'static str; 2],
            #[serde(rename = "entitySecretCiphertext")]
            entity_secret_ciphertext: String,
            #[serde(rename = "walletSetId")]
            wallet_set_id: String,
            #[serde(rename = "accountType")]
            account_type: &'static str,
            count: u8,
            metadata: [WalletMetadata; 1],
        }
        #[derive(Serialize)]
        struct WalletMetadata {
            name: String,
            #[serde(rename = "refId")]
            ref_id: String,
        }
        #[derive(Deserialize)]
        struct WalletsEnvelope {
            data: WalletsData,
        }
        #[derive(Deserialize)]
        struct WalletsData {
            wallets: Vec<CircleWallet>,
        }

        let ref_id = user_ref_id(user_id);
        let body = CreateWalletReq {
            idempotency_key: user_id.to_string(),
            blockchains: [ARC_BLOCKCHAIN, BASE_BLOCKCHAIN],
            entity_secret_ciphertext: self.entity_secret_ciphertext().await?,
            wallet_set_id: self.config.circle_wallet_set_id.clone(),
            account_type: "SCA",
            count: 1,
            metadata: [WalletMetadata {
                name: format!("Aegis account {}", &ref_id[..8]),
                ref_id,
            }],
        };

        let envelope: WalletsEnvelope = self
            .http
            .post(self.endpoint("/w3s/developer/wallets"))
            .header("Authorization", self.auth_header())
            .header("X-Request-Id", Uuid::new_v4().to_string())
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle network: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle create_wallets: {e}")))?
            .json()
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!("circle create_wallets decode: {e}"))
            })?;

        let rows: Vec<(String, String, String)> = envelope
            .data
            .wallets
            .into_iter()
            .map(|w| (w.id, w.address, w.blockchain))
            .collect();
        Ok(materialize_wallet(&rows))
    }

    async fn entity_secret_ciphertext(&self) -> crate::error::Result<String> {
        #[derive(Deserialize)]
        struct PublicKeyEnvelope {
            data: PublicKeyData,
        }
        #[derive(Deserialize)]
        struct PublicKeyData {
            #[serde(rename = "publicKey")]
            public_key: String,
        }

        let envelope: PublicKeyEnvelope = self
            .http
            .get(self.endpoint("/w3s/config/entity/publicKey"))
            .header("Authorization", self.auth_header())
            .header("X-Request-Id", Uuid::new_v4().to_string())
            .send()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle network: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle public_key: {e}")))?
            .json()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle public_key decode: {e}")))?;

        encrypt_entity_secret(&self.config.circle_entity_secret, &envelope.data.public_key)
    }
}

#[async_trait]
impl WalletProvider for CircleProvider<'_> {
    async fn provision_wallet(&self, user_id: Uuid) -> crate::error::Result<Option<WalletInfo>> {
        if let Some(existing) = self.fetch_user_wallets(user_id).await? {
            return Ok(Some(existing));
        }
        self.create_user_wallets(user_id).await
    }
}

#[derive(Deserialize)]
struct CircleWallet {
    id: String,
    address: String,
    blockchain: String,
    state: Option<String>,
}

/// Pair Circle's per-chain wallet rows into one `WalletInfo`. Returns `None`
/// until both ARC and BASE rows are present.
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

fn user_ref_id(user_id: Uuid) -> String {
    user_id.to_string()
}

fn encrypt_entity_secret(
    entity_secret_hex: &str,
    public_key: &str,
) -> crate::error::Result<String> {
    let entity_secret_hex = entity_secret_hex.trim().trim_start_matches("0x");
    let entity_secret = hex::decode(entity_secret_hex).map_err(|e| {
        AppError::ServiceUnavailable(format!("circle entity secret must be hex encoded: {e}"))
    })?;
    if entity_secret.len() != 32 {
        return Err(AppError::ServiceUnavailable(
            "circle entity secret must decode to 32 bytes".into(),
        ));
    }

    let public_key = normalize_public_key_pem(public_key);
    let public_key = RsaPublicKey::from_public_key_pem(&public_key)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("circle public_key parse: {e}")))?;
    let encrypted = public_key
        .encrypt(&mut OsRng, Oaep::new::<Sha256>(), &entity_secret)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("circle entity secret encrypt: {e}")))?;
    Ok(BASE64_STANDARD.encode(encrypted))
}

fn normalize_public_key_pem(public_key: &str) -> String {
    let trimmed = public_key.trim().replace("\\n", "\n");
    if trimmed.contains("BEGIN PUBLIC KEY") {
        trimmed
    } else {
        format!("-----BEGIN PUBLIC KEY-----\n{trimmed}\n-----END PUBLIC KEY-----")
    }
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
    async fn provision_wallet(&self, user_id: Uuid) -> crate::error::Result<Option<WalletInfo>> {
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
    async fn mock_provision_wallet_returns_addresses() {
        let p = MockProvider;
        let id = Uuid::new_v4();
        let w = p.provision_wallet(id).await.unwrap().unwrap();
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

    #[test]
    fn normalizes_public_key_body_to_pem() {
        let pem = normalize_public_key_pem("abc");
        assert!(pem.starts_with("-----BEGIN PUBLIC KEY-----"));
        assert!(pem.ends_with("-----END PUBLIC KEY-----"));
    }
}
