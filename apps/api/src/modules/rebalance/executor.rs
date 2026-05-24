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

use chrono::Utc;
use uuid::Uuid;

use crate::config::Config;
use crate::error::{AppError, Result};
use crate::modules::rebalance::adapters;
use crate::modules::rebalance::cross_chain::build_hook_payload;
use crate::modules::rebalance::models::{ChainKey, LegKind, PlannedLeg};
use crate::modules::rebalance::quote::ValidatedQuote;
use crate::modules::rebalance::registry::{
    capabilities::RuntimeCapabilities, route::RouteLeg, ticket::ExecutionTicket, tokens,
};
use crate::modules::sse::{RebalanceLegPayload, RebalancePlanPayload, SseEvent};
use crate::modules::wallet_routes;
use crate::router::AppState;

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

async fn user_for_portfolio(state: &AppState, portfolio_id: Uuid) -> Result<Uuid> {
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

        let mut holdings_changed = false;

        // Sell-side holdings writeback: a successful non-USDC → USDC leg must
        // reduce the sold allocation before the dashboard reloads. Tax lot
        // disposal is separate bookkeeping and cannot be the holdings source
        // of truth.
        if is_sell_leg(kind, leg) {
            if let Err(e) =
                record_allocation_disposal_for_leg(state, portfolio_id, leg, filled_qty).await
            {
                tracing::warn!(leg_id=?leg.id, error=%e, "allocations: disposal writeback failed");
            } else {
                holdings_changed = true;
            }

            // Tax lot disposal: a successful sell leg that produced USDC
            // closes the oldest matching open lots FIFO. Best-effort —
            // failures here log + continue rather than rolling back the
            // already-confirmed leg.
            if let Err(e) = record_tax_disposal_for_leg(state, portfolio_id, leg).await {
                tracing::warn!(leg_id=?leg.id, error=%e, "tax: lot disposal failed");
            }
        }

        // Buy-side holdings writeback: USDC → asset legs increment the
        // corresponding allocations row. Without this Portfolio Value
        // stays $0 even after real on-chain swaps confirm, which makes
        // the entire dashboard look broken to the user. Recompute the
        // portfolio total after every buy so a *partial* plan (e.g.
        // USYC leg reverts on Arc but BTC/ETH/SOL already settled) still
        // surfaces the holdings the user actually owns.
        if is_buy_leg(kind, leg) {
            if let Err(e) = record_acquisition_for_leg(state, portfolio_id, leg, filled_qty).await {
                tracing::warn!(leg_id=?leg.id, error=%e, "allocations: acquisition writeback failed");
            } else {
                holdings_changed = true;
            }
        }

        if holdings_changed {
            if let Err(e) = recompute_portfolio_values(state, portfolio_id).await {
                tracing::warn!(?portfolio_id, error=%e, "portfolios: value recompute failed");
            };
        }

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

    if plan_total > 0.0 {
        let payer_address = wallet_routes::arc_address_for_portfolio(
            &state.db,
            portfolio_id,
            &state.config.circle_wallet_set_id,
        )
        .await?
        .unwrap_or_default();

        if payer_address.is_empty() && !(state.config.execution_mock || state.config.circle_mock) {
            return Err(AppError::Internal(anyhow::anyhow!(
                "cannot settle protocol fee: Arc wallet route missing for portfolio {portfolio_id}"
            )));
        }

        let payer_address = if payer_address.is_empty() {
            "0x0000000000000000000000000000000000000000"
        } else {
            payer_address.as_str()
        };

        let settlement_tx = crate::modules::billing::service::settle_protocol_fee_via_nanopayments(
            &state.config,
            payer_address,
            plan_total,
        )
        .await
        .ok()
        .flatten();

        if let Err(e) = crate::modules::billing::service::record_protocol_fee(
            &state.db,
            rebalance_id,
            plan_total,
            settlement_tx.as_deref(),
        )
        .await
        {
            tracing::warn!(rebalance_id=?rebalance_id, error=%e, "billing: protocol fee record failed");
        }
    }

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

/// The real on-chain fill of a quote's non-USDC asset, in whole token units,
/// taken from the quoter's `expected_asset_units` (the pool's real exchange
/// rate). `None` for a pure USDC↔USDC bridge or an un-priced/zero quote.
fn quote_filled_qty(quote: &ValidatedQuote) -> Option<f64> {
    let symbol = if !quote.src_token.eq_ignore_ascii_case(tokens::USDC) {
        quote.src_token.as_str()
    } else if !quote.dest_token.eq_ignore_ascii_case(tokens::USDC) {
        quote.dest_token.as_str()
    } else {
        return None;
    };
    if quote.expected_asset_units == 0 {
        return None;
    }
    let decimals = tokens::token(symbol)?.decimals;
    let qty = quote.expected_asset_units as f64 / 10f64.powi(i32::from(decimals));
    (qty.is_finite() && qty > 0.0).then_some(qty)
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
    let route_leg = RouteLeg::from_parts(
        kind.as_str(),
        leg.src_chain.clone(),
        leg.dest_chain.clone(),
        leg.src_symbol.clone(),
        leg.dest_symbol.clone(),
        leg.amount_usdc,
    )
    .ok_or_else(|| AppError::Internal(anyhow::anyhow!("unparsable leg kind")))?;

    let now = Utc::now();
    let src_chain = ChainKey::parse(leg.src_chain.as_deref().unwrap_or(""))
        .or_else(|| ChainKey::parse(leg.dest_chain.as_deref().unwrap_or("")));
    let dest_chain = ChainKey::parse(leg.dest_chain.as_deref().unwrap_or("")).or(src_chain);
    let amount_base = (leg.amount_usdc * 1_000_000.0) as u128;

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
                leg.min_out,
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

fn blockchain_for_chain(chain: ChainKey) -> &'static str {
    wallet_routes::blockchain_for_chain(chain)
}

fn parse_kind(s: &str) -> Result<LegKind> {
    Ok(match s {
        "local_swap" => LegKind::LocalSwap,
        "cross_chain_burn" => LegKind::CrossChainBurn,
        "cross_chain_mint" => LegKind::CrossChainMint,
        "park_usyc" => LegKind::ParkUsyc,
        "redeem_usyc" => LegKind::RedeemUsyc,
        "fx_stablefx" => LegKind::FxStablefx,
        other => return Err(AppError::BadRequest(format!("unknown leg kind: {other}"))),
    })
}

#[derive(sqlx::FromRow, Clone)]
struct LegRow {
    id: Uuid,
    leg_index: i32,
    kind: String,
    src_chain: Option<String>,
    dest_chain: Option<String>,
    src_symbol: Option<String>,
    dest_symbol: Option<String>,
    amount_usdc: f64,
    /// Planner-computed minimum destination output (token units, slippage
    /// applied). Set on CrossChainBurn hook-swap legs; `None` for plain
    /// USDC bridges. Used to size the hook's `min_out`.
    min_out: Option<f64>,
    /// Per-leg state-machine status (`pending`/`submitted`/`confirmed`/`failed`).
    /// Read on every walk so a resumed plan can skip legs already confirmed
    /// rather than re-submitting them. NOT NULL DEFAULT in the schema.
    status: String,
    /// How many times this leg has been submitted (bumped before each dispatch).
    /// Read on every walk so a persistently-reverting leg can be capped at
    /// `MAX_LEG_ATTEMPTS` rather than re-dispatching forever across resumes.
    /// NOT NULL DEFAULT 0 (migration 0038).
    attempt_count: i32,
}

/// Maximum number of submit attempts for a single leg before it is failed.
/// Bounds runaway retries across resumes (migration 0038's `attempt_count`).
const MAX_LEG_ATTEMPTS: i32 = 5;

fn protocol_fee_notional_from_legs(legs: &[LegRow]) -> f64 {
    legs.iter()
        .filter(|leg| leg.kind != LegKind::CrossChainMint.as_str())
        .map(|leg| leg.amount_usdc)
        .sum()
}

/// Deterministic per-leg fingerprint, stamped once at plan creation and
/// persisted in `rebalance_legs.idempotency_key`.
///
/// Its job is the DB-level `(rebalance_id, idempotency_key)` UNIQUE index: if a
/// plan-creation is ever retried for the same logical leg, the same fingerprint
/// collides instead of admitting a duplicate row. The amount is rounded to whole
/// USDC cents so a sub-cent notional re-fetch maps to the same key. (The
/// at-submit, cross-resume dedup against Circle is separate — that uses the
/// stable leg-id-derived key in `circle_exec`, not this column.)
///
/// Shape: `rebalance_id:leg_index:kind:src>dest:rounded_amount`.
fn idempotency_key_for_leg(
    rebalance_id: Uuid,
    leg_index: i32,
    kind: &str,
    src_symbol: Option<&str>,
    dest_symbol: Option<&str>,
    amount_usdc: f64,
) -> String {
    let src = src_symbol.unwrap_or("none");
    let dest = dest_symbol.unwrap_or("none");
    let rounded_cents = (amount_usdc * 100.0).round() as i64;
    format!("{rebalance_id}:{leg_index}:{kind}:{src}>{dest}:{rounded_cents}")
}

/// A leg whose funds moved but whose final action failed — its asset is stranded
/// as idle USDC in the user's wallet. Used by `remaining_delta_after_strand` to
/// replan the still-needed exposure.
#[derive(Debug, Clone, PartialEq)]
pub struct StrandedLeg {
    pub dest_symbol: String,
    pub amount_usdc: f64,
}

/// The exposure a follow-up rebalance still needs after some legs stranded.
#[derive(Debug, Clone, PartialEq)]
pub struct RemainingDelta {
    pub dest_symbol: String,
    pub amount_usdc: f64,
}

/// Given the original plan and which legs stranded (funds landed as idle USDC
/// instead of reaching their destination asset), compute the per-symbol
/// exposure a follow-up rebalance still needs to acquire.
///
/// Pure: no DB, no side effects — this is the verifiable core of recovery. The
/// returned deltas are *not* auto-executed; they surface for user approval via
/// the same two-gate model. A stranded leg's USDC is already sitting in the
/// wallet, so the follow-up only needs to re-acquire the destination asset for
/// the stranded notional (the bridge/sell portion of the plan has already
/// settled). USDC destinations and non-positive amounts are dropped — there's
/// nothing to re-buy. Same-symbol strands are summed so a split buy that
/// stranded on two chains replans as one delta. Output is sorted by symbol for
/// determinism.
pub fn remaining_delta_after_strand(stranded: &[StrandedLeg]) -> Vec<RemainingDelta> {
    use std::collections::BTreeMap;

    let mut by_symbol: BTreeMap<String, f64> = BTreeMap::new();
    for leg in stranded {
        if leg.amount_usdc <= 0.0 {
            continue;
        }
        if leg.dest_symbol.eq_ignore_ascii_case("USDC") || leg.dest_symbol.is_empty() {
            continue;
        }
        *by_symbol.entry(leg.dest_symbol.clone()).or_insert(0.0) += leg.amount_usdc;
    }

    by_symbol
        .into_iter()
        .map(|(dest_symbol, amount_usdc)| RemainingDelta {
            dest_symbol,
            amount_usdc,
        })
        .collect()
}

/// Whether a failed leg leaves funds stranded as idle USDC in the user's wallet.
///
/// A `cross_chain_mint` whose companion burn already settled means USDC has
/// landed at the destination; if the *acquiring* action (the hook swap on the
/// burn, or a follow-on local swap) then fails, that USDC is stranded — not
/// lost. We mark the leg `stranded_asset` and record the USDC as cash rather
/// than failing the whole plan. A pre-funds-moved failure (e.g. the burn itself
/// reverts, or a local swap reverts before any token leaves the wallet) is a
/// clean halt with nothing stranded.
fn leg_strands_funds_on_failure(kind: LegKind, leg: &LegRow, prior_confirmed: &[LegRow]) -> bool {
    match kind {
        // Plain-bridge baseline: burn → mint → local swap. The mint is the leg
        // that *lands* the destination USDC, so a same-chain swap strands its
        // input USDC only when a companion `CrossChainMint` already confirmed on
        // this swap's chain. Without that preceding bridge, a revert returns the
        // USDC atomically — nothing stranded. (Pairs by chain to the same mint
        // `pending_funding_dependency` waits on.)
        LegKind::LocalSwap => {
            let chain = leg.src_chain.as_deref().and_then(ChainKey::parse);
            chain.is_some()
                && prior_confirmed.iter().any(|l| {
                    l.kind == LegKind::CrossChainMint.as_str()
                        && l.dest_chain.as_deref().and_then(ChainKey::parse) == chain
                })
        }
        // A failed mint means the destination USDC never landed (it's still in
        // CCTP transit, recoverable by re-mint via the existing attestation), and
        // a failed burn leaves the source USDC in place — neither leaves idle
        // cash to strand. (A source-burn confirmation alone does NOT mean the
        // mint landed: that is the mint leg's job.)
        LegKind::CrossChainMint | LegKind::CrossChainBurn => false,
        // Park / FX: funds leave the wallet atomically with the acquire, so a
        // revert returns them — nothing stranded.
        LegKind::ParkUsyc | LegKind::RedeemUsyc | LegKind::FxStablefx => false,
    }
}

/// If this leg spends USDC on a chain that a prior confirmed cross-chain mint
/// just delivered USDC to, return `(chain, min_usdc)` the dependent leg must
/// wait to credit before submitting. This is the timing-race guard (B1): a
/// bridge mint reports `CONFIRMED` before Circle's balance indexer reflects the
/// new USDC, so a swap that spends it would fail closed.
///
/// Pure (DB-free) so the dependency decision is unit-testable. Returns `None`
/// when the leg doesn't spend USDC or no prior mint targeted its spend chain.
fn pending_funding_dependency(leg: &LegRow, confirmed: &[LegRow]) -> Option<(ChainKey, f64)> {
    // Only a USDC-spending leg (a buy swap, or a burn) can race a fresh mint.
    if leg.src_symbol.as_deref() != Some(tokens::USDC) {
        return None;
    }
    let spend_chain = leg.src_chain.as_deref().and_then(ChainKey::parse)?;
    let delivered_here = confirmed.iter().any(|c| {
        c.kind == LegKind::CrossChainMint.as_str()
            && c.dest_chain.as_deref().and_then(ChainKey::parse) == Some(spend_chain)
    });
    if !delivered_here {
        return None;
    }
    Some((spend_chain, leg.amount_usdc))
}

async fn mark_leg_submitted(
    state: &AppState,
    rebalance_id: Uuid,
    leg_id: Uuid,
    user_id: Uuid,
    leg: &LegRow,
) -> Result<()> {
    sqlx::query(
        "UPDATE rebalance_legs SET status = 'submitted', submitted_at = NOW() WHERE id = $1",
    )
    .bind(leg_id)
    .execute(&state.db)
    .await?;
    broadcast_leg(
        state,
        rebalance_id,
        leg_id,
        user_id,
        leg,
        "submitted",
        None,
        None,
    );
    Ok(())
}

async fn mark_leg_confirmed(
    state: &AppState,
    rebalance_id: Uuid,
    leg_id: Uuid,
    user_id: Uuid,
    leg: &LegRow,
    tx_hash: &str,
    cctp_hash: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "UPDATE rebalance_legs SET status = 'confirmed', confirmed_at = NOW(),
                                  tx_hash = $2, cctp_message_hash = $3
         WHERE id = $1",
    )
    .bind(leg_id)
    .bind(tx_hash)
    .bind(cctp_hash)
    .execute(&state.db)
    .await?;

    broadcast_leg(
        state,
        rebalance_id,
        leg_id,
        user_id,
        leg,
        "confirmed",
        Some(tx_hash),
        None,
    );
    Ok(())
}

async fn mark_leg_failed(
    state: &AppState,
    rebalance_id: Uuid,
    leg_id: Uuid,
    user_id: Uuid,
    leg: &LegRow,
    reason: &str,
) -> Result<()> {
    sqlx::query("UPDATE rebalance_legs SET status = 'failed', failure_reason = $2 WHERE id = $1")
        .bind(leg_id)
        .bind(reason)
        .execute(&state.db)
        .await?;
    broadcast_leg(
        state,
        rebalance_id,
        leg_id,
        user_id,
        leg,
        "failed",
        None,
        Some(reason),
    );
    Ok(())
}

/// Increment the leg's submit attempt counter. Called before each network
/// dispatch so retries are observable and a runaway leg can be capped.
async fn bump_attempt_count(state: &AppState, leg_id: Uuid) -> Result<()> {
    sqlx::query("UPDATE rebalance_legs SET attempt_count = attempt_count + 1 WHERE id = $1")
        .bind(leg_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// Mark a leg whose funds moved but whose final action failed. The leg is
/// `failed` (it did not reach its destination asset) and `stranded_asset` (its
/// USDC is now idle cash in the user's wallet). Surfaced for a follow-up
/// approval-gated replan rather than bricking the whole plan.
async fn mark_leg_stranded(
    state: &AppState,
    rebalance_id: Uuid,
    leg_id: Uuid,
    user_id: Uuid,
    leg: &LegRow,
    reason: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE rebalance_legs
            SET status = 'failed', stranded_asset = TRUE, failure_reason = $2
          WHERE id = $1",
    )
    .bind(leg_id)
    .bind(reason)
    .execute(&state.db)
    .await?;
    broadcast_leg(
        state,
        rebalance_id,
        leg_id,
        user_id,
        leg,
        "failed",
        None,
        Some(reason),
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn broadcast_leg(
    state: &AppState,
    rebalance_id: Uuid,
    leg_id: Uuid,
    user_id: Uuid,
    leg: &LegRow,
    status: &str,
    tx_hash: Option<&str>,
    failure_reason: Option<&str>,
) {
    let payload = RebalanceLegPayload {
        user_id,
        id: leg_id,
        rebalance_id,
        leg_index: leg.leg_index,
        kind: leg.kind.clone(),
        src_chain: leg.src_chain.clone(),
        dest_chain: leg.dest_chain.clone(),
        src_symbol: leg.src_symbol.clone(),
        dest_symbol: leg.dest_symbol.clone(),
        amount_usdc: leg.amount_usdc,
        status: status.to_string(),
        tx_hash: tx_hash.map(str::to_string),
        failure_reason: failure_reason.map(str::to_string),
        updated_at: Utc::now(),
    };
    let _ = state.sse.send(SseEvent::RebalanceLegUpdate(payload));
}

// ── Tax lot disposal ──────────────────────────────────────────────────────

fn is_sell_leg(kind: LegKind, leg: &LegRow) -> bool {
    // A sell-side leg moves a non-USDC asset into USDC.
    matches!(
        kind,
        LegKind::LocalSwap | LegKind::RedeemUsyc | LegKind::FxStablefx
    ) && leg.dest_symbol.as_deref() == Some("USDC")
        && leg.src_symbol.as_deref() != Some("USDC")
}

/// A buy-side leg acquires a non-USDC asset for USDC. Covers local swaps,
/// USYC park, EURC FX, and cross-chain burns whose hook performs the
/// destination swap (dest_symbol carries the volatile target).
fn is_buy_leg(kind: LegKind, leg: &LegRow) -> bool {
    let dest = leg.dest_symbol.as_deref().unwrap_or("");
    if dest.is_empty() || dest == "USDC" {
        return false;
    }
    matches!(
        kind,
        LegKind::LocalSwap | LegKind::ParkUsyc | LegKind::FxStablefx | LegKind::CrossChainBurn
    )
}

/// Increment `allocations.quantity` by the asset amount acquired on a buy
/// leg. Best-effort — failures here log and continue; the on-chain leg is
/// already settled.
///
/// `filled_qty` is the real on-chain fill from the executed swap quote (the
/// pool's true exchange rate). It is the source of truth so `allocations.quantity`
/// matches the tokens that actually landed on-chain — not `amount_usdc / mainnet_price`,
/// which diverges badly on testnet pools. Falls back to the price-derived estimate
/// only when the leg can't supply a real fill (mock mode, cross-chain hook swap).
async fn record_acquisition_for_leg(
    state: &AppState,
    portfolio_id: Uuid,
    leg: &LegRow,
    filled_qty: Option<f64>,
) -> Result<()> {
    let Some(symbol) = leg.dest_symbol.as_deref() else {
        return Ok(());
    };
    let spot_price = latest_spot_price_with_stable_fallback(state, symbol).await;
    let Some(acquired_qty) = settled_quantity(filled_qty, leg.amount_usdc, spot_price) else {
        return Ok(());
    };

    let result = sqlx::query(
        "UPDATE allocations
            SET quantity = quantity + $3,
                value_usd = (quantity + $3) * $4,
                updated_at = NOW()
          WHERE portfolio_id = $1 AND asset_symbol = $2",
    )
    .bind(portfolio_id)
    .bind(symbol)
    .bind(acquired_qty)
    .bind(spot_price)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        // No matching allocations row — happens if the target weights changed
        // mid-flight. Insert a fresh row so the holding isn't silently lost.
        sqlx::query(
            "INSERT INTO allocations (id, portfolio_id, asset_symbol, quantity,
                                       target_weight, current_weight, value_usd)
             VALUES ($1, $2, $3, $4, 0, 0, $5)
             ON CONFLICT DO NOTHING",
        )
        .bind(Uuid::new_v4())
        .bind(portfolio_id)
        .bind(symbol)
        .bind(acquired_qty)
        .bind(acquired_qty * spot_price)
        .execute(&state.db)
        .await?;
    }
    Ok(())
}

/// Decrement `allocations.quantity` by the asset amount sold on a sell leg.
/// `filled_qty` is the real token input the swap spent (from the quote); it wins
/// over the price-derived estimate so the sold quantity matches on-chain.
async fn record_allocation_disposal_for_leg(
    state: &AppState,
    portfolio_id: Uuid,
    leg: &LegRow,
    filled_qty: Option<f64>,
) -> Result<()> {
    let Some(symbol) = leg.src_symbol.as_deref() else {
        return Ok(());
    };
    let spot_price = latest_spot_price_with_stable_fallback(state, symbol).await;
    let Some(sold_qty) = settled_quantity(filled_qty, leg.amount_usdc, spot_price) else {
        return Ok(());
    };

    sqlx::query(
        "UPDATE allocations
            SET quantity = GREATEST(quantity - $3, 0),
                value_usd = GREATEST(quantity - $3, 0) * $4,
                updated_at = NOW()
          WHERE portfolio_id = $1 AND asset_symbol = $2",
    )
    .bind(portfolio_id)
    .bind(symbol)
    .bind(sold_qty)
    .bind(spot_price)
    .execute(&state.db)
    .await?;
    Ok(())
}

/// Recompute portfolio totals and allocation weights from allocation values.
/// Called after every confirmed buy/sell writeback so the dashboard and the
/// next planner run read the same post-trade state.
async fn recompute_portfolio_values(state: &AppState, portfolio_id: Uuid) -> Result<()> {
    sqlx::query(
        "UPDATE allocations a
            SET current_weight = CASE
                WHEN totals.total_value_usd > 0
                THEN (a.value_usd / totals.total_value_usd) * 100
                ELSE 0
            END,
            updated_at = NOW()
          FROM (
            SELECT COALESCE(SUM(value_usd), 0)::DOUBLE PRECISION AS total_value_usd
            FROM allocations
            WHERE portfolio_id = $1
          ) totals
          WHERE a.portfolio_id = $1",
    )
    .bind(portfolio_id)
    .execute(&state.db)
    .await?;

    sqlx::query(
        "UPDATE portfolios
            SET total_value_usd = COALESCE(
                (SELECT SUM(value_usd) FROM allocations WHERE portfolio_id = $1),
                0
            ),
            updated_at = NOW()
          WHERE id = $1",
    )
    .bind(portfolio_id)
    .execute(&state.db)
    .await?;
    Ok(())
}

/// Most recent USD price from `price_history` for the given symbol. Returns
/// `None` when the symbol isn't tracked (USYC, etc.). `price_history.price_usd`
/// is `numeric(20,8)` — cast to DOUBLE PRECISION here so sqlx can decode into
/// f64 without raising a type-mismatch.
async fn latest_spot_price(state: &AppState, symbol: &str) -> Option<f64> {
    sqlx::query_scalar::<_, f64>(
        "SELECT price_usd::DOUBLE PRECISION FROM price_history
          WHERE symbol = $1 ORDER BY fetched_at DESC LIMIT 1",
    )
    .bind(symbol)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
}

async fn latest_spot_price_with_stable_fallback(state: &AppState, symbol: &str) -> f64 {
    latest_spot_price(state, symbol).await.unwrap_or({
        if matches!(symbol, "USYC" | "USDC" | "EURC") {
            1.0
        } else {
            0.0
        }
    })
}

fn quantity_for_notional(amount_usdc: f64, spot_price: f64) -> Option<f64> {
    if amount_usdc <= 0.0 || spot_price <= 0.0 {
        return None;
    }
    Some(amount_usdc / spot_price)
}

/// The quantity to write to holdings for a settled leg. The real on-chain fill
/// (`filled_qty`, from the executed quote) is authoritative because it reflects
/// the pool's true exchange rate; the `amount_usdc / spot_price` estimate is a
/// last resort for legs that can't report a fill (mock mode, cross-chain hook
/// swap). This is the fix for holdings showing `amount_usdc / mainnet_price`
/// instead of the tokens that actually landed.
fn settled_quantity(filled_qty: Option<f64>, amount_usdc: f64, spot_price: f64) -> Option<f64> {
    match filled_qty {
        Some(q) if q.is_finite() && q > 0.0 => Some(q),
        _ => quantity_for_notional(amount_usdc, spot_price),
    }
}

async fn record_tax_disposal_for_leg(
    state: &AppState,
    portfolio_id: Uuid,
    leg: &LegRow,
) -> Result<()> {
    let Some(symbol) = leg.src_symbol.as_deref() else {
        return Ok(());
    };
    let allocation_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM allocations WHERE portfolio_id = $1 AND asset_symbol = $2",
    )
    .bind(portfolio_id)
    .bind(symbol)
    .fetch_optional(&state.db)
    .await?;
    let Some(allocation_id) = allocation_id else {
        return Ok(());
    };

    // Best-effort quantity = amount_usdc / current price. The exact realized
    // quantity is known only by the destination-chain swap router; the
    // executor's job is to close enough basis to keep harvest signals
    // accurate, not to be the source of truth for fills.
    let snapshot = crate::modules::market_data::service::fetch_snapshot(state.prices.as_ref())
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("market snapshot: {e}")))?;
    let price = snapshot
        .assets
        .iter()
        .find(|a| a.symbol == symbol)
        .map(|a| a.price_usd)
        .unwrap_or(0.0);
    if price <= 0.0 {
        return Ok(());
    }
    let qty = leg.amount_usdc / price;
    crate::modules::tax::service::record_disposal(state, allocation_id, qty).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leg(kind: LegKind, amount_usdc: f64) -> LegRow {
        LegRow {
            id: Uuid::new_v4(),
            leg_index: 0,
            kind: kind.as_str().to_string(),
            src_chain: None,
            dest_chain: None,
            src_symbol: None,
            dest_symbol: None,
            amount_usdc,
            min_out: None,
            status: "pending".into(),
            attempt_count: 0,
        }
    }

    fn swap_leg(src: &str, dest: &str) -> LegRow {
        LegRow {
            id: Uuid::new_v4(),
            leg_index: 0,
            kind: LegKind::LocalSwap.as_str().to_string(),
            src_chain: Some(ChainKey::Base.as_str().to_string()),
            dest_chain: Some(ChainKey::Base.as_str().to_string()),
            src_symbol: Some(src.to_string()),
            dest_symbol: Some(dest.to_string()),
            amount_usdc: 600.0,
            min_out: None,
            status: "pending".into(),
            attempt_count: 0,
        }
    }

    fn strand(dest: &str, amount: f64) -> StrandedLeg {
        StrandedLeg {
            dest_symbol: dest.to_string(),
            amount_usdc: amount,
        }
    }

    #[test]
    fn protocol_fee_notional_excludes_cctp_mint_receive_side() {
        let legs = vec![
            leg(LegKind::CrossChainBurn, 100.0),
            leg(LegKind::CrossChainMint, 100.0),
            leg(LegKind::LocalSwap, 25.0),
        ];

        assert_eq!(protocol_fee_notional_from_legs(&legs), 125.0);
    }

    #[test]
    fn protocol_fee_notional_counts_single_chain_and_usyc_legs() {
        let legs = vec![
            leg(LegKind::ParkUsyc, 50.0),
            leg(LegKind::RedeemUsyc, 20.0),
            leg(LegKind::FxStablefx, 10.0),
        ];

        assert_eq!(protocol_fee_notional_from_legs(&legs), 80.0);
    }

    #[test]
    fn local_swap_into_usdc_is_sell_not_buy() {
        let sell = swap_leg("BTC", "USDC");

        assert!(is_sell_leg(LegKind::LocalSwap, &sell));
        assert!(!is_buy_leg(LegKind::LocalSwap, &sell));
    }

    #[test]
    fn local_swap_from_usdc_is_buy_not_sell() {
        let buy = swap_leg("USDC", "ETH");

        assert!(is_buy_leg(LegKind::LocalSwap, &buy));
        assert!(!is_sell_leg(LegKind::LocalSwap, &buy));
    }

    #[test]
    fn quantity_for_notional_rejects_zero_or_missing_prices() {
        assert_eq!(quantity_for_notional(600.0, 100_000.0), Some(0.006));
        assert_eq!(quantity_for_notional(0.0, 100_000.0), None);
        assert_eq!(quantity_for_notional(600.0, 0.0), None);
    }

    #[test]
    fn settled_quantity_prefers_real_on_chain_fill_over_price_derived() {
        // The bug: $20 of WETH on a testnet pool actually lands 0.0708 WETH,
        // but amount_usdc / mainnet_price gives ~0.0096. The real fill must win.
        let real_fill = 0.0708;
        let mainnet_price = 2080.0;
        let amount_usdc = 20.0;
        // Price-derived would be far off.
        let price_derived = quantity_for_notional(amount_usdc, mainnet_price).unwrap();
        assert!((price_derived - 0.0096).abs() < 0.0005);
        // settled_quantity returns the real fill, not the price-derived value.
        assert_eq!(
            settled_quantity(Some(real_fill), amount_usdc, mainnet_price),
            Some(real_fill)
        );
    }

    #[test]
    fn settled_quantity_falls_back_to_price_when_no_fill() {
        // No on-chain fill (mock mode / cross-chain hook) → price-derived.
        assert_eq!(settled_quantity(None, 600.0, 100_000.0), Some(0.006));
        // A zero/non-finite fill is ignored in favor of the price-derived value.
        assert_eq!(settled_quantity(Some(0.0), 600.0, 100_000.0), Some(0.006));
        assert_eq!(
            settled_quantity(Some(f64::NAN), 600.0, 100_000.0),
            Some(0.006)
        );
    }

    #[test]
    fn quote_filled_qty_scales_by_token_decimals_for_buy_and_sell() {
        let now = Utc::now();
        // Buy: USDC→ETH, quoter says 0.0708 WETH (18dp base units).
        let mut buy = ValidatedQuote::cctp_one_to_one(ChainKey::Base, ChainKey::Base, 0, now);
        buy.src_token = "USDC".into();
        buy.dest_token = "ETH".into();
        buy.expected_asset_units = 70_800_000_000_000_000; // 0.0708 * 1e18
        let q = quote_filled_qty(&buy).unwrap();
        assert!((q - 0.0708).abs() < 1e-9);

        // Sell: ETH→USDC, quoter says the wallet spends 0.5 WETH.
        let mut sell = ValidatedQuote::cctp_one_to_one(ChainKey::Base, ChainKey::Base, 0, now);
        sell.src_token = "ETH".into();
        sell.dest_token = "USDC".into();
        sell.expected_asset_units = 500_000_000_000_000_000; // 0.5 * 1e18
        assert_eq!(quote_filled_qty(&sell), Some(0.5));
    }

    #[test]
    fn quote_filled_qty_is_none_for_pure_usdc_bridge() {
        // A USDC↔USDC bridge has no non-USDC asset, so there is nothing to record.
        let bridge =
            ValidatedQuote::cctp_one_to_one(ChainKey::Arc, ChainKey::Base, 40_000_000, Utc::now());
        assert_eq!(quote_filled_qty(&bridge), None);
    }

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

    // ── Idempotency key derivation ────────────────────────────────────────

    #[test]
    fn idempotency_key_is_deterministic_for_same_leg() {
        let id = Uuid::new_v4();
        let a = idempotency_key_for_leg(id, 2, "local_swap", Some("USDC"), Some("ETH"), 600.0);
        let b = idempotency_key_for_leg(id, 2, "local_swap", Some("USDC"), Some("ETH"), 600.0);
        assert_eq!(a, b, "same logical leg must derive the same key");
        assert_eq!(a, format!("{id}:2:local_swap:USDC>ETH:60000"));
    }

    #[test]
    fn idempotency_key_rounds_subcent_amount_drift_to_same_key() {
        // A price re-fetch nudges the notional by a fraction of a cent on a
        // resume; the rounded key must still match so we don't double-submit.
        let id = Uuid::new_v4();
        let a = idempotency_key_for_leg(id, 0, "local_swap", Some("USDC"), Some("BTC"), 600.001);
        let b = idempotency_key_for_leg(id, 0, "local_swap", Some("USDC"), Some("BTC"), 599.999);
        assert_eq!(a, b, "sub-cent drift must collapse to the same key");
    }

    #[test]
    fn idempotency_key_differs_across_legs_and_plans() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let base = idempotency_key_for_leg(id1, 0, "local_swap", Some("USDC"), Some("ETH"), 100.0);
        // Different leg index.
        assert_ne!(
            base,
            idempotency_key_for_leg(id1, 1, "local_swap", Some("USDC"), Some("ETH"), 100.0)
        );
        // Different kind.
        assert_ne!(
            base,
            idempotency_key_for_leg(id1, 0, "cross_chain_burn", Some("USDC"), Some("ETH"), 100.0)
        );
        // Different token pair.
        assert_ne!(
            base,
            idempotency_key_for_leg(id1, 0, "local_swap", Some("USDC"), Some("BTC"), 100.0)
        );
        // Different amount (≥ 1 cent).
        assert_ne!(
            base,
            idempotency_key_for_leg(id1, 0, "local_swap", Some("USDC"), Some("ETH"), 100.5)
        );
        // Different rebalance.
        assert_ne!(
            base,
            idempotency_key_for_leg(id2, 0, "local_swap", Some("USDC"), Some("ETH"), 100.0)
        );
    }

    #[test]
    fn idempotency_key_handles_missing_symbols() {
        let id = Uuid::new_v4();
        let k = idempotency_key_for_leg(id, 3, "cross_chain_mint", None, None, 250.0);
        assert_eq!(k, format!("{id}:3:cross_chain_mint:none>none:25000"));
    }

    // ── Resume / skip-confirmed (status-driven, DB-free portion) ──────────

    #[test]
    fn confirmed_leg_status_marks_skip_on_resume() {
        // The resume guard keys off leg.status == "confirmed". A confirmed leg
        // must be skippable; a pending one must not.
        let mut confirmed = swap_leg("USDC", "ETH");
        confirmed.status = "confirmed".into();
        assert_eq!(confirmed.status, "confirmed");

        let pending = swap_leg("USDC", "ETH");
        assert_eq!(pending.status, "pending");
        assert_ne!(pending.status, "confirmed");
    }

    // ── Strand decision (which failures leave funds stranded) ─────────────

    fn mint_leg(dest: ChainKey, amount: f64) -> LegRow {
        LegRow {
            id: Uuid::new_v4(),
            leg_index: 1,
            kind: LegKind::CrossChainMint.as_str().to_string(),
            src_chain: Some(ChainKey::Arc.as_str().to_string()),
            dest_chain: Some(dest.as_str().to_string()),
            src_symbol: Some("USDC".into()),
            dest_symbol: Some("USDC".into()),
            amount_usdc: amount,
            min_out: None,
            status: "confirmed".into(),
            attempt_count: 0,
        }
    }

    #[test]
    fn dependent_swap_waits_for_minted_usdc_on_same_chain() {
        // Mint delivered USDC to Base; the next leg is a USDC→ETH swap on Base.
        let confirmed = vec![mint_leg(ChainKey::Base, 40.0)];
        let dep = swap_leg("USDC", "ETH"); // Base→Base, amount 600
        assert_eq!(
            pending_funding_dependency(&dep, &confirmed),
            Some((ChainKey::Base, 600.0))
        );
    }

    #[test]
    fn no_funding_wait_without_prior_mint_on_that_chain() {
        // A mint to Arc doesn't gate a Base swap.
        let confirmed = vec![mint_leg(ChainKey::Arc, 40.0)];
        let dep = swap_leg("USDC", "ETH"); // Base
        assert_eq!(pending_funding_dependency(&dep, &confirmed), None);
        // No prior mint at all → no wait.
        assert_eq!(pending_funding_dependency(&dep, &[]), None);
    }

    #[test]
    fn sell_leg_does_not_wait_for_funding() {
        // A sell spends the non-USDC asset, not bridged USDC → no funding wait.
        let confirmed = vec![mint_leg(ChainKey::Base, 40.0)];
        let sell = swap_leg("ETH", "USDC");
        assert_eq!(pending_funding_dependency(&sell, &confirmed), None);
    }

    #[test]
    fn mint_failure_does_not_strand_even_after_burn() {
        // A failed mint means the destination USDC never landed (still in CCTP
        // transit, re-mintable) — a source-burn confirmation does NOT imply idle
        // cash, so the mint leg must not be marked stranded.
        let prior = vec![leg(LegKind::CrossChainBurn, 500.0)];
        let mint = mint_leg(ChainKey::Base, 500.0);
        assert!(!leg_strands_funds_on_failure(
            LegKind::CrossChainMint,
            &mint,
            &prior
        ));
    }

    #[test]
    fn local_swap_failure_after_mint_strands_idle_usdc() {
        // burn → mint (confirmed on Base) → local swap on Base fails: the bridged
        // USDC is now idle cash on Base, so the swap leg strands for the replan.
        let confirmed = vec![mint_leg(ChainKey::Base, 600.0)];
        let swap = swap_leg("USDC", "ETH"); // Base → Base
        assert!(leg_strands_funds_on_failure(
            LegKind::LocalSwap,
            &swap,
            &confirmed
        ));
    }

    #[test]
    fn local_swap_failure_without_prior_mint_does_not_strand() {
        // A same-chain swap with no preceding bridge reverts atomically — the
        // USDC returns to the wallet, nothing stranded.
        let swap = swap_leg("USDC", "ETH");
        assert!(!leg_strands_funds_on_failure(
            LegKind::LocalSwap,
            &swap,
            &[]
        ));
    }

    #[test]
    fn burn_park_fx_failures_do_not_strand() {
        // A burn failure leaves source USDC in place; park / FX revert atomically
        // — none leave idle cash, even when a prior mint confirmed.
        let confirmed = vec![mint_leg(ChainKey::Base, 600.0)];
        for kind in [
            LegKind::CrossChainBurn,
            LegKind::ParkUsyc,
            LegKind::RedeemUsyc,
            LegKind::FxStablefx,
        ] {
            assert!(
                !leg_strands_funds_on_failure(kind, &leg(kind, 500.0), &confirmed),
                "{kind:?} must not strand on failure"
            );
        }
    }

    // ── Recovery: remaining-delta replan ──────────────────────────────────

    #[test]
    fn remaining_delta_re_buys_each_stranded_asset() {
        let stranded = vec![strand("ETH", 300.0), strand("BTC", 200.0)];
        let remaining = remaining_delta_after_strand(&stranded);
        // Sorted by symbol for determinism: BTC then ETH.
        assert_eq!(
            remaining,
            vec![
                RemainingDelta {
                    dest_symbol: "BTC".into(),
                    amount_usdc: 200.0
                },
                RemainingDelta {
                    dest_symbol: "ETH".into(),
                    amount_usdc: 300.0
                },
            ]
        );
    }

    #[test]
    fn remaining_delta_sums_same_symbol_strands() {
        // A split buy that stranded on two chains replans as one delta.
        let stranded = vec![strand("ETH", 120.0), strand("ETH", 80.0)];
        let remaining = remaining_delta_after_strand(&stranded);
        assert_eq!(
            remaining,
            vec![RemainingDelta {
                dest_symbol: "ETH".into(),
                amount_usdc: 200.0
            }]
        );
    }

    #[test]
    fn remaining_delta_drops_usdc_and_nonpositive() {
        // USDC strands are already cash (nothing to re-buy); zero/negative
        // amounts are noise.
        let stranded = vec![
            strand("USDC", 500.0),
            strand("usdc", 100.0),
            strand("ETH", 0.0),
            strand("BTC", -10.0),
            strand("", 50.0),
            strand("SOL", 75.0),
        ];
        let remaining = remaining_delta_after_strand(&stranded);
        assert_eq!(
            remaining,
            vec![RemainingDelta {
                dest_symbol: "SOL".into(),
                amount_usdc: 75.0
            }]
        );
    }

    #[test]
    fn remaining_delta_empty_when_nothing_stranded() {
        assert!(remaining_delta_after_strand(&[]).is_empty());
    }
}
