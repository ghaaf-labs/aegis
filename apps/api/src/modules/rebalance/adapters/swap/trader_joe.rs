//! Trader Joe Liquidity Book (LB v2.2) swap adapter for Avalanche.
//! Behind the `real-swap` cargo feature.

#[cfg(feature = "real-swap")]
use alloy::{
    primitives::{Address, U256},
    providers::ProviderBuilder,
};
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
use super::super::RealReceipt;
#[cfg(feature = "real-swap")]
use super::{
    address_to_hex, confirm_allowance, swap_addresses, ILBQuoter, ILBRouter, IERC20Swap,
    SLIPPAGE_BPS,
};
#[cfg(feature = "real-swap")]
use super::SwapDir;

/// Resolved args for a Trader Joe LB swap, bundled so `lb_execute` keeps a small
/// signature (mirrors `CircleSwapArgs` on the V3 path).
#[cfg(feature = "real-swap")]
pub(super) struct LbSwapArgs {
    pub(super) chain: ChainKey,
    pub(super) dir: SwapDir,
    pub(super) amount_in: u128,
    pub(super) min_out: u128,
}

/// Price a Trader Joe LB (Avax) USDC↔token swap via the `LBQuoter` and build a
/// fresh `ValidatedQuote`. Buys size by exact-input (USDC budget → token out);
/// sells size by exact-output (token in → exact USDC realized), matching the
/// V3 path's `amount_in`/`min_out` convention.
#[cfg(feature = "real-swap")]
pub(super) async fn lb_quote(
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
        cfg.chain(chain)
            .rpc_url
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
    use super::LB_BIN_STEP;
    ILBRouter::Path {
        pairBinSteps: vec![U256::from(LB_BIN_STEP)],
        versions: vec![ILBRouter::Version::V2_2],
        tokenPath: vec![token_in, token_out],
    }
}

/// Execute a Trader Joe LB (Avax) swap. Fails closed when the LB router/quoter
/// addresses are unset (`swap_addresses` returns an error). Mirrors the EOA /
/// non-custodial split of the V3 path.
#[cfg(feature = "real-swap")]
pub(super) async fn lb_execute(
    cfg: &Config,
    http: &reqwest::Client,
    db: &crate::db::Db,
    user_id: uuid::Uuid,
    leg_id: uuid::Uuid,
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
            &format!("{leg_id}:lb-approve"),
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
            &format!("{leg_id}:lb-swap"),
        )
        .await?;
        return Ok(RealReceipt {
            tx_hash,
            cctp_message_hash: None,
        });
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

    let approve_c = IERC20Swap::new(approve_token, &provider);
    let want = U256::from(amount_in).saturating_mul(U256::from(2u64));
    let have = match approve_c.allowance(recipient, router).call().await {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!(chain = chain.as_str(), error = %e, "swap(lb): allowance read failed; assuming zero");
            U256::ZERO
        }
    };
    if have < want {
        approve_c
            .approve(router, want)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("LB approve send: {e}"))?
            .get_receipt()
            .await
            .map_err(|e| anyhow::anyhow!("LB approve receipt: {e}"))?;
        // Confirm allowance on-chain before the swap (parity with the Circle path).
        confirm_allowance(cfg, chain, approve_token, recipient, router, amount_in).await?;
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
