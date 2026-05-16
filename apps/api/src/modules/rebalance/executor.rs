//! Rebalance plan executor.
//!
//! Walks a persisted `rebalances` row's legs in order, dispatches each by
//! `LegKind`, updates the DB on every transition, and broadcasts
//! `rebalance.leg.update` SSE events filtered to the owning user.
//!
//! Failure semantics: if any leg fails, the plan halts in `failed` state.
//! There is no mid-plan retry — manual replan is a new POST. This avoids
//! double-spend on partial CCTP commits.

use chrono::Utc;
use sha2::Digest as _;
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::modules::rebalance::cross_chain::{build_hook_payload, CctpClient};
use crate::modules::rebalance::models::{ChainKey, LegKind, PlannedLeg};
use crate::modules::sse::{RebalanceLegPayload, RebalancePlanPayload, SseEvent};
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

    let mut tx = state.db.begin().await?;
    let rebalance_id: Uuid = sqlx::query_scalar(
        "INSERT INTO rebalances (portfolio_id, decision_id, status, total_legs, total_gas_usdc)
         VALUES ($1, $2, 'planned', $3, $4)
         RETURNING id",
    )
    .bind(portfolio_id)
    .bind(decision_id)
    .bind(legs.len() as i32)
    .bind(total_gas_usdc)
    .fetch_one(&mut *tx)
    .await?;

    for leg in legs {
        sqlx::query(
            "INSERT INTO rebalance_legs
               (rebalance_id, leg_index, kind, src_chain, dest_chain,
                src_symbol, dest_symbol, amount_usdc, min_out, status)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending')",
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
    use crate::modules::paymaster::service::{estimate, PaymasterChain};
    use std::collections::HashSet;

    let mut chains: HashSet<ChainKey> = HashSet::new();
    for leg in legs {
        if let Some(c) = leg.dest_chain.or(leg.src_chain) {
            chains.insert(c);
        }
    }
    let mut total = 0.0;
    for c in chains {
        let chain = match c {
            ChainKey::Arc => PaymasterChain::Arc,
            ChainKey::Base => PaymasterChain::Base,
        };
        if let Ok(e) = estimate(&state.config, chain, "rebalance").await {
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
        if let Err(e) = walk_legs(&st, rebalance_id, user_id).await {
            tracing::error!(?rebalance_id, error=%e, "rebalance walk failed");
            let reason = format!("{e}");
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
                dest_symbol, amount_usdc
         FROM rebalance_legs
         WHERE rebalance_id = $1
         ORDER BY leg_index ASC",
    )
    .bind(rebalance_id)
    .fetch_all(&state.db)
    .await?;

    for leg in legs {
        let kind = parse_kind(&leg.kind)?;
        mark_leg_submitted(state, rebalance_id, leg.id, user_id, &leg).await?;

        let (tx_hash, cctp_hash) = match dispatch(state, rebalance_id, kind, &leg).await {
            Ok(v) => v,
            Err(e) => {
                mark_leg_failed(state, rebalance_id, leg.id, user_id, &leg, &format!("{e}"))
                    .await?;
                return Err(e);
            }
        };

        mark_leg_confirmed(
            state,
            rebalance_id,
            leg.id,
            user_id,
            &leg,
            &tx_hash,
            cctp_hash.as_deref(),
        )
        .await?;

        // Tax lot disposal: a successful sell leg that produced USDC closes
        // the oldest matching open lots FIFO. Best-effort — failures here
        // log + continue rather than rolling back the (already on-chain) leg.
        if is_sell_leg(kind, &leg) {
            if let Err(e) = record_disposal_for_leg(state, portfolio_id, &leg).await {
                tracing::warn!(leg_id=?leg.id, error=%e, "tax: lot disposal failed");
            }
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

    // Record the 25 bps protocol fee once for the entire plan, against the
    // sum of leg notionals. The leg loop above intentionally does not touch
    // billing — fee-per-leg would multiply the fee by the leg count.
    let plan_total: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_usdc), 0)::DOUBLE PRECISION
         FROM rebalance_legs WHERE rebalance_id = $1",
    )
    .bind(rebalance_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0.0);

    if plan_total > 0.0 {
        // Resolve the real payer: portfolio → user → users.arc_address.
        // Falling back to the zero address (the pre-audit behaviour) would let
        // the facilitator silently accept invalid payments — fail loudly here
        // instead so the operator notices a misprovisioned wallet.
        let payer_address: String = sqlx::query_scalar(
            "SELECT u.arc_address
               FROM portfolios p
               JOIN users u ON u.id = p.user_id
              WHERE p.id = $1",
        )
        .bind(portfolio_id)
        .fetch_optional(&state.db)
        .await?
        .flatten()
        .ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!(
                "cannot settle protocol fee: users.arc_address missing for portfolio {portfolio_id}"
            ))
        })?;

        let settlement_tx = crate::modules::billing::service::settle_protocol_fee_via_nanopayments(
            &state.config,
            &payer_address,
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

async fn dispatch(
    state: &AppState,
    rebalance_id: Uuid,
    kind: LegKind,
    leg: &LegRow,
) -> Result<(String, Option<String>)> {
    match kind {
        LegKind::CrossChainBurn => {
            let src = ChainKey::parse(leg.src_chain.as_deref().unwrap_or(""))
                .ok_or_else(|| AppError::BadRequest("missing src_chain".into()))?;
            let dest = ChainKey::parse(leg.dest_chain.as_deref().unwrap_or(""))
                .ok_or_else(|| AppError::BadRequest("missing dest_chain".into()))?;
            // F-EXEC-1b (2026-05-16): the prior code took a 32-hex-char UUID
            // and sliced `[..40]` which panicked (UUID hex is 32 chars, not
            // 40). Worse, the value was used as the *recipient* in the hook
            // payload — meaningless data, USDC would mint to a dead address.
            //
            // Real fix: look up the user's destination-chain EOA address
            // (users.arc_address when dest=Arc, users.base_address when
            // dest=Base) and use it as the hook recipient. The minted USDC
            // arrives at the destination-chain RebalanceExecutor, which
            // then transfers to this recipient.
            let recipient_col = match dest {
                ChainKey::Arc => "arc_address",
                ChainKey::Base => "base_address",
            };
            let recipient: String = sqlx::query_scalar(&format!(
                "SELECT u.{recipient_col}
                 FROM users u
                 JOIN portfolios p ON p.user_id = u.id
                 JOIN rebalances r ON r.portfolio_id = p.id
                 WHERE r.id = $1"
            ))
            .bind(rebalance_id)
            .fetch_one(&state.db)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("recipient lookup: {e}")))?;
            if recipient.is_empty() {
                return Err(AppError::Internal(anyhow::anyhow!(
                    "user.{recipient_col} is empty; cannot route mint without a destination address"
                )));
            }
            // The planner doesn't yet resolve dest_symbol (e.g. "BTC") to a
            // concrete ERC-20 address on the destination chain, and the leg
            // row doesn't carry the address either. Until the planner stores
            // (chain, token) tuples, we route USDC-only burns: zero address
            // tokenOut + minOut=0 tells the destination RebalanceExecutor to
            // skip the Uniswap leg and just credit USDC. Real volatile-asset
            // hooks land when the planner gains address lookup.
            let pool_fee = 3000u32;
            let token_out_zero = "0x0000000000000000000000000000000000000000";
            let hook = build_hook_payload(
                &recipient,
                token_out_zero,
                pool_fee,
                0,
                (chrono::Utc::now().timestamp() + 600) as u64,
            );
            let client = CctpClient::new(&state.http, &state.config);
            let r = client
                .deposit_for_burn(src, dest, leg.amount_usdc, &hook)
                .await?;
            Ok((r.tx_hash, Some(r.message_hash)))
        }
        LegKind::CrossChainMint => {
            let src = ChainKey::parse(leg.src_chain.as_deref().unwrap_or(""))
                .ok_or_else(|| AppError::BadRequest("missing src_chain".into()))?;
            let dest = ChainKey::parse(leg.dest_chain.as_deref().unwrap_or(""))
                .ok_or_else(|| AppError::BadRequest("missing dest_chain".into()))?;
            let client = CctpClient::new(&state.http, &state.config);
            // The companion burn leg already produced a message_hash; the
            // executor reads it back from rebalance_legs in production.
            let burn_hash = sqlx::query_scalar::<_, Option<String>>(
                "SELECT cctp_message_hash FROM rebalance_legs
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

            let att = client
                .wait_for_attestation(src.domain_id(), &burn_hash)
                .await?;
            let r = client.receive_message(dest, &att).await?;
            Ok((r.tx_hash, None))
        }
        LegKind::LocalSwap | LegKind::ParkUsyc | LegKind::RedeemUsyc | LegKind::FxStablefx => {
            // Mock receipt — production swaps go through the destination-chain
            // RebalanceExecutor or the relevant Circle module (treasury / fx).
            let mut h = sha2::Sha256::new();
            h.update(kind.as_str().as_bytes());
            h.update(b":");
            h.update(leg.id.as_bytes());
            Ok((format!("0x{}", hex::encode(h.finalize())), None))
        }
    }
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

#[derive(sqlx::FromRow)]
struct LegRow {
    id: Uuid,
    leg_index: i32,
    kind: String,
    src_chain: Option<String>,
    dest_chain: Option<String>,
    src_symbol: Option<String>,
    dest_symbol: Option<String>,
    amount_usdc: f64,
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

async fn record_disposal_for_leg(state: &AppState, portfolio_id: Uuid, leg: &LegRow) -> Result<()> {
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
    let snapshot = crate::modules::market_data::service::fetch_snapshot(&state.http, &state.config)
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
