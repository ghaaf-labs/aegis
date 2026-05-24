use rust_decimal::prelude::ToPrimitive;
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::modules::rebalance::models::LegKind;
use crate::modules::wallet_routes;
use crate::router::AppState;

use super::legs::{is_buy_leg, is_sell_leg, settled_quantity, LegRow};

/// Most recent USD price from `price_history` for the given symbol. Returns
/// `None` when the symbol isn't tracked (USYC, etc.). `price_history.price_usd`
/// is `numeric(20,8)` — cast to DOUBLE PRECISION here so sqlx can decode into
/// f64 without raising a type-mismatch.
pub(super) async fn latest_spot_price(state: &AppState, symbol: &str) -> Option<f64> {
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

pub(super) async fn latest_spot_price_with_stable_fallback(state: &AppState, symbol: &str) -> f64 {
    latest_spot_price(state, symbol).await.unwrap_or({
        if matches!(symbol, "USYC" | "USDC" | "EURC") {
            1.0
        } else {
            0.0
        }
    })
}

/// Increment `allocations.quantity` and economic value by the asset acquired on
/// a buy leg. Best-effort — failures here log and continue; the on-chain leg is
/// already settled.
///
/// `filled_qty` is the real on-chain fill from the executed swap quote (the
/// pool's true exchange rate). It is the source of truth so `allocations.quantity`
/// matches the tokens that actually landed on-chain. `value_usd` intentionally
/// moves by the economic notional the user approved, not `quantity × mainnet
/// spot`; testnet pools can fill at non-economic ratios, and using that for UI
/// value makes holdings look wildly wrong.
pub(super) async fn record_acquisition_for_leg(
    state: &AppState,
    portfolio_id: Uuid,
    leg: &LegRow,
    filled_qty: Option<f64>,
) -> Result<()> {
    let Some(symbol) = leg.dest_symbol.as_deref() else {
        return Ok(());
    };
    let spot_price = latest_spot_price_with_stable_fallback(state, symbol).await;
    let Some(acquired_qty) = settled_quantity(
        filled_qty,
        leg.amount_usdc.to_f64().unwrap_or(0.0),
        spot_price,
    ) else {
        return Ok(());
    };

    let result = sqlx::query(
        "UPDATE allocations
            SET quantity = quantity + $3,
                value_usd = value_usd + $4,
                updated_at = NOW()
          WHERE portfolio_id = $1 AND asset_symbol = $2",
    )
    .bind(portfolio_id)
    .bind(symbol)
    .bind(acquired_qty)
    .bind(leg.amount_usdc)
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
        .bind(leg.amount_usdc)
        .execute(&state.db)
        .await?;
    }
    Ok(())
}

/// Decrement `allocations.quantity` by the asset amount sold on a sell leg.
/// `filled_qty` is the real token input the swap spent (from the quote); it wins
/// over the price-derived estimate so the sold quantity matches on-chain.
pub(super) async fn record_allocation_disposal_for_leg(
    state: &AppState,
    portfolio_id: Uuid,
    leg: &LegRow,
    filled_qty: Option<f64>,
) -> Result<()> {
    let Some(symbol) = leg.src_symbol.as_deref() else {
        return Ok(());
    };
    let spot_price = latest_spot_price_with_stable_fallback(state, symbol).await;
    let Some(sold_qty) = settled_quantity(
        filled_qty,
        leg.amount_usdc.to_f64().unwrap_or(0.0),
        spot_price,
    ) else {
        return Ok(());
    };

    sqlx::query(
        "UPDATE allocations
            SET quantity = GREATEST(quantity - $3, 0),
                value_usd = GREATEST(value_usd - $4, 0),
                updated_at = NOW()
          WHERE portfolio_id = $1 AND asset_symbol = $2",
    )
    .bind(portfolio_id)
    .bind(symbol)
    .bind(sold_qty)
    .bind(leg.amount_usdc)
    .execute(&state.db)
    .await?;
    Ok(())
}

/// Recompute portfolio totals and allocation weights from allocation values.
/// Called after every confirmed buy/sell writeback so the dashboard and the
/// next planner run read the same post-trade state.
pub(super) async fn recompute_portfolio_values(state: &AppState, portfolio_id: Uuid) -> Result<()> {
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

pub(super) async fn record_tax_disposal_for_leg(
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
    let qty = leg.amount_usdc.to_f64().unwrap_or(0.0) / price;
    crate::modules::tax::service::record_disposal(state, allocation_id, qty).await
}

/// Mirror a just-confirmed leg into the holdings ledger: a sell reduces the
/// sold allocation (and closes FIFO tax lots), a buy increments the acquired
/// allocation, and the portfolio totals are recomputed once if either side
/// moved — so a *partial* plan (e.g. the USYC leg reverts on Arc but BTC/ETH
/// already settled) still surfaces the holdings the user actually owns.
///
/// Best-effort throughout: the on-chain leg is already settled, so a bookkeeping
/// miss logs and continues rather than unwinding a confirmed trade. Tax-lot
/// disposal is separate bookkeeping and can never be the holdings source of
/// truth, so its failure doesn't block the allocation writeback.
pub(super) async fn apply_leg_writeback(
    state: &AppState,
    portfolio_id: Uuid,
    kind: LegKind,
    leg: &LegRow,
    filled_qty: Option<f64>,
) {
    let mut holdings_changed = false;

    if is_sell_leg(kind, leg) {
        if let Err(e) =
            record_allocation_disposal_for_leg(state, portfolio_id, leg, filled_qty).await
        {
            tracing::warn!(leg_id=?leg.id, error=%e, "allocations: disposal writeback failed");
        } else {
            holdings_changed = true;
        }
        if let Err(e) = record_tax_disposal_for_leg(state, portfolio_id, leg).await {
            tracing::warn!(leg_id=?leg.id, error=%e, "tax: lot disposal failed");
        }
    }

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
        }
    }
}

/// Settle the 25 bps protocol fee for an executed plan via Nanopayments and
/// record it. `notional` is the billed economic notional (CCTP mint legs are
/// excluded upstream — they're the receive-side of a burn, not a second move).
/// No-op when the notional rounds to zero.
pub(super) async fn settle_protocol_fee(
    state: &AppState,
    rebalance_id: Uuid,
    portfolio_id: Uuid,
    notional: f64,
) -> Result<()> {
    if notional <= 0.0 {
        return Ok(());
    }

    // 25 bps of the executed notional — NOT the notional itself. Compute once
    // so the on-chain Nanopayment and the `rebalance_fees` row settle the same
    // amount.
    let fee = crate::modules::billing::service::protocol_fee(notional);

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
        fee,
    )
    .await
    .ok()
    .flatten();

    if let Err(e) = crate::modules::billing::service::record_protocol_fee(
        &state.db,
        rebalance_id,
        fee,
        settlement_tx.as_deref(),
    )
    .await
    {
        tracing::warn!(rebalance_id=?rebalance_id, error=%e, "billing: protocol fee record failed");
    }
    Ok(())
}
