//! Backtest service.
//!
//! `run_backtest` takes the current allocation, the proposed allocation,
//! and a daily price series; returns side-by-side return / Sharpe / max-DD
//! metrics for the window. Pure function — no DB, no time. The handler
//! loads inputs from Postgres and calls this.

use std::collections::HashMap;

use serde::Serialize;

/// One day's prices keyed by asset symbol. Missing symbols are excluded
/// from the day's portfolio revaluation rather than treated as zero — a
/// missing data point shouldn't blow up the entire metric.
pub type DayPrices = HashMap<String, f64>;

/// Weighted exposure to a symbol. Weights are fractions (0..1) and must sum
/// to ≤1. Cash residual is held flat (0% return).
#[derive(Debug, Clone)]
pub struct WeightedExposure {
    pub symbol: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegMetrics {
    pub total_return_pct: f64,
    pub sharpe: f64,
    pub max_drawdown_pct: f64,
    /// Number of daily observations used. Below 5 the metrics are unreliable;
    /// the handler flags it for the UI.
    pub observations: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestResult {
    pub current: LegMetrics,
    pub proposed: LegMetrics,
    /// `proposed.total_return_pct - current.total_return_pct`. Positive means
    /// the recommendation would have outperformed.
    pub delta_total_return_pct: f64,
    pub window_days: i32,
}

/// Pure-function backtest.
///
/// * `series` — daily price snapshots, chronologically ordered (oldest first).
/// * `current` / `proposed` — weighted exposures (fractions, sum ≤ 1).
pub fn run_backtest(
    series: &[DayPrices],
    current: &[WeightedExposure],
    proposed: &[WeightedExposure],
) -> BacktestResult {
    let current_returns = simulate(series, current);
    let proposed_returns = simulate(series, proposed);

    let cur = metrics(&current_returns);
    let prop = metrics(&proposed_returns);

    BacktestResult {
        delta_total_return_pct: prop.total_return_pct - cur.total_return_pct,
        current: cur,
        proposed: prop,
        window_days: series.len().saturating_sub(1).min(i32::MAX as usize) as i32,
    }
}

/// Buy-and-hold portfolio return per day. Index 0 is the first day's return
/// vs day 0's price — i.e. an empty series returns empty.
fn simulate(series: &[DayPrices], weights: &[WeightedExposure]) -> Vec<f64> {
    if series.len() < 2 {
        return Vec::new();
    }
    let mut daily: Vec<f64> = Vec::with_capacity(series.len() - 1);
    for window in series.windows(2) {
        let prev = &window[0];
        let next = &window[1];
        let mut weighted_return = 0.0;
        for w in weights {
            let p0 = prev.get(&w.symbol).copied();
            let p1 = next.get(&w.symbol).copied();
            if let (Some(p0), Some(p1)) = (p0, p1) {
                if p0 > 0.0 {
                    weighted_return += w.weight * ((p1 - p0) / p0);
                }
            }
        }
        daily.push(weighted_return);
    }
    daily
}

fn metrics(daily_returns: &[f64]) -> LegMetrics {
    if daily_returns.is_empty() {
        return LegMetrics {
            total_return_pct: 0.0,
            sharpe: 0.0,
            max_drawdown_pct: 0.0,
            observations: 0,
        };
    }
    // Cumulative compounded return.
    let cum: f64 = daily_returns.iter().fold(1.0, |acc, r| acc * (1.0 + r));
    let total_return_pct = (cum - 1.0) * 100.0;

    // Annualized Sharpe — daily mean / daily stddev * sqrt(365). Zero rf rate
    // is fine for relative comparison of two strategies on the same window.
    let n = daily_returns.len() as f64;
    let mean = daily_returns.iter().sum::<f64>() / n;
    let variance = daily_returns
        .iter()
        .map(|r| (r - mean).powi(2))
        .sum::<f64>()
        / n.max(1.0);
    let stddev = variance.sqrt();
    let sharpe = if stddev > 0.0 {
        mean / stddev * (365.0_f64).sqrt()
    } else {
        0.0
    };

    // Max drawdown: walk the equity curve, track running peak, compare.
    let mut equity = 1.0;
    let mut peak = 1.0;
    let mut max_dd = 0.0;
    for r in daily_returns {
        equity *= 1.0 + r;
        if equity > peak {
            peak = equity;
        }
        let dd = (peak - equity) / peak;
        if dd > max_dd {
            max_dd = dd;
        }
    }

    LegMetrics {
        total_return_pct: round_2(total_return_pct),
        sharpe: round_2(sharpe),
        max_drawdown_pct: round_2(max_dd * 100.0),
        observations: daily_returns.len(),
    }
}

fn round_2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(pairs: &[(&str, f64)]) -> DayPrices {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn w(symbol: &str, weight: f64) -> WeightedExposure {
        WeightedExposure {
            symbol: symbol.into(),
            weight,
        }
    }

    #[test]
    fn empty_series_returns_zero_metrics() {
        let r = run_backtest(&[], &[w("BTC", 1.0)], &[w("BTC", 1.0)]);
        assert_eq!(r.current.observations, 0);
        assert_eq!(r.proposed.observations, 0);
        assert!((r.delta_total_return_pct).abs() < 1e-9);
    }

    #[test]
    fn single_asset_buy_and_hold_matches_price_move() {
        // BTC goes 100 → 110 → 121 — total +21%.
        let series = vec![
            day(&[("BTC", 100.0)]),
            day(&[("BTC", 110.0)]),
            day(&[("BTC", 121.0)]),
        ];
        let r = run_backtest(&series, &[w("BTC", 1.0)], &[w("BTC", 1.0)]);
        assert!((r.current.total_return_pct - 21.0).abs() < 0.5);
        assert_eq!(r.current.observations, 2);
    }

    #[test]
    fn proposed_outperforms_when_overweighting_riser() {
        // BTC +20%, ETH 0. Current 50/50, proposed 100/0.
        let series = vec![
            day(&[("BTC", 100.0), ("ETH", 100.0)]),
            day(&[("BTC", 120.0), ("ETH", 100.0)]),
        ];
        let r = run_backtest(
            &series,
            &[w("BTC", 0.5), w("ETH", 0.5)],
            &[w("BTC", 1.0), w("ETH", 0.0)],
        );
        assert!(r.delta_total_return_pct > 0.0);
        // Proposed = 20%, current = 10%. Delta ≈ 10.
        assert!((r.delta_total_return_pct - 10.0).abs() < 0.5);
    }

    #[test]
    fn max_drawdown_captures_peak_to_trough() {
        // 100 → 120 → 60 → 80. Peak 120, trough 60. DD = (120-60)/120 = 50%.
        let series = vec![
            day(&[("BTC", 100.0)]),
            day(&[("BTC", 120.0)]),
            day(&[("BTC", 60.0)]),
            day(&[("BTC", 80.0)]),
        ];
        let r = run_backtest(&series, &[w("BTC", 1.0)], &[w("BTC", 1.0)]);
        assert!(r.current.max_drawdown_pct > 49.0 && r.current.max_drawdown_pct < 51.0);
    }

    #[test]
    fn missing_prices_are_skipped_not_zeroed() {
        // ETH price only on the second day — shouldn't crash; weight effectively unused.
        let series = vec![
            day(&[("BTC", 100.0)]),
            day(&[("BTC", 105.0), ("ETH", 50.0)]),
        ];
        let r = run_backtest(
            &series,
            &[w("BTC", 0.5), w("ETH", 0.5)],
            &[w("BTC", 0.5), w("ETH", 0.5)],
        );
        // Only BTC contributed (50% * 5%) ≈ 2.5%.
        assert!((r.current.total_return_pct - 2.5).abs() < 0.1);
    }
}
