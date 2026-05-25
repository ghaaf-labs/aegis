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

mod dispatch;
mod ledger;
// Public so the formally-verified leg state machine is reachable as crate API
// (the live executor adopts it as it migrates off the boolean `stranded` flag).
pub mod leg_state;
mod leg_status;
mod legs;
mod stranding;

pub use stranding::{remaining_delta_after_strand, RemainingDelta, StrandedLeg};

use dispatch::{dispatch, LegDispatch};

use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::modules::rebalance::models::{ChainKey, PlannedLeg};
use crate::modules::sse::{RebalancePlanPayload, SseEvent};
use crate::router::AppState;

use leg_status::{
    bump_attempt_count, mark_leg_confirmed, mark_leg_failed, mark_leg_quoted, mark_leg_stranded,
    mark_leg_submitted,
};
use legs::{parse_kind, LegRow, MAX_LEG_ATTEMPTS};
use stranding::{
    idempotency_key_for_leg, leg_strands_funds_on_failure, pending_funding_dependency,
    protocol_fee_notional_from_legs,
};

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
    // Serialize review creation per portfolio. If two plan requests race, the
    // later committed review supersedes the older draft before it can be
    // approved, so there is never more than one approval-eligible planned
    // rebalance competing for the same wallet cash.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text, 0))")
        .bind(portfolio_id.to_string())
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE rebalances
            SET status = 'cancelled',
                failure_reason = 'Superseded by a newer review.'
          WHERE portfolio_id = $1 AND status = 'planned'",
    )
    .bind(portfolio_id)
    .execute(&mut *tx)
    .await?;

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
               (rebalance_id, leg_index, depends_on, kind, src_chain, dest_chain,
                src_symbol, dest_symbol, amount_usdc, min_out, status, idempotency_key)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'pending', $11)",
        )
        .bind(rebalance_id)
        .bind(leg.leg_index)
        .bind(&leg.deps)
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

fn leg_with_executed_amount(leg: &LegRow, executed_amount_usdc: Decimal) -> LegRow {
    let mut out = leg.clone();
    out.amount_usdc = executed_amount_usdc;
    out
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
        "WITH target AS (
             SELECT id, portfolio_id
             FROM rebalances
             WHERE id = $1 AND status = 'planned'
         ),
         updated AS (
             UPDATE rebalances r
                SET status = 'executing', approved_at = NOW()
               FROM target t
              WHERE r.id = t.id
                AND NOT EXISTS (
                    SELECT 1
                    FROM rebalances active
                    WHERE active.portfolio_id = t.portfolio_id
                      AND active.id <> r.id
                      AND active.status = 'executing'
                )
              RETURNING r.portfolio_id
         )
         SELECT portfolio_id FROM updated",
    )
    .bind(rebalance_id)
    .fetch_optional(&state.db)
    .await?;
    let portfolio_id = portfolio_id.ok_or_else(|| {
        AppError::Conflict(format!(
            "rebalance {rebalance_id} is not ready to execute, or another rebalance is already executing for this portfolio"
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

/// Kahn's topological sort over `legs` using `LegRow::depends_on`. Returns
/// the positions in `legs` in a valid execution order (all deps before their
/// dependents). Returns `Err` if the depends_on graph has a cycle (cannot
/// happen with well-formed plans — would indicate a planning bug).
fn topological_leg_order(legs: &[LegRow]) -> Result<Vec<usize>> {
    // Map: leg_index → position in `legs` array.
    let idx_to_pos: std::collections::HashMap<i32, usize> = legs
        .iter()
        .enumerate()
        .map(|(pos, l)| (l.leg_index, pos))
        .collect();

    let n = legs.len();
    let mut indegree = vec![0usize; n];
    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); n];

    for (pos, leg) in legs.iter().enumerate() {
        for &dep_idx in &leg.depends_on {
            let Some(&dep_pos) = idx_to_pos.get(&dep_idx) else {
                return Err(AppError::Internal(anyhow::anyhow!(
                    "rebalance leg {} depends on missing leg {dep_idx}",
                    leg.leg_index
                )));
            };
            indegree[pos] += 1;
            dependents[dep_pos].push(pos);
        }
    }

    // Start with all legs that have no unmet deps (sorted for determinism).
    let mut ready: Vec<usize> = (0..n).filter(|&i| indegree[i] == 0).collect();
    ready.sort_unstable();
    let mut order = Vec::with_capacity(n);

    while let Some(&pos) = ready.first() {
        ready.remove(0);
        order.push(pos);
        let mut newly_ready: Vec<usize> = dependents[pos]
            .iter()
            .copied()
            .filter(|&d| {
                indegree[d] -= 1;
                indegree[d] == 0
            })
            .collect();
        newly_ready.sort_unstable();
        ready.extend(newly_ready);
        ready.sort_unstable();
    }

    if order.len() != n {
        return Err(AppError::Internal(anyhow::anyhow!(
            "rebalance leg DAG has a cycle — plan is malformed"
        )));
    }
    Ok(order)
}

async fn walk_legs(state: &AppState, rebalance_id: Uuid, user_id: Uuid) -> Result<()> {
    let portfolio_id: Uuid =
        sqlx::query_scalar("SELECT portfolio_id FROM rebalances WHERE id = $1")
            .bind(rebalance_id)
            .fetch_one(&state.db)
            .await?;

    let legs: Vec<LegRow> = sqlx::query_as(
        "SELECT id, leg_index, depends_on, kind, src_chain, dest_chain, src_symbol,
                dest_symbol, amount_usdc, min_out, status, attempt_count
         FROM rebalance_legs
         WHERE rebalance_id = $1
         ORDER BY leg_index ASC",
    )
    .bind(rebalance_id)
    .fetch_all(&state.db)
    .await?;

    // Build a topological execution order from the explicit `depends_on` DAG.
    // Legs with no deps start immediately; a leg becomes ready once every leg
    // in its `depends_on` list has been processed. This replaces the implicit
    // `leg_index` ordering and allows honest concurrency within independent
    // routes (future work) while guaranteeing CCTP mint waits on its burn.
    let topo_order = topological_leg_order(&legs)?;

    let mut confirmed_so_far: Vec<LegRow> = Vec::new();

    for pos in topo_order {
        let leg = &legs[pos];
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

        // Quote/submit are distinct FSM states even though the coarse SQL
        // `status` only has pending/submitted. Mark quoted before the network
        // handoff so the trace shows a leg passed local quote validation.
        mark_leg_quoted(state, rebalance_id, leg.id, user_id, leg).await?;

        // Bump the attempt counter on every submit so retries are observable and
        // a runaway leg can be capped. Done before the network call so a crash
        // mid-submit still records the attempt.
        bump_attempt_count(state, leg.id).await?;
        mark_leg_submitted(state, rebalance_id, leg.id, user_id, leg).await?;

        let LegDispatch {
            tx_hash,
            cctp_hash,
            executed_amount_usdc,
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
        let executed_leg = leg_with_executed_amount(leg, executed_amount_usdc);

        mark_leg_confirmed(
            state,
            rebalance_id,
            leg.id,
            user_id,
            &executed_leg,
            &tx_hash,
            cctp_hash.as_deref(),
        )
        .await?;
        confirmed_so_far.push(executed_leg.clone());

        // Mirror the confirmed leg into the holdings ledger (sell reduces /
        // buy increments the allocation, recompute totals once). Without this
        // Portfolio Value stays $0 after real swaps confirm.
        ledger::apply_leg_writeback(state, portfolio_id, kind, &executed_leg, filled_qty).await;

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
    let plan_total = protocol_fee_notional_from_legs(&confirmed_so_far);
    crate::modules::observability::counters::record_rebalance_succeeded(plan_total);
    ledger::settle_protocol_fee(state, rebalance_id, portfolio_id, plan_total).await?;

    Ok(())
}

#[cfg(test)]
mod dependency_tests {
    use rust_decimal::prelude::{FromPrimitive, ToPrimitive};

    use crate::modules::rebalance::models::LegKind;

    use super::{
        leg_with_executed_amount,
        legs::test_helpers::{make_leg, make_swap_leg},
        stranding::protocol_fee_notional_from_legs,
        topological_leg_order,
    };

    #[test]
    fn topological_order_respects_explicit_dependencies() {
        let mut burn = make_leg(LegKind::CrossChainBurn, 100.0);
        burn.leg_index = 10;
        let mut mint = make_leg(LegKind::CrossChainMint, 100.0);
        mint.leg_index = 20;
        mint.depends_on = vec![10];

        let order = topological_leg_order(&[mint, burn]).expect("valid DAG");
        assert_eq!(
            order,
            vec![1, 0],
            "burn position must precede mint position"
        );
    }

    #[test]
    fn topological_order_rejects_missing_dependencies() {
        let mut mint = make_leg(LegKind::CrossChainMint, 100.0);
        mint.leg_index = 1;
        mint.depends_on = vec![999];

        let err = topological_leg_order(&[mint]).expect_err("missing dep must fail closed");
        assert!(
            format!("{err}").contains("missing leg 999"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn topological_order_rejects_cycles() {
        let mut a = make_leg(LegKind::LocalSwap, 100.0);
        a.leg_index = 1;
        a.depends_on = vec![2];
        let mut b = make_leg(LegKind::LocalSwap, 100.0);
        b.leg_index = 2;
        b.depends_on = vec![1];

        let err = topological_leg_order(&[a, b]).expect_err("cycle must fail closed");
        assert!(
            format!("{err}").contains("cycle"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn executed_amount_replaces_planned_amount_for_confirmed_accounting() {
        let planned_burn = make_leg(LegKind::CrossChainBurn, 100.0);
        let planned_mint = make_leg(LegKind::CrossChainMint, 100.0);
        let planned_swap = make_swap_leg("USDC", "ETH");
        let actual = rust_decimal::Decimal::from_f64(87.5).unwrap();

        let burn = leg_with_executed_amount(&planned_burn, actual);
        let mint = leg_with_executed_amount(&planned_mint, actual);
        let swap = leg_with_executed_amount(&planned_swap, actual);

        assert_eq!(burn.amount_usdc.to_f64(), Some(87.5));
        assert_eq!(mint.amount_usdc.to_f64(), Some(87.5));
        assert_eq!(swap.amount_usdc.to_f64(), Some(87.5));
        assert_eq!(
            protocol_fee_notional_from_legs(&[burn, mint, swap]),
            175.0,
            "confirmed accounting must bill the executed burn+swap, not the original planned 100+100"
        );
    }
}
