//! Non-custodial contract execution via Circle developer-controlled wallets
//! (Part B0).
//!
//! The default real-execution path signs swap/burn/mint transactions with a
//! backend EOA (`CHAIN_PRIVATE_KEY_*`) — custodial. This module routes the same
//! calldata through the *user's* Circle developer-controlled wallet using
//! Circle's Create-Contract-Execution-Transaction API (entity-secret signed),
//! so the user's wallet is both the tx sender and the holder of funds. It is
//! selected at runtime by `Config::circle_wallet_exec` and is otherwise inert.
//!
//! Reuse note: the entity-secret encryption (`encrypt_entity_secret`), bearer
//! auth, and `circle_base_url` base-URL pattern mirror
//! `super::provider::CircleProvider` exactly — the same secret + key flow that
//! provisions wallets.
//!
//! LIVE-VERIFICATION CAVEAT: this path compiles and is unit-tested for request
//! body shape + wallet-id resolution, but its end-to-end behavior REQUIRES live
//! Circle developer creds + a funded user wallet (this environment has none) —
//! the same caveat as the existing real CCTP/swap EOA paths. The endpoint paths
//! below follow Circle's documented W3S developer transactions API; if the
//! sandbox shape differs, only the constants here change.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::provider::encrypt_entity_secret;
use crate::config::Config;
use crate::db::Db;
use crate::error::{AppError, Result};
use crate::modules::rebalance::models::ChainKey;
use crate::modules::wallet_routes::{wallet_id_for_user, ARC_TESTNET, BASE_SEPOLIA};

/// Circle W3S developer-controlled wallet contract-execution endpoint
/// (documented `POST /v1/w3s/developer/transactions/contractExecution`). The
/// entity-secret ciphertext authorizes the user's wallet to sign + broadcast.
const CONTRACT_EXECUTION_PATH: &str = "/v1/w3s/developer/transactions/contractExecution";
/// Transaction-status endpoint (`GET /v1/w3s/transactions/{id}`); polled until
/// the on-chain tx is confirmed/complete or failed.
const TRANSACTION_STATUS_PATH: &str = "/v1/w3s/transactions";

/// Poll cadence + ceiling for the contract-execution status loop. Mirrors the
/// CCTP attestation poll (start small, cap the wall-clock wait).
const POLL_INTERVAL: Duration = Duration::from_secs(3);
const POLL_TIMEOUT: Duration = Duration::from_secs(180);

fn blockchain_for_chain(chain: ChainKey) -> &'static str {
    match chain {
        ChainKey::Arc => ARC_TESTNET,
        ChainKey::Base => BASE_SEPOLIA,
    }
}

/// Submit a contract-execution transaction from the user's Circle wallet on
/// `chain` and poll until it confirms, returning the on-chain tx hash.
///
/// `call_data_hex` is the ABI-encoded calldata for the target contract call
/// (0x-prefixed). `amount_native` is the native-token value to attach (USDC on
/// Arc; almost always `None`/0 for our ERC-20 + bridge calls). The user's
/// wallet is the sender and signer — non-custodial by construction.
#[allow(clippy::too_many_arguments)]
pub async fn submit_contract_execution(
    http: &reqwest::Client,
    cfg: &Config,
    db: &Db,
    user_id: Uuid,
    chain: ChainKey,
    contract_address: &str,
    call_data_hex: &str,
    amount_native: Option<u128>,
) -> Result<String> {
    require_dev_wallet_config(cfg)?;

    let blockchain = blockchain_for_chain(chain);
    let wallet_id = wallet_id_for_user(db, user_id, blockchain, &cfg.circle_wallet_set_id)
        .await?
        .ok_or_else(|| {
            AppError::ServiceUnavailable(format!(
                "no live Circle wallet for user on {blockchain}; cannot execute non-custodially"
            ))
        })?;

    let ciphertext = entity_secret_ciphertext(http, cfg).await?;
    let body = ContractExecutionReq {
        idempotency_key: Uuid::new_v4().to_string(),
        wallet_id,
        contract_address: contract_address.to_string(),
        call_data: normalize_hex(call_data_hex),
        amount: amount_native.map(native_amount_string),
        entity_secret_ciphertext: ciphertext,
        fee_level: "MEDIUM",
    };

    let url = format!("{}{}", cfg.circle_base_url, CONTRACT_EXECUTION_PATH);
    let response = http
        .post(&url)
        .header("Authorization", auth_header(cfg))
        .header("X-Request-Id", Uuid::new_v4().to_string())
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!("circle contractExecution network: {e}"))
        })?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("circle contractExecution body: {e}")))?;
    if !status.is_success() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "circle contractExecution: HTTP {status}: {text}"
        )));
    }
    let envelope: TransactionEnvelope = serde_json::from_str(&text)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("circle contractExecution decode: {e}")))?;

    poll_until_settled(http, cfg, &envelope.data.id).await
}

async fn poll_until_settled(
    http: &reqwest::Client,
    cfg: &Config,
    transaction_id: &str,
) -> Result<String> {
    let url = format!(
        "{}{}/{}",
        cfg.circle_base_url, TRANSACTION_STATUS_PATH, transaction_id
    );
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() >= POLL_TIMEOUT {
            return Err(AppError::Internal(anyhow::anyhow!(
                "circle contractExecution status poll timed out for {transaction_id}"
            )));
        }

        let envelope: TransactionEnvelope = http
            .get(&url)
            .header("Authorization", auth_header(cfg))
            .header("X-Request-Id", Uuid::new_v4().to_string())
            .send()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle tx status network: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle tx status: {e}")))?
            .json()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle tx status decode: {e}")))?;

        let tx = envelope.data;
        match tx.state.as_deref() {
            // Circle marks broadcast txs CONFIRMED, then COMPLETE on finality.
            // The tx hash is available as soon as it's on-chain.
            Some("CONFIRMED") | Some("COMPLETE") => {
                return tx.tx_hash.ok_or_else(|| {
                    AppError::Internal(anyhow::anyhow!(
                        "circle reported {transaction_id} settled but returned no txHash"
                    ))
                });
            }
            Some("FAILED") | Some("CANCELLED") | Some("DENIED") => {
                return Err(AppError::Internal(anyhow::anyhow!(
                    "circle contractExecution {transaction_id} ended in state {:?}",
                    tx.state
                )));
            }
            _ => {}
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

async fn entity_secret_ciphertext(http: &reqwest::Client, cfg: &Config) -> Result<String> {
    #[derive(Deserialize)]
    struct PublicKeyEnvelope {
        data: PublicKeyData,
    }
    #[derive(Deserialize)]
    struct PublicKeyData {
        #[serde(rename = "publicKey")]
        public_key: String,
    }

    let url = format!("{}/v1/w3s/config/entity/publicKey", cfg.circle_base_url);
    let envelope: PublicKeyEnvelope = http
        .get(&url)
        .header("Authorization", auth_header(cfg))
        .header("X-Request-Id", Uuid::new_v4().to_string())
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("circle public_key network: {e}")))?
        .error_for_status()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("circle public_key: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("circle public_key decode: {e}")))?;

    encrypt_entity_secret(&cfg.circle_entity_secret, &envelope.data.public_key)
}

fn auth_header(cfg: &Config) -> String {
    format!("Bearer {}", cfg.circle_api_key)
}

fn require_dev_wallet_config(cfg: &Config) -> Result<()> {
    if cfg.circle_wallet_set_id.trim().is_empty() {
        return Err(AppError::ServiceUnavailable(
            "circle developer wallet set is not configured".into(),
        ));
    }
    if cfg.circle_entity_secret.trim().is_empty() {
        return Err(AppError::ServiceUnavailable(
            "circle entity secret is not configured".into(),
        ));
    }
    Ok(())
}

fn normalize_hex(call_data_hex: &str) -> String {
    let trimmed = call_data_hex.trim();
    if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
        trimmed.to_string()
    } else {
        format!("0x{trimmed}")
    }
}

/// Circle expects the native-token value as a decimal string in the chain's
/// base unit (6dp for USDC-gas on Arc). Our ERC-20 + bridge calls attach no
/// native value, so this is exercised only if a caller passes `Some`.
fn native_amount_string(amount_native: u128) -> String {
    let whole = amount_native / 1_000_000;
    let frac = amount_native % 1_000_000;
    if frac == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{frac:06}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

#[derive(Serialize)]
struct ContractExecutionReq {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "walletId")]
    wallet_id: String,
    #[serde(rename = "contractAddress")]
    contract_address: String,
    #[serde(rename = "callData")]
    call_data: String,
    #[serde(rename = "amount", skip_serializing_if = "Option::is_none")]
    amount: Option<String>,
    #[serde(rename = "entitySecretCiphertext")]
    entity_secret_ciphertext: String,
    #[serde(rename = "feeLevel")]
    fee_level: &'static str,
}

#[derive(Deserialize)]
struct TransactionEnvelope {
    data: TransactionData,
}

#[derive(Deserialize)]
struct TransactionData {
    id: String,
    #[serde(default)]
    state: Option<String>,
    #[serde(rename = "txHash", default)]
    tx_hash: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_chain_to_supported_blockchain() {
        assert_eq!(blockchain_for_chain(ChainKey::Arc), ARC_TESTNET);
        assert_eq!(blockchain_for_chain(ChainKey::Base), BASE_SEPOLIA);
    }

    #[test]
    fn normalize_hex_adds_prefix_when_missing() {
        assert_eq!(normalize_hex("abcd"), "0xabcd");
        assert_eq!(normalize_hex("0xABCD"), "0xABCD");
        assert_eq!(normalize_hex("  0x1234  "), "0x1234");
    }

    #[test]
    fn native_amount_string_renders_usdc_6dp() {
        assert_eq!(native_amount_string(0), "0");
        assert_eq!(native_amount_string(1_000_000), "1");
        assert_eq!(native_amount_string(1_500_000), "1.5");
        assert_eq!(native_amount_string(250_000), "0.25");
    }

    #[test]
    fn contract_execution_body_uses_circle_camel_case_keys() {
        let body = ContractExecutionReq {
            idempotency_key: "id-1".into(),
            wallet_id: "wallet-1".into(),
            contract_address: "0xrouter".into(),
            call_data: "0xdeadbeef".into(),
            amount: None,
            entity_secret_ciphertext: "cipher".into(),
            fee_level: "MEDIUM",
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["idempotencyKey"], "id-1");
        assert_eq!(json["walletId"], "wallet-1");
        assert_eq!(json["contractAddress"], "0xrouter");
        assert_eq!(json["callData"], "0xdeadbeef");
        assert_eq!(json["entitySecretCiphertext"], "cipher");
        assert_eq!(json["feeLevel"], "MEDIUM");
        // `amount` is omitted (not null) when no native value is attached.
        assert!(json.get("amount").is_none());
    }

    #[test]
    fn contract_execution_body_includes_amount_when_present() {
        let body = ContractExecutionReq {
            idempotency_key: "id-2".into(),
            wallet_id: "wallet-2".into(),
            contract_address: "0xc".into(),
            call_data: "0x00".into(),
            amount: Some(native_amount_string(1_500_000)),
            entity_secret_ciphertext: "cipher".into(),
            fee_level: "MEDIUM",
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["amount"], "1.5");
    }

    #[test]
    fn transaction_envelope_decodes_state_and_hash() {
        let raw = r#"{"data":{"id":"tx-1","state":"CONFIRMED","txHash":"0xabc"}}"#;
        let env: TransactionEnvelope = serde_json::from_str(raw).unwrap();
        assert_eq!(env.data.id, "tx-1");
        assert_eq!(env.data.state.as_deref(), Some("CONFIRMED"));
        assert_eq!(env.data.tx_hash.as_deref(), Some("0xabc"));
    }

    #[test]
    fn transaction_envelope_tolerates_pending_without_hash() {
        let raw = r#"{"data":{"id":"tx-2","state":"INITIATED"}}"#;
        let env: TransactionEnvelope = serde_json::from_str(raw).unwrap();
        assert_eq!(env.data.state.as_deref(), Some("INITIATED"));
        assert!(env.data.tx_hash.is_none());
    }
}
