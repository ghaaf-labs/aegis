use std::collections::{BTreeMap, HashMap};

use rust_decimal::Decimal;
use uuid::Uuid;

use crate::error::Result;
use crate::modules::rebalance::models::ChainKey;
use crate::modules::rebalance::registry::RuntimeCapabilities;
use crate::modules::rebalance::reservations::{reserved_usdc_per_chain, ReservationLeg};
use crate::router::AppState;

const REAL_EXECUTION_CHAIN_USDC_BUFFER: Decimal = Decimal::from_parts(2, 0, 0, false, 0);

pub(super) fn reserve_real_execution_usdc_buffer(
    cfg: &crate::config::Config,
    usdc_per_chain: &mut HashMap<ChainKey, Decimal>,
) {
    let caps = RuntimeCapabilities::from_config(cfg);
    if !caps.real_mode {
        return;
    }

    for amount in usdc_per_chain.values_mut() {
        if *amount > Decimal::ZERO {
            *amount = (*amount - REAL_EXECUTION_CHAIN_USDC_BUFFER).max(Decimal::ZERO);
        }
    }
}

#[derive(sqlx::FromRow)]
struct ReservationLegRow {
    rebalance_id: Uuid,
    leg_index: i32,
    depends_on: Vec<i32>,
    kind: String,
    src_chain: Option<String>,
    dest_chain: Option<String>,
    amount_usdc: Decimal,
}

pub(super) async fn subtract_active_reservations(
    state: &AppState,
    user_id: Uuid,
    pool: &mut HashMap<ChainKey, Decimal>,
) -> Result<()> {
    let rows: Vec<ReservationLegRow> = sqlx::query_as(
        "SELECT l.rebalance_id, l.leg_index, l.depends_on, l.kind, l.src_chain, l.dest_chain, l.amount_usdc
         FROM rebalance_legs l
         JOIN rebalances r ON r.id = l.rebalance_id
         JOIN portfolios p ON p.id = r.portfolio_id
         WHERE p.user_id = $1 AND r.status = 'executing'",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    if rows.is_empty() {
        return Ok(());
    }

    let mut by_plan: HashMap<Uuid, Vec<ReservationLeg>> = HashMap::new();
    for row in rows {
        by_plan
            .entry(row.rebalance_id)
            .or_default()
            .push(ReservationLeg {
                leg_index: row.leg_index,
                depends_on: row.depends_on,
                kind: row.kind,
                src_chain: row
                    .src_chain
                    .and_then(|c| ChainKey::parse(&c.to_lowercase())),
                dest_chain: row
                    .dest_chain
                    .and_then(|c| ChainKey::parse(&c.to_lowercase())),
                amount_usdc: row.amount_usdc,
            });
    }
    let mut reserved: BTreeMap<ChainKey, Decimal> = BTreeMap::new();
    for legs in by_plan.values() {
        for (chain, amount) in reserved_usdc_per_chain(legs) {
            *reserved.entry(chain).or_insert(Decimal::ZERO) += amount;
        }
    }

    for (chain, amount) in reserved {
        if let Some(available) = pool.get_mut(&chain) {
            *available = (*available - amount).max(Decimal::ZERO);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_execution_pool_keeps_chain_usdc_buffer() {
        let mut cfg = crate::config::test_config();
        cfg.execution_mock = false;
        cfg.circle_mock = false;
        let mut pool = HashMap::from([
            (ChainKey::Arc, Decimal::new(325, 2)),
            (ChainKey::Base, Decimal::new(150, 2)),
            (ChainKey::EthSepolia, Decimal::new(20, 0)),
        ]);

        reserve_real_execution_usdc_buffer(&cfg, &mut pool);

        assert_eq!(
            pool.get(&ChainKey::Arc).copied(),
            Some(Decimal::new(125, 2))
        );
        assert_eq!(pool.get(&ChainKey::Base).copied(), Some(Decimal::ZERO));
        assert_eq!(
            pool.get(&ChainKey::EthSepolia).copied(),
            Some(Decimal::new(18, 0))
        );
    }

    #[test]
    fn mock_execution_pool_does_not_keep_chain_buffer() {
        let cfg = crate::config::test_config();
        let mut pool = HashMap::from([
            (ChainKey::Arc, Decimal::new(325, 2)),
            (ChainKey::Base, Decimal::new(150, 2)),
        ]);

        reserve_real_execution_usdc_buffer(&cfg, &mut pool);

        assert_eq!(
            pool.get(&ChainKey::Arc).copied(),
            Some(Decimal::new(325, 2))
        );
        assert_eq!(
            pool.get(&ChainKey::Base).copied(),
            Some(Decimal::new(150, 2))
        );
    }
}
