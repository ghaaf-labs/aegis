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
//! The venue is resolved per chain via `Config::chain(c).swap_router` /
//! `.swap_quoter`: Base = Aerodrome Slipstream, OP = Velodrome,
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

mod circle;
mod trader_joe;
mod uniswap;

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

// Bring these into scope so sibling submodules can import them via `super::`.
// Plain (private) `use` — they are swap-internal; a `pub(super)` re-export would
// over-expose them past the swap module and fail E0365 against their own
// `pub(super)` declarations.
#[cfg(feature = "real-swap")]
use circle::CircleSwapArgs;
#[cfg(feature = "real-swap")]
use trader_joe::LbSwapArgs;

/// Uniswap V3 pool fee tier used for USDC↔token routing (0.3%).
#[cfg(feature = "real-swap")]
pub(super) const POOL_FEE: u32 = 3000;
/// Slippage tolerance applied to the quoted output to derive `min_out`.
#[cfg(feature = "real-swap")]
pub(super) const SLIPPAGE_BPS: u32 = 50;

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
/// `NeedsAddress` because their `chain(c).swap_router`/`.swap_quoter` are empty.
pub fn capability_for(cfg: &Config, chain: ChainKey) -> AdapterCapability {
    if !cfg!(feature = "real-swap") {
        AdapterCapability::NeedsFeature
    } else if !tokens::is_real_addr(&cfg.chain(chain).swap_quoter)
        || !tokens::is_real_addr(&cfg.chain(chain).swap_router)
    {
        AdapterCapability::NeedsAddress
    } else if !cfg.circle_wallet_exec && cfg.chain(chain).private_key.trim().is_empty() {
        // The EOA path signs the swap on `chain`; the non-custodial path
        // (`circle_wallet_exec`) submits it from the user's Circle wallet and
        // needs no backend signing key.
        AdapterCapability::NeedsSigner
    } else {
        AdapterCapability::Live
    }
}

#[cfg(feature = "real-swap")]
use alloy::{
    primitives::{Address, U256},
    providers::ProviderBuilder,
    sol,
};

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
pub(super) const LB_BIN_STEP: u64 = 20;

/// A USDC↔token swap direction with the non-USDC token symbol. The token
/// symbol is only read in the `real-swap` build (the no-feature path just
/// validates the direction and returns a "feature off" error).
#[cfg_attr(not(feature = "real-swap"), allow(dead_code))]
pub(super) enum SwapDir {
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
pub(super) fn swap_direction(src_symbol: Option<&str>, dest_symbol: Option<&str>) -> Option<SwapDir> {
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
                uniswap::real_quote_buy(cfg, chain, &token, leg.amount_usdc, now).await
            }
            (SwapVenue::UniswapV3, SwapDir::Sell(token)) => {
                uniswap::real_quote_sell(cfg, chain, &token, leg.amount_usdc, now).await
            }
            (SwapVenue::TraderJoeLb, dir) => trader_joe::lb_quote(cfg, chain, &dir, leg.amount_usdc, now).await,
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
        uniswap::real_execute(cfg, http, db, user_id, ticket).await
    }
}

/// Resolve the four on-chain addresses a swap needs on `chain`: USDC, the
/// non-USDC token's ERC-20, the V3-compatible router, and the quoter. Every
/// lookup goes through the per-chain `Config` helpers, so the same code path
/// serves Base (Aerodrome), OP (Velodrome), and Eth/Arb (Uniswap V3). Any
/// unconfigured address fails closed here rather than reaching the venue.
#[cfg(feature = "real-swap")]
pub(super) fn swap_addresses(
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
    let router = cfg
        .chain(chain)
        .swap_router
        .parse::<Address>()
        .map_err(|_| {
            AppError::Internal(anyhow::anyhow!("bad swap router on {}", chain.as_str()))
        })?;
    let quoter = cfg
        .chain(chain)
        .swap_quoter
        .parse::<Address>()
        .map_err(|_| {
            AppError::Internal(anyhow::anyhow!("bad swap quoter on {}", chain.as_str()))
        })?;
    Ok((usdc, token, router, quoter))
}

#[cfg(feature = "real-swap")]
pub(super) fn address_to_hex(addr: Address) -> String {
    format!("0x{}", hex::encode(addr.as_slice()))
}

/// Confirm an ERC-20 `approve` is on-chain-effective before submitting a swap
/// (B3). Circle's contractExecution poll already confirms the approve tx, but a
/// read-only `allowance()` check guarantees the spender can actually pull the
/// input token — otherwise the swap reverts ("STF"/transfer-from failed). Reads
/// the live allowance with a short bounded retry to absorb indexer/node lag.
#[cfg(feature = "real-swap")]
pub(super) async fn confirm_allowance(
    cfg: &Config,
    chain: ChainKey,
    token: Address,
    owner: Address,
    spender: Address,
    min_allowance: u128,
) -> Result<()> {
    let provider = ProviderBuilder::new().connect_http(
        cfg.chain(chain)
            .rpc_url
            .parse()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("bad rpc url: {e}")))?,
    );
    let erc20 = IERC20Swap::new(token, &provider);
    let want = U256::from(min_allowance);
    // ~30s total at a 2s cadence — congested testnets can lag allowance-state
    // propagation 15–30s after an approve mines, longer than a fixed sleep covers.
    const ATTEMPTS: u32 = 15;
    for attempt in 0..ATTEMPTS {
        let have = match erc20.allowance(owner, spender).call().await {
            Ok(a) => a,
            Err(e) => {
                // A read failure is transient (RPC blip) — log and retry rather
                // than treating it as a real zero allowance.
                tracing::warn!(
                    chain = chain.as_str(),
                    attempt,
                    error = %e,
                    "confirm_allowance: allowance read failed; retrying"
                );
                U256::ZERO
            }
        };
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
        cfg.chains[ChainKey::AvaxFuji.index()].swap_router =
            "0x1111111111111111111111111111111111111111".into();
        cfg.chains[ChainKey::AvaxFuji.index()].swap_quoter =
            "0x2222222222222222222222222222222222222222".into();
        cfg.chains[ChainKey::AvaxFuji.index()].private_key = "0xab".into();
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
        cfg.chains[ChainKey::OpSepolia.index()].swap_router =
            "0x1111111111111111111111111111111111111111".into();
        cfg.chains[ChainKey::OpSepolia.index()].swap_quoter =
            "0x2222222222222222222222222222222222222222".into();
        cfg.chains[ChainKey::OpSepolia.index()].private_key = "0xab".into();
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
        cfg.chains[ChainKey::OpSepolia.index()].usdc =
            "0x036CbD53842c5426634e7929541eC2318f3dCF7e".into();
        cfg.weth_op = "0x4200000000000000000000000000000000000006".into();
        cfg.chains[ChainKey::OpSepolia.index()].swap_router =
            "0x1111111111111111111111111111111111111111".into();
        cfg.chains[ChainKey::OpSepolia.index()].swap_quoter =
            "0x2222222222222222222222222222222222222222".into();

        let (usdc, token, router, quoter) =
            swap_addresses(&cfg, ChainKey::OpSepolia, "ETH").expect("OP venue resolves");
        assert_eq!(
            address_to_hex(usdc),
            cfg.chain(ChainKey::OpSepolia).usdc.to_ascii_lowercase()
        );
        assert_eq!(address_to_hex(token), cfg.weth_op.to_ascii_lowercase());
        assert_eq!(
            address_to_hex(router),
            cfg.chain(ChainKey::OpSepolia).swap_router
        );
        assert_eq!(
            address_to_hex(quoter),
            cfg.chain(ChainKey::OpSepolia).swap_quoter
        );

        // Base has no venue configured here → fail closed.
        assert!(swap_addresses(&cfg, ChainKey::Base, "ETH").is_err());
        // Avax (Trader Joe LB, no V3 venue) always fails closed.
        assert!(swap_addresses(&cfg, ChainKey::AvaxFuji, "ETH").is_err());
    }
}
