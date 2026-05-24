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
use crate::modules::wallet_routes::wallet_id_for_user;

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
/// How many *consecutive* unparsable / transient status responses we tolerate
/// before failing the leg. Circle's status endpoint occasionally returns a
/// truncated body or an alternate envelope shape mid-flight; a single blip must
/// not brick an in-flight rebalance. A genuine `FAILED`/`CANCELLED` state is
/// always surfaced immediately — this retry only covers responses we could not
/// read at all.
const MAX_DECODE_RETRIES: u32 = 3;

/// Wall-clock ceiling for the post-funding balance confirmation (B1). A CCTP V2
/// burn → attestation → mint round-trip, plus Circle's balance indexer lag, can
/// take a couple of minutes on testnet; this matches the CCTP attestation
/// budget so a dependent swap waits long enough rather than racing the credit.
const FUNDING_WAIT_TIMEOUT: Duration = Duration::from_secs(180);
/// Tolerance on the awaited amount, covering CCTP transfer fees + 6dp rounding
/// so a standard transfer that delivers ~99.9% of the notional still satisfies
/// the wait (the swap is sized off the same notional and clamps to balance).
const FUNDING_WAIT_TOLERANCE: f64 = 0.99;

fn blockchain_for_chain(chain: ChainKey) -> &'static str {
    crate::modules::wallet_routes::blockchain_for_chain(chain)
}

/// Bounded poll: wait until the user's Circle wallet holds at least
/// `min_amount` USDC (within `FUNDING_WAIT_TOLERANCE`) on `chain`.
///
/// Closes the multi-leg race (B1) where a dependent swap is submitted before
/// the bridged/minted USDC has credited the destination wallet — Circle marks
/// the mint `CONFIRMED` before its balance indexer reflects the new USDC, so
/// the swap would spend funds that aren't there yet and Circle returns `FAILED`.
/// Errors (so the plan halts cleanly) if the balance never reaches the target
/// within the timeout; the bridged USDC remains visible as Gateway cash.
pub async fn wait_for_usdc_balance(
    http: &reqwest::Client,
    cfg: &Config,
    db: &Db,
    user_id: Uuid,
    chain: ChainKey,
    min_amount: f64,
) -> Result<()> {
    if min_amount <= 0.0 {
        return Ok(());
    }
    let target = min_amount * FUNDING_WAIT_TOLERANCE;
    let start = std::time::Instant::now();
    let mut last_seen = 0.0_f64;
    loop {
        match crate::modules::gateway::service::fetch_chain_usdc(http, cfg, db, user_id, chain)
            .await
        {
            Ok(balance) => {
                last_seen = balance;
                if balance >= target {
                    return Ok(());
                }
            }
            Err(e) => {
                // A transient balance-read failure should not abort the wait; the
                // timeout below is the real backstop.
                tracing::warn!(
                    chain = chain.as_str(),
                    error = %e,
                    "funding wait: balance read failed; retrying"
                );
            }
        }
        if start.elapsed() >= FUNDING_WAIT_TIMEOUT {
            return Err(AppError::Internal(anyhow::anyhow!(
                "bridged USDC did not credit on {} in time (need {:.2}, saw {:.2}); funds remain as Gateway cash for a follow-up plan",
                chain.as_str(),
                min_amount,
                last_seen
            )));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Submit a contract-execution transaction from the user's Circle wallet on
/// `chain` and poll until it confirms, returning the on-chain tx hash.
///
/// `call_data_hex` is the ABI-encoded calldata for the target contract call
/// (0x-prefixed). `amount_native` is the native-token value to attach (USDC on
/// Arc; almost always `None`/0 for our ERC-20 + bridge calls). The user's
/// wallet is the sender and signer — non-custodial by construction.
///
/// `idempotency_key` MUST be deterministic and stable across resumes/retries of
/// the *same* logical call (e.g. derived from the leg id + step). Circle dedupes
/// on it, so a re-submit after a lost HTTP response — or a resumed walk that
/// re-dispatches a `submitted` leg — returns the original transaction instead of
/// broadcasting a duplicate. A random key here would let a single approve/burn/
/// swap execute twice.
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
    idempotency_key: &str,
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
        idempotency_key: deterministic_idempotency_key(idempotency_key),
        wallet_id,
        contract_address: contract_address.to_string(),
        call_data: normalize_hex(call_data_hex),
        amount: amount_native.map(native_amount_string),
        entity_secret_ciphertext: ciphertext,
        // GAS: `feeLevel` only picks the priority tier — it does not sponsor gas.
        // The user's Circle wallet must be able to pay: on Arc gas is native USDC
        // (which the wallet already holds), but on Base/Eth/Arb/Avax the wallet
        // needs native gas, sponsored via Circle Gas Station policy configured on
        // the wallet set. Without that, the first approve/burn on a non-Arc chain
        // fails for lack of gas — a deployment prerequisite, not a code path here.
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
    // Count *consecutive* responses we couldn't read (network blip or
    // unparsable body). Reset to zero on any successfully-decoded poll.
    let mut consecutive_decode_failures: u32 = 0;

    loop {
        if start.elapsed() >= POLL_TIMEOUT {
            return Err(AppError::Internal(anyhow::anyhow!(
                "circle contractExecution status poll timed out for {transaction_id}"
            )));
        }

        // Fetch the raw body first so a transient/alternate shape is a tolerated
        // decode failure, not an instant leg failure. A genuine FAILED state is
        // still surfaced immediately below once we *can* read the body.
        let raw_body = match fetch_status_body(http, cfg, &url).await {
            Ok(body) => body,
            Err(transient) => {
                consecutive_decode_failures += 1;
                if consecutive_decode_failures >= MAX_DECODE_RETRIES {
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "circle tx status unreadable for {transaction_id} after {MAX_DECODE_RETRIES} attempts: {transient}"
                    )));
                }
                tracing::warn!(
                    %transaction_id,
                    attempt = consecutive_decode_failures,
                    error = %transient,
                    "circle tx status transient read failure; retrying"
                );
                tokio::time::sleep(POLL_INTERVAL).await;
                continue;
            }
        };

        let tx = match parse_transaction_status(&raw_body) {
            Some(tx) => {
                consecutive_decode_failures = 0;
                tx
            }
            None => {
                consecutive_decode_failures += 1;
                if consecutive_decode_failures >= MAX_DECODE_RETRIES {
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "circle tx status body unparsable for {transaction_id} after {MAX_DECODE_RETRIES} attempts"
                    )));
                }
                tracing::warn!(
                    %transaction_id,
                    attempt = consecutive_decode_failures,
                    "circle tx status body unparsable; retrying"
                );
                tokio::time::sleep(POLL_INTERVAL).await;
                continue;
            }
        };

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
                let reason = tx
                    .error_reason
                    .as_deref()
                    .map(|r| format!(" ({r})"))
                    .unwrap_or_default();
                return Err(AppError::Internal(anyhow::anyhow!(
                    "circle contractExecution {transaction_id} ended in state {:?}{reason}",
                    tx.state
                )));
            }
            _ => {}
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Fetch the raw status-endpoint body, treating network errors and non-success
/// HTTP codes as transient (the caller retries up to `MAX_DECODE_RETRIES`).
async fn fetch_status_body(http: &reqwest::Client, cfg: &Config, url: &str) -> Result<String> {
    let resp = http
        .get(url)
        .header("Authorization", auth_header(cfg))
        .header("X-Request-Id", Uuid::new_v4().to_string())
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("circle tx status network: {e}")))?
        .error_for_status()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("circle tx status http: {e}")))?;
    resp.text()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("circle tx status body read: {e}")))
}

/// Parse a status body into the inner transaction. Tolerates the documented
/// `{data:{transaction:{…}}}` shape and an alternate `{data:{…}}` shape some
/// sandbox responses use (the contractExecution POST shape). `None` ⇒ the body
/// could not be read as either, so the caller retries.
fn parse_transaction_status(body: &str) -> Option<TransactionData> {
    if let Ok(env) = serde_json::from_str::<TransactionStatusEnvelope>(body) {
        return Some(env.data.transaction);
    }
    // Fall back to the flatter envelope (id/state/txHash directly under `data`).
    serde_json::from_str::<TransactionEnvelope>(body)
        .ok()
        .map(|env| env.data)
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

/// Map a caller's stable idempotency seed (e.g. `"<leg_id>:burn"`) to a
/// deterministic UUID — the format Circle's `idempotencyKey` expects. The same
/// seed always yields the same key, so a retried/resumed submit of the same
/// logical call dedupes at Circle instead of broadcasting a duplicate tx.
fn deterministic_idempotency_key(seed: &str) -> String {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, seed.as_bytes()).to_string()
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
    /// Circle's machine reason for a failed transaction (e.g.
    /// `INSUFFICIENT_FUNDS`, `EXECUTION_REVERTED`). Surfaced in the leg's
    /// failure reason so the ledger shows the real cause, not just "FAILED".
    #[serde(rename = "errorReason", default)]
    error_reason: Option<String>,
}

/// GET `/v1/w3s/transactions/{id}` nests the transaction under
/// `data.transaction`, unlike the contractExecution POST which returns the id
/// at `data.id`. Decode the status poll against this shape (reusing the POST
/// envelope here drops the whole body — `id` is "missing" — and stalls the leg).
#[derive(Deserialize)]
struct TransactionStatusEnvelope {
    data: TransactionStatusData,
}

#[derive(Deserialize)]
struct TransactionStatusData {
    transaction: TransactionData,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::wallet_routes::{ARC_TESTNET, BASE_SEPOLIA};

    #[test]
    fn idempotency_key_is_deterministic_per_seed() {
        // Same seed → same UUID, so a resumed/retried submit dedupes at Circle.
        let a = deterministic_idempotency_key("leg-1:cctp-burn");
        let b = deterministic_idempotency_key("leg-1:cctp-burn");
        assert_eq!(a, b);
        // Different step → different key, so approve and burn never collide.
        assert_ne!(a, deterministic_idempotency_key("leg-1:cctp-approve"));
        // Different leg → different key.
        assert_ne!(a, deterministic_idempotency_key("leg-2:cctp-burn"));
        // Circle expects a UUID-format key.
        assert_eq!(a.len(), 36);
    }

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

    #[test]
    fn parse_transaction_status_reads_nested_shape() {
        let raw = r#"{"data":{"transaction":{"id":"tx-1","state":"CONFIRMED","txHash":"0xabc"}}}"#;
        let tx = parse_transaction_status(raw).expect("nested shape parses");
        assert_eq!(tx.state.as_deref(), Some("CONFIRMED"));
        assert_eq!(tx.tx_hash.as_deref(), Some("0xabc"));
    }

    #[test]
    fn parse_transaction_status_falls_back_to_flat_shape() {
        // Some sandbox responses return the flatter POST-style envelope.
        let raw = r#"{"data":{"id":"tx-9","state":"COMPLETE","txHash":"0xdef"}}"#;
        let tx = parse_transaction_status(raw).expect("flat shape parses");
        assert_eq!(tx.state.as_deref(), Some("COMPLETE"));
        assert_eq!(tx.tx_hash.as_deref(), Some("0xdef"));
    }

    #[test]
    fn parse_transaction_status_returns_none_for_unparsable_body() {
        // A truncated / HTML / empty body is a transient read failure, not a
        // FAILED state — the caller retries rather than bricking the leg.
        assert!(parse_transaction_status("").is_none());
        assert!(parse_transaction_status("<html>502 Bad Gateway</html>").is_none());
        assert!(parse_transaction_status(r#"{"data":{"#).is_none());
    }
}
