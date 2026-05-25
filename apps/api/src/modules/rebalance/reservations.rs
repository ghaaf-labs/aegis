//! Reservation ledger (INV-8): the spendable quantity of any asset is
//! `settleable = balance − reserved`, where `reserved` is locked by other
//! in-flight sagas.
//!
//! This is the structural cure for concurrent double-spend / "I thought I had
//! the funds" 409s: a plan reserves what it intends to spend *before* it is
//! exposed for approval, and a second concurrent plan can only ever see — and
//! lock — `balance − already_reserved`. Pure logic; the DB `reservations` table
//! persists it across processes. Money is `Decimal` here (a money boundary),
//! not the planner's `f64`.

use std::collections::BTreeMap;

use rust_decimal::Decimal;

#[derive(Debug, Default, Clone)]
pub struct ReservationLedger {
    /// Asset key → total currently locked by active sagas.
    reserved: BTreeMap<String, Decimal>,
}

/// A reservation was refused because it exceeded the spendable balance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsufficientSettleable {
    pub asset: String,
    pub requested: Decimal,
    pub settleable: Decimal,
}

impl ReservationLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reserved(&self, asset: &str) -> Decimal {
        self.reserved.get(asset).copied().unwrap_or(Decimal::ZERO)
    }

    /// Spendable right now = `balance − already_reserved`, never negative.
    pub fn settleable(&self, asset: &str, balance: Decimal) -> Decimal {
        (balance - self.reserved(asset)).max(Decimal::ZERO)
    }

    /// Lock `amount` of `asset` iff it fits within `settleable`. Fails closed —
    /// the caller must not spend what it could not reserve. A non-positive
    /// amount is a no-op (nothing to lock).
    pub fn try_reserve(
        &mut self,
        asset: &str,
        amount: Decimal,
        balance: Decimal,
    ) -> Result<(), InsufficientSettleable> {
        if amount <= Decimal::ZERO {
            return Ok(());
        }
        let settleable = self.settleable(asset, balance);
        if amount > settleable {
            return Err(InsufficientSettleable {
                asset: asset.to_string(),
                requested: amount,
                settleable,
            });
        }
        *self
            .reserved
            .entry(asset.to_string())
            .or_insert(Decimal::ZERO) += amount;
        Ok(())
    }

    /// Release a previously-held reservation (plan abandoned, or the leg settled
    /// and the lock is no longer needed). Clamps at zero and prunes empty keys.
    pub fn release(&mut self, asset: &str, amount: Decimal) {
        if let Some(reserved) = self.reserved.get_mut(asset) {
            *reserved = (*reserved - amount).max(Decimal::ZERO);
            if reserved.is_zero() {
                self.reserved.remove(asset);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn settleable_is_balance_minus_reserved() {
        let mut led = ReservationLedger::new();
        led.try_reserve("USDC", dec!(30), dec!(100)).unwrap();
        assert_eq!(led.settleable("USDC", dec!(100)), dec!(70));
    }

    #[test]
    fn concurrent_sagas_cannot_double_spend_the_same_units() {
        // Saga A locks 70 of a 100 balance; saga B may only take the remaining 30.
        let mut led = ReservationLedger::new();
        led.try_reserve("USDC", dec!(70), dec!(100)).unwrap();
        let err = led.try_reserve("USDC", dec!(40), dec!(100)).unwrap_err();
        assert_eq!(err.settleable, dec!(30));
        assert_eq!(err.requested, dec!(40));
        // ...but 30 still fits.
        led.try_reserve("USDC", dec!(30), dec!(100)).unwrap();
        assert_eq!(led.settleable("USDC", dec!(100)), dec!(0));
    }

    #[test]
    fn release_restores_settleable_and_prunes() {
        let mut led = ReservationLedger::new();
        led.try_reserve("ETH", dec!(1), dec!(2)).unwrap();
        led.release("ETH", dec!(1));
        assert_eq!(led.reserved("ETH"), dec!(0));
        assert_eq!(led.settleable("ETH", dec!(2)), dec!(2));
    }

    #[test]
    fn non_positive_reservation_is_a_noop() {
        let mut led = ReservationLedger::new();
        led.try_reserve("USDC", dec!(0), dec!(100)).unwrap();
        led.try_reserve("USDC", dec!(-5), dec!(100)).unwrap();
        assert_eq!(led.reserved("USDC"), dec!(0));
    }

    #[test]
    fn reserving_the_whole_balance_then_anything_more_fails() {
        let mut led = ReservationLedger::new();
        led.try_reserve("USDC", dec!(100), dec!(100)).unwrap();
        assert!(led.try_reserve("USDC", dec!(1), dec!(100)).is_err());
        assert_eq!(led.settleable("USDC", dec!(100)), dec!(0));
    }
}
