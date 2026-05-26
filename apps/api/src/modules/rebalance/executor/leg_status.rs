use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use uuid::Uuid;

use crate::error::Result;
use crate::modules::sse::{RebalanceLegPayload, SseEvent};
use crate::router::AppState;

use super::leg_state::LegState;
use super::legs::LegRow;

pub(super) async fn mark_leg_quoted(
    state: &AppState,
    rebalance_id: Uuid,
    leg_id: Uuid,
    user_id: Uuid,
    leg: &LegRow,
) -> Result<()> {
    sqlx::query("UPDATE rebalance_legs SET leg_state = $2 WHERE id = $1")
        .bind(leg_id)
        .bind(LegState::Quoted.as_str())
        .execute(&state.db)
        .await?;
    broadcast_leg(
        state,
        rebalance_id,
        leg_id,
        user_id,
        leg,
        "pending",
        LegState::Quoted.as_str(),
        None,
        None,
    );
    Ok(())
}

pub(super) async fn mark_leg_submitted(
    state: &AppState,
    rebalance_id: Uuid,
    leg_id: Uuid,
    user_id: Uuid,
    leg: &LegRow,
) -> Result<()> {
    sqlx::query(
        "UPDATE rebalance_legs SET status = 'submitted', leg_state = $2, submitted_at = NOW() WHERE id = $1",
    )
    .bind(leg_id)
    .bind(LegState::Submitted.as_str())
    .execute(&state.db)
    .await?;
    broadcast_leg(
        state,
        rebalance_id,
        leg_id,
        user_id,
        leg,
        "submitted",
        LegState::Submitted.as_str(),
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
        "UPDATE rebalance_legs SET status = 'confirmed', leg_state = $4, confirmed_at = NOW(),
                                  tx_hash = $2, cctp_message_hash = $3, amount_usdc = $5
         WHERE id = $1",
    )
    .bind(leg_id)
    .bind(tx_hash)
    .bind(cctp_hash)
    .bind(confirmed_leg_state(&leg.kind).as_str())
    .bind(leg.amount_usdc)
    .execute(&state.db)
    .await?;

    broadcast_leg(
        state,
        rebalance_id,
        leg_id,
        user_id,
        leg,
        "confirmed",
        confirmed_leg_state(&leg.kind).as_str(),
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
    sqlx::query(
        "UPDATE rebalance_legs SET status = 'failed', leg_state = $3, failure_reason = $2 WHERE id = $1",
    )
    .bind(leg_id)
    .bind(reason)
    .bind(LegState::Failed.as_str())
    .execute(&state.db)
    .await?;
    broadcast_leg(
        state,
        rebalance_id,
        leg_id,
        user_id,
        leg,
        "failed",
        LegState::Failed.as_str(),
        None,
        Some(reason),
    );
    Ok(())
}

/// The `leg_state` a confirmed leg records, by kind. A cross-chain transfer is
/// three leg rows (burn → mint → acquire) in this executor, so the persisted
/// state reflects where the funds are after each phase (the FSM funds-location
/// model, §8/§17): a confirmed **burn** leaves funds in flight, a confirmed
/// **mint** lands them as USDC on the destination, and only an **acquire**
/// (local swap / USYC park / FX) reaches the target asset. This makes the
/// per-leg state honest for cross-chain plans instead of flattening every
/// confirmed leg to `Confirmed`.
pub(super) fn confirmed_leg_state(kind: &str) -> LegState {
    match kind {
        "cross_chain_burn" => LegState::BridgeInFlight,
        "cross_chain_mint" => LegState::BridgeLanded,
        _ => LegState::Confirmed,
    }
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
            SET status = 'failed', stranded_asset = TRUE, leg_state = $3, failure_reason = $2
          WHERE id = $1",
    )
    .bind(leg_id)
    .bind(reason)
    .bind(LegState::StrandedReserve.as_str())
    .execute(&state.db)
    .await?;
    broadcast_leg(
        state,
        rebalance_id,
        leg_id,
        user_id,
        leg,
        "failed",
        LegState::StrandedReserve.as_str(),
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
    leg_state: &str,
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
        leg_state: leg_state.to_string(),
        tx_hash: tx_hash.map(str::to_string),
        failure_reason: failure_reason.map(str::to_string),
        updated_at: Utc::now(),
    };
    let _ = state.sse.send(SseEvent::RebalanceLegUpdate(payload));
}

#[cfg(test)]
mod tests {
    use super::confirmed_leg_state;
    use crate::modules::rebalance::executor::leg_state::{FundsLocation, LegState};

    #[test]
    fn confirmed_leg_state_reflects_funds_location_by_kind() {
        // The persisted state for a confirmed leg must match where the funds
        // physically are (the fund-safety model), per cross-chain phase.
        assert_eq!(
            confirmed_leg_state("cross_chain_burn"),
            LegState::BridgeInFlight
        );
        assert_eq!(
            confirmed_leg_state("cross_chain_mint"),
            LegState::BridgeLanded
        );
        assert_eq!(confirmed_leg_state("local_swap"), LegState::Confirmed);
        assert_eq!(confirmed_leg_state("park_usyc"), LegState::Confirmed);

        assert_eq!(
            confirmed_leg_state("cross_chain_burn").funds_location(),
            FundsLocation::InFlight,
            "a confirmed burn has funds in flight, not at the target"
        );
        assert_eq!(
            confirmed_leg_state("cross_chain_mint").funds_location(),
            FundsLocation::Usdc,
            "a confirmed mint has landed USDC on the destination"
        );
        assert_eq!(
            confirmed_leg_state("local_swap").funds_location(),
            FundsLocation::TargetAsset
        );
    }
}
