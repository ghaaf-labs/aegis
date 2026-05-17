//! Tax service: surface harvestable losses per portfolio.
//!
//! Data flow: pull every open lot for the portfolio, join with current market
//! prices, run `fifo::loss_for_allocation`, return the per-allocation summary
//! the strategist prompt consumes via `{{ harvestable_losses }}`.

use std::collections::HashMap;

use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::modules::market_data::{service::fetch_snapshot, MarketSnapshot};
use crate::router::AppState;

use super::fifo::{loss_for_allocation, plan_disposal, DisposalError};
use super::models::{CostBasisLot, HarvestableLoss};

/// Return all open allocations on the portfolio that are sitting at an
/// unrealized loss vs current market prices.
pub async fn harvestable_losses(
    state: &AppState,
    user_id: Uuid,
    portfolio_id: Uuid,
) -> Result<Vec<HarvestableLoss>> {
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM portfolios WHERE id = $1 AND user_id = $2)",
    )
    .bind(portfolio_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    if !owned {
        // Hide existence by treating as empty rather than 404 — the handler
        // returns 404 explicitly when called directly.
        return Ok(Vec::new());
    }

    #[derive(sqlx::FromRow)]
    struct AllocRow {
        id: Uuid,
        asset_symbol: String,
    }
    let allocations: Vec<AllocRow> =
        sqlx::query_as("SELECT id, asset_symbol FROM allocations WHERE portfolio_id = $1")
            .bind(portfolio_id)
            .fetch_all(&state.db)
            .await?;
    if allocations.is_empty() {
        return Ok(Vec::new());
    }

    let alloc_ids: Vec<Uuid> = allocations.iter().map(|a| a.id).collect();
    let lots: Vec<CostBasisLot> = sqlx::query_as(
        "SELECT id, allocation_id, acquired_at, quantity, basis_usd, disposed_at
         FROM cost_basis_lots
         WHERE allocation_id = ANY($1) AND disposed_at IS NULL",
    )
    .bind(&alloc_ids)
    .fetch_all(&state.db)
    .await?;

    let snapshot = fetch_snapshot(state.prices.as_ref())
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("market snapshot: {e}")))?;
    let price_for = build_price_map(&snapshot);

    let mut by_alloc: HashMap<Uuid, Vec<CostBasisLot>> = HashMap::new();
    for lot in lots {
        by_alloc.entry(lot.allocation_id).or_default().push(lot);
    }

    let mut out = Vec::new();
    for alloc in allocations {
        let Some(lots) = by_alloc.get(&alloc.id) else {
            continue;
        };
        let Some(price) = price_for.get(&alloc.asset_symbol).copied() else {
            continue;
        };
        if let Some(loss) =
            loss_for_allocation(portfolio_id, alloc.id, &alloc.asset_symbol, lots, price)
        {
            out.push(loss);
        }
    }
    out.sort_by(|a, b| {
        b.unrealized_loss_usd
            .partial_cmp(&a.unrealized_loss_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(out)
}

/// Aggregate harvestable losses to a single USD scalar (used by the
/// scheduler to decide whether to trigger an analyze on harvest grounds).
pub async fn total_harvestable_usd(
    state: &AppState,
    user_id: Uuid,
    portfolio_id: Uuid,
) -> Result<f64> {
    let losses = harvestable_losses(state, user_id, portfolio_id).await?;
    Ok(losses.iter().map(|l| l.unrealized_loss_usd).sum())
}

/// Close `qty` units of `allocation_id` against the FIFO lot stack. Writes
/// `disposed_at` on the affected lots. Used by the executor when an actual
/// sell leg confirms.
pub async fn record_disposal(state: &AppState, allocation_id: Uuid, qty: f64) -> Result<()> {
    let lots: Vec<CostBasisLot> = sqlx::query_as(
        "SELECT id, allocation_id, acquired_at, quantity, basis_usd, disposed_at
         FROM cost_basis_lots
         WHERE allocation_id = $1 AND disposed_at IS NULL
         ORDER BY acquired_at ASC",
    )
    .bind(allocation_id)
    .fetch_all(&state.db)
    .await?;

    let plan = match plan_disposal(&lots, qty) {
        Ok(p) => p,
        Err(DisposalError::InsufficientLots { .. }) => {
            // Insufficient open lots — log and bail. The agent may have a
            // stale view of the position; do not silently over-close.
            tracing::warn!(
                ?allocation_id,
                qty,
                "tax: insufficient open lots for disposal"
            );
            return Ok(());
        }
    };

    let mut tx = state.db.begin().await?;
    for (lot_id, take) in &plan {
        // If we take exactly the lot's quantity, mark the whole lot disposed.
        // Otherwise split: shrink the original lot, insert a disposed sibling.
        let lot = lots.iter().find(|l| &l.id == lot_id).unwrap();
        if (take - lot.quantity).abs() < 1e-9 {
            sqlx::query("UPDATE cost_basis_lots SET disposed_at = NOW() WHERE id = $1")
                .bind(lot_id)
                .execute(&mut *tx)
                .await?;
        } else {
            let basis_share = lot.basis_usd * (take / lot.quantity);
            sqlx::query(
                "UPDATE cost_basis_lots
                    SET quantity = quantity - $2, basis_usd = basis_usd - $3
                    WHERE id = $1",
            )
            .bind(lot_id)
            .bind(take)
            .bind(basis_share)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "INSERT INTO cost_basis_lots (allocation_id, acquired_at, quantity, basis_usd, disposed_at)
                 VALUES ($1, $2, $3, $4, NOW())",
            )
            .bind(allocation_id)
            .bind(lot.acquired_at + chrono::Duration::microseconds(1))
            .bind(take)
            .bind(basis_share)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

fn build_price_map(snapshot: &MarketSnapshot) -> HashMap<String, f64> {
    snapshot
        .assets
        .iter()
        .map(|a| (a.symbol.clone(), a.price_usd))
        .collect()
}
