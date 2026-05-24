//! Uniswap V3-compatible swap adapter (buy + sell) and the top-level real
//! execute dispatcher. Behind the `real-swap` cargo feature.

#[cfg(feature = "real-swap")]
use alloy::{primitives::U256, providers::ProviderBuilder};
#[cfg(feature = "real-swap")]
use chrono::{DateTime, Duration, Utc};

#[cfg(feature = "real-swap")]
use crate::config::Config;
#[cfg(feature = "real-swap")]
use crate::error::{AppError, Result};

#[cfg(feature = "real-swap")]
use super::super::super::models::ChainKey;
#[cfg(feature = "real-swap")]
use super::super::super::quote::{ValidatedQuote, MAX_QUOTE_TTL_SECS};
#[cfg(feature = "real-swap")]
use super::super::super::registry::ticket::ExecutionTicket;
#[cfg(feature = "real-swap")]
use super::super::RealReceipt;
#[cfg(feature = "real-swap")]
use super::{
    best_buy_tier, best_sell_tier, swap_addresses, swap_direction, swap_venue, CircleSwapArgs,
    IQuoterV2, ISwapRouter02, LbSwapArgs, SwapDir, SwapVenue, FEE_TIERS, POOL_FEE, SLIPPAGE_BPS,
};

#[cfg(feature = "real-swap")]
pub(super) async fn real_quote_buy(
    cfg: &Config,
    chain: ChainKey,
    token_symbol: &str,
    amount_usdc: f64,
    now: DateTime<Utc>,
) -> Result<ValidatedQuote> {
    let (usdc, token, _router, quoter) = swap_addresses(cfg, chain, token_symbol)?;
    let amount_in = (amount_usdc * 1_000_000.0) as u128;

    let provider = ProviderBuilder::new().connect_http(
        cfg.chain(chain)
            .rpc_url
            .parse()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("bad rpc url: {e}")))?,
    );
    let q = IQuoterV2::new(quoter, &provider);

    // Best-execution: quote every fee tier and route through the pool that
    // returns the most token for this exact USDC input. A tier with no pool
    // reverts — skip it and keep the rest; fail closed only when no tier has
    // liquidity. "Different markets, highest amount."
    let mut quotes: Vec<(u32, u128)> = Vec::new();
    let mut last_err: Option<String> = None;
    for &fee in &FEE_TIERS {
        let params = IQuoterV2::QuoteExactInputSingleParams {
            tokenIn: usdc,
            tokenOut: token,
            amountIn: U256::from(amount_in),
            fee: fee.try_into().expect("fee fits uint24"),
            sqrtPriceLimitX96: alloy::primitives::Uint::<160, 3>::ZERO,
        };
        match q.quoteExactInputSingle(params).call().await {
            Ok(out) => match out.amountOut.try_into() {
                Ok(amount_out) => quotes.push((fee, amount_out)),
                Err(_) => last_err = Some(format!("amountOut overflow on fee {fee}")),
            },
            Err(e) => last_err = Some(e.to_string()),
        }
    }
    let (fee_tier, amount_out) = best_buy_tier(&quotes).ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!(
            "no Uniswap V3 pool with liquidity for USDC→{token_symbol} on {} across fee tiers {FEE_TIERS:?}{}",
            chain.as_str(),
            last_err.map(|e| format!(" (last error: {e})")).unwrap_or_default(),
        ))
    })?;
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
        fee_tier: Some(fee_tier),
    })
}

#[cfg(feature = "real-swap")]
pub(super) async fn real_quote_sell(
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
        cfg.chain(chain)
            .rpc_url
            .parse()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("bad rpc url: {e}")))?,
    );
    let q = IQuoterV2::new(quoter, &provider);

    // Best-execution for a sell: quote every fee tier and route through the pool
    // that spends the *least* token to realize the target USDC.
    let mut quotes: Vec<(u32, u128)> = Vec::new();
    let mut last_err: Option<String> = None;
    for &fee in &FEE_TIERS {
        let params = IQuoterV2::QuoteExactOutputSingleParams {
            tokenIn: token,
            tokenOut: usdc,
            amount: U256::from(amount_out_usdc),
            fee: fee.try_into().expect("fee fits uint24"),
            sqrtPriceLimitX96: alloy::primitives::Uint::<160, 3>::ZERO,
        };
        match q.quoteExactOutputSingle(params).call().await {
            Ok(out) => match out.amountIn.try_into() {
                Ok(amount_in_token) => quotes.push((fee, amount_in_token)),
                Err(_) => last_err = Some(format!("amountIn overflow on fee {fee}")),
            },
            Err(e) => last_err = Some(e.to_string()),
        }
    }
    let (fee_tier, amount_in_token) = best_sell_tier(&quotes).ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!(
            "no Uniswap V3 pool with liquidity for {token_symbol}→USDC on {} across fee tiers {FEE_TIERS:?}{}",
            chain.as_str(),
            last_err.map(|e| format!(" (last error: {e})")).unwrap_or_default(),
        ))
    })?;
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
        fee_tier: Some(fee_tier),
    })
}

#[cfg(feature = "real-swap")]
pub(super) async fn real_execute(
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
        return super::trader_joe::lb_execute(
            cfg,
            http,
            db,
            user_id,
            ticket.leg_id(),
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
    // Execute on the exact pool the quote priced (best-execution tier selection),
    // falling back to the default tier for older quotes that carry none.
    let fee_tier = q.fee_tier.unwrap_or(POOL_FEE);

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
        return super::circle::circle_wallet_swap(
            cfg,
            http,
            db,
            user_id,
            ticket.leg_id(),
            CircleSwapArgs {
                chain,
                router,
                approve_token,
                token_in,
                token_out,
                amount_in,
                min_out,
                is_sell,
                fee_tier,
            },
        )
        .await;
    }

    let key_bytes = hex::decode(cfg.chain(chain).private_key.trim_start_matches("0x"))
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid private key")))?;
    let signer = PrivateKeySigner::from_slice(&key_bytes)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid private key")))?;
    let wallet = EthereumWallet::from(signer);
    let provider = ProviderBuilder::new().wallet(wallet).connect_http(
        cfg.chain(chain)
            .rpc_url
            .parse()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("bad rpc url: {e}")))?,
    );
    let recipient = provider.default_signer_address();

    // Approve the input token to the router (with headroom) if allowance short.
    let approve_c = super::IERC20Swap::new(approve_token, &provider);
    let want = U256::from(amount_in).saturating_mul(U256::from(2u64));
    let have = match approve_c.allowance(recipient, router).call().await {
        Ok(a) => a,
        Err(e) => {
            // Don't silently treat an RPC blip as a zero allowance (it would burn
            // gas on an unnecessary re-approve) — log and assume zero this once.
            tracing::warn!(chain = chain.as_str(), error = %e, "swap(v3): allowance read failed; assuming zero");
            U256::ZERO
        }
    };
    if have < want {
        approve_c
            .approve(router, want)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("token approve send: {e}"))?
            .get_receipt()
            .await
            .map_err(|e| anyhow::anyhow!("token approve receipt: {e}"))?;
        // Confirm the allowance is on-chain-effective before submitting the swap.
        // A stale RPC node can still read the pre-approve trie and revert the swap
        // pre-flight even though the approve mined (parity with the Circle path).
        super::confirm_allowance(cfg, chain, approve_token, recipient, router, amount_in).await?;
    }

    let router_c = ISwapRouter02::new(router, &provider);
    let fee = fee_tier.try_into().expect("fee fits uint24");
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
