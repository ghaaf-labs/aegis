//! Per-chain swap adapter — real USDC↔token swaps on Uniswap V3 (Base Sepolia).
//!
//! `quote()` prices a buy via the QuoterV2 and produces a `ValidatedQuote`
//! with a slippage-adjusted `min_out`; `execute()` performs the swap via
//! SwapRouter02 after approving USDC. Both are gated behind the `real-swap`
//! cargo feature — without it (or without a configured venue / token address)
//! the route fails closed upstream and these return an error rather than a
//! synthetic success.
//!
//! Scope: the buy direction (USDC → token) is wired, which is the realistic
//! Base Sepolia path (USDC → WETH). Sells (token → USDC) fail closed until the
//! exact-output / price-sized path lands.

use chrono::{DateTime, Utc};

use crate::config::Config;
use crate::error::{AppError, Result};

use super::super::quote::ValidatedQuote;
use super::super::registry::capabilities::AdapterCapability;
use super::super::registry::route::RouteLeg;
use super::super::registry::ticket::ExecutionTicket;
use super::super::registry::tokens;
use super::RealReceipt;

/// Uniswap V3 pool fee tier used for USDC↔token routing (0.3%).
#[cfg(feature = "real-swap")]
const POOL_FEE: u32 = 3000;
/// Slippage tolerance applied to the quoted output to derive `min_out`.
#[cfg(feature = "real-swap")]
const SLIPPAGE_BPS: u32 = 50;

/// Capability of the per-chain swap venue (Uniswap V3 on Base Sepolia). This is
/// venue-level; per-token executability (a token's Base ERC-20 + pool) is
/// resolved in the route rule engine.
pub fn capability(cfg: &Config) -> AdapterCapability {
    if !cfg!(feature = "real-swap") {
        AdapterCapability::NeedsFeature
    } else if !tokens::is_real_addr(&cfg.uniswap_v3_quoter_base)
        || !tokens::is_real_addr(&cfg.uniswap_v3_router_base)
    {
        AdapterCapability::NeedsAddress
    } else if cfg.chain_private_key_base.trim().is_empty() {
        AdapterCapability::NeedsSigner
    } else {
        AdapterCapability::Live
    }
}

#[cfg(feature = "real-swap")]
use super::super::models::ChainKey;
#[cfg(feature = "real-swap")]
use super::super::quote::MAX_QUOTE_TTL_SECS;
#[cfg(feature = "real-swap")]
use alloy::{
    primitives::{Address, U256},
    providers::ProviderBuilder,
    sol,
};
#[cfg(feature = "real-swap")]
use chrono::Duration;

#[cfg(feature = "real-swap")]
sol! {
    #[sol(rpc)]
    interface IQuoterV2 {
        struct QuoteExactInputSingleParams {
            address tokenIn;
            address tokenOut;
            uint256 amountIn;
            uint24 fee;
            uint160 sqrtPriceLimitX96;
        }
        function quoteExactInputSingle(QuoteExactInputSingleParams memory params)
            external
            returns (
                uint256 amountOut,
                uint160 sqrtPriceX96After,
                uint32 initializedTicksCrossed,
                uint256 gasEstimate
            );
    }

    #[sol(rpc)]
    interface ISwapRouter02 {
        struct ExactInputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            uint256 amountIn;
            uint256 amountOutMinimum;
            uint160 sqrtPriceLimitX96;
        }
        function exactInputSingle(ExactInputSingleParams calldata params)
            external
            payable
            returns (uint256 amountOut);
    }

    #[sol(rpc)]
    interface IERC20Swap {
        function approve(address spender, uint256 amount) external returns (bool);
        function allowance(address owner, address spender) external view returns (uint256);
    }
}

/// Resolve the (USDC-in, token-out) symbols for a buy. Returns `None` for a
/// sell or anything that isn't a USDC→token swap.
fn buy_token_symbol(src_symbol: Option<&str>, dest_symbol: Option<&str>) -> Option<String> {
    match (src_symbol, dest_symbol) {
        (Some(s), Some(d)) if s.eq_ignore_ascii_case("USDC") && !d.eq_ignore_ascii_case("USDC") => {
            Some(d.to_string())
        }
        _ => None,
    }
}

/// Price a USDC→token buy and build a fresh `ValidatedQuote`.
pub async fn quote(cfg: &Config, leg: &RouteLeg, now: DateTime<Utc>) -> Result<ValidatedQuote> {
    let token_symbol = buy_token_symbol(leg.src_symbol.as_deref(), leg.dest_symbol.as_deref())
        .ok_or_else(|| {
            AppError::BadRequest(
                "swap adapter supports USDC→token buys only; sells are not wired yet".into(),
            )
        })?;

    #[cfg(not(feature = "real-swap"))]
    {
        let _ = (cfg, now, token_symbol);
        Err(AppError::Internal(anyhow::anyhow!(
            "real-swap feature not enabled; build with --features real-swap"
        )))
    }

    #[cfg(feature = "real-swap")]
    {
        real_quote(cfg, &token_symbol, leg.amount_usdc, now).await
    }
}

/// Execute a USDC→token buy authorized by `ticket`.
pub async fn execute(cfg: &Config, ticket: &ExecutionTicket) -> Result<RealReceipt> {
    #[cfg(not(feature = "real-swap"))]
    {
        let _ = (cfg, ticket);
        Err(AppError::Internal(anyhow::anyhow!(
            "real-swap feature not enabled; build with --features real-swap"
        )))
    }

    #[cfg(feature = "real-swap")]
    {
        real_execute(cfg, ticket).await
    }
}

#[cfg(feature = "real-swap")]
fn base_addresses(
    cfg: &Config,
    token_symbol: &str,
) -> Result<(Address, Address, Address, Address)> {
    let usdc = cfg
        .usdc_base
        .parse::<Address>()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("bad USDC on Base")))?;
    let token = tokens::token(token_symbol)
        .and_then(|t| t.address_for(cfg, ChainKey::Base))
        .ok_or_else(|| AppError::BadRequest(format!("{token_symbol} has no Base ERC-20")))?
        .parse::<Address>()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("bad token address on Base")))?;
    let router = cfg
        .uniswap_v3_router_base
        .parse::<Address>()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("bad Uniswap router on Base")))?;
    let quoter = cfg
        .uniswap_v3_quoter_base
        .parse::<Address>()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("bad Uniswap quoter on Base")))?;
    Ok((usdc, token, router, quoter))
}

#[cfg(feature = "real-swap")]
async fn real_quote(
    cfg: &Config,
    token_symbol: &str,
    amount_usdc: f64,
    now: DateTime<Utc>,
) -> Result<ValidatedQuote> {
    let (usdc, token, _router, quoter) = base_addresses(cfg, token_symbol)?;
    let amount_in = (amount_usdc * 1_000_000.0) as u128;

    let provider = ProviderBuilder::new().connect_http(
        cfg.base_rpc_url
            .parse()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("bad Base rpc url: {e}")))?,
    );
    let q = IQuoterV2::new(quoter, &provider);
    let params = IQuoterV2::QuoteExactInputSingleParams {
        tokenIn: usdc,
        tokenOut: token,
        amountIn: U256::from(amount_in),
        fee: POOL_FEE.try_into().expect("fee fits uint24"),
        sqrtPriceLimitX96: alloy::primitives::Uint::<160, 3>::ZERO,
    };
    let out =
        q.quoteExactInputSingle(params).call().await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("quoter call failed (no pool?): {e}"))
        })?;
    let amount_out: u128 = out
        .amountOut
        .try_into()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("quoted amountOut overflow")))?;
    if amount_out == 0 {
        return Err(AppError::Internal(anyhow::anyhow!(
            "quoter returned zero output; no liquidity"
        )));
    }
    let min_out = amount_out.saturating_mul((10_000 - SLIPPAGE_BPS) as u128) / 10_000;

    Ok(ValidatedQuote {
        quote_id: uuid::Uuid::new_v4(),
        issued_at: now,
        expires_at: now + Duration::seconds(MAX_QUOTE_TTL_SECS),
        src_token: "USDC".into(),
        dest_token: token_symbol.to_string(),
        src_chain: ChainKey::Base,
        dest_chain: ChainKey::Base,
        amount_in,
        min_out: min_out.max(1),
        slippage_bps: SLIPPAGE_BPS,
        deadline: (now + Duration::seconds(600)).timestamp() as u64,
        provider: "uniswap-v3".into(),
    })
}

#[cfg(feature = "real-swap")]
async fn real_execute(cfg: &Config, ticket: &ExecutionTicket) -> Result<RealReceipt> {
    use alloy::network::EthereumWallet;
    use alloy::providers::WalletProvider;
    use alloy::signers::local::PrivateKeySigner;

    let token_symbol = ticket.dest_symbol().to_string();
    let (usdc, token, router, _quoter) = base_addresses(cfg, &token_symbol)?;
    let amount_in = ticket.quote().amount_in;
    let min_out = ticket.quote().min_out;

    let key_bytes = hex::decode(cfg.chain_private_key_base.trim_start_matches("0x"))
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid Base private key")))?;
    let signer = PrivateKeySigner::from_slice(&key_bytes)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid Base private key")))?;
    let wallet = EthereumWallet::from(signer);
    let provider = ProviderBuilder::new().wallet(wallet).connect_http(
        cfg.base_rpc_url
            .parse()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("bad Base rpc url: {e}")))?,
    );
    let recipient = provider.default_signer_address();

    // Approve USDC to the router (with headroom) if the allowance is short.
    let usdc_token = IERC20Swap::new(usdc, &provider);
    let want = U256::from(amount_in).saturating_mul(U256::from(2u64));
    let have = usdc_token
        .allowance(recipient, router)
        .call()
        .await
        .unwrap_or(U256::ZERO);
    if have < want {
        usdc_token
            .approve(router, want)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("USDC approve send: {e}"))?
            .get_receipt()
            .await
            .map_err(|e| anyhow::anyhow!("USDC approve receipt: {e}"))?;
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }

    let router_c = ISwapRouter02::new(router, &provider);
    let params = ISwapRouter02::ExactInputSingleParams {
        tokenIn: usdc,
        tokenOut: token,
        fee: POOL_FEE.try_into().expect("fee fits uint24"),
        recipient,
        amountIn: U256::from(amount_in),
        amountOutMinimum: U256::from(min_out),
        sqrtPriceLimitX96: alloy::primitives::Uint::<160, 3>::ZERO,
    };
    let receipt = router_c
        .exactInputSingle(params)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("swap send: {e}"))?
        .get_receipt()
        .await
        .map_err(|e| anyhow::anyhow!("swap receipt: {e}"))?;

    Ok(RealReceipt {
        tx_hash: receipt.transaction_hash.to_string(),
        cctp_message_hash: None,
    })
}
