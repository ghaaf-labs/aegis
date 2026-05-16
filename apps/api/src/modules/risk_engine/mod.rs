pub mod regime;
pub mod regime_backtest;

#[allow(unused_imports)]
pub use regime::{classify, compute_signals, MarketRegime, RegimeClassification, RegimeSignals};

use crate::modules::market_data::AssetPrice;
use crate::modules::portfolio::models::Allocation;

#[allow(dead_code)]
pub struct RiskReport {
    pub score: u8,
    pub concentration_risk: f64,
    pub volatility_score: f64,
    pub drift_score: f64,
    pub summary: String,
}

pub fn evaluate(allocations: &[Allocation], prices: &[AssetPrice]) -> RiskReport {
    let concentration = concentration_risk(allocations);
    let volatility = volatility_score(prices);
    let drift = drift_score(allocations);

    let raw = concentration * 0.4 + volatility * 0.35 + drift * 0.25;
    let score = (raw * 100.0).min(100.0) as u8;

    let summary = match score {
        0..=30 => "Portfolio risk is within conservative bounds.",
        31..=60 => "Moderate risk level. Monitor concentration and drift.",
        _ => "Elevated risk. Consider rebalancing to reduce exposure.",
    }
    .to_string();

    RiskReport {
        score,
        concentration_risk: concentration,
        volatility_score: volatility,
        drift_score: drift,
        summary,
    }
}

fn concentration_risk(allocations: &[Allocation]) -> f64 {
    if allocations.is_empty() {
        return 0.0;
    }
    let max_weight = allocations
        .iter()
        .map(|a| a.current_weight)
        .fold(0f64, f64::max);
    (max_weight / 100.0).min(1.0)
}

fn volatility_score(prices: &[AssetPrice]) -> f64 {
    if prices.is_empty() {
        return 0.5;
    }
    let avg_volatility: f64 =
        prices.iter().map(|p| p.change_24h.abs()).sum::<f64>() / prices.len() as f64;
    (avg_volatility / 20.0).min(1.0)
}

fn drift_score(allocations: &[Allocation]) -> f64 {
    if allocations.is_empty() {
        return 0.0;
    }
    let total_drift: f64 = allocations
        .iter()
        .map(|a| (a.current_weight - a.target_weight).abs())
        .sum();
    (total_drift / (allocations.len() as f64 * 20.0)).min(1.0)
}
