//! Market regime classifier — hybrid statistical + LLM.
//!
//! Three features are computed deterministically from the current market
//! snapshot (BTC realized vol proxy, cross-asset directional correlation,
//! max drawdown). The features are then handed to a fast LLM via
//! `ModelRoute::RegimeClassify` to produce the final `RiskOn`/`Neutral`/
//! `RiskOff` label with a confidence.
//!
//! The classifier is intentionally simple for Sprint 1: features approximate
//! 30d / 90d windows using the snapshot's 24h and 7d change data. The
//! `docs/05-open-questions.md` file documents the limitations.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::ModelRoute;
use crate::modules::ai::{Message, OpenRouterClient, PromptKey};
use crate::modules::market_data::MarketSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketRegime {
    RiskOn,
    Neutral,
    RiskOff,
}

impl MarketRegime {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RiskOn => "risk_on",
            Self::Neutral => "neutral",
            Self::RiskOff => "risk_off",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RegimeSignals {
    pub btc_vol_30d: f64,
    pub corr_90d: f64,
    pub max_drawdown: f64,
    pub fear_greed: u8,
    pub btc_dominance: f64,
}

#[derive(Debug, Clone)]
pub struct RegimeClassification {
    pub regime: MarketRegime,
    pub confidence: f32,
    pub signals: RegimeSignals,
    /// One-sentence rationale from the LLM; surfaced in tracing logs and
    /// persisted via the SSE `regime.flip` event for the UI tooltip.
    #[allow(dead_code)]
    pub rationale: String,
}

/// Classify the current regime from a market snapshot.
///
/// The `prompt_template` argument is the rendered `regime.md` template — the
/// caller passes it in so we don't depend on the registry directly here
/// (keeps this module test-friendly).
pub async fn classify(
    ai: &OpenRouterClient<'_>,
    snapshot: &MarketSnapshot,
    prompts: &crate::modules::ai::PromptRegistry,
) -> anyhow::Result<RegimeClassification> {
    let signals = compute_signals(snapshot);

    let features = json!({
        "btc_vol_30d": signals.btc_vol_30d,
        "corr_90d": signals.corr_90d,
        "max_drawdown": signals.max_drawdown,
        "fear_greed": signals.fear_greed,
        "btc_dominance": signals.btc_dominance,
    });

    let mut ctx = HashMap::new();
    ctx.insert("features_json", serde_json::to_string_pretty(&features)?);
    let prompt = prompts.render(PromptKey::Regime, &ctx);

    let response = ai
        .chat(
            ModelRoute::RegimeClassify,
            vec![
                Message::system(prompt),
                Message::user("Label the regime.".to_string()),
            ],
        )
        .await?;

    let parsed = parse_label(&response.content)?;

    Ok(RegimeClassification {
        regime: parsed.regime,
        confidence: parsed.confidence.clamp(0.0, 1.0),
        signals,
        rationale: parsed.rationale,
    })
}

#[derive(Deserialize)]
struct ModelLabel {
    regime: MarketRegime,
    #[serde(default)]
    confidence: f32,
    #[serde(default)]
    rationale: String,
}

fn parse_label(raw: &str) -> anyhow::Result<ModelLabel> {
    // Reasoning models (Qwen, DeepSeek) often emit prose preamble then a
    // fenced JSON block. The shared helper handles all three shapes.
    let stripped = crate::modules::ai::strip_json_fences(raw);
    serde_json::from_str(stripped)
        .map_err(|e| anyhow::anyhow!("regime classifier: invalid JSON ({e}): {raw}"))
}

/// Compute statistical regime features from the latest snapshot (fast path).
///
/// This is a lightweight approximation used when we don't have enough
/// historical price data yet. Once `price_history` has sufficient ticks,
/// callers should prefer `compute_real_signals_from_history`.
pub fn compute_signals(snapshot: &MarketSnapshot) -> RegimeSignals {
    let btc_change = snapshot
        .assets
        .iter()
        .find(|a| a.symbol == "BTC")
        .map(|a| a.change_24h.abs())
        .unwrap_or(0.0);

    // Annualized vol proxy using 24h change (still useful as fallback).
    let btc_vol_30d = (btc_change.abs() / 100.0) * (365f64.sqrt());

    let corr_90d = directional_agreement(snapshot);

    let max_drawdown = snapshot
        .assets
        .iter()
        .map(|a| a.change_24h.min(0.0).abs() / 100.0)
        .fold(0f64, f64::max);

    RegimeSignals {
        btc_vol_30d,
        corr_90d,
        max_drawdown,
        fear_greed: snapshot.fear_greed_index,
        btc_dominance: snapshot.btc_dominance,
    }
}

/// Computes higher-quality regime signals directly from `price_history`.
///
/// This is the real implementation for Phase 1+:
/// - 30d realized volatility for BTC (annualized std dev of daily log returns)
/// - 90d average pairwise correlation across major assets
///
/// Falls back to snapshot-based approximation if not enough history exists yet.
pub async fn compute_real_signals_from_history(
    db: &crate::db::Db,
    snapshot: &MarketSnapshot,
) -> RegimeSignals {
    // Try to get real 30d BTC realized vol
    let btc_vol_30d = compute_realized_vol(db, "BTC", 30).await.unwrap_or_else(|| {
        // Fallback to snapshot proxy
        let btc_change = snapshot
            .assets
            .iter()
            .find(|a| a.symbol == "BTC")
            .map(|a| a.change_24h.abs())
            .unwrap_or(0.0);
        (btc_change.abs() / 100.0) * (365f64.sqrt())
    });

    // Try to get real 90d average correlation (BTC-ETH, BTC-SOL, ETH-SOL, etc.)
    let corr_90d = compute_average_correlation(db, 90).await.unwrap_or_else(|| {
        directional_agreement(snapshot)
    });

    let max_drawdown = snapshot
        .assets
        .iter()
        .map(|a| a.change_24h.min(0.0).abs() / 100.0)
        .fold(0f64, f64::max);

    RegimeSignals {
        btc_vol_30d,
        corr_90d,
        max_drawdown,
        fear_greed: snapshot.fear_greed_index,
        btc_dominance: snapshot.btc_dominance,
    }
}

/// Annualized 30-day realized volatility from log returns in price_history.
async fn compute_realized_vol(db: &crate::db::Db, symbol: &str, days: i32) -> Option<f64> {
    let prices: Vec<f64> = sqlx::query_scalar(
        r#"
        SELECT price_usd
        FROM price_history
        WHERE symbol = $1
          AND fetched_at > NOW() - ($2::int || ' days')::interval
        ORDER BY fetched_at ASC
        "#,
    )
    .bind(symbol)
    .bind(days)
    .fetch_all(db)
    .await
    .ok()?;

    if prices.len() < 10 {
        return None;
    }

    // Compute log returns
    let returns: Vec<f64> = prices
        .windows(2)
        .map(|w| (w[1] / w[0]).ln())
        .collect();

    let n = returns.len() as f64;
    let mean = returns.iter().sum::<f64>() / n;
    let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let daily_std = variance.sqrt();

    // Annualize (assuming ~365 trading days for crypto)
    Some(daily_std * (365.0_f64).sqrt())
}

/// Average Pearson correlation across major pairs over the window.
async fn compute_average_correlation(db: &crate::db::Db, days: i32) -> Option<f64> {
    let pairs = [("BTC", "ETH"), ("BTC", "SOL"), ("ETH", "SOL"), ("BTC", "AVAX")];
    let mut corrs = Vec::new();

    for (a, b) in pairs {
        if let Some(r) = fetch_pair_correlation(db, a, b, days).await {
            corrs.push(r);
        }
    }

    if corrs.is_empty() {
        None
    } else {
        Some(corrs.iter().sum::<f64>() / corrs.len() as f64)
    }
}

async fn fetch_pair_correlation(db: &crate::db::Db, a: &str, b: &str, days: i32) -> Option<f64> {
    let prices_a: Vec<f64> = sqlx::query_scalar(
        "SELECT price_usd FROM price_history WHERE symbol = $1 AND fetched_at > NOW() - ($3::int || ' days')::interval ORDER BY fetched_at",
    )
    .bind(a)
    .bind(b)
    .bind(days)
    .fetch_all(db)
    .await
    .ok()?;

    let prices_b: Vec<f64> = sqlx::query_scalar(
        "SELECT price_usd FROM price_history WHERE symbol = $2 AND fetched_at > NOW() - ($3::int || ' days')::interval ORDER BY fetched_at",
    )
    .bind(a)
    .bind(b)
    .bind(days)
    .fetch_all(db)
    .await
    .ok()?;

    if prices_a.len() < 10 || prices_b.len() < 10 {
        return None;
    }

    let rets_a: Vec<f64> = prices_a.windows(2).map(|w| (w[1]/w[0]).ln()).collect();
    let rets_b: Vec<f64> = prices_b.windows(2).map(|w| (w[1]/w[0]).ln()).collect();

    crate::modules::agent::tools::correlation::pearson(&rets_a, &rets_b)
}

fn directional_agreement(snapshot: &MarketSnapshot) -> f64 {
    if snapshot.assets.is_empty() {
        return 0.0;
    }
    let total = snapshot.assets.len() as f64;
    let up = snapshot
        .assets
        .iter()
        .filter(|a| a.change_24h > 0.0)
        .count() as f64;
    let down = total - up;
    // Range [0, 1]: 0 when half up / half down, 1 when all one direction.
    ((up - down).abs()) / total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::market_data::AssetPrice;
    use chrono::Utc;

    fn snap(changes: &[(&str, f64)]) -> MarketSnapshot {
        MarketSnapshot {
            assets: changes
                .iter()
                .map(|(sym, ch)| AssetPrice {
                    symbol: (*sym).into(),
                    price_usd: 100.0,
                    change_24h: *ch,
                    change_7d: 0.0,
                    market_cap: 1_000_000.0,
                    volume_24h: 0.0,
                    updated_at: Utc::now(),
                })
                .collect(),
            fear_greed_index: 50,
            total_market_cap_usd: 0.0,
            btc_dominance: 50.0,
            captured_at: Utc::now(),
        }
    }

    #[test]
    fn directional_agreement_all_up_is_one() {
        let s = snap(&[("BTC", 1.0), ("ETH", 2.0), ("SOL", 0.5)]);
        assert!((directional_agreement(&s) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn directional_agreement_balanced_is_zero() {
        let s = snap(&[("BTC", 1.0), ("ETH", -1.0)]);
        assert!(directional_agreement(&s).abs() < 1e-9);
    }

    #[test]
    fn compute_signals_extracts_max_drawdown() {
        let s = snap(&[("BTC", -8.0), ("ETH", -3.0), ("SOL", 1.0)]);
        let sig = compute_signals(&s);
        assert!((sig.max_drawdown - 0.08).abs() < 1e-9);
    }

    #[test]
    fn parse_label_handles_fenced_json() {
        let raw = "```json\n{\"regime\":\"risk_off\",\"confidence\":0.8,\"rationale\":\"x\"}\n```";
        let p = parse_label(raw).unwrap();
        assert_eq!(p.regime, MarketRegime::RiskOff);
        assert!((p.confidence - 0.8).abs() < 1e-6);
    }

    #[test]
    fn parse_label_handles_plain_json() {
        let raw = r#"{"regime":"risk_on","confidence":0.5,"rationale":"y"}"#;
        let p = parse_label(raw).unwrap();
        assert_eq!(p.regime, MarketRegime::RiskOn);
    }

    #[test]
    fn parse_label_rejects_bad_label() {
        let raw = r#"{"regime":"unknown","confidence":0.5}"#;
        assert!(parse_label(raw).is_err());
    }
}
