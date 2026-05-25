use std::collections::{HashMap, HashSet};

use crate::domain::token::native_chain;
use crate::modules::rebalance::models::ChainKey;
use crate::modules::rebalance::registry::{executable_token_symbols, RuntimeCapabilities};
use crate::modules::wallet_routes;

use super::super::outcome::DeferredTarget;

/// Fold any target weight for a non-executable sleeve into USDC before the
/// planner builds legs.
pub(super) fn fold_nonexecutable_targets_into_usdc(
    cfg: &crate::config::Config,
    target_weights: &mut HashMap<String, f64>,
) -> Vec<DeferredTarget> {
    if target_weights.is_empty() {
        return Vec::new();
    }
    let caps = RuntimeCapabilities::from_config(cfg);
    if !caps.real_mode {
        return Vec::new();
    }
    let executable = executable_token_symbols(&caps, cfg);
    retain_executable_targets(target_weights, &executable)
}

fn retain_executable_targets(
    target_weights: &mut HashMap<String, f64>,
    executable: &[&str],
) -> Vec<DeferredTarget> {
    let mut folded = 0.0;
    let mut deferred = Vec::new();
    target_weights.retain(|symbol, weight| {
        let keep = symbol.eq_ignore_ascii_case("USDC")
            || executable.iter().any(|e| e.eq_ignore_ascii_case(symbol));
        if !keep && weight.is_finite() && *weight > 0.0 {
            folded += *weight;
            deferred.push(DeferredTarget {
                symbol: symbol.clone(),
                target_weight: *weight,
                reason: deferred_reason(symbol),
            });
        }
        keep
    });
    if folded > 0.0 {
        *target_weights.entry("USDC".to_string()).or_insert(0.0) += folded;
    }
    deferred.sort_by(|a, b| {
        a.symbol
            .to_ascii_lowercase()
            .cmp(&b.symbol.to_ascii_lowercase())
    });
    deferred
}

fn deferred_reason(symbol: &str) -> String {
    let chain = native_chain(symbol).as_str();
    format!("No live execution route on {chain} right now; held as USDC reserve until one opens.")
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
    target_weights.retain(|symbol, _| match native_chain(symbol) {
        ChainKey::Arc => arc_allowed,
        ChainKey::Base => base_allowed,
        _ => true,
    });
}

fn route_preference_set(route_preferences: &serde_json::Value, key: &str) -> HashSet<String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

        assert_eq!(
            targets.keys().cloned().collect::<HashSet<_>>(),
            HashSet::from(["USYC".to_string()])
        );
    }

    #[test]
    fn route_preferences_keep_eurc_when_base_selected() {
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
    fn fold_moves_nonexecutable_target_weight_into_usdc() {
        let mut targets = HashMap::from([
            ("ETH".to_string(), 0.28),
            ("EURC".to_string(), 0.10),
            ("cbBTC".to_string(), 0.12),
            ("USDC".to_string(), 0.50),
        ]);

        let deferred = retain_executable_targets(&mut targets, &["USDC", "ETH"]);

        assert_eq!(targets.get("ETH").copied(), Some(0.28));
        assert!(!targets.contains_key("EURC"));
        assert!(!targets.contains_key("cbBTC"));
        let usdc = targets.get("USDC").copied().unwrap_or_default();
        assert!((usdc - 0.72).abs() < 1e-9);
        let symbols: Vec<&str> = deferred.iter().map(|d| d.symbol.as_str()).collect();
        assert_eq!(symbols, vec!["cbBTC", "EURC"]);
        assert!(deferred.iter().all(|d| !d.reason.is_empty()));
    }

    #[test]
    fn fold_reduces_all_nonexecutable_target_to_usdc() {
        let mut targets = HashMap::from([("EURC".to_string(), 1.0)]);

        retain_executable_targets(&mut targets, &["USDC", "ETH"]);

        assert_eq!(targets, HashMap::from([("USDC".to_string(), 1.0)]));
    }
}
