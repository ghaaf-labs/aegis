//! Circle developer-controlled wallet swap (non-custodial path, Part B0).
//! Behind the `real-swap` cargo feature.

#[cfg(feature = "real-swap")]
use alloy::primitives::{Address, U256};

#[cfg(feature = "real-swap")]
use crate::config::Config;
#[cfg(feature = "real-swap")]
use crate::error::{AppError, Result};

#[cfg(feature = "real-swap")]
use super::super::super::models::ChainKey;
#[cfg(feature = "real-swap")]
use super::super::RealReceipt;
#[cfg(feature = "real-swap")]
use super::{address_to_hex, confirm_allowance, IERC20Swap, ISwapRouter02, POOL_FEE};

/// Resolved on-chain args for a non-custodial swap. Built once in `real_execute`
/// so the Circle-wallet helper doesn't re-derive directions/addresses.
#[cfg(feature = "real-swap")]
pub(super) struct CircleSwapArgs {
    pub(super) chain: ChainKey,
    pub(super) router: Address,
    pub(super) approve_token: Address,
    pub(super) token_in: Address,
    pub(super) token_out: Address,
    pub(super) amount_in: u128,
    pub(super) min_out: u128,
    pub(super) is_sell: bool,
}

/// Non-custodial swap (Part B0): ABI-encode the input-token `approve` and the
/// router swap call, then submit each from the user's Circle developer-controlled
/// wallet on the swap's chain. The user's wallet is the tx sender, holds the
/// input token, and is the swap recipient — non-custodial by construction.
#[cfg(feature = "real-swap")]
pub(super) async fn circle_wallet_swap(
    cfg: &Config,
    http: &reqwest::Client,
    db: &crate::db::Db,
    user_id: uuid::Uuid,
    leg_id: uuid::Uuid,
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
        &format!("{leg_id}:swap-approve"),
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
        &format!("{leg_id}:swap"),
    )
    .await?;

    Ok(RealReceipt {
        tx_hash,
        cctp_message_hash: None,
    })
}
