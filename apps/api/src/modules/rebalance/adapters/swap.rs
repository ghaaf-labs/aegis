//! Per-chain swap adapter — real USDC↔token swaps on Uniswap V3 (Base Sepolia).
//!
//! `quote()` prices a buy via the QuoterV2 and produces a `ValidatedQuote`
//! with a slippage-adjusted `min_out`; `execute()` performs the swap via
//! SwapRouter02 after approving USDC. Both are gated behind the `real-swap`
//! cargo feature — without it (or without a configured venue / token address)
//! the route fails closed upstream and these return an error rather than a
//! synthetic success.
//!
//! Both directions are wired: buys (USDC → token) via `exactInputSingle` sized
//! by the USDC budget, and sells (token → USDC) via `exactOutputSingle` sized
//! by the USDC value the planner wants to realize (no token price needed).
//! Anything that isn't a USDC↔token swap fails closed upstream.

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

        struct QuoteExactOutputSingleParams {
            address tokenIn;
            address tokenOut;
            uint256 amount;
            uint24 fee;
            uint160 sqrtPriceLimitX96;
        }
        function quoteExactOutputSingle(QuoteExactOutputSingleParams memory params)
            external
            returns (
                uint256 amountIn,
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

        struct ExactOutputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            uint256 amountOut;
            uint256 amountInMaximum;
            uint160 sqrtPriceLimitX96;
        }
        function exactOutputSingle(ExactOutputSingleParams calldata params)
            external
            payable
            returns (uint256 amountIn);
    }

    #[sol(rpc)]
    interface IERC20Swap {
        function approve(address spender, uint256 amount) external returns (bool);
        function allowance(address owner, address spender) external view returns (uint256);
    }
}

/// A USDC↔token swap direction with the non-USDC token symbol. The token
/// symbol is only read in the `real-swap` build (the no-feature path just
/// validates the direction and returns a "feature off" error).
#[cfg_attr(not(feature = "real-swap"), allow(dead_code))]
enum SwapDir {
    /// USDC → token (buy the token with `amount_usdc`).
    Buy(String),
    /// token → USDC (sell, realizing `amount_usdc` of USDC).
    Sell(String),
}

/// Classify a leg as a buy or sell. Returns `None` for anything that isn't a
/// USDC↔token swap (e.g. token↔token, which the planner never emits).
fn swap_direction(src_symbol: Option<&str>, dest_symbol: Option<&str>) -> Option<SwapDir> {
    match (src_symbol, dest_symbol) {
        (Some(s), Some(d)) if s.eq_ignore_ascii_case("USDC") && !d.eq_ignore_ascii_case("USDC") => {
            Some(SwapDir::Buy(d.to_string()))
        }
        (Some(s), Some(d)) if !s.eq_ignore_ascii_case("USDC") && d.eq_ignore_ascii_case("USDC") => {
            Some(SwapDir::Sell(s.to_string()))
        }
        _ => None,
    }
}

/// Price a USDC↔token swap (buy or sell) and build a fresh `ValidatedQuote`.
pub async fn quote(cfg: &Config, leg: &RouteLeg, now: DateTime<Utc>) -> Result<ValidatedQuote> {
    let dir = swap_direction(leg.src_symbol.as_deref(), leg.dest_symbol.as_deref())
        .ok_or_else(|| AppError::BadRequest("swap adapter handles USDC↔token swaps only".into()))?;

    #[cfg(not(feature = "real-swap"))]
    {
        let _ = (cfg, now, dir);
        Err(AppError::Internal(anyhow::anyhow!(
            "real-swap feature not enabled; build with --features real-swap"
        )))
    }

    #[cfg(feature = "real-swap")]
    {
        match dir {
            SwapDir::Buy(token) => real_quote_buy(cfg, &token, leg.amount_usdc, now).await,
            SwapDir::Sell(token) => real_quote_sell(cfg, &token, leg.amount_usdc, now).await,
        }
    }
}

/// Execute a USDC↔token swap authorized by `ticket`. `db` + `user_id` are only
/// read in the non-custodial (`circle_wallet_exec`) path, where the swap is
/// submitted from the user's Circle developer-controlled wallet.
pub async fn execute(
    cfg: &Config,
    http: &reqwest::Client,
    db: &crate::db::Db,
    user_id: uuid::Uuid,
    ticket: &ExecutionTicket,
) -> Result<RealReceipt> {
    #[cfg(not(feature = "real-swap"))]
    {
        let _ = (cfg, http, db, user_id, ticket);
        Err(AppError::Internal(anyhow::anyhow!(
            "real-swap feature not enabled; build with --features real-swap"
        )))
    }

    #[cfg(feature = "real-swap")]
    {
        real_execute(cfg, http, db, user_id, ticket).await
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
async fn real_quote_buy(
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
async fn real_quote_sell(
    cfg: &Config,
    token_symbol: &str,
    amount_usdc: f64,
    now: DateTime<Utc>,
) -> Result<ValidatedQuote> {
    let (usdc, token, _router, quoter) = base_addresses(cfg, token_symbol)?;
    // Realize ~`amount_usdc` of USDC; size the sell by exact-output so we never
    // need the token's spot price.
    let amount_out_usdc = (amount_usdc * 1_000_000.0) as u128;

    let provider = ProviderBuilder::new().connect_http(
        cfg.base_rpc_url
            .parse()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("bad Base rpc url: {e}")))?,
    );
    let q = IQuoterV2::new(quoter, &provider);
    let params = IQuoterV2::QuoteExactOutputSingleParams {
        tokenIn: token,
        tokenOut: usdc,
        amount: U256::from(amount_out_usdc),
        fee: POOL_FEE.try_into().expect("fee fits uint24"),
        sqrtPriceLimitX96: alloy::primitives::Uint::<160, 3>::ZERO,
    };
    let out = q.quoteExactOutputSingle(params).call().await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "quoter (exact-output) call failed (no pool?): {e}"
        ))
    })?;
    let amount_in_token: u128 = out
        .amountIn
        .try_into()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("quoted amountIn overflow")))?;
    if amount_in_token == 0 {
        return Err(AppError::Internal(anyhow::anyhow!(
            "quoter returned zero input; no liquidity"
        )));
    }
    // Slippage headroom on the max token input.
    let max_in = amount_in_token.saturating_mul((10_000 + SLIPPAGE_BPS) as u128) / 10_000;

    Ok(ValidatedQuote {
        quote_id: uuid::Uuid::new_v4(),
        issued_at: now,
        expires_at: now + Duration::seconds(MAX_QUOTE_TTL_SECS),
        src_token: token_symbol.to_string(),
        dest_token: "USDC".into(),
        src_chain: ChainKey::Base,
        dest_chain: ChainKey::Base,
        amount_in: max_in.max(1), // max token to spend
        min_out: amount_out_usdc, // exact USDC realized
        slippage_bps: SLIPPAGE_BPS,
        deadline: (now + Duration::seconds(600)).timestamp() as u64,
        provider: "uniswap-v3".into(),
    })
}

#[cfg(feature = "real-swap")]
async fn real_execute(
    cfg: &Config,
    http: &reqwest::Client,
    db: &crate::db::Db,
    user_id: uuid::Uuid,
    ticket: &ExecutionTicket,
) -> Result<RealReceipt> {
    use alloy::network::EthereumWallet;
    use alloy::providers::WalletProvider;
    use alloy::signers::local::PrivateKeySigner;

    let q = ticket.quote();
    let dir = swap_direction(Some(q.src_token.as_str()), Some(q.dest_token.as_str()))
        .ok_or_else(|| AppError::BadRequest("swap ticket is not a USDC↔token swap".into()))?;
    let is_sell = matches!(dir, SwapDir::Sell(_));
    let token_symbol = match &dir {
        SwapDir::Buy(t) | SwapDir::Sell(t) => t.clone(),
    };
    let (usdc, token, router, _quoter) = base_addresses(cfg, &token_symbol)?;
    let amount_in = q.amount_in;
    let min_out = q.min_out;

    // The router pulls the INPUT token: USDC on a buy, the sold token on a sell.
    let (token_in, token_out, approve_token) = if is_sell {
        (token, usdc, token)
    } else {
        (usdc, token, usdc)
    };

    // Part B0 — non-custodial: submit the approve + swap from the user's Circle
    // developer-controlled wallet. The user's Base wallet is the tx sender, the
    // token holder, and the swap recipient. Falls through to the EOA path when
    // the flag is off.
    if cfg.circle_wallet_exec {
        return circle_wallet_swap(
            cfg,
            http,
            db,
            user_id,
            CircleSwapArgs {
                router,
                approve_token,
                token_in,
                token_out,
                amount_in,
                min_out,
                is_sell,
            },
        )
        .await;
    }

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

    // Approve the input token to the router (with headroom) if allowance short.
    let approve_c = IERC20Swap::new(approve_token, &provider);
    let want = U256::from(amount_in).saturating_mul(U256::from(2u64));
    let have = approve_c
        .allowance(recipient, router)
        .call()
        .await
        .unwrap_or(U256::ZERO);
    if have < want {
        approve_c
            .approve(router, want)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("token approve send: {e}"))?
            .get_receipt()
            .await
            .map_err(|e| anyhow::anyhow!("token approve receipt: {e}"))?;
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }

    let router_c = ISwapRouter02::new(router, &provider);
    let fee = POOL_FEE.try_into().expect("fee fits uint24");
    let zero_limit = alloy::primitives::Uint::<160, 3>::ZERO;
    let receipt = if is_sell {
        // exactOutput: realize `min_out` USDC, spending up to `amount_in` token.
        let params = ISwapRouter02::ExactOutputSingleParams {
            tokenIn: token_in,
            tokenOut: token_out,
            fee,
            recipient,
            amountOut: U256::from(min_out),
            amountInMaximum: U256::from(amount_in),
            sqrtPriceLimitX96: zero_limit,
        };
        router_c
            .exactOutputSingle(params)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("sell swap send: {e}"))?
            .get_receipt()
            .await
            .map_err(|e| anyhow::anyhow!("sell swap receipt: {e}"))?
    } else {
        // exactInput: spend `amount_in` USDC for at least `min_out` token.
        let params = ISwapRouter02::ExactInputSingleParams {
            tokenIn: token_in,
            tokenOut: token_out,
            fee,
            recipient,
            amountIn: U256::from(amount_in),
            amountOutMinimum: U256::from(min_out),
            sqrtPriceLimitX96: zero_limit,
        };
        router_c
            .exactInputSingle(params)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("buy swap send: {e}"))?
            .get_receipt()
            .await
            .map_err(|e| anyhow::anyhow!("buy swap receipt: {e}"))?
    };

    Ok(RealReceipt {
        tx_hash: receipt.transaction_hash.to_string(),
        cctp_message_hash: None,
    })
}

/// Resolved on-chain args for a non-custodial swap. Built once in `real_execute`
/// so the Circle-wallet helper doesn't re-derive directions/addresses.
#[cfg(feature = "real-swap")]
struct CircleSwapArgs {
    router: Address,
    approve_token: Address,
    token_in: Address,
    token_out: Address,
    amount_in: u128,
    min_out: u128,
    is_sell: bool,
}

/// Non-custodial swap (Part B0): ABI-encode the input-token `approve` and the
/// router swap call, then submit each from the user's Circle developer-controlled
/// wallet on Base. The user's Base wallet is the tx sender, holds the input
/// token, and is the swap recipient — non-custodial by construction.
#[cfg(feature = "real-swap")]
async fn circle_wallet_swap(
    cfg: &Config,
    http: &reqwest::Client,
    db: &crate::db::Db,
    user_id: uuid::Uuid,
    args: CircleSwapArgs,
) -> Result<RealReceipt> {
    use alloy::sol_types::SolCall;

    let recipient: Address = crate::modules::wallet_routes::base_address_for_user(
        db,
        user_id,
        &cfg.circle_wallet_set_id,
    )
    .await?
    .ok_or_else(|| {
        AppError::ServiceUnavailable("user has no live Base wallet for non-custodial swap".into())
    })?
    .parse()
    .map_err(|_| AppError::Internal(anyhow::anyhow!("user Base wallet address unparsable")))?;

    let router_str = cfg.uniswap_v3_router_base.clone();
    let approve_token_str = address_to_hex(args.approve_token);

    // 1) approve(router, amount_in*2) from the user's wallet.
    let want = U256::from(args.amount_in).saturating_mul(U256::from(2u64));
    let approve_calldata = IERC20Swap::approveCall {
        spender: args.router,
        amount: want,
    }
    .abi_encode();
    crate::modules::wallet::circle_exec::submit_contract_execution(
        http,
        cfg,
        db,
        user_id,
        ChainKey::Base,
        &approve_token_str,
        &hex::encode(approve_calldata),
        None,
    )
    .await?;

    // 2) the swap itself (exact-output sell or exact-input buy).
    let fee = POOL_FEE.try_into().expect("fee fits uint24");
    let zero_limit = alloy::primitives::Uint::<160, 3>::ZERO;
    let swap_calldata = if args.is_sell {
        ISwapRouter02::exactOutputSingleCall {
            params: ISwapRouter02::ExactOutputSingleParams {
                tokenIn: args.token_in,
                tokenOut: args.token_out,
                fee,
                recipient,
                amountOut: U256::from(args.min_out),
                amountInMaximum: U256::from(args.amount_in),
                sqrtPriceLimitX96: zero_limit,
            },
        }
        .abi_encode()
    } else {
        ISwapRouter02::exactInputSingleCall {
            params: ISwapRouter02::ExactInputSingleParams {
                tokenIn: args.token_in,
                tokenOut: args.token_out,
                fee,
                recipient,
                amountIn: U256::from(args.amount_in),
                amountOutMinimum: U256::from(args.min_out),
                sqrtPriceLimitX96: zero_limit,
            },
        }
        .abi_encode()
    };
    let tx_hash = crate::modules::wallet::circle_exec::submit_contract_execution(
        http,
        cfg,
        db,
        user_id,
        ChainKey::Base,
        &router_str,
        &hex::encode(swap_calldata),
        None,
    )
    .await?;

    Ok(RealReceipt {
        tx_hash,
        cctp_message_hash: None,
    })
}

#[cfg(feature = "real-swap")]
fn address_to_hex(addr: Address) -> String {
    format!("0x{}", hex::encode(addr.as_slice()))
}
