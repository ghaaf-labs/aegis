//! Rebalance plan executor.
//!
//! Walks a persisted `rebalances` row's legs in order, dispatches each by
//! `LegKind`, updates the DB on every transition, and broadcasts
//! `rebalance.leg.update` SSE events filtered to the owning user.
//!
//! Failure semantics: a leg whose funds had not yet moved fails the plan
//! cleanly (`failed`, nothing stranded). A leg whose funds already moved (a
//! bridge mint landed USDC, but the acquiring swap then failed or its funding
//! never credited) is marked `stranded_asset`: the plan still ends `failed`,
//! but the idle USDC stays visible via the Gateway unified balance and a
//! follow-up rebalance replans only the still-needed exposure
//! (`remaining_delta_after_strand`). Resume is idempotent — a re-walk skips
//! already-`confirmed` legs and caps per-leg submit attempts so a persistently
//! reverting leg can't spin forever.

mod ledger;
mod leg_status;
mod legs;
mod stranding;

pub use stranding::{remaining_delta_after_strand, RemainingDelta, StrandedLeg};

use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use uuid::Uuid;

use crate::config::Config;
use crate::error::{AppError, Result};
use crate::modules::rebalance::adapters;
use crate::modules::rebalance::cross_chain::build_hook_payload;
use crate::modules::rebalance::models::{ChainKey, LegKind, PlannedLeg};
use crate::modules::rebalance::quote::ValidatedQuote;
use crate::modules::rebalance::registry::{
    capabilities::RuntimeCapabilities, route::RouteLeg, ticket::ExecutionTicket,
};
use crate::modules::sse::{RebalancePlanPayload, SseEvent};
use crate::modules::wallet_routes;
use crate::router::AppState;

use leg_status::{
    bump_attempt_count, mark_leg_confirmed, mark_leg_failed, mark_leg_stranded, mark_leg_submitted,
};
use legs::{blockchain_for_chain, parse_kind, quote_filled_qty, LegRow, MAX_LEG_ATTEMPTS};
use stranding::{idempotency_key_for_leg, leg_strands_funds_on_failure, pending_funding_dependency, protocol_fee_notional_from_legs};

/// Persist a planned set of legs as a new `rebalances` + `rebalance_legs`
/// rows. Status starts as `planned`; the user must approve via
/// `POST /rebalance/:id/execute` to transition into `approved → executing`.
///
/// `total_gas_usdc` is the sum of the Paymaster fee estimate across each
/// distinct destination chain in the plan — what the user sees in the
/// approval modal before signing.
pub async fn create_plan(
    state: &AppState,
    portfolio_id: Uuid,
    decision_id: Uuid,
    legs: &[PlannedLeg],
) -> Result<Uuid> {
    let total_gas_usdc = estimate_total_gas(state, legs).await;

    // Tag the plan with the mode it will execute in so public metrics can count
    // only real, completed executions (migration 0033).
    let execution_mode = if state.config.execution_mock || state.config.circle_mock {
        "mock"
    } else {
        "real"
    };

    let mut tx = state.db.begin().await?;
    let rebalance_id: Uuid = sqlx::query_scalar(
        "INSERT INTO rebalances (portfolio_id, decision_id, status, total_legs, total_gas_usdc, execution_mode)
         VALUES ($1, $2, 'planned', $3, $4, $5)
         RETURNING id",
    )
    .bind(portfolio_id)
    .bind(decision_id)
    .bind(legs.len() as i32)
    .bind(total_gas_usdc)
    .bind(execution_mode)
    .fetch_one(&mut *tx)
    .await?;

    for leg in legs {
        // Stamp the deterministic idempotency key at plan time so a resumed or
        // retried walk recomputes the same value and the UNIQUE index rejects a
        // double-submit. The key is fixed by the plan, not by the submit, so it
        // is stable across attempts.
        let idempotency_key = idempotency_key_for_leg(
            rebalance_id,
            leg.leg_index,
            leg.kind.as_str(),
            leg.src_symbol.as_deref(),
            leg.dest_symbol.as_deref(),
            leg.amount_usdc,
        );
        sqlx::query(
            "INSERT INTO rebalance_legs
               (rebalance_id, leg_index, kind, src_chain, dest_chain,
                src_symbol, dest_symbol, amount_usdc, min_out, status, idempotency_key)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending', $10)",
        )
        .bind(rebalance_id)
        .bind(leg.leg_index)
        .bind(leg.kind.as_str())
        .bind(leg.src_chain.map(|c| c.as_str()))
        .bind(leg.dest_chain.map(|c| c.as_str()))
        .bind(leg.src_symbol.as_deref())
        .bind(leg.dest_symbol.as_deref())
        .bind(leg.amount_usdc)
        .bind(leg.min_out)
        .bind(&idempotency_key)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    // Broadcast plan-created event so the UI can swap to plan view.
    let user_id = user_for_portfolio(state, portfolio_id).await?;
    let _ = state
        .sse
        .send(SseEvent::RebalancePlanCreated(RebalancePlanPayload {
            user_id,
            id: rebalance_id,
            portfolio_id,
            decision_id,
            status: "planned".into(),
            total_legs: legs.len() as i32,
            completed_legs: 0,
            total_gas_usdc: Some(total_gas_usdc),
            created_at: Utc::now(),
        }));

    Ok(rebalance_id)
}

/// Sum the Paymaster fee estimate across distinct chains a plan touches.
/// Conservative — for a single user-action UX we show the gross USDC the
/// user might spend, not a per-leg breakdown.
async fn estimate_total_gas(state: &AppState, legs: &[PlannedLeg]) -> f64 {
    use crate::modules::paymaster::service::estimate;
    use std::collections::HashSet;

    let mut chains: HashSet<ChainKey> = HashSet::new();
    for leg in legs {
        // A cross-chain leg pays gas on BOTH ends (source approve/burn + dest
        // mint), so collect both chains — `dest.or(src)` would drop the source
        // side and under-quote the preview persisted as `total_gas_usdc`.
        for c in [leg.src_chain, leg.dest_chain].into_iter().flatten() {
            chains.insert(c);
        }
    }
    let mut total = 0.0;
    for c in chains {
        // The paymaster estimates per `ChainKey`: Arc gas is native USDC, every
        // other EVM execution chain (Base/Eth/Arb/Avax) pays ETH-style gas via
        // the live `eth_gasPrice` path.
        if let Ok(e) = estimate(&state.config, c, "rebalance").await {
            total += e.fee_usdc;
        }
    }
    total
}

/// User-approved execution. Transitions `planned → approved → executing`
/// atomically — the SQL update only matches a row in `planned` state, so
/// concurrent approval calls return `Conflict` instead of double-spawning.
pub async fn approve_and_execute(state: AppState, rebalance_id: Uuid) -> Result<()> {
    let portfolio_id: Option<Uuid> = sqlx::query_scalar(
        "UPDATE rebalances
            SET status = 'executing', approved_at = NOW()
          WHERE id = $1 AND status = 'planned'
          RETURNING portfolio_id",
    )
    .bind(rebalance_id)
    .fetch_optional(&state.db)
    .await?;
    let portfolio_id = portfolio_id.ok_or_else(|| {
        AppError::Conflict(format!(
            "rebalance {rebalance_id} not in 'planned' state or already approved"
        ))
    })?;

    let user_id = user_for_portfolio(&state, portfolio_id).await?;
    let st = state.clone();
    tokio::spawn(async move {
        use futures_util::FutureExt;
        // Catch a *panic* (e.g. an unexpected `unwrap` deep in an adapter or a
        // malformed external response) so it can't silently abort the task and
        // leave the `rebalances` row stuck in `executing` forever. Both a
        // returned `Err` and a panic fall through to the same failure cleanup.
        let outcome = std::panic::AssertUnwindSafe(walk_legs(&st, rebalance_id, user_id))
            .catch_unwind()
            .await;
        let failure_reason = match outcome {
            Ok(Ok(())) => None,
            Ok(Err(e)) => Some(format!("{e}")),
            Err(_panic) => Some("rebalance executor panicked".to_string()),
        };
        let Some(reason) = failure_reason else {
            return;
        };

        tracing::error!(?rebalance_id, reason = %reason, "rebalance walk failed");
        crate::modules::observability::counters::record_rebalance_failed();
        let _ = sqlx::query(
            "UPDATE rebalances SET status = 'failed', failure_reason = $2,
                                   completed_at = NOW()
             WHERE id = $1 AND status = 'executing'",
        )
        .bind(rebalance_id)
        .bind(&reason)
        .execute(&st.db)
        .await;

        // If a protocol fee was recorded before the failure (e.g. partial
        // success, or a future per-leg billing path), reverse it. No-op
        // when the rebalance failed before fee recording.
        if let Err(refund_err) = crate::modules::billing::service::refund_protocol_fee(
            &st.db,
            &st.config,
            rebalance_id,
            &reason,
        )
        .await
        {
            tracing::warn!(
                ?rebalance_id,
                error = %refund_err,
                "billing: refund_protocol_fee failed after rebalance failure"
            );
        }
    });
    Ok(())
}

pub(super) async fn user_for_portfolio(state: &AppState, portfolio_id: Uuid) -> Result<Uuid> {
    let user_id: Uuid = sqlx::query_scalar("SELECT user_id FROM portfolios WHERE id = $1")
        .bind(portfolio_id)
        .fetch_one(&state.db)
        .await?;
    Ok(user_id)
}

async fn walk_legs(state: &AppState, rebalance_id: Uuid, user_id: Uuid) -> Result<()> {
    let portfolio_id: Uuid =
        sqlx::query_scalar("SELECT portfolio_id FROM rebalances WHERE id = $1")
            .bind(rebalance_id)
            .fetch_one(&state.db)
            .await?;

    let legs: Vec<LegRow> = sqlx::query_as(
        "SELECT id, leg_index, kind, src_chain, dest_chain, src_symbol,
                dest_symbol, amount_usdc, min_out, status, attempt_count
         FROM rebalance_legs
         WHERE rebalance_id = $1
         ORDER BY leg_index ASC",
    )
    .bind(rebalance_id)
    .fetch_all(&state.db)
    .await?;

    // Track which legs have already settled this run so a failure can decide
    // whether funds moved (and the leg should strand) or not.
    let mut confirmed_so_far: Vec<LegRow> = Vec::new();

    for leg in &legs {
        let kind = parse_kind(&leg.kind)?;

        // Reconcile-on-restart: a resumed or retried walk must never re-submit a
        // leg that already confirmed. The per-leg DB state machine is the source
        // of truth — skip confirmed legs so a confirmed CCTP burn/swap is never
        // double-submitted. (Stranded legs also confirmed their fund movement;
        // they're left as-is for the follow-up replan, not retried here.)
        if leg.status == "confirmed" {
            confirmed_so_far.push(leg.clone());
            continue;
        }

        // Cap runaway retries: a leg that has been submitted `MAX_LEG_ATTEMPTS`
        // times across resumes without confirming is failed rather than
        // dispatched again, so a persistently-reverting leg can't spin forever
        // (each resume would otherwise bump the counter and re-submit).
        if leg.attempt_count >= MAX_LEG_ATTEMPTS {
            let reason =
                format!("leg exceeded {MAX_LEG_ATTEMPTS} submit attempts without confirming");
            mark_leg_failed(state, rebalance_id, leg.id, user_id, leg, &reason).await?;
            return Err(AppError::Internal(anyhow::anyhow!(reason)));
        }

        // Post-funding confirmation (B1): if this leg spends USDC on a chain a
        // prior confirmed bridge/mint just delivered to, wait for that USDC to
        // actually credit before submitting — otherwise the swap races the mint
        // and Circle returns FAILED. This is non-custodial-only by construction:
        // `wait_for_usdc_balance` polls the *user's* Circle/Gateway balance,
        // which is exactly where the non-custodial mint lands. On the EOA path
        // the mint credits the backend EOA atomically with its receipt (no Circle
        // indexer in the loop), and the dependent swap's own on-chain allowance
        // confirmation guards the residual node-state lag — so removing this gate
        // would make the EOA path wait on a balance that never moves.
        if state.config.circle_wallet_exec
            && !(state.config.execution_mock || state.config.circle_mock)
        {
            if let Some((fund_chain, min_usdc)) = pending_funding_dependency(leg, &confirmed_so_far)
            {
                if let Err(e) = crate::modules::wallet::circle_exec::wait_for_usdc_balance(
                    &state.http,
                    &state.config,
                    &state.db,
                    user_id,
                    fund_chain,
                    min_usdc,
                )
                .await
                {
                    // The prior bridge already delivered USDC to the wallet, so
                    // it's idle cash (visible via the Gateway balance) — mark the
                    // dependent leg stranded so a follow-up replan re-acquires the
                    // target, rather than failing it as if no funds moved.
                    let reason = format!("{e}");
                    mark_leg_stranded(state, rebalance_id, leg.id, user_id, leg, &reason).await?;
                    return Err(e);
                }
            }
        }

        // Bump the attempt counter on every submit so retries are observable and
        // a runaway leg can be capped. Done before the network call so a crash
        // mid-submit still records the attempt.
        bump_attempt_count(state, leg.id).await?;
        mark_leg_submitted(state, rebalance_id, leg.id, user_id, leg).await?;

        let LegDispatch {
            tx_hash,
            cctp_hash,
            filled_qty,
        } = match dispatch(state, rebalance_id, kind, leg, user_id).await {
            Ok(v) => v,
            Err(e) => {
                let reason = format!("{e}");
                if leg_strands_funds_on_failure(kind, leg, &confirmed_so_far) {
                    // Funds already moved (e.g. a bridge mint landed USDC) but
                    // the final action failed. The idle USDC stays in the user's
                    // wallet and is surfaced by the Gateway unified balance — the
                    // single source of truth for wallet cash — so we only mark
                    // the leg stranded for the follow-up replan. (Writing it as a
                    // USDC `allocations` row would double-count it against the
                    // Gateway balance the cash model already adds in.)
                    mark_leg_stranded(state, rebalance_id, leg.id, user_id, leg, &reason).await?;
                } else {
                    mark_leg_failed(state, rebalance_id, leg.id, user_id, leg, &reason).await?;
                }
                return Err(e);
            }
        };

        mark_leg_confirmed(
            state,
            rebalance_id,
            leg.id,
            user_id,
            leg,
            &tx_hash,
            cctp_hash.as_deref(),
        )
        .await?;
        confirmed_so_far.push(leg.clone());

        // Mirror the confirmed leg into the holdings ledger (sell reduces /
        // buy increments the allocation, recompute totals once). Without this
        // Portfolio Value stays $0 after real swaps confirm.
        ledger::apply_leg_writeback(state, portfolio_id, kind, leg, filled_qty).await;

        sqlx::query(
            "UPDATE rebalances
                SET completed_legs = completed_legs + 1
                WHERE id = $1",
        )
        .bind(rebalance_id)
        .execute(&state.db)
        .await?;
    }

    sqlx::query(
        "UPDATE rebalances
            SET status = 'completed', completed_at = NOW()
            WHERE id = $1",
    )
    .bind(rebalance_id)
    .execute(&state.db)
    .await?;

    // Record the 25 bps protocol fee once for the executed economic notional.
    // CCTP mint legs are the receive-side accounting event for a burn leg, not
    // a second user-initiated movement. Billing them would double-charge bridge
    // plans and disagree with the review UI/history totals.
    let plan_total = protocol_fee_notional_from_legs(&legs);
    crate::modules::observability::counters::record_rebalance_succeeded(plan_total);
    ledger::settle_protocol_fee(state, rebalance_id, portfolio_id, plan_total).await?;

    Ok(())
}

/// Outcome of dispatching one leg: the on-chain hashes plus the real, on-chain
/// fill of the leg's non-USDC asset (whole token units) when the executed quote
/// can supply it. `filled_qty` is the source of truth for the holdings
/// writeback — `None` falls back to the price-derived estimate (mock mode, or a
/// cross-chain hook swap whose destination fill isn't known pre-execution).
struct LegDispatch {
    tx_hash: String,
    cctp_hash: Option<String>,
    filled_qty: Option<f64>,
}

async fn dispatch(
    state: &AppState,
    rebalance_id: Uuid,
    kind: LegKind,
    leg: &LegRow,
    user_id: Uuid,
) -> Result<LegDispatch> {
    let _ = rebalance_id;
    let caps = RuntimeCapabilities::from_config(&state.config);

    // Opt-in mock mode (tests/CI/offline dev): simulate every leg with a
    // clearly-labelled mock receipt. Unreachable when running against real
    // APIs, so a synthetic hash can never stand in for a real transaction.
    if !caps.real_mode {
        let r = adapters::mock_receipt(kind, leg.id);
        return Ok(LegDispatch {
            tx_hash: r.tx_hash,
            cctp_hash: None,
            filled_qty: None,
        });
    }

    // Real mode: the leg must clear the route registry and (for swaps) carry a
    // fresh on-chain quote before an `ExecutionTicket` can be minted. There is
    // no real dispatch path without a ticket, so a fake hash cannot be produced
    // here by construction. Blocked routes (USYC disabled, StableFX KYB-gated,
    // missing address/feature/signer) fail closed at `mint`.
    let amount_usdc_f64 = leg.amount_usdc.to_f64().unwrap_or(0.0);
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
    let amount_base = (amount_usdc_f64 * 1_000_000.0) as u128;

    let quote = match kind {
        LegKind::LocalSwap => adapters::swap::quote(&state.config, &route_leg, now).await?,
        _ => {
            let s = src_chain.ok_or_else(|| AppError::BadRequest("missing src_chain".into()))?;
            let d = dest_chain.ok_or_else(|| AppError::BadRequest("missing dest_chain".into()))?;
            ValidatedQuote::cctp_one_to_one(s, d, amount_base, now)
        }
    };

    let ticket = ExecutionTicket::mint(&caps, &state.config, leg.id, &route_leg, quote, now)
        .map_err(|e| AppError::BadRequest(e.detail()))?;

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
            let hook = build_cross_chain_hook(
                &state.config,
                &recipient,
                ticket.dest_chain(),
                leg.dest_symbol.as_deref(),
                leg.min_out.and_then(|d| d.to_f64()),
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
                filled_qty,
            })
        }
        LegKind::CrossChainMint => {
            // The companion burn leg already produced a tx_hash; read it back.
            let burn_hash = sqlx::query_scalar::<_, Option<String>>(
                "SELECT tx_hash FROM rebalance_legs
                 WHERE rebalance_id = (SELECT rebalance_id FROM rebalance_legs WHERE id = $1)
                   AND kind = 'cross_chain_burn'
                   AND leg_index = $2 - 1",
            )
            .bind(leg.id)
            .bind(leg.leg_index)
            .fetch_optional(&state.db)
            .await?
            .flatten()
            .unwrap_or_default();
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
        return Ok(build_hook_payload(recipient, usdc, 3000, 0, deadline));
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
        .map(|m| (m * 10f64.powi(spec.decimals as i32)) as u128)
        .unwrap_or(0);

    Ok(build_hook_payload(
        recipient,
        token_addr,
        3000,
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
        cfg.weth_base = "0x4200000000000000000000000000000000000006".into();
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
            build_cross_chain_hook(&cfg, "0xR", ChainKey::Base, None, None, Utc::now()).unwrap();
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
            Utc::now(),
        )
        .unwrap();
        assert_eq!(hook.token_out, cfg.weth_base);
        assert_eq!(hook.min_out, 500_000_000_000_000_000);
        assert_eq!(hook.pool_fee, 3000);
    }

    #[test]
    fn cross_chain_hook_fails_closed_without_dest_erc20() {
        let mut cfg = hook_cfg();
        cfg.weth_base = String::new();
        let err = build_cross_chain_hook(
            &cfg,
            "0xR",
            ChainKey::Base,
            Some("ETH"),
            Some(0.5),
            Utc::now(),
        )
        .unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn cross_chain_hook_missing_min_out_defaults_to_zero() {
        let cfg = hook_cfg();
        let hook =
            build_cross_chain_hook(&cfg, "0xR", ChainKey::Base, Some("ETH"), None, Utc::now())
                .unwrap();
        assert_eq!(hook.token_out, cfg.weth_base);
        assert_eq!(hook.min_out, 0);
    }
}
