use std::collections::HashMap;

use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::modules::gateway::service::GatewayBalance;
use crate::modules::rebalance::models::{ChainKey, SellSources};
use crate::router::AppState;

use super::valuation::stable_planning_price;

pub(super) async fn load_gateway_balance(
    state: &AppState,
    user_id: Uuid,
) -> Result<GatewayBalance> {
    match crate::modules::gateway::service::fetch_balance_for_user(
        &state.db,
        &state.http,
        &state.config,
        user_id,
    )
    .await
    {
        Ok(balance) => Ok(balance),
        Err(e) => {
            if !state.config.execution_mock && !state.config.circle_mock {
                return Err(AppError::Conflict(
                    "Gateway balance is unavailable, so Aegis cannot build a real rebalance plan safely. Retry after Circle Gateway responds."
                        .into(),
                ));
            }
            tracing::warn!(error=%e, ?user_id, "gateway balance fetch failed; mock planner will use zero pool");
            Ok(GatewayBalance::default())
        }
    }
}

pub(super) fn usdc_pool_from_balance(balance: &GatewayBalance) -> HashMap<ChainKey, f64> {
    let mut pool: HashMap<ChainKey, f64> = HashMap::new();
    pool.insert(ChainKey::Arc, 0.0);
    pool.insert(ChainKey::Base, 0.0);
    for (chain, amount) in &balance.per_chain {
        if let Some(key) = ChainKey::parse(chain.to_lowercase().as_str()) {
            pool.insert(key, *amount);
        }
    }
    pool
}

pub(super) fn wallet_holding_values_by_chain(
    balance: &GatewayBalance,
    prices: &HashMap<String, f64>,
    executable: &[&str],
) -> HashMap<String, HashMap<ChainKey, f64>> {
    let mut values: HashMap<String, HashMap<ChainKey, f64>> = HashMap::new();
    for (chain_str, tokens) in &balance.token_balances_by_chain {
        let Some(chain) = ChainKey::parse(&chain_str.to_lowercase()) else {
            continue;
        };
        for (symbol, qty) in tokens {
            if symbol.eq_ignore_ascii_case("USDC")
                || !executable.iter().any(|e| e.eq_ignore_ascii_case(symbol))
            {
                continue;
            }
            let Some(price) = prices
                .get(symbol)
                .copied()
                .or_else(|| stable_planning_price(symbol))
                .filter(|p| p.is_finite() && *p > 0.0)
            else {
                continue;
            };
            let value = qty * price;
            if value > 0.0 {
                *values
                    .entry(symbol.clone())
                    .or_default()
                    .entry(chain)
                    .or_insert(0.0) += value;
            }
        }
    }
    if executable.iter().any(|e| e.eq_ignore_ascii_case("EURC")) {
        for (chain_str, qty) in &balance.per_chain_eurc {
            let Some(chain) = ChainKey::parse(&chain_str.to_lowercase()) else {
                continue;
            };
            let Some(price) = prices
                .get("EURC")
                .copied()
                .or_else(|| stable_planning_price("EURC"))
                .filter(|p| p.is_finite() && *p > 0.0)
            else {
                continue;
            };
            let value = qty * price;
            if value > 0.0 {
                *values
                    .entry("EURC".to_string())
                    .or_default()
                    .entry(chain)
                    .or_insert(0.0) += value;
            }
        }
    }
    values
}

pub(super) fn wallet_holdings_marked(
    sell_sources: &HashMap<String, SellSources>,
) -> Vec<(String, f64, f64)> {
    let mut marked: Vec<(String, f64, f64)> = sell_sources
        .iter()
        .filter_map(|(symbol, sources)| match sources {
            SellSources::ByChain(by_chain) => Some((symbol.clone(), 0.0, by_chain.values().sum())),
            SellSources::CanonicalFallback | SellSources::Frozen => None,
        })
        .filter(|(_, _, value): &(String, f64, f64)| value.is_finite() && *value > 0.0)
        .collect();
    marked.sort_by(|a, b| a.0.cmp(&b.0));
    marked
}

pub(super) fn frozen_holdings_value(
    balance: &GatewayBalance,
    prices: &HashMap<String, f64>,
    executable: &[&str],
) -> f64 {
    balance
        .token_balances_by_chain
        .values()
        .flat_map(|tokens| tokens.iter())
        .filter(|(symbol, _)| {
            !symbol.eq_ignore_ascii_case("USDC")
                && !executable.iter().any(|e| e.eq_ignore_ascii_case(symbol))
        })
        .filter_map(|(symbol, qty)| {
            let price = prices.get(symbol).copied()?;
            let value = qty * price;
            (price.is_finite() && value > 0.0).then_some(value)
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallet_holding_values_preserves_actual_holding_chains() {
        let mut balance = GatewayBalance::default();
        balance.token_balances_by_chain.insert(
            "base".to_string(),
            HashMap::from([("ETH".to_string(), 0.5), ("RANDO".to_string(), 100.0)]),
        );
        balance.token_balances_by_chain.insert(
            "arc".to_string(),
            HashMap::from([("ETH".to_string(), 0.25)]),
        );
        let prices = HashMap::from([("ETH".to_string(), 2000.0), ("RANDO".to_string(), 1.0)]);

        let values = wallet_holding_values_by_chain(&balance, &prices, &["ETH"]);
        let sell_sources = values
            .clone()
            .into_iter()
            .map(|(symbol, values)| (symbol, SellSources::by_chain(values)))
            .collect();
        let marked = wallet_holdings_marked(&sell_sources);

        assert_eq!(marked.len(), 1);
        assert_eq!(marked[0].0, "ETH");
        assert!((marked[0].2 - 1500.0).abs() < 1e-9);
        assert!((values["ETH"][&ChainKey::Base] - 1000.0).abs() < 1e-9);
        assert!((values["ETH"][&ChainKey::Arc] - 500.0).abs() < 1e-9);
    }

    #[test]
    fn frozen_holdings_value_sums_track_only_holdings_only() {
        let mut balance = GatewayBalance::default();
        balance.token_balances_by_chain.insert(
            "base".to_string(),
            HashMap::from([("ETH".to_string(), 0.5), ("RANDO".to_string(), 100.0)]),
        );
        let prices = HashMap::from([("ETH".to_string(), 2000.0), ("RANDO".to_string(), 3.0)]);

        let frozen = frozen_holdings_value(&balance, &prices, &["ETH"]);
        assert!((frozen - 300.0).abs() < 1e-9);
    }
}
