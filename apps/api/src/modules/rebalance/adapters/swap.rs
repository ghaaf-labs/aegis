//! Per-chain swap adapter — real USDC↔token swaps on a Uniswap-V3-compatible
//! venue, resolved from the swap leg's chain.
//!
//! `quote()` prices a buy via the QuoterV2 and produces a `ValidatedQuote`
//! with a slippage-adjusted `min_out`; `execute()` performs the swap via
//! SwapRouter02 after approving USDC. Both are gated behind the `real-swap`
//! cargo feature — without it (or without a configured venue / token address)
//! the route fails closed upstream and these return an error rather than a
//! synthetic success.
//!
//! The venue is resolved per chain via `Config::swap_router_for` /
//! `swap_quoter_for`: Base = Aerodrome Slipstream, OP = Velodrome,
//! Eth/Arb = Uniswap V3. All three expose the same V3-style
//! `exactInputSingle`/`exactOutputSingle` router + QuoterV2 surface, so the
//! single `sol!` interface below works against any of their addresses. Avax's
//! Trader Joe Liquidity Book (LB v2.2) uses a different `LBRouter` ABI (a
//! bin-step + version `Path`), so `quote`/`execute` dispatch on `SwapVenue` and
//! the LB calls go through their own `sol!` interface. Arc has no AMM venue.
//!
//! Both directions are wired: buys (USDC → token) via `exactInputSingle` sized
//! by the USDC budget, and sells (token → USDC) via `exactOutputSingle` sized
//! by the USDC value the planner wants to realize (no token price needed).
//! Anything that isn't a USDC↔token swap (or sits on a non-swap chain) fails
//! closed upstream.

use chrono::{DateTime, Utc};

use crate::config::Config;
use crate::error::{AppError, Result};

use super::super::models::ChainKey;
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

/// Aggregate capability of the swap adapter, anchored on Base (the chain whose
/// venue is wired today). The route rule engine still resolves per-token
/// executability against a token's configured ERC-20; per-chain venue readiness
/// is reported by `capability_for`. Keeping the aggregate Base-anchored means
/// `RuntimeCapabilities::swap` is byte-for-byte what it was before per-chain
/// resolution landed.
pub fn capability(cfg: &Config) -> AdapterCapability {
    capability_for(cfg, ChainKey::Base)
}

/// Per-chain swap-venue capability. A chain is `Live` only when `real-swap` is
/// compiled, its V3-compatible router + quoter are configured, and its signer
/// is present. Chains with no AMM venue (Arc, Avax/Trader-Joe-LB) report
/// `NeedsAddress` because their `swap_router_for`/`swap_quoter_for` are empty.
pub fn capability_for(cfg: &Config, chain: ChainKey) -> AdapterCapability {
    if !cfg!(feature = "real-swap") {
        AdapterCapability::NeedsFeature
    } else if !tokens::is_real_addr(cfg.swap_quoter_for(chain))
        || !tokens::is_real_addr(cfg.swap_router_for(chain))
    {
        AdapterCapability::NeedsAddress
    } else if cfg.chain_private_key_for(chain).trim().is_empty() {
        AdapterCapability::NeedsSigner
    } else {
        AdapterCapability::Live
    }
}

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

// Trader Joe Liquidity Book (LB v2.2) on Avalanche. The router takes a `Path`
// of (pairBinSteps, versions, tokenPath) rather than the V3 single-pool fee
// tier; the quoter returns the best path's `amounts`. Both behind `real-swap`.
#[cfg(feature = "real-swap")]
sol! {
    #[sol(rpc)]
    #[allow(clippy::too_many_arguments)]
    interface ILBRouter {
        enum Version { V1, V2, V2_1, V2_2 }

        struct Path {
            uint256[] pairBinSteps;
            Version[] versions;
            address[] tokenPath;
        }

        function swapExactTokensForTokens(
            uint256 amountIn,
            uint256 amountOutMin,
            Path path,
            address to,
            uint256 deadline
        ) external returns (uint256 amountOut);

        function swapTokensForExactTokens(
            uint256 amountOut,
            uint256 amountInMax,
            Path path,
            address to,
            uint256 deadline
        ) external returns (uint256[] amountsIn);
    }

    #[sol(rpc)]
    interface ILBQuoter {
        struct Quote {
            address[] route;
            address[] pairs;
            uint256[] binSteps;
            uint8[] versions;
            uint128[] amounts;
            uint128[] virtualAmountsWithoutSlippage;
            uint128[] fees;
        }

        function findBestPathFromAmountIn(address[] route, uint128 amountIn)
            external
            view
            returns (Quote memory);

        function findBestPathFromAmountOut(address[] route, uint128 amountOut)
            external
            view
            returns (Quote memory);
    }
}

/// LB v2.2 bin step used for the USDC↔token pair (the canonical Avax USDC pool
/// uses a 20-bp bin step). Encoded into the LB `Path`.
#[cfg(feature = "real-swap")]
const LB_BIN_STEP: u64 = 20;

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

/// The execution chain a swap leg settles on. A swap is same-chain, so we take
/// the destination (falling back to source) and require it to be a wired
/// execution chain. Returns `None` for an unset/unparsable/non-execution chain,
/// which the caller turns into a fail-closed error.
fn swap_chain(leg: &RouteLeg) -> Option<ChainKey> {
    leg.dest_chain
        .as_deref()
        .or(leg.src_chain.as_deref())
        .and_then(ChainKey::parse)
        .filter(|c| c.is_execution())
}

/// Which on-chain DEX a swap leg routes through. The V3-style venues
/// (Aerodrome/Velodrome/Uniswap V3) share one router ABI; Trader Joe LB on Avax
/// has its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapVenue {
    /// Uniswap-V3-compatible `exactInputSingle`/`exactOutputSingle` venue.
    UniswapV3,
    /// Trader Joe Liquidity Book (LB v2.2) on Avalanche.
    TraderJoeLb,
}

/// Resolve the DEX venue for a swap chain. Only Avax uses Trader Joe LB; every
/// other configured swap chain uses the V3-style surface.
pub fn swap_venue(chain: ChainKey) -> SwapVenue {
    match chain {
        ChainKey::AvaxFuji => SwapVenue::TraderJoeLb,
        _ => SwapVenue::UniswapV3,
    }
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
    let chain = swap_chain(leg).ok_or_else(|| {
        AppError::BadRequest("swap leg has no executable same-chain venue".into())
    })?;

    #[cfg(not(feature = "real-swap"))]
    {
        let _ = (cfg, now, dir, chain);
        Err(AppError::Internal(anyhow::anyhow!(
            "real-swap feature not enabled; build with --features real-swap"
        )))
    }

    #[cfg(feature = "real-swap")]
    {
        match (swap_venue(chain), dir) {
            (SwapVenue::UniswapV3, SwapDir::Buy(token)) => {
                real_quote_buy(cfg, chain, &token, leg.amount_usdc, now).await
            }
            (SwapVenue::UniswapV3, SwapDir::Sell(token)) => {
                real_quote_sell(cfg, chain, &token, leg.amount_usdc, now).await
            }
            (SwapVenue::TraderJoeLb, dir) => lb_quote(cfg, chain, &dir, leg.amount_usdc, now).await,
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

/// Resolve the four on-chain addresses a swap needs on `chain`: USDC, the
/// non-USDC token's ERC-20, the V3-compatible router, and the quoter. Every
/// lookup goes through the per-chain `Config` helpers, so the same code path
/// serves Base (Aerodrome), OP (Velodrome), and Eth/Arb (Uniswap V3). Any
/// unconfigured address fails closed here rather than reaching the venue.
#[cfg(feature = "real-swap")]
fn swap_addresses(
    cfg: &Config,
    chain: ChainKey,
    token_symbol: &str,
) -> Result<(Address, Address, Address, Address)> {
    let usdc = tokens::token(tokens::USDC)
        .and_then(|t| t.address_for(cfg, chain))
        .ok_or_else(|| AppError::BadRequest(format!("USDC has no ERC-20 on {}", chain.as_str())))?
        .parse::<Address>()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("bad USDC on {}", chain.as_str())))?;
    let token = tokens::token(token_symbol)
        .and_then(|t| t.address_for(cfg, chain))
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "{token_symbol} has no ERC-20 on {}",
                chain.as_str()
            ))
        })?
        .parse::<Address>()
        .map_err(|_| {
            AppError::Internal(anyhow::anyhow!("bad token address on {}", chain.as_str()))
        })?;
    let router = cfg.swap_router_for(chain).parse::<Address>().map_err(|_| {
        AppError::Internal(anyhow::anyhow!("bad swap router on {}", chain.as_str()))
    })?;
    let quoter = cfg.swap_quoter_for(chain).parse::<Address>().map_err(|_| {
        AppError::Internal(anyhow::anyhow!("bad swap quoter on {}", chain.as_str()))
    })?;
    Ok((usdc, token, router, quoter))
}

#[cfg(feature = "real-swap")]
async fn real_quote_buy(
    cfg: &Config,
    chain: ChainKey,
    token_symbol: &str,
    amount_usdc: f64,
    now: DateTime<Utc>,
) -> Result<ValidatedQuote> {
    let (usdc, token, _router, quoter) = swap_addresses(cfg, chain, token_symbol)?;
    let amount_in = (amount_usdc * 1_000_000.0) as u128;

    let provider = ProviderBuilder::new().connect_http(
        cfg.rpc_url_for(chain)
            .parse()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("bad rpc url: {e}")))?,
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
        src_chain: chain,
        dest_chain: chain,
        amount_in,
        min_out: min_out.max(1),
        // Record the quoter's expected token output (the real pool rate), so the
        // executor writes the holdings quantity that actually lands on-chain.
        expected_asset_units: amount_out,
        slippage_bps: SLIPPAGE_BPS,
        deadline: (now + Duration::seconds(600)).timestamp() as u64,
        provider: "uniswap-v3".into(),
    })
}

#[cfg(feature = "real-swap")]
async fn real_quote_sell(
    cfg: &Config,
    chain: ChainKey,
    token_symbol: &str,
    amount_usdc: f64,
    now: DateTime<Utc>,
) -> Result<ValidatedQuote> {
    let (usdc, token, _router, quoter) = swap_addresses(cfg, chain, token_symbol)?;
    // Realize ~`amount_usdc` of USDC; size the sell by exact-output so we never
    // need the token's spot price.
    let amount_out_usdc = (amount_usdc * 1_000_000.0) as u128;

    let provider = ProviderBuilder::new().connect_http(
        cfg.rpc_url_for(chain)
            .parse()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("bad rpc url: {e}")))?,
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
        src_chain: chain,
        dest_chain: chain,
        amount_in: max_in.max(1), // max token to spend
        min_out: amount_out_usdc, // exact USDC realized
        // The quoter's expected token *input* — what leaves the wallet — so the
        // sell-side holdings writeback decrements the real token quantity.
        expected_asset_units: amount_in_token,
        slippage_bps: SLIPPAGE_BPS,
        deadline: (now + Duration::seconds(600)).timestamp() as u64,
        provider: "uniswap-v3".into(),
    })
}

/// Price a Trader Joe LB (Avax) USDC↔token swap via the `LBQuoter` and build a
/// fresh `ValidatedQuote`. Buys size by exact-input (USDC budget → token out);
/// sells size by exact-output (token in → exact USDC realized), matching the
/// V3 path's `amount_in`/`min_out` convention.
#[cfg(feature = "real-swap")]
async fn lb_quote(
    cfg: &Config,
    chain: ChainKey,
    dir: &SwapDir,
    amount_usdc: f64,
    now: DateTime<Utc>,
) -> Result<ValidatedQuote> {
    let token_symbol = match dir {
        SwapDir::Buy(t) | SwapDir::Sell(t) => t.as_str(),
    };
    let (usdc, token, _router, quoter) = swap_addresses(cfg, chain, token_symbol)?;
    let amount_units = (amount_usdc * 1_000_000.0) as u128;

    let provider = ProviderBuilder::new().connect_http(
        cfg.rpc_url_for(chain)
            .parse()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("bad rpc url: {e}")))?,
    );
    let q = ILBQuoter::new(quoter, &provider);

    match dir {
        SwapDir::Buy(_) => {
            let route = vec![usdc, token];
            let quote = q
                .findBestPathFromAmountIn(route, amount_units)
                .call()
                .await
                .map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("LB quoter (amountIn) failed: {e}"))
                })?;
            let amount_out = lb_last_amount(&quote.amounts)?;
            let min_out = amount_out.saturating_mul((10_000 - SLIPPAGE_BPS) as u128) / 10_000;
            Ok(ValidatedQuote {
                quote_id: uuid::Uuid::new_v4(),
                issued_at: now,
                expires_at: now + Duration::seconds(MAX_QUOTE_TTL_SECS),
                src_token: "USDC".into(),
                dest_token: token_symbol.to_string(),
                src_chain: chain,
                dest_chain: chain,
                amount_in: amount_units,
                min_out: min_out.max(1),
                expected_asset_units: amount_out,
                slippage_bps: SLIPPAGE_BPS,
                deadline: (now + Duration::seconds(600)).timestamp() as u64,
                provider: "trader-joe-lb".into(),
            })
        }
        SwapDir::Sell(_) => {
            let route = vec![token, usdc];
            let quote = q
                .findBestPathFromAmountOut(route, amount_units)
                .call()
                .await
                .map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("LB quoter (amountOut) failed: {e}"))
                })?;
            // amounts[0] is the token input required for the requested USDC out.
            let amount_in_token = lb_first_amount(&quote.amounts)?;
            let max_in = amount_in_token.saturating_mul((10_000 + SLIPPAGE_BPS) as u128) / 10_000;
            Ok(ValidatedQuote {
                quote_id: uuid::Uuid::new_v4(),
                issued_at: now,
                expires_at: now + Duration::seconds(MAX_QUOTE_TTL_SECS),
                src_token: token_symbol.to_string(),
                dest_token: "USDC".into(),
                src_chain: chain,
                dest_chain: chain,
                amount_in: max_in.max(1),
                min_out: amount_units,
                expected_asset_units: amount_in_token,
                slippage_bps: SLIPPAGE_BPS,
                deadline: (now + Duration::seconds(600)).timestamp() as u64,
                provider: "trader-joe-lb".into(),
            })
        }
    }
}

/// The final swap output the LB quoter reports (`amounts.last()`), rejecting an
/// empty path or a zero (no-liquidity) result.
#[cfg(feature = "real-swap")]
fn lb_last_amount(amounts: &[u128]) -> Result<u128> {
    let out = *amounts
        .last()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("LB quote returned no amounts")))?;
    if out == 0 {
        return Err(AppError::Internal(anyhow::anyhow!(
            "LB quote returned zero output; no liquidity"
        )));
    }
    Ok(out)
}

/// The token input the LB quoter reports for an exact-output sell
/// (`amounts.first()`), rejecting an empty path or a zero result.
#[cfg(feature = "real-swap")]
fn lb_first_amount(amounts: &[u128]) -> Result<u128> {
    let amt = *amounts
        .first()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("LB quote returned no amounts")))?;
    if amt == 0 {
        return Err(AppError::Internal(anyhow::anyhow!(
            "LB quote returned zero input; no liquidity"
        )));
    }
    Ok(amt)
}

/// Build the LB v2.2 `Path` for a single-hop USDC↔token swap: one bin step, one
/// `V2_2` version, and the two-token path.
#[cfg(feature = "real-swap")]
fn lb_path(token_in: Address, token_out: Address) -> ILBRouter::Path {
    ILBRouter::Path {
        pairBinSteps: vec![U256::from(LB_BIN_STEP)],
        versions: vec![ILBRouter::Version::V2_2],
        tokenPath: vec![token_in, token_out],
    }
}

/// Resolved args for a Trader Joe LB swap, bundled so `lb_execute` keeps a small
/// signature (mirrors `CircleSwapArgs` on the V3 path).
#[cfg(feature = "real-swap")]
struct LbSwapArgs {
    chain: ChainKey,
    dir: SwapDir,
    amount_in: u128,
    min_out: u128,
}

/// Execute a Trader Joe LB (Avax) swap. Fails closed when the LB router/quoter
/// addresses are unset (`swap_addresses` returns an error). Mirrors the EOA /
/// non-custodial split of the V3 path.
#[cfg(feature = "real-swap")]
async fn lb_execute(
    cfg: &Config,
    http: &reqwest::Client,
    db: &crate::db::Db,
    user_id: uuid::Uuid,
    args: LbSwapArgs,
) -> Result<RealReceipt> {
    use alloy::network::EthereumWallet;
    use alloy::providers::WalletProvider;
    use alloy::signers::local::PrivateKeySigner;
    use alloy::sol_types::SolCall;

    let LbSwapArgs {
        chain,
        dir,
        amount_in,
        min_out,
    } = args;
    let is_sell = matches!(dir, SwapDir::Sell(_));
    let token_symbol = match &dir {
        SwapDir::Buy(t) | SwapDir::Sell(t) => t.as_str(),
    };
    let (usdc, token, router, _quoter) = swap_addresses(cfg, chain, token_symbol)?;
    let (token_in, token_out, approve_token) = if is_sell {
        (token, usdc, token)
    } else {
        (usdc, token, usdc)
    };
    let deadline = U256::from((Utc::now() + Duration::seconds(600)).timestamp() as u64);

    if cfg.circle_wallet_exec {
        let recipient: Address = crate::modules::wallet_routes::address_for_chain(
            db,
            user_id,
            chain,
            &cfg.circle_wallet_set_id,
        )
        .await?
        .ok_or_else(|| {
            AppError::ServiceUnavailable(format!(
                "user has no live {} wallet for non-custodial LB swap",
                chain.as_str()
            ))
        })?
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("user wallet address unparsable")))?;

        let want = U256::from(amount_in).saturating_mul(U256::from(2u64));
        let approve_calldata = IERC20Swap::approveCall {
            spender: router,
            amount: want,
        }
        .abi_encode();
        crate::modules::wallet::circle_exec::submit_contract_execution(
            http,
            cfg,
            db,
            user_id,
            chain,
            &address_to_hex(approve_token),
            &hex::encode(approve_calldata),
            None,
        )
        .await?;

        // B3: confirm the allowance is on-chain-effective before the LB swap.
        confirm_allowance(cfg, chain, approve_token, recipient, router, amount_in).await?;

        let path = lb_path(token_in, token_out);
        let swap_calldata = if is_sell {
            ILBRouter::swapTokensForExactTokensCall {
                amountOut: U256::from(min_out),
                amountInMax: U256::from(amount_in),
                path,
                to: recipient,
                deadline,
            }
            .abi_encode()
        } else {
            ILBRouter::swapExactTokensForTokensCall {
                amountIn: U256::from(amount_in),
                amountOutMin: U256::from(min_out),
                path,
                to: recipient,
                deadline,
            }
            .abi_encode()
        };
        let tx_hash = crate::modules::wallet::circle_exec::submit_contract_execution(
            http,
            cfg,
            db,
            user_id,
            chain,
            &address_to_hex(router),
            &hex::encode(swap_calldata),
            None,
        )
        .await?;
        return Ok(RealReceipt {
            tx_hash,
            cctp_message_hash: None,
        });
    }

    let key_bytes = hex::decode(cfg.chain_private_key_for(chain).trim_start_matches("0x"))
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid private key")))?;
    let signer = PrivateKeySigner::from_slice(&key_bytes)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid private key")))?;
    let wallet = EthereumWallet::from(signer);
    let provider = ProviderBuilder::new().wallet(wallet).connect_http(
        cfg.rpc_url_for(chain)
            .parse()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("bad rpc url: {e}")))?,
    );
    let recipient = provider.default_signer_address();

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
            .map_err(|e| anyhow::anyhow!("LB approve send: {e}"))?
            .get_receipt()
            .await
            .map_err(|e| anyhow::anyhow!("LB approve receipt: {e}"))?;
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }

    let router_c = ILBRouter::new(router, &provider);
    let path = lb_path(token_in, token_out);
    let receipt = if is_sell {
        router_c
            .swapTokensForExactTokens(
                U256::from(min_out),
                U256::from(amount_in),
                path,
                recipient,
                deadline,
            )
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("LB sell swap send: {e}"))?
            .get_receipt()
            .await
            .map_err(|e| anyhow::anyhow!("LB sell swap receipt: {e}"))?
    } else {
        router_c
            .swapExactTokensForTokens(
                U256::from(amount_in),
                U256::from(min_out),
                path,
                recipient,
                deadline,
            )
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("LB buy swap send: {e}"))?
            .get_receipt()
            .await
            .map_err(|e| anyhow::anyhow!("LB buy swap receipt: {e}"))?
    };

    Ok(RealReceipt {
        tx_hash: receipt.transaction_hash.to_string(),
        cctp_message_hash: None,
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
    // A swap is same-chain: the ticket's quote stamped src==dest at quote time.
    let chain = q.dest_chain;

    // Trader Joe LB (Avax) has its own router ABI — dispatch the whole swap to
    // the LB path. Every other chain uses the V3-style surface below.
    if swap_venue(chain) == SwapVenue::TraderJoeLb {
        return lb_execute(
            cfg,
            http,
            db,
            user_id,
            LbSwapArgs {
                chain,
                dir,
                amount_in: q.amount_in,
                min_out: q.min_out,
            },
        )
        .await;
    }

    let (usdc, token, router, _quoter) = swap_addresses(cfg, chain, &token_symbol)?;
    let amount_in = q.amount_in;
    let min_out = q.min_out;

    // The router pulls the INPUT token: USDC on a buy, the sold token on a sell.
    let (token_in, token_out, approve_token) = if is_sell {
        (token, usdc, token)
    } else {
        (usdc, token, usdc)
    };

    // Part B0 — non-custodial: submit the approve + swap from the user's Circle
    // developer-controlled wallet on the swap's chain. The user's wallet is the
    // tx sender, the token holder, and the swap recipient. Falls through to the
    // EOA path when the flag is off.
    if cfg.circle_wallet_exec {
        return circle_wallet_swap(
            cfg,
            http,
            db,
            user_id,
            CircleSwapArgs {
                chain,
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

    let key_bytes = hex::decode(cfg.chain_private_key_for(chain).trim_start_matches("0x"))
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid private key")))?;
    let signer = PrivateKeySigner::from_slice(&key_bytes)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid private key")))?;
    let wallet = EthereumWallet::from(signer);
    let provider = ProviderBuilder::new().wallet(wallet).connect_http(
        cfg.rpc_url_for(chain)
            .parse()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("bad rpc url: {e}")))?,
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
    chain: ChainKey,
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
/// wallet on the swap's chain. The user's wallet is the tx sender, holds the
/// input token, and is the swap recipient — non-custodial by construction.
#[cfg(feature = "real-swap")]
async fn circle_wallet_swap(
    cfg: &Config,
    http: &reqwest::Client,
    db: &crate::db::Db,
    user_id: uuid::Uuid,
    args: CircleSwapArgs,
) -> Result<RealReceipt> {
    use alloy::sol_types::SolCall;

    let recipient: Address = crate::modules::wallet_routes::address_for_chain(
        db,
        user_id,
        args.chain,
        &cfg.circle_wallet_set_id,
    )
    .await?
    .ok_or_else(|| {
        AppError::ServiceUnavailable(format!(
            "user has no live {} wallet for non-custodial swap",
            args.chain.as_str()
        ))
    })?
    .parse()
    .map_err(|_| AppError::Internal(anyhow::anyhow!("user wallet address unparsable")))?;

    let router_str = address_to_hex(args.router);
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
        args.chain,
        &approve_token_str,
        &hex::encode(approve_calldata),
        None,
    )
    .await?;

    // B3: confirm the allowance is on-chain-effective before spending against it.
    confirm_allowance(
        cfg,
        args.chain,
        args.approve_token,
        recipient,
        args.router,
        args.amount_in,
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
        args.chain,
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

/// Confirm an ERC-20 `approve` is on-chain-effective before submitting a swap
/// (B3). Circle's contractExecution poll already confirms the approve tx, but a
/// read-only `allowance()` check guarantees the spender can actually pull the
/// input token — otherwise the swap reverts ("STF"/transfer-from failed). Reads
/// the live allowance with a short bounded retry to absorb indexer/node lag.
#[cfg(feature = "real-swap")]
async fn confirm_allowance(
    cfg: &Config,
    chain: ChainKey,
    token: Address,
    owner: Address,
    spender: Address,
    min_allowance: u128,
) -> Result<()> {
    let provider = ProviderBuilder::new().connect_http(
        cfg.rpc_url_for(chain)
            .parse()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("bad rpc url: {e}")))?,
    );
    let erc20 = IERC20Swap::new(token, &provider);
    let want = U256::from(min_allowance);
    const ATTEMPTS: u32 = 5;
    for attempt in 0..ATTEMPTS {
        let have = erc20
            .allowance(owner, spender)
            .call()
            .await
            .unwrap_or(U256::ZERO);
        if have >= want {
            return Ok(());
        }
        if attempt + 1 < ATTEMPTS {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }
    Err(AppError::Internal(anyhow::anyhow!(
        "ERC-20 allowance not effective on {} after approve; not submitting swap to avoid an on-chain revert",
        chain.as_str()
    )))
}

#[cfg(test)]
mod tests {
    use super::super::super::models::LegKind;
    use super::*;

    fn swap_leg(src: &str, dest: &str) -> RouteLeg {
        RouteLeg {
            kind: LegKind::LocalSwap,
            src_chain: Some(src.into()),
            dest_chain: Some(dest.into()),
            src_symbol: Some("USDC".into()),
            dest_symbol: Some("ETH".into()),
            amount_usdc: 40.0,
        }
    }

    #[test]
    fn avax_routes_to_trader_joe_lb_others_to_uniswap_v3() {
        assert_eq!(swap_venue(ChainKey::AvaxFuji), SwapVenue::TraderJoeLb);
        for chain in [
            ChainKey::Base,
            ChainKey::EthSepolia,
            ChainKey::ArbSepolia,
            ChainKey::OpSepolia,
            ChainKey::Arc,
        ] {
            assert_eq!(swap_venue(chain), SwapVenue::UniswapV3, "{chain:?}");
        }
    }

    #[cfg(feature = "real-swap")]
    #[test]
    fn avax_lb_venue_capability_tracks_its_lb_router() {
        let mut cfg = crate::config::test_config();
        // Unconfigured LB venue → NeedsAddress (fail closed).
        assert_eq!(
            capability_for(&cfg, ChainKey::AvaxFuji),
            AdapterCapability::NeedsAddress
        );
        // Wire the LB router + quoter + signer → Live.
        cfg.trader_joe_lb_router_avax = "0x1111111111111111111111111111111111111111".into();
        cfg.trader_joe_lb_quoter_avax = "0x2222222222222222222222222222222222222222".into();
        cfg.chain_private_key_avax = "0xab".into();
        assert_eq!(
            capability_for(&cfg, ChainKey::AvaxFuji),
            AdapterCapability::Live
        );
    }

    #[test]
    fn swap_chain_resolves_same_chain_execution_legs() {
        assert_eq!(swap_chain(&swap_leg("base", "base")), Some(ChainKey::Base));
        assert_eq!(swap_chain(&swap_leg("arc", "arc")), Some(ChainKey::Arc));
    }

    #[test]
    fn swap_chain_fails_closed_for_unparsable_or_unset_chains() {
        // All six EVM testnets are execution chains now; only an unparsable /
        // non-EVM chain or an unset chain must fail closed.
        assert_eq!(swap_chain(&swap_leg("solana", "solana")), None);
        let mut l = swap_leg("base", "base");
        l.src_chain = None;
        l.dest_chain = None;
        assert_eq!(swap_chain(&l), None);
    }

    #[test]
    fn capability_for_needs_address_when_venue_empty() {
        // Default cfg has no venue addresses on any chain.
        let cfg = crate::config::test_config();
        for chain in [
            ChainKey::Base,
            ChainKey::EthSepolia,
            ChainKey::ArbSepolia,
            ChainKey::OpSepolia,
            ChainKey::Arc,
            ChainKey::AvaxFuji,
        ] {
            let cap = capability_for(&cfg, chain);
            #[cfg(feature = "real-swap")]
            assert_eq!(cap, AdapterCapability::NeedsAddress, "{chain:?}");
            #[cfg(not(feature = "real-swap"))]
            assert_eq!(cap, AdapterCapability::NeedsFeature, "{chain:?}");
        }
    }

    #[cfg(feature = "real-swap")]
    #[test]
    fn capability_for_distinguishes_chains_by_their_own_venue() {
        let mut cfg = crate::config::test_config();
        // Wire only OP's venue + signer; Base stays unconfigured.
        cfg.uniswap_v3_router_op = "0x1111111111111111111111111111111111111111".into();
        cfg.uniswap_v3_quoter_op = "0x2222222222222222222222222222222222222222".into();
        cfg.chain_private_key_op = "0xab".into();
        assert_eq!(
            capability_for(&cfg, ChainKey::OpSepolia),
            AdapterCapability::Live
        );
        // Base still NeedsAddress — proves the resolution is per-chain, not a
        // single shared venue.
        assert_eq!(
            capability_for(&cfg, ChainKey::Base),
            AdapterCapability::NeedsAddress
        );
        // Arc / Avax have no V3 venue at all → NeedsAddress regardless.
        assert_eq!(
            capability_for(&cfg, ChainKey::Arc),
            AdapterCapability::NeedsAddress
        );
        assert_eq!(
            capability_for(&cfg, ChainKey::AvaxFuji),
            AdapterCapability::NeedsAddress
        );
    }

    #[cfg(feature = "real-swap")]
    #[test]
    fn swap_addresses_resolve_from_the_legs_chain() {
        let mut cfg = crate::config::test_config();
        cfg.usdc_op = "0x036CbD53842c5426634e7929541eC2318f3dCF7e".into();
        cfg.weth_op = "0x4200000000000000000000000000000000000006".into();
        cfg.uniswap_v3_router_op = "0x1111111111111111111111111111111111111111".into();
        cfg.uniswap_v3_quoter_op = "0x2222222222222222222222222222222222222222".into();

        let (usdc, token, router, quoter) =
            swap_addresses(&cfg, ChainKey::OpSepolia, "ETH").expect("OP venue resolves");
        assert_eq!(address_to_hex(usdc), cfg.usdc_op.to_ascii_lowercase());
        assert_eq!(address_to_hex(token), cfg.weth_op.to_ascii_lowercase());
        assert_eq!(address_to_hex(router), cfg.uniswap_v3_router_op);
        assert_eq!(address_to_hex(quoter), cfg.uniswap_v3_quoter_op);

        // Base has no venue configured here → fail closed.
        assert!(swap_addresses(&cfg, ChainKey::Base, "ETH").is_err());
        // Avax (Trader Joe LB, no V3 venue) always fails closed.
        assert!(swap_addresses(&cfg, ChainKey::AvaxFuji, "ETH").is_err());
    }
}
