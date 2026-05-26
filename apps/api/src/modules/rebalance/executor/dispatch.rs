//! Per-leg dispatch: turn one planned `LegRow` into an on-chain action.
//!
//! `walk_legs` (the saga loop in the parent module) owns ordering, retries, and
//! the DB state machine; this module owns *how* a single leg executes — the
//! mock short-circuit, the live-balance clamp, the route/ticket mint, and the
//! per-`LegKind` adapter calls — plus the CCTP hook builder. Split out of the
//! executor's saga loop per the decomposition in spec §8.

use chrono::Utc;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::config::Config;
use crate::domain::units::{
    apply_bps_margin, base_units_to_whole_token, whole_token_to_base_units,
};
use crate::error::{AppError, Result};
use crate::modules::rebalance::adapters;
use crate::modules::rebalance::cross_chain::build_hook_payload;
use crate::modules::rebalance::models::{ChainKey, LegKind};
use crate::modules::rebalance::quote::ValidatedQuote;
use crate::modules::rebalance::registry::{
    capabilities::RuntimeCapabilities, route::RouteLeg, ticket::ExecutionTicket, tokens,
};
use crate::modules::wallet_routes;
use crate::router::AppState;

use super::leg_status::mark_leg_quoted;
use super::legs::{blockchain_for_chain, quote_filled_qty, LegRow};

/// Fraction of the live USDC balance a buy-swap may spend, leaving a small
/// cushion for gas/rounding so the clamped `amountIn` never tips back over the
/// wallet's balance and re-triggers Circle's `INSUFFICIENT_TOKEN`.
const LIVE_BALANCE_SPEND_MARGIN: f64 = 0.995;
const LIVE_TOKEN_SPEND_MARGIN_BPS: u32 = 9_950;

/// Outcome of dispatching one leg: the on-chain hashes plus the real, on-chain
/// fill of the leg's non-USDC asset (whole token units) when the executed quote
/// can supply it. `filled_qty` is the source of truth for the holdings
/// writeback — `None` falls back to the price-derived estimate (mock mode, or a
/// cross-chain hook swap whose destination fill isn't known pre-execution).
pub(super) struct LegDispatch {
    pub(super) tx_hash: String,
    pub(super) cctp_hash: Option<String>,
    pub(super) executed_amount_usdc: Decimal,
    pub(super) filled_qty: Option<f64>,
}

pub(super) async fn dispatch(
    state: &AppState,
    rebalance_id: Uuid,
    kind: LegKind,
    leg: &LegRow,
    user_id: Uuid,
) -> Result<LegDispatch> {
    let caps = RuntimeCapabilities::from_config(&state.config);

    // Opt-in mock mode (tests/CI/offline dev): simulate every leg with a
    // clearly-labelled mock receipt. Unreachable when running against real
    // APIs, so a synthetic hash can never stand in for a real transaction.
    if !caps.real_mode {
        let r = adapters::mock_receipt(kind, leg.id);
        return Ok(LegDispatch {
            tx_hash: r.tx_hash,
            cctp_hash: None,
            executed_amount_usdc: leg.amount_usdc,
            filled_qty: None,
        });
    }

    // Real mode: the leg must clear the route registry and (for swaps) carry a
    // fresh on-chain quote before an `ExecutionTicket` can be minted. There is
    // no real dispatch path without a ticket, so a fake hash cannot be produced
    // here by construction. Blocked routes (USYC disabled, StableFX KYB-gated,
    // missing address/feature/signer) fail closed at `mint`.
    let mut amount_usdc_f64 = leg.amount_usdc.to_f64().unwrap_or(0.0);

    // Clamp a USDC-spending leg's amount to the wallet's *live* balance on the
    // chain it debits, before quoting/minting. Leg amounts are sized once at plan
    // time from a Gateway snapshot; by the time a leg runs, CCTP fees on a prior
    // bridge (minted USDC < planned), earlier spends, a stale snapshot, or an
    // interrupted prior plan can leave less USDC than planned — Circle then
    // rejects with INSUFFICIENT_TOKEN. Re-reading and spending min(planned, live)
    // under-deploys at worst instead of failing the whole plan. Covers BOTH the
    // post-bridge buy-swap and the cross-chain burn (both debit USDC from a
    // wallet). Non-custodial (Circle) path only — that's where `fetch_chain_usdc`
    // reflects the wallet the leg actually spends from.
    if state.config.circle_wallet_exec {
        let spend_chain = match kind {
            LegKind::LocalSwap if leg.src_symbol.as_deref() == Some("USDC") => {
                leg.dest_chain.as_deref().or(leg.src_chain.as_deref())
            }
            LegKind::CrossChainBurn => leg.src_chain.as_deref(),
            _ => None,
        }
        .and_then(ChainKey::parse);
        if let Some(chain) = spend_chain {
            if let Ok(live) = crate::modules::gateway::service::fetch_chain_usdc(
                &state.http,
                &state.config,
                &state.db,
                user_id,
                chain,
            )
            .await
            {
                let spendable = live * LIVE_BALANCE_SPEND_MARGIN;
                if spendable < amount_usdc_f64 {
                    tracing::info!(
                        chain = chain.as_str(),
                        kind = kind.as_str(),
                        planned = amount_usdc_f64,
                        spendable,
                        "clamping USDC-spending leg to live balance"
                    );
                    amount_usdc_f64 = spendable;
                }
            }
        }
    }

    let executed_amount_usdc = Decimal::from_f64(amount_usdc_f64)
        .ok_or_else(|| AppError::BadRequest("USDC amount is outside executable range".into()))?;

    let route_leg = RouteLeg::from_parts(
        kind.as_str(),
        leg.src_chain.clone(),
        leg.dest_chain.clone(),
        leg.src_symbol.clone(),
        leg.dest_symbol.clone(),
        amount_usdc_f64,
    )
    .ok_or_else(|| AppError::Internal(anyhow::anyhow!("unparsable leg kind")))?;

    let now = Utc::now();
    let src_chain = ChainKey::parse(leg.src_chain.as_deref().unwrap_or(""))
        .or_else(|| ChainKey::parse(leg.dest_chain.as_deref().unwrap_or("")));
    let dest_chain = ChainKey::parse(leg.dest_chain.as_deref().unwrap_or("")).or(src_chain);
    let amount_base = usdc_decimal_to_base_units(executed_amount_usdc)?;

    let quote = match kind {
        LegKind::LocalSwap => adapters::swap::quote(&state.config, &route_leg, now).await?,
        _ => {
            let s = src_chain.ok_or_else(|| AppError::BadRequest("missing src_chain".into()))?;
            let d = dest_chain.ok_or_else(|| AppError::BadRequest("missing dest_chain".into()))?;
            ValidatedQuote::cctp_one_to_one(s, d, amount_base, now)
        }
    };
    ensure_sell_quote_is_funded(state, kind, leg, user_id, &quote).await?;

    let ticket = ExecutionTicket::mint(&caps, &state.config, leg.id, &route_leg, quote, now)
        .map_err(|e| AppError::BadRequest(e.detail()))?;
    mark_leg_quoted(state, rebalance_id, leg.id, user_id, leg).await?;

    // The real on-chain fill (from the executed quote) drives the holdings
    // writeback. A USDC↔USDC bridge leg yields `None` here naturally.
    let filled_qty = quote_filled_qty(ticket.quote());

    match kind {
        LegKind::CrossChainBurn => {
            // Recipient embedded in the hook payload: where the destination
            // RebalanceExecutor forwards the minted (and optionally swapped)
            // funds. Non-custodial path → the user's Circle wallet on the dest
            // chain; custodial (EOA) path → the backend signer that holds funds
            // in motion and runs the destination swap (a synthetic/EOA user has
            // no Circle wallet route, so the lookup would be empty).
            let recipient = if state.config.circle_wallet_exec {
                wallet_routes::address_for_user(
                    &state.db,
                    user_id,
                    blockchain_for_chain(ticket.dest_chain()),
                    &state.config.circle_wallet_set_id,
                )
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("recipient lookup: {e}")))?
                .unwrap_or_default()
            } else {
                adapters::cctp::eoa_address_for(&state.config, ticket.dest_chain())
                    .unwrap_or_default()
            };
            if recipient.is_empty() {
                return Err(AppError::Internal(anyhow::anyhow!(
                    "destination wallet address is empty; cannot route mint"
                )));
            }
            // Build the hook from the planned leg. A USDC destination is a plain
            // bridge (tokenOut == dest USDC → the RebalanceExecutor forwards the
            // minted USDC). A non-USDC destination is a hooked swap: the
            // destination RebalanceExecutor swaps USDC→token atomically on mint.
            let pool_fee = hook_pool_fee(
                state,
                ticket.dest_chain(),
                leg.dest_symbol.as_deref(),
                amount_usdc_f64,
                now,
            )
            .await?;
            let hook = build_cross_chain_hook(
                &state.config,
                &recipient,
                ticket.dest_chain(),
                leg.dest_symbol.as_deref(),
                leg.min_out.and_then(|d| d.to_f64()),
                pool_fee,
                now,
            )?;
            let r = adapters::cctp::burn(
                &state.config,
                &state.http,
                &state.db,
                user_id,
                &ticket,
                &hook,
            )
            .await?;
            Ok(LegDispatch {
                tx_hash: r.tx_hash,
                cctp_hash: r.cctp_message_hash,
                executed_amount_usdc,
                filled_qty,
            })
        }
        LegKind::CrossChainMint => {
            // The companion burn leg already produced a tx_hash; read it back
            // through the explicit DAG dependency, not by assuming adjacency.
            let (burn_hash, burn_amount): (String, Decimal) =
                sqlx::query_as::<_, (Option<String>, Decimal)>(
                    "SELECT tx_hash, amount_usdc FROM rebalance_legs
                 WHERE rebalance_id = $1
                   AND kind = 'cross_chain_burn'
                   AND status = 'confirmed'
                   AND leg_index = ANY($2)
                 ORDER BY leg_index ASC
                 LIMIT 1",
                )
                .bind(rebalance_id)
                .bind(&leg.depends_on)
                .fetch_optional(&state.db)
                .await?
                .and_then(|(hash, amount)| hash.map(|h| (h, amount)))
                .ok_or_else(|| {
                    AppError::Internal(anyhow::anyhow!(
                        "cross_chain_mint leg {} has no confirmed burn dependency",
                        leg.leg_index
                    ))
                })?;
            let r = adapters::cctp::mint(
                &state.config,
                &state.http,
                &state.db,
                user_id,
                &ticket,
                &burn_hash,
            )
            .await?;
            Ok(LegDispatch {
                tx_hash: r.tx_hash,
                cctp_hash: None,
                executed_amount_usdc: burn_amount,
                filled_qty,
            })
        }
        LegKind::LocalSwap => {
            let r =
                adapters::swap::execute(&state.config, &state.http, &state.db, user_id, &ticket)
                    .await?;
            Ok(LegDispatch {
                tx_hash: r.tx_hash,
                cctp_hash: r.cctp_message_hash,
                executed_amount_usdc,
                filled_qty,
            })
        }
        // Unreachable: USYC (disabled) and StableFX (KYB-gated) legs fail closed
        // at `mint` above, so real dispatch never reaches them.
        LegKind::ParkUsyc | LegKind::RedeemUsyc | LegKind::FxStablefx => {
            Err(AppError::BadRequest("route is not executable".into()))
        }
    }
}

async fn ensure_sell_quote_is_funded(
    state: &AppState,
    kind: LegKind,
    leg: &LegRow,
    user_id: Uuid,
    quote: &ValidatedQuote,
) -> Result<()> {
    if !state.config.circle_wallet_exec
        || state.config.execution_mock
        || state.config.circle_mock
        || !matches!(kind, LegKind::LocalSwap)
        || leg.dest_symbol.as_deref() != Some(tokens::USDC)
        || leg.src_symbol.as_deref() == Some(tokens::USDC)
    {
        return Ok(());
    }
    let symbol = leg
        .src_symbol
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("sell leg missing source token".into()))?;
    let chain = leg
        .src_chain
        .as_deref()
        .and_then(ChainKey::parse)
        .or_else(|| leg.dest_chain.as_deref().and_then(ChainKey::parse))
        .ok_or_else(|| AppError::BadRequest("sell leg missing chain".into()))?;
    let spec = tokens::token(symbol)
        .ok_or_else(|| AppError::BadRequest(format!("unknown sell token {symbol}")))?;
    let live_units = crate::modules::gateway::service::fetch_chain_token_balance_units(
        &state.http,
        &state.config,
        &state.db,
        user_id,
        chain,
        symbol,
        spec.decimals,
    )
    .await?;
    let spendable_units = apply_bps_margin(live_units, LIVE_TOKEN_SPEND_MARGIN_BPS);
    if quote.amount_in <= spendable_units {
        return Ok(());
    }
    let needed = base_units_to_whole_token(quote.amount_in, spec.decimals);
    let spendable = base_units_to_whole_token(spendable_units, spec.decimals);
    Err(AppError::Conflict(format!(
        "Live {symbol} balance on {} cannot fund the on-chain sell quote. Quote needs {:.8} {symbol}; spendable wallet balance is {:.8}. Build a fresh review after funding or choose a smaller move.",
        chain.as_str(),
        needed,
        spendable
    )))
}

fn usdc_decimal_to_base_units(amount_usdc: Decimal) -> Result<u128> {
    (amount_usdc * Decimal::from(1_000_000_u64))
        .trunc()
        .to_u128()
        .ok_or_else(|| AppError::BadRequest("USDC amount is outside executable range".into()))
}

async fn hook_pool_fee(
    state: &AppState,
    dest_chain: ChainKey,
    dest_symbol: Option<&str>,
    amount_usdc: f64,
    now: chrono::DateTime<Utc>,
) -> Result<u32> {
    let symbol = dest_symbol.unwrap_or(tokens::USDC);
    if symbol.eq_ignore_ascii_case(tokens::USDC) {
        return Ok(3000);
    }
    let route_leg = RouteLeg::from_parts(
        "local_swap",
        Some(dest_chain.as_str().to_string()),
        Some(dest_chain.as_str().to_string()),
        Some(tokens::USDC.to_string()),
        Some(symbol.to_string()),
        amount_usdc,
    )
    .ok_or_else(|| AppError::Internal(anyhow::anyhow!("unparsable hook swap leg")))?;
    let quote = adapters::swap::quote(&state.config, &route_leg, now).await?;
    quote.fee_tier.ok_or_else(|| {
        AppError::BadRequest(format!(
            "{symbol} hook swap on {} did not return a V3 pool fee tier",
            dest_chain.as_str()
        ))
    })
}

/// Build the 160-byte CCTP V2 hook payload for a cross-chain burn.
///
/// USDC destination (or unset symbol): tokenOut = the destination chain's USDC
/// so the RebalanceExecutor takes its passthrough fast path (no swap, minOut
/// irrelevant). Non-USDC destination: tokenOut = the token's destination ERC-20
/// so the executor performs the atomic USDC→token swap on mint. `min_out` is the
/// planner's slippage-protected target in token units, converted to base units.
///
/// Fails closed: a non-USDC destination with no configured ERC-20 (or an
/// unconfigured destination USDC) returns an error rather than emitting a hook
/// with a zero tokenOut that the hardened contract would reject/refund anyway.
fn build_cross_chain_hook(
    cfg: &Config,
    recipient: &str,
    dest_chain: ChainKey,
    dest_symbol: Option<&str>,
    min_out: Option<f64>,
    pool_fee: u32,
    now: chrono::DateTime<Utc>,
) -> Result<crate::modules::rebalance::cross_chain::HookPayload> {
    use crate::modules::rebalance::registry::tokens;

    let deadline = (now.timestamp() + 600) as u64;
    let symbol = dest_symbol.unwrap_or(tokens::USDC);

    if symbol.eq_ignore_ascii_case(tokens::USDC) {
        // Plain USDC bridge — tokenOut is the destination USDC; the executor
        // forwards it directly. minOut is unused on the passthrough path.
        let usdc = tokens::token(tokens::USDC)
            .and_then(|t| t.address_for(cfg, dest_chain))
            .ok_or_else(|| {
                AppError::Internal(anyhow::anyhow!(
                    "USDC address unconfigured on {dest_chain:?}; cannot route bridge hook"
                ))
            })?;
        return Ok(build_hook_payload(recipient, usdc, pool_fee, 0, deadline));
    }

    let spec = tokens::token(symbol)
        .ok_or_else(|| AppError::BadRequest(format!("unknown destination token {symbol}")))?;
    let token_addr = spec.address_for(cfg, dest_chain).ok_or_else(|| {
        AppError::BadRequest(format!(
            "{symbol} has no configured ERC-20 on {dest_chain:?}; cross-chain swap cannot route"
        ))
    })?;

    // Planner min_out is in whole token units; the contract compares against the
    // raw on-chain amount, so scale by the token's decimals. Default to 0 when
    // the planner could not price the leg (the contract still refunds on a real
    // slippage miss, but a priced min_out is the first line of defense).
    let min_out_base = min_out
        .filter(|m| m.is_finite() && *m > 0.0)
        .map(|m| whole_token_to_base_units(m, spec.decimals))
        .unwrap_or(0);

    Ok(build_hook_payload(
        recipient,
        token_addr,
        pool_fee,
        min_out_base,
        deadline,
    ))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::config::Config;
    use crate::error::AppError;
    use crate::modules::rebalance::models::ChainKey;

    use super::build_cross_chain_hook;

    fn hook_cfg() -> Config {
        let mut cfg = crate::config::test_config();
        cfg.chains[ChainKey::Base.index()].usdc =
            "0x036CbD53842c5426634e7929541eC2318f3dCF7e".into();
        cfg.set_token_address(
            "ETH",
            ChainKey::Base,
            "0x4200000000000000000000000000000000000006",
        );
        cfg
    }

    #[test]
    fn cross_chain_hook_usdc_dest_uses_passthrough() {
        let cfg = hook_cfg();
        let hook = build_cross_chain_hook(
            &cfg,
            "0xRecipient",
            ChainKey::Base,
            Some("USDC"),
            None,
            3000,
            Utc::now(),
        )
        .unwrap();
        // tokenOut == dest USDC → contract takes the passthrough fast path.
        assert_eq!(hook.token_out, cfg.chain(ChainKey::Base).usdc);
        assert_eq!(hook.min_out, 0);
    }

    #[test]
    fn cross_chain_hook_none_symbol_defaults_to_usdc() {
        let cfg = hook_cfg();
        let hook =
            build_cross_chain_hook(&cfg, "0xR", ChainKey::Base, None, None, 3000, Utc::now())
                .unwrap();
        assert_eq!(hook.token_out, cfg.chain(ChainKey::Base).usdc);
    }

    #[test]
    fn cross_chain_hook_volatile_dest_uses_token_and_scales_min_out() {
        let cfg = hook_cfg();
        // ETH = 18 decimals; planner min_out of 0.5 ETH → 5e17 base units.
        let hook = build_cross_chain_hook(
            &cfg,
            "0xR",
            ChainKey::Base,
            Some("ETH"),
            Some(0.5),
            500,
            Utc::now(),
        )
        .unwrap();
        assert_eq!(hook.token_out, "0x4200000000000000000000000000000000000006");
        assert_eq!(hook.min_out, 500_000_000_000_000_000);
        assert_eq!(hook.pool_fee, 500);
    }

    #[test]
    fn cross_chain_hook_fails_closed_without_dest_erc20() {
        let mut cfg = hook_cfg();
        cfg.set_token_address("ETH", ChainKey::Base, "");
        let err = build_cross_chain_hook(
            &cfg,
            "0xR",
            ChainKey::Base,
            Some("ETH"),
            Some(0.5),
            3000,
            Utc::now(),
        )
        .unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn cross_chain_hook_missing_min_out_defaults_to_zero() {
        let cfg = hook_cfg();
        let hook = build_cross_chain_hook(
            &cfg,
            "0xR",
            ChainKey::Base,
            Some("ETH"),
            None,
            10000,
            Utc::now(),
        )
        .unwrap();
        assert_eq!(hook.token_out, "0x4200000000000000000000000000000000000006");
        assert_eq!(hook.min_out, 0);
        assert_eq!(hook.pool_fee, 10000);
    }
}
