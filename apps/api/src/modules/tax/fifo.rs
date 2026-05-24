//! Pure FIFO lot-matching helpers.
//!
//! Disposal logic: close oldest open lots first. If the disposal quantity
//! exceeds the sum of open lots, the caller has been served stale data and
//! the helper returns an error — never silently over-closes.

use rust_decimal::prelude::ToPrimitive;

use super::models::{CostBasisLot, HarvestableLoss, HarvestableLot};
use uuid::Uuid;

/// Compute the per-allocation unrealized loss across open lots given the
/// current per-unit price. Returns `None` if the allocation is not at a loss
/// (i.e. current value ≥ basis across the lot set).
pub fn loss_for_allocation(
    portfolio_id: Uuid,
    allocation_id: Uuid,
    symbol: &str,
    lots: &[CostBasisLot],
    current_price_usd: f64,
) -> Option<HarvestableLoss> {
    let mut loss_lots = Vec::new();
    let mut total_loss = 0.0;
    for lot in lots.iter().filter(|l| l.disposed_at.is_none()) {
        let qty = lot.quantity.to_f64().unwrap_or(0.0);
        let basis = lot.basis_usd.to_f64().unwrap_or(0.0);
        let current_value = qty * current_price_usd;
        let lot_loss = basis - current_value;
        if lot_loss > 0.0 {
            total_loss += lot_loss;
            loss_lots.push(HarvestableLot {
                lot_id: lot.id,
                acquired_at: lot.acquired_at,
                quantity: qty,
                basis_usd: basis,
                current_value_usd: current_value,
            });
        }
    }
    if total_loss <= 0.0 {
        return None;
    }
    Some(HarvestableLoss {
        portfolio_id,
        allocation_id,
        symbol: symbol.to_string(),
        unrealized_loss_usd: total_loss,
        lots: loss_lots,
    })
}

/// Plan a FIFO close: given `qty_to_close`, return the (lot, qty_closed)
/// pairs in FIFO order. Caller writes the disposals in a transaction.
pub fn plan_disposal(
    lots: &[CostBasisLot],
    qty_to_close: f64,
) -> Result<Vec<(Uuid, f64)>, DisposalError> {
    if qty_to_close <= 0.0 {
        return Ok(Vec::new());
    }
    let mut open: Vec<&CostBasisLot> = lots.iter().filter(|l| l.disposed_at.is_none()).collect();
    open.sort_by_key(|l| l.acquired_at);

    let mut remaining = qty_to_close;
    let mut plan = Vec::new();
    for lot in open {
        if remaining <= 0.0 {
            break;
        }
        let lot_qty = lot.quantity.to_f64().unwrap_or(0.0);
        let take = remaining.min(lot_qty);
        plan.push((lot.id, take));
        remaining -= take;
    }
    if remaining > 1e-9 {
        return Err(DisposalError::InsufficientLots {
            short_by_qty: remaining,
        });
    }
    Ok(plan)
}

#[derive(Debug, thiserror::Error)]
pub enum DisposalError {
    #[error("insufficient open lots: short by {short_by_qty}")]
    InsufficientLots { short_by_qty: f64 },
}

#[cfg(test)]
mod tests {
    use rust_decimal::prelude::FromPrimitive;
    use rust_decimal::Decimal;

    use super::*;
    use chrono::TimeZone;

    fn lot(id: u8, qty: f64, basis: f64, disposed: bool) -> CostBasisLot {
        CostBasisLot {
            id: Uuid::from_bytes([id; 16]),
            allocation_id: Uuid::nil(),
            acquired_at: Utc
                .with_ymd_and_hms(2026, 1, 1 + (id as u32 % 28), 0, 0, 0)
                .unwrap(),
            quantity: Decimal::from_f64(qty).unwrap_or_default(),
            basis_usd: Decimal::from_f64(basis).unwrap_or_default(),
            disposed_at: if disposed {
                Some(Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap())
            } else {
                None
            },
        }
    }

    use chrono::Utc;

    #[test]
    fn loss_when_price_drops_below_basis() {
        let lots = vec![lot(1, 1.0, 100.0, false)];
        let l = loss_for_allocation(Uuid::nil(), Uuid::nil(), "ETH", &lots, 80.0).unwrap();
        assert!((l.unrealized_loss_usd - 20.0).abs() < 1e-9);
        assert_eq!(l.lots.len(), 1);
    }

    #[test]
    fn no_loss_when_price_above_basis() {
        let lots = vec![lot(1, 1.0, 100.0, false)];
        assert!(loss_for_allocation(Uuid::nil(), Uuid::nil(), "ETH", &lots, 110.0).is_none());
    }

    #[test]
    fn loss_aggregates_across_lots_skipping_winners() {
        let lots = vec![
            lot(1, 1.0, 100.0, false), // bought at 100, now 80 → $20 loss
            lot(2, 1.0, 50.0, false),  // bought at 50, now 80 → winner, skipped
            lot(3, 2.0, 200.0, false), // bought at 100/u, now 80/u → $40 loss
        ];
        let l = loss_for_allocation(Uuid::nil(), Uuid::nil(), "BTC", &lots, 80.0).unwrap();
        assert!((l.unrealized_loss_usd - 60.0).abs() < 1e-9);
        assert_eq!(l.lots.len(), 2, "winning lot should be excluded");
    }

    #[test]
    fn disposed_lots_are_ignored() {
        let lots = vec![lot(1, 1.0, 100.0, true)];
        assert!(loss_for_allocation(Uuid::nil(), Uuid::nil(), "ETH", &lots, 50.0).is_none());
    }

    #[test]
    fn disposal_plan_is_fifo() {
        let mut lots = vec![
            lot(1, 1.0, 100.0, false),
            lot(2, 1.0, 100.0, false),
            lot(3, 1.0, 100.0, false),
        ];
        // Force a non-FIFO insertion order to confirm the helper sorts.
        lots.swap(0, 2);

        let plan = plan_disposal(&lots, 2.5).unwrap();
        assert_eq!(plan.len(), 3);
        assert_eq!(plan[0].1, 1.0);
        assert_eq!(plan[1].1, 1.0);
        assert!((plan[2].1 - 0.5).abs() < 1e-9);
    }

    #[test]
    fn disposal_plan_errors_when_short() {
        let lots = vec![lot(1, 1.0, 100.0, false)];
        let err = plan_disposal(&lots, 2.0).unwrap_err();
        match err {
            DisposalError::InsufficientLots { short_by_qty } => {
                assert!((short_by_qty - 1.0).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn empty_disposal_is_noop() {
        let lots = vec![lot(1, 1.0, 100.0, false)];
        assert!(plan_disposal(&lots, 0.0).unwrap().is_empty());
    }
}
