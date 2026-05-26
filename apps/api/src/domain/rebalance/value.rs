//! `ValueUsd` — the canonical USD valuation primitive (INV-2).
//!
//! A `ValueUsd` can *only* be produced by `mark(quantity, price)` — a live mark.
//! The inner field is private, and there is deliberately **no** `From<f64>`, no
//! `Deserialize`, and no constructor from a raw stored amount. That makes it a
//! compile-time fact that any `ValueUsd` flowing through the system came from a
//! live (quantity × price) mark, never from a stale stored weight or percentage.
//! This is what structurally severs the "stale `current_weight`" disease: a
//! position the wallet no longer holds marks to `ValueUsd::ZERO`, so it cannot
//! masquerade as a held position anywhere value is consumed.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct ValueUsd(Decimal);

impl ValueUsd {
    pub const ZERO: ValueUsd = ValueUsd(Decimal::ZERO);

    /// The single live constructor: `value = quantity × price_usd`. A negative
    /// product (bad data) clamps to zero so a `ValueUsd` is never negative.
    pub fn mark(quantity: Decimal, price_usd: Decimal) -> ValueUsd {
        ValueUsd((quantity * price_usd).max(Decimal::ZERO))
    }

    pub fn as_decimal(self) -> Decimal {
        self.0
    }

    pub fn to_f64(self) -> f64 {
        self.0.to_f64().unwrap_or(0.0)
    }

    pub fn is_positive(self) -> bool {
        self.0 > Decimal::ZERO
    }

    /// The fraction of `total` this value represents (0 when `total` is zero).
    pub fn weight_of(self, total: ValueUsd) -> Decimal {
        if total.0 > Decimal::ZERO {
            self.0 / total.0
        } else {
            Decimal::ZERO
        }
    }
}

impl std::ops::Add for ValueUsd {
    type Output = ValueUsd;
    fn add(self, other: ValueUsd) -> ValueUsd {
        ValueUsd(self.0 + other.0)
    }
}

impl std::iter::Sum for ValueUsd {
    fn sum<I: Iterator<Item = ValueUsd>>(iter: I) -> ValueUsd {
        ValueUsd(iter.map(|v| v.0).sum())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn mark_is_quantity_times_price() {
        assert_eq!(
            ValueUsd::mark(dec!(0.5), dec!(2000)).as_decimal(),
            dec!(1000)
        );
    }

    #[test]
    fn a_position_no_longer_held_marks_to_zero() {
        // The phantom-row cure: zero quantity ⇒ zero value, regardless of any
        // stale stored percentage that might once have claimed otherwise.
        let v = ValueUsd::mark(dec!(0), dec!(2113.77));
        assert_eq!(v, ValueUsd::ZERO);
        assert!(!v.is_positive());
    }

    #[test]
    fn negative_marks_clamp_to_zero() {
        assert_eq!(ValueUsd::mark(dec!(-1), dec!(10)), ValueUsd::ZERO);
    }

    #[test]
    fn weight_of_is_share_of_total_and_zero_safe() {
        let eth = ValueUsd::mark(dec!(1), dec!(75));
        let usdc = ValueUsd::mark(dec!(25), dec!(1));
        let total = eth + usdc;
        assert_eq!(eth.weight_of(total), dec!(0.75));
        assert_eq!(eth.weight_of(ValueUsd::ZERO), Decimal::ZERO);
    }

    #[test]
    fn sum_aggregates_marked_values() {
        let total: ValueUsd = [
            ValueUsd::mark(dec!(1), dec!(10)),
            ValueUsd::mark(dec!(2), dec!(5)),
        ]
        .into_iter()
        .sum();
        assert_eq!(total.as_decimal(), dec!(20));
    }
}
