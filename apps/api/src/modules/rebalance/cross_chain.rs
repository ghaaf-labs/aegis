//! CCTP V2 client.
//!
//! `deposit_for_burn` initiates a burn on the source chain; the planner
//! attaches our hook payload (recipient, tokenOut, fee, minOut, deadline) so
//! that on attestation, the destination-chain `RebalanceExecutor` performs
//! the swap atomically.
//!
//! In `execution_mock` mode (the default), all calls return deterministic
//! fixtures so unit and integration tests don't touch any RPC. Production
//! deployments flip the env var and supply real `CHAIN_PRIVATE_KEY_*` keys.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::error::{AppError, Result};
use crate::modules::rebalance::models::ChainKey;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HookPayload {
    /// Final wallet that receives `token_out` on the destination chain.
    pub recipient: String,
    /// ERC-20 to receive. If equal to USDC, the hook skips the Uniswap leg.
    pub token_out: String,
    /// Uniswap V3 pool fee tier (500 / 3000 / 10000).
    pub pool_fee: u32,
    /// Minimum output the swap must yield. Computed by the quoter with a
    /// 50bps slippage tolerance applied.
    pub min_out: u128,
    /// Hook expiry (unix seconds). Planner sets `now + 600`.
    pub deadline: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BurnReceipt {
    pub tx_hash: String,
    pub message_hash: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MintReceipt {
    pub tx_hash: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Attestation {
    pub message: String,
    pub attestation: String,
}

pub struct CctpClient<'a> {
    http: &'a reqwest::Client,
    config: &'a Config,
}

impl<'a> CctpClient<'a> {
    pub fn new(http: &'a reqwest::Client, config: &'a Config) -> Self {
        Self { http, config }
    }

    /// Burn `amount_usdc` USDC on `src` and mint the same amount on `dest`
    /// to `RebalanceExecutor`, which then invokes the hook payload.
    pub async fn deposit_for_burn(
        &self,
        src: ChainKey,
        dest: ChainKey,
        amount_usdc: f64,
        hook: &HookPayload,
    ) -> Result<BurnReceipt> {
        if self.config.execution_mock {
            return Ok(mock_burn_receipt(src, dest, amount_usdc, hook));
        }

        // Production path: build a signed transaction calling
        // `ICCTPV2TokenMessenger.depositForBurnWithCaller(...)` on `src`'s
        // TokenMessenger. Left as a TODO because the testnet sandbox doesn't
        // hand out the real ABI on hackathon day; the executor falls back to
        // mock receipts unless the operator explicitly opts in via env.
        Err(AppError::Internal(anyhow::anyhow!(
            "real CCTP burn submission not implemented — set EXECUTION_MOCK=true"
        )))
    }

    /// Poll Circle's attestation API until the message is attested or the
    /// timeout (default 180s) elapses. Backoff: 2s → 4s → 8s → 16s capped.
    ///
    /// `src_domain` is the CCTP V2 domain id of the chain that produced the
    /// burn. Circle's V2 endpoint is `/v2/messages/{srcDomain}/{messageHash}`
    /// — without the domain segment the API returns 404.
    pub async fn wait_for_attestation(
        &self,
        src_domain: u32,
        message_hash: &str,
    ) -> Result<Attestation> {
        if self.config.execution_mock {
            return Ok(mock_attestation(message_hash));
        }

        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(self.config.cctp_attestation_timeout_secs);
        let mut delay = Duration::from_secs(2);
        let max_delay = Duration::from_secs(16);

        loop {
            if start.elapsed() >= timeout {
                return Err(AppError::Internal(anyhow::anyhow!(
                    "cctp attestation timed out after {}s",
                    self.config.cctp_attestation_timeout_secs
                )));
            }

            let url = format!(
                "{}/v2/messages/{}/{}",
                self.config.cctp_attestation_url, src_domain, message_hash
            );
            let resp = self.http.get(&url).send().await;
            match resp {
                Ok(r) if r.status().is_success() => {
                    if let Ok(att) = r.json::<Attestation>().await {
                        return Ok(att);
                    }
                }
                Ok(_) => {
                    // 404 / 503 — attestation still being produced. Backoff.
                }
                Err(e) => {
                    tracing::debug!(error=%e, "attestation poll transient error");
                }
            }
            tokio::time::sleep(delay).await;
            delay = (delay * 2).min(max_delay);
        }
    }

    pub async fn receive_message(
        &self,
        dest: ChainKey,
        attestation: &Attestation,
    ) -> Result<MintReceipt> {
        if self.config.execution_mock {
            return Ok(MintReceipt {
                tx_hash: mock_tx_hash("mint", dest.as_str(), &attestation.message),
            });
        }
        Err(AppError::Internal(anyhow::anyhow!(
            "real CCTP receiveMessage not implemented — set EXECUTION_MOCK=true"
        )))
    }
}

/// Build the on-chain hook payload (160 bytes) the destination
/// `RebalanceExecutor.handleReceiveMessage` decodes.
pub fn build_hook_payload(
    recipient: &str,
    token_out: &str,
    pool_fee: u32,
    min_out: u128,
    deadline: u64,
) -> HookPayload {
    HookPayload {
        recipient: recipient.to_string(),
        token_out: token_out.to_string(),
        pool_fee,
        min_out,
        deadline,
    }
}

fn mock_tx_hash(kind: &str, chain: &str, salt: &str) -> String {
    let mut h = Sha256::new();
    h.update(kind.as_bytes());
    h.update(b":");
    h.update(chain.as_bytes());
    h.update(b":");
    h.update(salt.as_bytes());
    format!("0x{}", hex::encode(h.finalize()))
}

fn mock_burn_receipt(
    src: ChainKey,
    dest: ChainKey,
    amount: f64,
    hook: &HookPayload,
) -> BurnReceipt {
    let salt = format!(
        "{}->{}:{}:{}",
        src.as_str(),
        dest.as_str(),
        amount,
        hook.recipient
    );
    let mut h = Sha256::new();
    h.update(b"msg:");
    h.update(salt.as_bytes());
    let message_hash = format!("0x{}", hex::encode(h.finalize()));
    BurnReceipt {
        tx_hash: mock_tx_hash("burn", src.as_str(), &salt),
        message_hash,
    }
}

fn mock_attestation(message_hash: &str) -> Attestation {
    Attestation {
        message: format!("0x{}", hex::encode(message_hash.as_bytes())),
        attestation: format!(
            "0x{}",
            hex::encode(format!("att:{message_hash}").as_bytes())
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn cfg() -> Config {
        // Reuse the test_config helper indirectly by constructing minimal env.
        let mut c = crate::config::Config {
            database_url: "postgres://test".into(),
            jwt_secret: "secret".into(),
            jwt_expiry_hours: 24,
            host: "0.0.0.0".into(),
            port: 8080,
            openrouter_api_key: "t".into(),
            openrouter_base_url: "https://or".into(),
            model_regime: "r".into(),
            model_strategist: "s".into(),
            model_critic: "c".into(),
            model_tax: "t".into(),
            model_commentary: "x".into(),
            openrouter_app_name: "Aegis".into(),
            openrouter_app_url: None,
            coingecko_api_key: None,
            sse_price_tick_secs: 5,
            circle_api_key: "k".into(),
            circle_base_url: "https://circle".into(),
            circle_env: "sandbox".into(),
            circle_mock: true,
            arc_rpc_url: "https://arc".into(),
            base_rpc_url: "https://base".into(),
            gateway_poll_secs: 10,
            faucet_max_usdc_per_day: 100.0,
            cors_allow_origin: "*".into(),
            session_cookie_name: "j".into(),
            session_cookie_secure: false,
            cctp_attestation_url: "https://iris".into(),
            cctp_attestation_timeout_secs: 180,
            chain_private_key_arc: String::new(),
            chain_private_key_base: String::new(),
            execution_mock: true,
            scheduler_tick_secs: 300,
            scheduler_cooldown_secs: 1800,
            harvest_threshold_usd: 50.0,
            digest_hour_utc: 8,
            resend_api_key: String::new(),
            digest_from: "x".into(),
            digest_secret: "s".into(),
            public_base_url: "http://localhost:3000".into(),
            api_base_url: "http://localhost:8080".into(),
        };
        c.execution_mock = true;
        c
    }

    #[tokio::test]
    async fn mock_burn_is_deterministic() {
        let http = reqwest::Client::new();
        let cfg = cfg();
        let client = CctpClient::new(&http, &cfg);
        let hook = build_hook_payload("0xabc", "0xdef", 3000, 1_000_000, 1_700_000_000);

        let r1 = client
            .deposit_for_burn(ChainKey::Arc, ChainKey::Base, 100.0, &hook)
            .await
            .unwrap();
        let r2 = client
            .deposit_for_burn(ChainKey::Arc, ChainKey::Base, 100.0, &hook)
            .await
            .unwrap();
        assert_eq!(r1, r2, "mock burn must be deterministic");
        assert!(r1.tx_hash.starts_with("0x"));
        assert!(r1.message_hash.starts_with("0x"));
    }

    #[tokio::test]
    async fn mock_attestation_roundtrips() {
        let http = reqwest::Client::new();
        let cfg = cfg();
        let client = CctpClient::new(&http, &cfg);
        let hook = build_hook_payload("0xabc", "0xdef", 3000, 1, 1);
        let burn = client
            .deposit_for_burn(ChainKey::Arc, ChainKey::Base, 50.0, &hook)
            .await
            .unwrap();
        let att = client
            .wait_for_attestation(ChainKey::Arc.domain_id(), &burn.message_hash)
            .await
            .unwrap();
        assert!(!att.message.is_empty());
        assert!(!att.attestation.is_empty());
        let mint = client.receive_message(ChainKey::Base, &att).await.unwrap();
        assert!(mint.tx_hash.starts_with("0x"));
    }

    #[test]
    fn build_hook_payload_sets_fields() {
        let h = build_hook_payload("0xrecipient", "0xeth", 3000, 999, 1_700_000_600);
        assert_eq!(h.recipient, "0xrecipient");
        assert_eq!(h.token_out, "0xeth");
        assert_eq!(h.pool_fee, 3000);
        assert_eq!(h.min_out, 999);
        assert_eq!(h.deadline, 1_700_000_600);
    }
}
