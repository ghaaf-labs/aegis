use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use uuid::Uuid;

use crate::error::Result;
use crate::modules::sse::{RebalanceLegPayload, SseEvent};
use crate::router::AppState;

use super::legs::LegRow;

pub(super) async fn mark_leg_submitted(
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

pub(super) async fn mark_leg_confirmed(
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

pub(super) async fn mark_leg_failed(
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
pub(super) async fn bump_attempt_count(state: &AppState, leg_id: Uuid) -> Result<()> {
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
pub(super) async fn mark_leg_stranded(
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
pub(super) fn broadcast_leg(
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
        amount_usdc: leg.amount_usdc.to_f64().unwrap_or(0.0),
        status: status.to_string(),
        tx_hash: tx_hash.map(str::to_string),
        failure_reason: failure_reason.map(str::to_string),
        updated_at: Utc::now(),
    };
    let _ = state.sse.send(SseEvent::RebalanceLegUpdate(payload));
}
