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

#[cfg(feature = "real-cctp")]
use alloy::{
    primitives::{Address, Bytes, U256},
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
    sol,
    sol_types::SolValue,
};

#[cfg(feature = "real-cctp")]
sol! {
    #[sol(rpc)]
    #[allow(clippy::too_many_arguments)]
    interface ICCTPV2TokenMessenger {
        // CCTP V2 standard burn (no hook).
        function depositForBurn(
            uint256 amount,
            uint32 destinationDomain,
            bytes32 mintRecipient,
            address burnToken,
            bytes32 destinationCaller,
            uint256 maxFee,
            uint32 minFinalityThreshold
        ) external returns (uint64 nonce);

        // CCTP V2 hook-enabled burn. hookData is delivered to
        // RebalanceExecutor.handleReceiveMessage on the destination chain
        // for the atomic USDC -> tokenOut swap.
        function depositForBurnWithHook(
            uint256 amount,
            uint32 destinationDomain,
            bytes32 mintRecipient,
            address burnToken,
            bytes32 destinationCaller,
            uint256 maxFee,
            uint32 minFinalityThreshold,
            bytes calldata hookData
        ) external returns (uint64 nonce);

        event MessageSent(bytes message);
    }

    #[sol(rpc)]
    interface IMessageTransmitter {
        function receiveMessage(
            bytes calldata message,
            bytes calldata attestation
        ) external returns (bool success);
    }

    // USDC must be approve()'d to the TokenMessenger before depositForBurn,
    // or the contract reverts on its internal `transferFrom(sender, …)`.
    #[sol(rpc)]
    interface IERC20 {
        function approve(address spender, uint256 amount) external returns (bool);
        function allowance(address owner, address spender) external view returns (uint256);
    }

    // Hook payload exactly as the RebalanceExecutor expects in handleReceiveMessage
    struct HookExecutionPayload {
        address recipient;
        address tokenOut;
        uint24 poolFee;
        uint256 minOut;
        uint256 deadline;
    }
}

/// Encodes a Rust HookPayload into the exact 160-byte ABI encoding expected by RebalanceExecutor.handleReceiveMessage
#[cfg(feature = "real-cctp")]
pub fn encode_hook_payload(hook: &HookPayload) -> Bytes {
    let payload = HookExecutionPayload {
        recipient: hook.recipient.parse().expect("valid recipient address"),
        tokenOut: hook.token_out.parse().expect("valid tokenOut address"),
        poolFee: hook.pool_fee.try_into().expect("poolFee fits in uint24"),
        minOut: U256::from(hook.min_out),
        deadline: U256::from(hook.deadline),
    };
    payload.abi_encode().into()
}

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

#[derive(Debug, Deserialize)]
struct IrisV2Envelope {
    messages: Vec<IrisV2Message>,
}

#[derive(Debug, Deserialize)]
struct IrisV2Message {
    attestation: Option<String>,
    message: Option<String>,
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

        #[cfg(not(feature = "real-cctp"))]
        {
            let _ = (src, dest, amount_usdc, hook);
            Err(AppError::Internal(anyhow::anyhow!(
                "real-cctp feature not enabled. Build with --features real-cctp and set EXECUTION_MOCK=false"
            )))
        }

        #[cfg(feature = "real-cctp")]
        {
            self.real_deposit_for_burn(src, dest, amount_usdc, hook)
                .await
        }
    }

    /// Poll Circle's attestation API until the message is attested or the
    /// timeout (default 180s) elapses. Backoff: 2s → 4s → 8s → 16s capped.
    ///
    /// `src_domain` is the CCTP V2 domain id of the chain that produced the
    /// burn. Circle's V2 endpoint is `/v2/messages/{srcDomain}/{messageHash}`
    /// — without the domain segment the API returns 404.
    #[cfg(feature = "real-cctp")]
    async fn real_deposit_for_burn(
        &self,
        src: ChainKey,
        dest: ChainKey,
        amount_usdc: f64,
        hook: &HookPayload,
    ) -> Result<BurnReceipt> {
        use alloy::network::EthereumWallet;

        let amount = (amount_usdc * 1_000_000.0) as u128;

        let private_key = match src {
            ChainKey::Arc => &self.config.chain_private_key_arc,
            ChainKey::Base => &self.config.chain_private_key_base,
        };

        let key_bytes = hex::decode(private_key.trim_start_matches("0x")).map_err(|_| {
            AppError::Internal(anyhow::anyhow!("invalid hex private key for {:?}", src))
        })?;
        let signer = PrivateKeySigner::from_slice(&key_bytes).map_err(|_| {
            AppError::Internal(anyhow::anyhow!("invalid private key for {:?}", src))
        })?;

        let wallet = EthereumWallet::from(signer);

        let rpc_url = match src {
            ChainKey::Arc => &self.config.arc_rpc_url,
            ChainKey::Base => &self.config.base_rpc_url,
        };

        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(
                rpc_url
                    .parse()
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("bad rpc url: {e}")))?,
            );

        // Choose addresses based on source chain
        let (token_messenger, usdc, executor_on_dest) = match src {
            ChainKey::Arc => (
                self.config
                    .cctp_token_messenger_arc
                    .parse::<Address>()
                    .map_err(|_| {
                        AppError::Internal(anyhow::anyhow!("bad CCTP TokenMessenger on Arc"))
                    })?,
                self.config
                    .usdc_arc
                    .parse::<Address>()
                    .map_err(|_| AppError::Internal(anyhow::anyhow!("bad USDC on Arc")))?,
                self.config
                    .rebalance_executor_base
                    .parse::<Address>()
                    .map_err(|_| {
                        AppError::Internal(anyhow::anyhow!("bad RebalanceExecutor on Base"))
                    })?,
            ),
            ChainKey::Base => (
                self.config
                    .cctp_token_messenger_base
                    .parse::<Address>()
                    .map_err(|_| {
                        AppError::Internal(anyhow::anyhow!("bad CCTP TokenMessenger on Base"))
                    })?,
                self.config
                    .usdc_base
                    .parse::<Address>()
                    .map_err(|_| AppError::Internal(anyhow::anyhow!("bad USDC on Base")))?,
                self.config
                    .rebalance_executor_arc
                    .parse::<Address>()
                    .map_err(|_| {
                        AppError::Internal(anyhow::anyhow!("bad RebalanceExecutor on Arc"))
                    })?,
            ),
        };

        // depositForBurn calls `USDC.transferFrom(msg.sender, tokenMessenger,
        // amount + fee)` internally — even with maxFee=0 the contract may
        // round-trip through a fee branch, so approve with headroom rather
        // than the exact amount.
        let usdc_token = IERC20::new(usdc, &provider);
        let approve_amount = U256::from(amount).saturating_mul(U256::from(2u64));
        let _approve_receipt = usdc_token
            .approve(token_messenger, approve_amount)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("USDC approve send error: {e}"))?
            .get_receipt()
            .await
            .map_err(|e| anyhow::anyhow!("USDC approve receipt error: {e}"))?;

        let contract = ICCTPV2TokenMessenger::new(token_messenger, &provider);

        // The 160-byte hook payload is forwarded by the MessageTransmitter to
        // RebalanceExecutor.handleReceiveMessage on the destination, which
        // decodes it and performs the atomic USDC -> tokenOut swap.
        let hook_data = encode_hook_payload(hook);

        // CCTP V2 finality threshold: 2000 = standard finality (~13min on
        // Base). 1000 (Fast Transfer) is sub-30s but requires a non-zero
        // maxFee — Circle sandbox returns delayReason="insufficient_fee"
        // when maxFee=0. Standard is free and adequate for the rebalance
        // cadence we run.
        const MIN_FINALITY_STANDARD: u32 = 2000;
        let max_fee = U256::ZERO;

        let receipt = contract
            .depositForBurnWithHook(
                U256::from(amount),
                dest.domain_id(),
                executor_on_dest.into_word(),
                usdc,
                executor_on_dest.into_word(),
                max_fee,
                MIN_FINALITY_STANDARD,
                hook_data,
            )
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("alloy send error: {e}"))?
            .get_receipt()
            .await
            .map_err(|e| anyhow::anyhow!("get_receipt error: {e}"))?;

        // Robust MessageSent extraction (works across Alloy Log type differences)
        let message_sent_topic: alloy::primitives::B256 =
            alloy::primitives::keccak256("MessageSent(bytes)");

        let message_hash = receipt
            .inner
            .logs()
            .iter()
            .find_map(|log| {
                if log.topics().first() == Some(&message_sent_topic) {
                    // The message is in the first (and only) topic or data depending on indexing.
                    // For CCTP MessageSent, the message is usually in the data.
                    let data = &log.data().data;
                    if !data.is_empty() {
                        Some(hex::encode(Sha256::digest(data)))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                AppError::Internal(anyhow::anyhow!("MessageSent event not found in receipt"))
            })?;

        Ok(BurnReceipt {
            tx_hash: receipt.transaction_hash.to_string(),
            message_hash,
        })
    }

    #[cfg(feature = "real-cctp")]
    pub async fn real_receive_message(
        &self,
        dest: ChainKey,
        message: &str,
        attestation: &str,
    ) -> Result<String> {
        use alloy::network::EthereumWallet;

        let private_key = match dest {
            ChainKey::Arc => &self.config.chain_private_key_arc,
            ChainKey::Base => &self.config.chain_private_key_base,
        };

        let signer = PrivateKeySigner::from_slice(
            &hex::decode(private_key.trim_start_matches("0x"))
                .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid hex key")))?,
        )
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid private key")))?;

        let wallet = EthereumWallet::from(signer);

        let rpc_url = match dest {
            ChainKey::Arc => &self.config.arc_rpc_url,
            ChainKey::Base => &self.config.base_rpc_url,
        };

        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(
                rpc_url
                    .parse()
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("bad rpc url: {e}")))?,
            );

        let transmitter: Address = match dest {
            ChainKey::Arc => self
                .config
                .cctp_message_transmitter_arc
                .parse()
                .map_err(|_| {
                    AppError::Internal(anyhow::anyhow!("bad MessageTransmitter on Arc"))
                })?,
            ChainKey::Base => self
                .config
                .cctp_message_transmitter_base
                .parse()
                .map_err(|_| {
                    AppError::Internal(anyhow::anyhow!("bad MessageTransmitter on Base"))
                })?,
        };

        let contract = IMessageTransmitter::new(transmitter, &provider);

        let message_bytes: alloy::primitives::Bytes = message
            .parse()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid message hex")))?;
        let attestation_bytes: alloy::primitives::Bytes = attestation
            .parse()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid attestation hex")))?;

        let receipt = contract
            .receiveMessage(message_bytes, attestation_bytes)
            .send()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("receiveMessage send error: {e}")))?
            .get_receipt()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("get_receipt error: {e}")))?;

        Ok(receipt.transaction_hash.to_string())
    }

    pub async fn wait_for_attestation(
        &self,
        src_domain: u32,
        burn_tx_hash: &str,
    ) -> Result<Attestation> {
        if self.config.execution_mock {
            return Ok(mock_attestation(burn_tx_hash));
        }

        // CCTP V2 attestations are looked up by source-domain + burn tx
        // hash: GET /v2/messages/{domain}?transactionHash={tx}. The
        // response wraps zero-or-more messages; we poll until exactly one
        // has `attestation != "PENDING"` and a non-null `message`.
        let url = format!(
            "{}/v2/messages/{}?transactionHash={}",
            self.config.cctp_attestation_url, src_domain, burn_tx_hash
        );

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

            match self.http.get(&url).send().await {
                Ok(r) if r.status().is_success() => {
                    if let Ok(env) = r.json::<IrisV2Envelope>().await {
                        if let Some(m) = env.messages.into_iter().next() {
                            if let (Some(msg), Some(att)) = (m.message, m.attestation) {
                                if att != "PENDING" && !msg.is_empty() {
                                    return Ok(Attestation {
                                        message: msg,
                                        attestation: att,
                                    });
                                }
                            }
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::debug!(error = %e, "attestation poll transient error");
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

        #[cfg(not(feature = "real-cctp"))]
        {
            let _ = (dest, attestation);
            Err(AppError::Internal(anyhow::anyhow!(
                "real-cctp feature not enabled. Build with --features real-cctp and set EXECUTION_MOCK=false"
            )))
        }

        #[cfg(feature = "real-cctp")]
        {
            let tx_hash = self
                .real_receive_message(dest, &attestation.message, &attestation.attestation)
                .await?;
            Ok(MintReceipt { tx_hash })
        }
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
            regime_backtest_enabled: false,
            // New real-execution + Nanopayments fields (defaults for tests)
            cctp_token_messenger_arc: String::new(),
            cctp_token_messenger_base: String::new(),
            cctp_message_transmitter_arc: String::new(),
            cctp_message_transmitter_base: String::new(),
            rebalance_executor_arc: String::new(),
            rebalance_executor_base: String::new(),
            usdc_arc: String::new(),
            usdc_base: String::new(),
            usyc_token_arc: String::new(),
            usyc_teller_arc: String::new(),
            usyc_oracle_arc: String::new(),
            nanopayments_facilitator_url: "https://gateway-api-testnet.circle.com".into(),
            nanopayments_seller_address: String::new(),
            nanopayments_treasury_address: String::new(),
            billing_v2_enabled: false,
            peg_defense_enabled: false,
            peg_monitor_tick_secs: 10,
            peg_fire_cooldown_secs: 1800,
            tax_export_v1_enabled: false,
            aum_stream_enabled: false,
            calibrated_conf_enabled: false,
            constitution_enabled: false,
            openrouter_budget_guard_usd: 0.05,
            stablefx_institutional_access: false,
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
            .wait_for_attestation(ChainKey::Arc.domain_id(), &burn.tx_hash)
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

    #[cfg(feature = "real-cctp")]
    #[test]
    fn encode_hook_payload_produces_160_byte_abi() {
        let hook = build_hook_payload(
            "0x1234567890123456789012345678901234567890",
            "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", // USDC
            3000,
            1_000_000_000_000_000_000u128,
            1_700_000_000,
        );
        let encoded = encode_hook_payload(&hook);
        // HookExecutionPayload is 5 words (160 bytes) exactly.
        assert_eq!(
            encoded.len(),
            160,
            "HookExecutionPayload must be exactly 160 bytes for RebalanceExecutor"
        );
        // First 32 bytes should be the recipient address left-padded.
        assert_eq!(
            &encoded[12..32],
            &hex::decode("1234567890123456789012345678901234567890").unwrap()[..]
        );
    }
}
