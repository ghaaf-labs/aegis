use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::domain::token::native_chain;
use crate::error::AppError;
use crate::error::Result;
use crate::modules::rebalance::models::{ChainKey, PlanInput};
use crate::modules::rebalance::registry::{executable_token_symbols, RuntimeCapabilities};
use crate::modules::wallet_routes;
use crate::router::AppState;

const REAL_EXECUTION_CHAIN_USDC_BUFFER: f64 = 2.0;

pub(super) async fn build_plan_input(state: &AppState, portfolio_id: Uuid) -> Result<PlanInput> {
    // The planner consumes fractions (0-1), but the persisted
    // `current_weight` can lag behind execution. Use confirmed dollar values
    // when present so a just-executed or partially deployed portfolio cannot
    // plan from stale percentages.
    let portfolio: (Uuid, f64, serde_json::Value) = sqlx::query_as(
        "SELECT user_id, total_value_usd::DOUBLE PRECISION, goal FROM portfolios WHERE id = $1",
    )
    .bind(portfolio_id)
    .fetch_one(&state.db)
    .await?;
    let user_id = portfolio.0;
    let portfolio_value_usd = portfolio.1;
    let goal = portfolio.2;

    let allocations: Vec<(String, f64, f64, f64)> = sqlx::query_as(
        "SELECT asset_symbol, current_weight::DOUBLE PRECISION,
                value_usd::DOUBLE PRECISION, quantity::DOUBLE PRECISION
         FROM allocations WHERE portfolio_id = $1",
    )
    .bind(portfolio_id)
    .fetch_all(&state.db)
    .await?;

    let mut target_weights = HashMap::new();
    if let Some(target_obj) = goal.get("targetAllocation").and_then(|v| v.as_object()) {
        for (k, v) in target_obj {
            if let Some(n) = v.as_f64() {
                target_weights.insert(k.clone(), n / 100.0);
            }
        }
    }
    apply_route_preferences_to_targets(&goal, &mut target_weights);
    apply_execution_capabilities_to_targets(&state.config, &mut target_weights);
    let wallet_cash_only = real_wallet_cash_only_planning(&state.config);

    let relevant_symbols: Vec<String> = allocations
        .iter()
        .map(|(sym, _, _, _)| sym.clone())
        .chain(target_weights.keys().cloned())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let prices = load_planning_prices(state, &relevant_symbols).await;

    let allocation_values: Vec<(String, f64, f64)> = allocations
        .into_iter()
        .map(|(sym, weight, value_usd, quantity)| {
            let marked = marked_allocation_value(&sym, value_usd, quantity, &prices);
            (sym, weight, marked)
        })
        .collect();

    let allocation_value_sum: f64 = if wallet_cash_only {
        0.0
    } else {
        allocation_values.iter().map(|(_, _, v)| v.max(0.0)).sum()
    };
    let invested_value_usd = if allocation_value_sum > 0.0 {
        allocation_value_sum
    } else if wallet_cash_only {
        0.0
    } else {
        portfolio_value_usd
    };
    let mut invested_weights = HashMap::new();
    if !wallet_cash_only && invested_value_usd > 0.0 {
        for (sym, weight, value_usd) in allocation_values {
            let confirmed_value = if allocation_value_sum > 0.0 {
                value_usd.max(0.0)
            } else {
                (weight / 100.0) * portfolio_value_usd
            };
            invested_weights.insert(sym, confirmed_value / invested_value_usd);
        }
    }

    if target_weights.is_empty() {
        // Portfolios without a goal fall back to "stay where you are".
        target_weights = invested_weights.clone();
    }

    let mut usdc_per_chain = load_gateway_pool(state, user_id).await?;
    reserve_real_execution_usdc_buffer(&state.config, &mut usdc_per_chain);
    let idle_usdc: f64 = usdc_per_chain.values().copied().sum();
    let plan_value_usd = invested_value_usd + idle_usdc;
    let current_weights = if idle_usdc > 0.0 && plan_value_usd > 0.0 {
        invested_weights
            .into_iter()
            .map(|(sym, weight)| {
                let invested_value = weight * invested_value_usd;
                (sym, invested_value / plan_value_usd)
            })
            .collect()
    } else {
        invested_weights
    };

    // Latest classified regime drives the "let winners run" asymmetric bands.
    let regime: Option<String> = sqlx::query_scalar(
        "SELECT regime FROM agent_decisions
         WHERE portfolio_id = $1 AND regime IS NOT NULL
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(portfolio_id)
    .fetch_optional(&state.db)
    .await?
    .flatten();

    Ok(PlanInput {
        portfolio_value_usd: plan_value_usd,
        current_weights,
        target_weights,
        usdc_per_chain,
        drift_threshold: 0.05,
        dust_threshold_usd: 5.0,
        prices,
        regime,
    })
}

fn real_wallet_cash_only_planning(cfg: &crate::config::Config) -> bool {
    let caps = RuntimeCapabilities::from_config(cfg);
    caps.real_mode && cfg.circle_wallet_exec
}

fn reserve_real_execution_usdc_buffer(
    cfg: &crate::config::Config,
    usdc_per_chain: &mut HashMap<ChainKey, f64>,
) {
    let caps = RuntimeCapabilities::from_config(cfg);
    if !caps.real_mode {
        return;
    }

    for amount in usdc_per_chain.values_mut() {
        if *amount > 0.0 {
            *amount = (*amount - REAL_EXECUTION_CHAIN_USDC_BUFFER).max(0.0);
        }
    }
}

pub(super) fn apply_execution_capabilities_to_targets(
    cfg: &crate::config::Config,
    target_weights: &mut HashMap<String, f64>,
) {
    if target_weights.is_empty() {
        return;
    }
    let caps = RuntimeCapabilities::from_config(cfg);
    if !caps.real_mode {
        return;
    }
    let executable = executable_token_symbols(&caps, cfg);
    retain_executable_targets(target_weights, &executable);
}

fn retain_executable_targets(target_weights: &mut HashMap<String, f64>, executable: &[&str]) {
    let mut dropped_weight = 0.0;
    target_weights.retain(|symbol, weight| {
        let keep = executable.iter().any(|e| e.eq_ignore_ascii_case(symbol));
        if !keep && weight.is_finite() && *weight > 0.0 {
            dropped_weight += *weight;
        }
        keep
    });
    if dropped_weight > 0.0 {
        *target_weights.entry("USDC".to_string()).or_insert(0.0) += dropped_weight;
    }
    if target_weights.is_empty() {
        target_weights.insert("USDC".to_string(), 1.0);
    }
}

pub(super) async fn load_planning_prices(
    state: &AppState,
    symbols: &[String],
) -> HashMap<String, f64> {
    if symbols.is_empty() {
        return HashMap::new();
    }

    let mut prices = crate::modules::market_data::service::fetch_snapshot(state.prices.as_ref())
        .await
        .map(|snapshot| {
            snapshot
                .assets
                .into_iter()
                .map(|asset| (asset.symbol, asset.price_usd))
                .filter(|(_, price)| price.is_finite() && *price > 0.0)
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();

    if prices.len() < symbols.len() {
        if let Ok(history) = crate::modules::market_data::service::get_historical_prices(
            &state.db,
            symbols,
            chrono::Utc::now(),
        )
        .await
        {
            for (symbol, price) in history {
                prices.entry(symbol).or_insert(price);
            }
        }
    }

    prices
}

pub(super) fn marked_allocation_value(
    symbol: &str,
    stored_value_usd: f64,
    quantity: f64,
    prices: &HashMap<String, f64>,
) -> f64 {
    let price = prices
        .get(symbol)
        .copied()
        .or_else(|| stable_planning_price(symbol));
    if quantity > 0.0 && stored_value_usd > 0.0 {
        if let Some(price) = price.filter(|p| p.is_finite() && *p > 0.0) {
            return quantity * price;
        }
    }
    stored_value_usd
}

pub(super) fn stable_planning_price(symbol: &str) -> Option<f64> {
    match symbol {
        "USDC" | "USYC" => Some(1.0),
        _ => None,
    }
}

pub(super) fn apply_route_preferences_to_targets(
    goal: &serde_json::Value,
    target_weights: &mut HashMap<String, f64>,
) {
    let Some(route_preferences) = goal.get("routePreferences") else {
        return;
    };

    let allowed_tokens = route_preference_set(route_preferences, "tokens");
    if !allowed_tokens.is_empty() {
        target_weights.retain(|symbol, _| {
            symbol == "USDC" || allowed_tokens.contains(&symbol.to_ascii_uppercase())
        });
    }

    let selected_networks = route_preference_set(route_preferences, "networks");
    if selected_networks.is_empty() {
        return;
    }

    let arc_allowed =
        selected_networks.contains(wallet_routes::ARC_TESTNET) || selected_networks.contains("ARC");
    let base_allowed = selected_networks.contains(wallet_routes::BASE_SEPOLIA)
        || selected_networks.contains("BASE");
    target_weights.retain(|symbol, _| {
        // Gate each target by the chain it lands on (registry canonical chain;
        // USYC→Arc, everything else→Base). Replaces the old native-symbol lists.
        match native_chain(symbol) {
            ChainKey::Arc => arc_allowed,
            ChainKey::Base => base_allowed,
            // Other execution chains aren't gated by the Arc/Base toggles.
            _ => true,
        }
    });
}

pub(super) fn route_preference_set(
    route_preferences: &serde_json::Value,
    key: &str,
) -> HashSet<String> {
    let mut values: HashSet<String> = route_preferences
        .get(key)
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .map(|v| v.trim().to_ascii_uppercase())
        .filter(|v| !v.is_empty())
        .collect();
    if values.remove("BTC_ETH_SOL") {
        values.insert("BTC".into());
        values.insert("ETH".into());
        values.insert("SOL".into());
    }
    values
}

/// Lookup unified USDC by chain from Circle Gateway. Real execution fails
/// closed when Gateway is unavailable; mock/demo mode degrades to a zero pool
/// so local review screens can still be exercised.
pub(super) async fn load_gateway_pool(
    state: &AppState,
    user_id: Uuid,
) -> Result<HashMap<ChainKey, f64>> {
    let mut pool: HashMap<ChainKey, f64> = HashMap::new();
    pool.insert(ChainKey::Arc, 0.0);
    pool.insert(ChainKey::Base, 0.0);

    match crate::modules::gateway::service::fetch_balance_for_user(
        &state.db,
        &state.http,
        &state.config,
        user_id,
    )
    .await
    {
        Ok(b) => {
            for (chain, amount) in b.per_chain {
                if let Some(key) = ChainKey::parse(chain.to_lowercase().as_str()) {
                    pool.insert(key, amount);
                }
            }
        }
        Err(e) => {
            if !state.config.execution_mock && !state.config.circle_mock {
                return Err(AppError::Conflict(
                    "Gateway balance is unavailable, so Aegis cannot build a real rebalance plan safely. Retry after Circle Gateway responds."
                        .into(),
                ));
            }
            tracing::warn!(error=%e, ?user_id, "gateway balance fetch failed; mock planner will use zero pool");
        }
    }
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn planning_value_prefers_live_price_and_stable_fallback() {
        let mut prices = HashMap::new();
        prices.insert("BTC".to_string(), 77_000.0);

        assert_eq!(marked_allocation_value("BTC", 840.0, 0.01, &prices), 770.0);
        assert_eq!(
            marked_allocation_value("USYC", 60.0, 60.0, &HashMap::new()),
            60.0
        );
        assert_eq!(
            marked_allocation_value("ETH", 300.0, 0.1, &HashMap::new()),
            300.0
        );
    }

    #[test]
    fn route_preferences_filter_unselected_target_tokens() {
        let goal = json!({
            "targetAllocation": {"USDC": 40, "BTC": 30, "ETH": 20, "USYC": 10},
            "routePreferences": {
                "networks": ["ARC-TESTNET", "BASE-SEPOLIA"],
                "tokens": ["USDC", "USYC"],
                "watchlist": ["BTC_ETH_SOL"]
            }
        });
        let mut targets = HashMap::from([
            ("USDC".to_string(), 0.40),
            ("BTC".to_string(), 0.30),
            ("ETH".to_string(), 0.20),
            ("USYC".to_string(), 0.10),
        ]);

        apply_route_preferences_to_targets(&goal, &mut targets);

        assert!(targets.contains_key("USDC"));
        assert!(targets.contains_key("USYC"));
        assert!(!targets.contains_key("BTC"));
        assert!(!targets.contains_key("ETH"));
    }

    #[test]
    fn route_preferences_filter_targets_by_selected_execution_networks() {
        let goal = json!({
            "routePreferences": {
                "networks": ["ARC-TESTNET"],
                "tokens": ["BTC_ETH_SOL", "USYC", "EURC"]
            }
        });
        let mut targets = HashMap::from([
            ("BTC".to_string(), 0.30),
            ("ETH".to_string(), 0.20),
            ("USYC".to_string(), 0.30),
            ("EURC".to_string(), 0.20),
        ]);

        apply_route_preferences_to_targets(&goal, &mut targets);

        // With only Arc selected, the Base-native sleeves (BTC/ETH and now EURC,
        // which trades on the Base USDC/EURC pool) drop out; only Arc-native
        // USYC survives.
        assert_eq!(
            targets
                .keys()
                .cloned()
                .collect::<std::collections::HashSet<_>>(),
            std::collections::HashSet::from(["USYC".to_string()])
        );
    }

    #[test]
    fn route_preferences_keep_eurc_when_base_selected() {
        // EURC is Base-native now (Base USDC/EURC DEX pool), so selecting Base
        // keeps it even when Arc is not selected.
        let goal = json!({
            "routePreferences": {
                "networks": ["BASE-SEPOLIA"],
                "tokens": ["USYC", "EURC"]
            }
        });
        let mut targets = HashMap::from([("USYC".to_string(), 0.50), ("EURC".to_string(), 0.50)]);

        apply_route_preferences_to_targets(&goal, &mut targets);

        assert!(targets.contains_key("EURC"));
        assert!(!targets.contains_key("USYC"));
    }

    #[test]
    fn executable_filter_moves_unavailable_target_weight_to_usdc() {
        let mut targets = HashMap::from([
            ("LINK".to_string(), 0.25),
            ("UNI".to_string(), 0.15),
            ("USDC".to_string(), 0.60),
        ]);

        retain_executable_targets(&mut targets, &["USDC", "LINK"]);

        assert_eq!(targets.get("LINK").copied(), Some(0.25));
        let usdc = targets.get("USDC").copied().unwrap_or_default();
        assert!((usdc - 0.75).abs() < 1e-9);
        assert!(!targets.contains_key("UNI"));
    }

    #[test]
    fn executable_filter_falls_back_to_usdc_when_all_targets_are_unavailable() {
        let mut targets = HashMap::from([("UNI".to_string(), 1.0)]);

        retain_executable_targets(&mut targets, &["USDC"]);

        assert_eq!(targets, HashMap::from([("USDC".to_string(), 1.0)]));
    }

    #[test]
    fn real_execution_pool_keeps_chain_usdc_buffer() {
        let mut cfg = crate::config::test_config();
        cfg.execution_mock = false;
        cfg.circle_mock = false;
        let mut pool = HashMap::from([
            (ChainKey::Arc, 3.25),
            (ChainKey::Base, 1.50),
            (ChainKey::EthSepolia, 20.0),
        ]);

        reserve_real_execution_usdc_buffer(&cfg, &mut pool);

        assert_eq!(pool.get(&ChainKey::Arc).copied(), Some(1.25));
        assert_eq!(pool.get(&ChainKey::Base).copied(), Some(0.0));
        assert_eq!(pool.get(&ChainKey::EthSepolia).copied(), Some(18.0));
    }

    #[test]
    fn mock_execution_pool_does_not_keep_chain_buffer() {
        let cfg = crate::config::test_config();
        let mut pool = HashMap::from([(ChainKey::Arc, 3.25), (ChainKey::Base, 1.50)]);

        reserve_real_execution_usdc_buffer(&cfg, &mut pool);

        assert_eq!(pool.get(&ChainKey::Arc).copied(), Some(3.25));
        assert_eq!(pool.get(&ChainKey::Base).copied(), Some(1.50));
    }

    #[test]
    fn real_circle_wallet_planning_uses_gateway_cash_only() {
        let mut cfg = crate::config::test_config();
        cfg.execution_mock = false;
        cfg.circle_mock = false;
        cfg.circle_wallet_exec = true;

        assert!(real_wallet_cash_only_planning(&cfg));
    }

    #[test]
    fn eoa_or_mock_planning_may_use_allocation_book() {
        let mut cfg = crate::config::test_config();
        cfg.execution_mock = false;
        cfg.circle_mock = false;
        cfg.circle_wallet_exec = false;
        assert!(!real_wallet_cash_only_planning(&cfg));

        cfg.execution_mock = true;
        cfg.circle_mock = true;
        cfg.circle_wallet_exec = true;
        assert!(!real_wallet_cash_only_planning(&cfg));
    }
}
