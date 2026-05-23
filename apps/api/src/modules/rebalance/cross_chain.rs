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
    providers::{ProviderBuilder, WalletProvider},
    signers::local::PrivateKeySigner,
    sol,
    sol_types::{SolCall, SolValue},
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

/// CCTP V2 finality thresholds. 2000 = standard finality (~13min on Base, free).
/// 1000 = Fast Transfer (sub-30s) but requires a non-zero `maxFee`.
#[cfg_attr(not(any(feature = "real-cctp", test)), allow(dead_code))]
const MIN_FINALITY_STANDARD: u32 = 2000;
#[cfg_attr(not(any(feature = "real-cctp", test)), allow(dead_code))]
const MIN_FINALITY_FAST: u32 = 1000;

/// One row of Circle's `/v2/burn/USDC/fees/{src}/{dest}` response: the
/// `minimumFee` (bps) charged for a burn at the given `finalityThreshold`.
#[cfg_attr(not(any(feature = "real-cctp", test)), allow(dead_code))]
#[derive(Debug, Clone, Deserialize, PartialEq)]
struct CctpFeeEntry {
    #[serde(rename = "finalityThreshold")]
    finality_threshold: u32,
    #[serde(rename = "minimumFee")]
    minimum_fee: u32,
}

/// The chosen burn parameters: a finality threshold plus the fee (in bps) Circle
/// charges at it. `fee_bps == 0` is the standard, free path.
#[cfg_attr(not(any(feature = "real-cctp", test)), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq)]
struct BurnFeeChoice {
    finality_threshold: u32,
    fee_bps: u32,
}

impl BurnFeeChoice {
    #[cfg_attr(not(any(feature = "real-cctp", test)), allow(dead_code))]
    const STANDARD: Self = Self {
        finality_threshold: MIN_FINALITY_STANDARD,
        fee_bps: 0,
    };
}

/// Select the Fast Transfer threshold + its quoted fee from Circle's fee table.
/// Falls back to the free standard path when no fast entry exists (so the
/// working path is never broken on a fee-table change). Never hardcodes a fee.
#[cfg_attr(not(any(feature = "real-cctp", test)), allow(dead_code))]
fn select_burn_fee(entries: &[CctpFeeEntry]) -> BurnFeeChoice {
    entries
        .iter()
        .find(|e| e.finality_threshold == MIN_FINALITY_FAST)
        .map(|e| BurnFeeChoice {
            finality_threshold: MIN_FINALITY_FAST,
            fee_bps: e.minimum_fee,
        })
        .unwrap_or(BurnFeeChoice::STANDARD)
}

/// Compute the absolute on-chain `maxFee` (USDC, 6 decimals) from a burn
/// `amount` and a fee in bps, rounding up so the burn never under-quotes.
#[cfg_attr(not(any(feature = "real-cctp", test)), allow(dead_code))]
fn max_fee_for(amount: u128, fee_bps: u32) -> u128 {
    if fee_bps == 0 {
        return 0;
    }
    amount
        .saturating_mul(fee_bps as u128)
        .div_ceil(10_000)
        .max(1)
}

pub struct CctpClient<'a> {
    http: &'a reqwest::Client,
    config: &'a Config,
    /// Execution context for the non-custodial path (Part B0). When set and
    /// `config.circle_wallet_exec` is true, burn/mint/approve are submitted from
    /// the user's Circle developer-controlled wallet instead of the backend EOA.
    /// Optional so the offline/mock tests construct a client without a DB.
    user: Option<UserExecContext<'a>>,
}

#[derive(Clone, Copy)]
struct UserExecContext<'a> {
    // Read only by the real-cctp non-custodial path; the default build attaches
    // the context but never dereferences it.
    #[cfg_attr(not(feature = "real-cctp"), allow(dead_code))]
    db: &'a crate::db::Db,
    #[cfg_attr(not(feature = "real-cctp"), allow(dead_code))]
    user_id: uuid::Uuid,
}

impl<'a> CctpClient<'a> {
    pub fn new(http: &'a reqwest::Client, config: &'a Config) -> Self {
        Self {
            http,
            config,
            user: None,
        }
    }

    /// Attach the owning user + DB so the non-custodial (`circle_wallet_exec`)
    /// path can resolve the user's Circle wallet as the tx sender.
    pub fn with_user(mut self, db: &'a crate::db::Db, user_id: uuid::Uuid) -> Self {
        self.user = Some(UserExecContext { db, user_id });
        self
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

        // Part B0 — non-custodial: submit the approve + burn from the user's
        // Circle developer-controlled wallet (entity-secret signed) instead of
        // the backend EOA. The user's wallet is the tx sender and holds the
        // funds. Falls through to the EOA path when the flag is off.
        if self.config.circle_wallet_exec {
            return self
                .circle_wallet_deposit_for_burn(src, dest, amount, hook)
                .await;
        }

        let private_key = self.config.chain_private_key_for(src);

        let key_bytes = hex::decode(private_key.trim_start_matches("0x")).map_err(|_| {
            AppError::Internal(anyhow::anyhow!("invalid hex private key for {:?}", src))
        })?;
        let signer = PrivateKeySigner::from_slice(&key_bytes).map_err(|_| {
            AppError::Internal(anyhow::anyhow!("invalid private key for {:?}", src))
        })?;

        let wallet = EthereumWallet::from(signer);

        let rpc_url = self.config.rpc_url_for(src);

        let provider = ProviderBuilder::new().wallet(wallet).connect_http(
            rpc_url
                .parse()
                .map_err(|e| AppError::Internal(anyhow::anyhow!("bad rpc url: {e}")))?,
        );

        // Source-chain TokenMessenger + USDC, and the destination-chain
        // RebalanceExecutor that becomes the CCTP mintRecipient.
        let token_messenger = self
            .config
            .cctp_token_messenger_for(src)
            .parse::<Address>()
            .map_err(|_| {
                AppError::Internal(anyhow::anyhow!("bad CCTP TokenMessenger on {:?}", src))
            })?;
        let usdc = self
            .config
            .usdc_for(src)
            .parse::<Address>()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("bad USDC on {:?}", src)))?;
        let executor_on_dest = self
            .config
            .rebalance_executor_for(dest)
            .parse::<Address>()
            .map_err(|_| {
                AppError::Internal(anyhow::anyhow!("bad RebalanceExecutor on {:?}", dest))
            })?;

        // depositForBurn calls `USDC.transferFrom(msg.sender, tokenMessenger,
        // amount + fee)` internally — even with maxFee=0 the contract may
        // round-trip through a fee branch, so approve with headroom rather
        // than the exact amount. Skip the approve if a previous run already
        // left sufficient allowance, both to save gas and to avoid the
        // pre-flight race some testnet RPCs hit when the approve receipt is
        // mined but eth_estimateGas reads stale state.
        let usdc_token = IERC20::new(usdc, &provider);
        let approve_amount = U256::from(amount).saturating_mul(U256::from(2u64));
        let signer_addr = provider.default_signer_address();
        let current_allowance = usdc_token
            .allowance(signer_addr, token_messenger)
            .call()
            .await
            .unwrap_or(U256::ZERO);
        if current_allowance < approve_amount {
            let _approve_receipt = usdc_token
                .approve(token_messenger, approve_amount)
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("USDC approve send error: {e}"))?
                .get_receipt()
                .await
                .map_err(|e| anyhow::anyhow!("USDC approve receipt error: {e}"))?;
            // Brief settle so depositForBurn's pre-flight sees the new
            // allowance even on testnet RPCs that lag a block.
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }

        let contract = ICCTPV2TokenMessenger::new(token_messenger, &provider);

        // The 160-byte hook payload is forwarded by the MessageTransmitter to
        // RebalanceExecutor.handleReceiveMessage on the destination, which
        // decodes it and performs the atomic USDC -> tokenOut swap.
        let hook_data = encode_hook_payload(hook);

        // Fast Transfer (threshold 1000) needs a non-zero maxFee fetched from
        // Circle's fee API — a zero fee on the fast path is rejected with
        // delayReason="insufficient_fee". Fall back to the free standard path
        // (threshold 2000, maxFee 0) when the fee API is unavailable.
        let fee_choice = self.resolve_burn_fee(src, dest).await;
        let max_fee = U256::from(max_fee_for(amount, fee_choice.fee_bps));

        // destinationCaller = bytes32(0) means any address can call
        // MessageTransmitter.receiveMessage on the destination chain.
        // The hook body (mintRecipient + swap params) is baked into the
        // message at burn time and cannot be manipulated by the relayer,
        // so unrestricted relay is safe for this flow. Setting this to
        // `executor_on_dest` (as we did initially) would require the
        // RebalanceExecutor contract to expose a function that forwards
        // to `receiveMessage` — it doesn't, so non-zero values here lock
        // the message out of any path to mint. See F-CCTP-5.
        let destination_caller = alloy::primitives::FixedBytes::<32>::ZERO;

        // Plain USDC bridge (hook tokenOut == destination USDC): mint directly to
        // the recipient EOA via `depositForBurn` — no hook, no executor hop, so
        // funds cannot strand at the executor waiting on a `relay()` the CCTP
        // core never calls. The destination swap (if any) is a separate
        // `LocalSwap` leg (the two-leg baseline). A hooked burn (tokenOut !=
        // USDC) still routes to the executor for the atomic path.
        let plain_bridge = hook
            .token_out
            .eq_ignore_ascii_case(self.config.usdc_for(dest));

        let receipt = if plain_bridge {
            let recipient = hook.recipient.parse::<Address>().map_err(|_| {
                AppError::Internal(anyhow::anyhow!("bad mint recipient for plain bridge"))
            })?;
            contract
                .depositForBurn(
                    U256::from(amount),
                    dest.domain_id(),
                    recipient.into_word(),
                    usdc,
                    destination_caller,
                    max_fee,
                    fee_choice.finality_threshold,
                )
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("alloy send error: {e}"))?
                .get_receipt()
                .await
                .map_err(|e| anyhow::anyhow!("get_receipt error: {e}"))?
        } else {
            contract
                .depositForBurnWithHook(
                    U256::from(amount),
                    dest.domain_id(),
                    executor_on_dest.into_word(),
                    usdc,
                    destination_caller,
                    max_fee,
                    fee_choice.finality_threshold,
                    hook_data,
                )
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("alloy send error: {e}"))?
                .get_receipt()
                .await
                .map_err(|e| anyhow::anyhow!("get_receipt error: {e}"))?
        };

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

        // Part B0 — non-custodial: mint via the user's Circle wallet.
        if self.config.circle_wallet_exec {
            return self
                .circle_wallet_receive_message(dest, message, attestation)
                .await;
        }

        let private_key = self.config.chain_private_key_for(dest);

        let signer = PrivateKeySigner::from_slice(
            &hex::decode(private_key.trim_start_matches("0x"))
                .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid hex key")))?,
        )
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid private key")))?;

        let wallet = EthereumWallet::from(signer);

        let rpc_url = self.config.rpc_url_for(dest);

        let provider = ProviderBuilder::new().wallet(wallet).connect_http(
            rpc_url
                .parse()
                .map_err(|e| AppError::Internal(anyhow::anyhow!("bad rpc url: {e}")))?,
        );

        let transmitter: Address = self
            .config
            .cctp_message_transmitter_for(dest)
            .parse()
            .map_err(|_| {
                AppError::Internal(anyhow::anyhow!("bad MessageTransmitter on {:?}", dest))
            })?;

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

    /// Non-custodial burn (Part B0): ABI-encode the USDC `approve` and
    /// `depositForBurnWithHook` calls and submit each from the user's Circle
    /// developer-controlled wallet. The user's wallet is the tx sender (and, in
    /// this model, the USDC holder), so the bridge is non-custodial.
    ///
    /// Unlike the EOA path we cannot read the burn receipt's `MessageSent` log
    /// directly (Circle returns a tx hash, not decoded logs). That is fine: the
    /// attestation lookup is keyed by the burn `transactionHash`, not by this
    /// `message_hash`, which is only persisted as an identifier. We derive a
    /// deterministic identifier from the tx hash so the receipt shape matches
    /// the EOA path.
    #[cfg(feature = "real-cctp")]
    async fn circle_wallet_deposit_for_burn(
        &self,
        src: ChainKey,
        dest: ChainKey,
        amount: u128,
        hook: &HookPayload,
    ) -> Result<BurnReceipt> {
        let user = self.user.ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!(
                "circle_wallet_exec set but no user context attached to CctpClient"
            ))
        })?;

        let token_messenger_str = self.config.cctp_token_messenger_for(src);
        let usdc_str = self.config.usdc_for(src);
        let executor_on_dest = self
            .config
            .rebalance_executor_for(dest)
            .parse::<Address>()
            .map_err(|_| {
                AppError::Internal(anyhow::anyhow!("bad RebalanceExecutor on {:?}", dest))
            })?;
        let token_messenger: Address = token_messenger_str
            .parse()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("bad CCTP TokenMessenger")))?;
        let usdc: Address = usdc_str
            .parse()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("bad USDC address")))?;

        // 1) approve(tokenMessenger, amount*2) — same headroom as the EOA path.
        let approve_amount = U256::from(amount).saturating_mul(U256::from(2u64));
        let approve_calldata = IERC20::approveCall {
            spender: token_messenger,
            amount: approve_amount,
        }
        .abi_encode();
        crate::modules::wallet::circle_exec::submit_contract_execution(
            self.http,
            self.config,
            user.db,
            user.user_id,
            src,
            usdc_str,
            &hex::encode(approve_calldata),
            None,
        )
        .await?;

        // 2) depositForBurnWithHook — identical args to the EOA path, including
        // the Fast Transfer threshold + fetched maxFee.
        let fee_choice = self.resolve_burn_fee(src, dest).await;
        let destination_caller = alloy::primitives::FixedBytes::<32>::ZERO;
        let burn_calldata = ICCTPV2TokenMessenger::depositForBurnWithHookCall {
            amount: U256::from(amount),
            destinationDomain: dest.domain_id(),
            mintRecipient: executor_on_dest.into_word(),
            burnToken: usdc,
            destinationCaller: destination_caller,
            maxFee: U256::from(max_fee_for(amount, fee_choice.fee_bps)),
            minFinalityThreshold: fee_choice.finality_threshold,
            hookData: encode_hook_payload(hook),
        }
        .abi_encode();
        let tx_hash = crate::modules::wallet::circle_exec::submit_contract_execution(
            self.http,
            self.config,
            user.db,
            user.user_id,
            src,
            token_messenger_str,
            &hex::encode(burn_calldata),
            None,
        )
        .await?;

        let message_hash = format!("0x{}", hex::encode(Sha256::digest(tx_hash.as_bytes())));
        Ok(BurnReceipt {
            tx_hash,
            message_hash,
        })
    }

    /// Non-custodial mint (Part B0): ABI-encode `receiveMessage(message,
    /// attestation)` and submit it from the user's Circle wallet on `dest`.
    #[cfg(feature = "real-cctp")]
    async fn circle_wallet_receive_message(
        &self,
        dest: ChainKey,
        message: &str,
        attestation: &str,
    ) -> Result<String> {
        let user = self.user.ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!(
                "circle_wallet_exec set but no user context attached to CctpClient"
            ))
        })?;

        let transmitter_str = self.config.cctp_message_transmitter_for(dest);
        let transmitter = transmitter_str.parse::<Address>().map_err(|_| {
            AppError::Internal(anyhow::anyhow!("bad MessageTransmitter on {:?}", dest))
        })?;
        let _ = transmitter;

        let message_bytes: Bytes = message
            .parse()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid message hex")))?;
        let attestation_bytes: Bytes = attestation
            .parse()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid attestation hex")))?;

        let calldata = IMessageTransmitter::receiveMessageCall {
            message: message_bytes,
            attestation: attestation_bytes,
        }
        .abi_encode();

        crate::modules::wallet::circle_exec::submit_contract_execution(
            self.http,
            self.config,
            user.db,
            user.user_id,
            dest,
            transmitter_str,
            &hex::encode(calldata),
            None,
        )
        .await
    }

    /// Resolve the Fast Transfer burn parameters for `src` → `dest` by querying
    /// Circle's fee API. Returns the standard (free, slow) path on any error so
    /// the working burn is never broken by an unreachable/changed fee endpoint.
    #[cfg(feature = "real-cctp")]
    async fn resolve_burn_fee(&self, src: ChainKey, dest: ChainKey) -> BurnFeeChoice {
        let url = format!(
            "{}/v2/burn/USDC/fees/{}/{}",
            self.config.cctp_attestation_url,
            src.domain_id(),
            dest.domain_id()
        );
        match self.http.get(&url).send().await {
            Ok(r) if r.status().is_success() => match r.json::<Vec<CctpFeeEntry>>().await {
                Ok(entries) => select_burn_fee(&entries),
                Err(e) => {
                    tracing::warn!(error = %e, "cctp fee parse failed; using standard finality");
                    BurnFeeChoice::STANDARD
                }
            },
            Ok(r) => {
                tracing::warn!(status = %r.status(), "cctp fee api non-200; using standard finality");
                BurnFeeChoice::STANDARD
            }
            Err(e) => {
                tracing::warn!(error = %e, "cctp fee api unreachable; using standard finality");
                BurnFeeChoice::STANDARD
            }
        }
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
        crate::config::test_config()
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
    fn fee_table_selects_fast_threshold_and_uses_parsed_fee() {
        // Sample shape of Circle's GET /v2/burn/USDC/fees/{src}/{dest} body:
        // one row per finality threshold, fee in bps under `minimumFee`.
        let body = r#"[
            {"finalityThreshold": 2000, "minimumFee": 0},
            {"finalityThreshold": 1000, "minimumFee": 1}
        ]"#;
        let entries: Vec<CctpFeeEntry> = serde_json::from_str(body).expect("fee body parses");
        let choice = select_burn_fee(&entries);
        assert_eq!(
            choice.finality_threshold, MIN_FINALITY_FAST,
            "fast threshold must be selected when present"
        );
        assert_eq!(choice.fee_bps, 1, "parsed minimumFee must drive the maxFee");

        // 100 USDC (6dp) at 1bps = 0.01 USDC = 10_000 base units (rounded up).
        let amount = 100_000_000u128;
        assert_eq!(max_fee_for(amount, choice.fee_bps), 10_000);
    }

    #[test]
    fn fee_table_falls_back_to_standard_when_no_fast_row() {
        let body = r#"[{"finalityThreshold": 2000, "minimumFee": 0}]"#;
        let entries: Vec<CctpFeeEntry> = serde_json::from_str(body).expect("fee body parses");
        let choice = select_burn_fee(&entries);
        assert_eq!(choice, BurnFeeChoice::STANDARD);
        assert_eq!(
            max_fee_for(100_000_000, choice.fee_bps),
            0,
            "standard path is free (maxFee 0)"
        );
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
