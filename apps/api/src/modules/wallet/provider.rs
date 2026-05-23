//! Circle Wallets provider.
//!
//! Auth now treats the verified email as the user action and provisions the
//! wallet server-side. The live provider therefore uses Circle's
//! developer-controlled wallet API directly: create/fetch SCA wallet routes
//! under our wallet set, keyed by the Aegis `users.id` as Circle `refId`.

use async_trait::async_trait;
use base64::engine::{general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use chrono::Utc;
use rand::rngs::OsRng;
use reqwest::Client;
use rsa::{pkcs8::DecodePublicKey, Oaep, RsaPublicKey};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use super::models::{WalletInfo, WalletNetwork};
use crate::config::Config;
use crate::error::AppError;
use crate::modules::wallet_routes::{ARC_TESTNET, BASE_SEPOLIA, SUPPORTED_WALLET_BLOCKCHAINS};

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

    async fn fetch_user_wallet_rows(
        &self,
        user_id: Uuid,
    ) -> crate::error::Result<Vec<WalletNetwork>> {
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

        let rows: Vec<WalletNetwork> = envelope
            .data
            .wallets
            .into_iter()
            .filter(|w| w.state.as_deref().is_none_or(|state| state == "LIVE"))
            .map(CircleWallet::into_network)
            .collect();
        Ok(rows)
    }

    async fn create_user_wallet_rows(
        &self,
        user_id: Uuid,
        blockchains: &[&'static str],
    ) -> crate::error::Result<Vec<WalletNetwork>> {
        self.require_dev_wallet_config()?;

        #[derive(Serialize)]
        struct CreateWalletReq {
            #[serde(rename = "idempotencyKey")]
            idempotency_key: String,
            blockchains: Vec<&'static str>,
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
            idempotency_key: route_idempotency_key(user_id, blockchains),
            blockchains: blockchains.to_vec(),
            entity_secret_ciphertext: self.entity_secret_ciphertext().await?,
            wallet_set_id: self.config.circle_wallet_set_id.clone(),
            account_type: "SCA",
            count: 1,
            metadata: [WalletMetadata {
                name: format!("Aegis account {}", &ref_id[..8]),
                ref_id,
            }],
        };

        let response = self
            .http
            .post(self.endpoint("/w3s/developer/wallets"))
            .header("Authorization", self.auth_header())
            .header("X-Request-Id", Uuid::new_v4().to_string())
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle network: {e}")))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle create_wallets body: {e}")))?;
        if !status.is_success() {
            return Err(AppError::Internal(anyhow::anyhow!(
                "circle create_wallets: HTTP {status}: {body}"
            )));
        }
        let envelope: WalletsEnvelope = serde_json::from_str(&body).map_err(|e| {
            AppError::Internal(anyhow::anyhow!("circle create_wallets decode: {e}"))
        })?;

        let rows: Vec<WalletNetwork> = envelope
            .data
            .wallets
            .into_iter()
            .map(CircleWallet::into_network)
            .collect();
        Ok(rows)
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
        let mut rows = self.fetch_user_wallet_rows(user_id).await?;
        let missing = missing_supported_blockchains(&rows);
        if !missing.is_empty() {
            let created = self.create_user_wallet_rows(user_id, &missing).await?;
            rows.extend(created);
        }
        Ok(materialize_wallet(&dedupe_wallet_networks(rows)))
    }
}

#[derive(Deserialize)]
struct CircleWallet {
    id: String,
    address: String,
    blockchain: String,
    #[serde(rename = "accountType")]
    account_type: Option<String>,
    state: Option<String>,
}

impl CircleWallet {
    fn into_network(self) -> WalletNetwork {
        WalletNetwork {
            blockchain: self.blockchain,
            wallet_id: self.id,
            address: self.address,
            account_type: self.account_type.unwrap_or_else(|| "SCA".into()),
            state: self.state.unwrap_or_else(|| "LIVE".into()),
        }
    }
}

/// Pair Circle's per-chain wallet rows into one account-level `WalletInfo`.
/// Circle stores one wallet row per blockchain, but EVM SCA rows can share the
/// same address. Aegis treats that as one account wallet with network routes.
fn materialize_wallet(rows: &[WalletNetwork]) -> Option<WalletInfo> {
    let mut arc = None;
    let mut base = None;
    let mut wallet_id = None;
    for network in rows {
        match network.blockchain.as_str() {
            ARC_TESTNET => {
                wallet_id.get_or_insert_with(|| network.wallet_id.clone());
                arc = Some(network.address.clone());
            }
            BASE_SEPOLIA => {
                wallet_id.get_or_insert_with(|| network.wallet_id.clone());
                base = Some(network.address.clone());
            }
            _ => {}
        }
    }
    Some(WalletInfo {
        wallet_id: wallet_id?,
        arc_address: arc?,
        base_address: base?,
        networks: rows.to_vec(),
        created_at: Utc::now(),
    })
}

fn missing_supported_blockchains(rows: &[WalletNetwork]) -> Vec<&'static str> {
    SUPPORTED_WALLET_BLOCKCHAINS
        .iter()
        .copied()
        .filter(|blockchain| {
            !rows.iter().any(|network| {
                network.blockchain == *blockchain
                    && network.account_type == "SCA"
                    && network.state == "LIVE"
            })
        })
        .collect()
}

fn dedupe_wallet_networks(rows: Vec<WalletNetwork>) -> Vec<WalletNetwork> {
    let mut deduped = Vec::with_capacity(rows.len());
    for blockchain in SUPPORTED_WALLET_BLOCKCHAINS {
        if let Some(row) = rows
            .iter()
            .find(|network| network.blockchain == blockchain)
            .cloned()
        {
            deduped.push(row);
        }
    }
    for row in rows {
        if !SUPPORTED_WALLET_BLOCKCHAINS.contains(&row.blockchain.as_str())
            && !deduped
                .iter()
                .any(|network| network.blockchain == row.blockchain)
        {
            deduped.push(row);
        }
    }
    deduped
}

fn route_idempotency_key(user_id: Uuid, blockchains: &[&'static str]) -> String {
    let seed = format!("{}:wallet-routes-v2:{}", user_id, blockchains.join(","));
    Uuid::new_v5(&Uuid::NAMESPACE_URL, seed.as_bytes()).to_string()
}

fn user_ref_id(user_id: Uuid) -> String {
    user_id.to_string()
}

pub fn encrypt_entity_secret(
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
            networks: SUPPORTED_WALLET_BLOCKCHAINS
                .iter()
                .map(|blockchain| mock_network(seed, blockchain))
                .collect(),
            created_at: Utc::now(),
        }))
    }
}

fn mock_network(seed: u64, blockchain: &str) -> WalletNetwork {
    WalletNetwork {
        blockchain: blockchain.into(),
        wallet_id: format!(
            "mock_wallet_{seed:016x}:{}",
            blockchain.to_ascii_lowercase()
        ),
        address: match blockchain {
            ARC_TESTNET => format!("0xARC{seed:040x}"),
            BASE_SEPOLIA => format!("0xBASE{seed:040x}"),
            _ => format!("0x{seed:040x}"),
        },
        account_type: "SCA".into(),
        state: "LIVE".into(),
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
        assert_eq!(w.networks.len(), SUPPORTED_WALLET_BLOCKCHAINS.len());
        assert!(w
            .networks
            .iter()
            .any(|network| network.blockchain == "ETH-SEPOLIA"));
    }

    #[test]
    fn materialize_wallet_pairs_arc_and_base_rows() {
        let rows = vec![
            WalletNetwork {
                wallet_id: "circle-w-1".into(),
                address: "0xARC1".into(),
                blockchain: ARC_TESTNET.into(),
                account_type: "SCA".into(),
                state: "LIVE".into(),
            },
            WalletNetwork {
                wallet_id: "circle-w-2".into(),
                address: "0xBASE2".into(),
                blockchain: BASE_SEPOLIA.into(),
                account_type: "SCA".into(),
                state: "LIVE".into(),
            },
            WalletNetwork {
                wallet_id: "circle-w-3".into(),
                address: "0xEVM3".into(),
                blockchain: "ETH-SEPOLIA".into(),
                account_type: "SCA".into(),
                state: "LIVE".into(),
            },
        ];
        let w = materialize_wallet(&rows).unwrap();
        assert_eq!(w.arc_address, "0xARC1");
        assert_eq!(w.base_address, "0xBASE2");
        assert_eq!(w.wallet_id, "circle-w-1");
        assert_eq!(w.networks.len(), 3);
    }

    #[test]
    fn materialize_wallet_returns_none_when_missing_a_chain() {
        let rows = vec![WalletNetwork {
            wallet_id: "circle-w-1".into(),
            address: "0xARC1".into(),
            blockchain: ARC_TESTNET.into(),
            account_type: "SCA".into(),
            state: "LIVE".into(),
        }];
        assert!(materialize_wallet(&rows).is_none());
    }

    #[test]
    fn missing_supported_blockchains_reports_only_absent_live_sca_routes() {
        let rows = vec![
            WalletNetwork {
                wallet_id: "circle-w-1".into(),
                address: "0xARC1".into(),
                blockchain: ARC_TESTNET.into(),
                account_type: "SCA".into(),
                state: "LIVE".into(),
            },
            WalletNetwork {
                wallet_id: "circle-w-2".into(),
                address: "0xBASE2".into(),
                blockchain: BASE_SEPOLIA.into(),
                account_type: "SCA".into(),
                state: "LIVE".into(),
            },
            WalletNetwork {
                wallet_id: "circle-w-3".into(),
                address: "0xETH3".into(),
                blockchain: "ETH-SEPOLIA".into(),
                account_type: "EOA".into(),
                state: "LIVE".into(),
            },
        ];
        assert_eq!(
            missing_supported_blockchains(&rows),
            vec!["ETH-SEPOLIA", "ARB-SEPOLIA", "AVAX-FUJI"]
        );
    }

    #[test]
    fn route_idempotency_key_changes_with_missing_route_set() {
        let user_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
        assert_eq!(
            route_idempotency_key(user_id, &["ETH-SEPOLIA", "ARB-SEPOLIA"]),
            "6ad5c34a-524c-55c3-a0a1-4771fe992c7c"
        );
    }

    #[test]
    fn normalizes_public_key_body_to_pem() {
        let pem = normalize_public_key_pem("abc");
        assert!(pem.starts_with("-----BEGIN PUBLIC KEY-----"));
        assert!(pem.ends_with("-----END PUBLIC KEY-----"));
    }
}
