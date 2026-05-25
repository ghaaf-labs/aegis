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

use std::collections::{BTreeMap, BTreeSet};

use rust_decimal::Decimal;

use crate::modules::rebalance::models::ChainKey;

/// A persisted leg, reduced to what reservation accounting needs. Built from the
/// user's in-flight (`executing`) rebalances so the ledger reflects real locks.
#[derive(Debug, Clone)]
pub struct ReservationLeg {
    pub leg_index: i32,
    pub depends_on: Vec<i32>,
    pub kind: String,
    pub src_chain: Option<ChainKey>,
    pub dest_chain: Option<ChainKey>,
    pub amount_usdc: Decimal,
}

/// How much *pre-existing* USDC each chain owes to one in-flight plan (INV-8).
///
/// Only legs that draw down balance the user already holds count — never USDC a
/// bridge mints mid-plan. A `cross_chain_burn` consumes its source chain; an
/// acquire (`local_swap`/`park_usyc`/`fx_stablefx`) consumes its own chain
/// *unless* it explicitly depends on a `cross_chain_mint` in the same plan
/// (then it spends bridged USDC, already accounted as the burn on the source).
/// A `cross_chain_mint` is a receive, never a reservation. Pure + grouped per
/// plan so the "fed by a mint" rule is decided within a single transfer.
pub fn reserved_usdc_per_chain(legs: &[ReservationLeg]) -> BTreeMap<ChainKey, Decimal> {
    let mint_leg_indexes: BTreeSet<i32> = legs
        .iter()
        .filter(|l| l.kind == "cross_chain_mint")
        .map(|l| l.leg_index)
        .collect();

    let mut reserved: BTreeMap<ChainKey, Decimal> = BTreeMap::new();
    for leg in legs {
        if leg.amount_usdc <= Decimal::ZERO {
            continue;
        }
        let chain = match leg.kind.as_str() {
            "cross_chain_burn" => leg.src_chain,
            "local_swap" | "park_usyc" | "fx_stablefx" => {
                let funded_by_mint = leg
                    .depends_on
                    .iter()
                    .any(|dep| mint_leg_indexes.contains(dep));
                leg.src_chain.or(leg.dest_chain).filter(|_| !funded_by_mint)
            }
            // mint = receive; redeem produces USDC; nothing else draws balance.
            _ => None,
        };
        if let Some(chain) = chain {
            *reserved.entry(chain).or_insert(Decimal::ZERO) += leg.amount_usdc;
        }
    }
    reserved
}

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

    fn leg(
        leg_index: i32,
        kind: &str,
        src: Option<ChainKey>,
        dest: Option<ChainKey>,
        amt: Decimal,
        depends_on: Vec<i32>,
    ) -> ReservationLeg {
        ReservationLeg {
            leg_index,
            depends_on,
            kind: kind.into(),
            src_chain: src,
            dest_chain: dest,
            amount_usdc: amt,
        }
    }

    #[test]
    fn cross_chain_plan_reserves_only_the_source_burn_not_the_bridged_swap() {
        // burn Arc 7.08 -> mint Base 7.08 -> swap Base 7.08: the Base swap spends
        // bridged USDC, so only the Arc source is reserved (no double-count).
        let legs = vec![
            leg(
                0,
                "cross_chain_burn",
                Some(ChainKey::Arc),
                Some(ChainKey::Base),
                dec!(7.08),
                vec![],
            ),
            leg(
                1,
                "cross_chain_mint",
                Some(ChainKey::Arc),
                Some(ChainKey::Base),
                dec!(7.08),
                vec![0],
            ),
            leg(
                2,
                "local_swap",
                Some(ChainKey::Base),
                Some(ChainKey::Base),
                dec!(7.08),
                vec![1],
            ),
        ];
        let reserved = reserved_usdc_per_chain(&legs);
        assert_eq!(reserved.get(&ChainKey::Arc).copied(), Some(dec!(7.08)));
        assert_eq!(
            reserved.get(&ChainKey::Base),
            None,
            "bridged swap is not a reservation"
        );
    }

    #[test]
    fn independent_same_chain_swap_on_a_minted_chain_still_reserves() {
        // A plan can both mint USDC to Base for one branch and independently
        // spend pre-existing Base USDC for another branch. Only the branch that
        // depends on the mint is exempt from pre-existing balance reservation.
        let legs = vec![
            leg(
                0,
                "cross_chain_burn",
                Some(ChainKey::Arc),
                Some(ChainKey::Base),
                dec!(10),
                vec![],
            ),
            leg(
                1,
                "cross_chain_mint",
                Some(ChainKey::Arc),
                Some(ChainKey::Base),
                dec!(10),
                vec![0],
            ),
            leg(
                2,
                "local_swap",
                Some(ChainKey::Base),
                Some(ChainKey::Base),
                dec!(10),
                vec![1],
            ),
            leg(
                3,
                "local_swap",
                Some(ChainKey::Base),
                Some(ChainKey::Base),
                dec!(20),
                vec![],
            ),
        ];

        let reserved = reserved_usdc_per_chain(&legs);
        assert_eq!(reserved.get(&ChainKey::Arc).copied(), Some(dec!(10)));
        assert_eq!(reserved.get(&ChainKey::Base).copied(), Some(dec!(20)));
    }

    #[test]
    fn same_chain_swap_reserves_its_own_chain() {
        // A standalone Base USDC->ETH swap draws pre-existing Base USDC.
        let legs = vec![leg(
            0,
            "local_swap",
            Some(ChainKey::Base),
            Some(ChainKey::Base),
            dec!(20),
            vec![],
        )];
        let reserved = reserved_usdc_per_chain(&legs);
        assert_eq!(reserved.get(&ChainKey::Base).copied(), Some(dec!(20)));
    }

    #[test]
    fn mint_only_and_nonpositive_legs_reserve_nothing() {
        let legs = vec![
            leg(
                0,
                "cross_chain_mint",
                Some(ChainKey::Arc),
                Some(ChainKey::Base),
                dec!(5),
                vec![],
            ),
            leg(
                1,
                "local_swap",
                Some(ChainKey::Base),
                Some(ChainKey::Base),
                dec!(0),
                vec![],
            ),
        ];
        assert!(reserved_usdc_per_chain(&legs).is_empty());
    }
}
