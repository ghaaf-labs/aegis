use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::error::Result;
use crate::modules::rebalance::models::{ChainKey, PlanInput, ARC_NATIVE_SYMBOLS, BASE_NATIVE_SYMBOLS};
use crate::modules::wallet_routes;
use crate::router::AppState;
use crate::error::AppError;

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

    let allocation_value_sum: f64 = allocation_values.iter().map(|(_, _, v)| v.max(0.0)).sum();
    let invested_value_usd = if allocation_value_sum > 0.0 {
        allocation_value_sum
    } else {
        portfolio_value_usd
    };
    let mut invested_weights = HashMap::new();
    if invested_value_usd > 0.0 {
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

    let usdc_per_chain = load_gateway_pool(state, user_id).await?;
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
        let symbol = symbol.as_str();
        if ARC_NATIVE_SYMBOLS.contains(&symbol) {
            return arc_allowed;
        }
        if BASE_NATIVE_SYMBOLS.contains(&symbol) {
            return base_allowed;
        }
        true
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
            targets.keys().cloned().collect::<std::collections::HashSet<_>>(),
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
}
