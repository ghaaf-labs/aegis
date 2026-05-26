use std::collections::HashMap;

use crate::router::AppState;

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
        "USDC" | "EURC" => Some(1.0),
        _ => None,
    }
}

pub(super) enum ValuationMode {
    WalletHoldings,
    AllocationBook,
}

pub(super) struct Valuation {
    pub(super) invested_weights: HashMap<String, f64>,
    pub(super) current_weights: HashMap<String, f64>,
    pub(super) plan_value_usd: f64,
}

pub(super) fn derive_valuation(
    mode: ValuationMode,
    marked_allocations: &[(String, f64, f64)],
    portfolio_value_usd: f64,
    idle_usdc: f64,
    frozen_value_usd: f64,
) -> Valuation {
    let allocation_value_sum: f64 = marked_allocations.iter().map(|(_, _, v)| v.max(0.0)).sum();
    let invested_value_usd = match mode {
        ValuationMode::WalletHoldings => allocation_value_sum,
        ValuationMode::AllocationBook => {
            if allocation_value_sum > 0.0 {
                allocation_value_sum
            } else {
                portfolio_value_usd
            }
        }
    };
    let mut invested_weights = HashMap::new();
    if invested_value_usd > 0.0 {
        for (sym, stale_weight_pct, marked_value) in marked_allocations {
            let confirmed_value = if allocation_value_sum > 0.0 {
                marked_value.max(0.0)
            } else {
                (stale_weight_pct / 100.0) * portfolio_value_usd
            };
            invested_weights.insert(sym.clone(), confirmed_value / invested_value_usd);
        }
    }
    let plan_value_usd = invested_value_usd + idle_usdc + frozen_value_usd;
    let current_weights = if (idle_usdc + frozen_value_usd) > 0.0 && plan_value_usd > 0.0 {
        invested_weights
            .iter()
            .map(|(sym, weight)| (sym.clone(), (weight * invested_value_usd) / plan_value_usd))
            .collect()
    } else {
        invested_weights.clone()
    };
    Valuation {
        invested_weights,
        current_weights,
        plan_value_usd,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn valuation_wallet_holdings_empty_wallet_is_cash_only() {
        let marked = vec![("ETH".to_string(), 100.0, 0.0)];
        let v = derive_valuation(ValuationMode::WalletHoldings, &marked, 1000.0, 50.0, 0.0);
        assert!(v.invested_weights.is_empty());
        assert!(v.current_weights.is_empty());
        assert!((v.plan_value_usd - 50.0).abs() < 1e-9);
    }

    #[test]
    fn valuation_wallet_holdings_values_sellable_positions() {
        let marked = vec![("ETH".to_string(), 0.0, 600.0)];
        let v = derive_valuation(ValuationMode::WalletHoldings, &marked, 0.0, 400.0, 0.0);
        assert!((v.invested_weights["ETH"] - 1.0).abs() < 1e-9);
        assert!((v.current_weights["ETH"] - 0.6).abs() < 1e-9);
        assert!((v.plan_value_usd - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn valuation_wallet_holdings_counts_frozen_track_only_value_in_nav() {
        let marked = vec![("ETH".to_string(), 0.0, 600.0)];
        let v = derive_valuation(ValuationMode::WalletHoldings, &marked, 0.0, 400.0, 1000.0);
        assert!((v.invested_weights["ETH"] - 1.0).abs() < 1e-9);
        assert!((v.current_weights["ETH"] - 0.3).abs() < 1e-9);
        assert!((v.plan_value_usd - 2000.0).abs() < 1e-9);
    }

    #[test]
    fn valuation_allocation_book_uses_confirmed_value_not_stale_percent() {
        let marked = vec![
            ("ETH".to_string(), 100.0, 0.0),
            ("BTC".to_string(), 0.0, 600.0),
        ];
        let v = derive_valuation(ValuationMode::AllocationBook, &marked, 600.0, 0.0, 0.0);
        assert!((v.invested_weights["BTC"] - 1.0).abs() < 1e-9);
        assert!(v.invested_weights["ETH"].abs() < 1e-9);
        assert!((v.plan_value_usd - 600.0).abs() < 1e-9);
    }

    #[test]
    fn valuation_allocation_book_falls_back_to_stale_percent_when_no_confirmed_value() {
        let marked = vec![
            ("BTC".to_string(), 60.0, 0.0),
            ("ETH".to_string(), 40.0, 0.0),
        ];
        let v = derive_valuation(ValuationMode::AllocationBook, &marked, 1000.0, 0.0, 0.0);
        assert!((v.invested_weights["BTC"] - 0.6).abs() < 1e-9);
        assert!((v.invested_weights["ETH"] - 0.4).abs() < 1e-9);
    }

    #[test]
    fn valuation_dilutes_current_weights_with_idle_usdc() {
        let marked = vec![("BTC".to_string(), 0.0, 600.0)];
        let v = derive_valuation(ValuationMode::AllocationBook, &marked, 600.0, 400.0, 0.0);
        assert!((v.invested_weights["BTC"] - 1.0).abs() < 1e-9);
        assert!((v.current_weights["BTC"] - 0.6).abs() < 1e-9);
        assert!((v.plan_value_usd - 1000.0).abs() < 1e-9);
    }
}
