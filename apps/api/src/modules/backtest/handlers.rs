//! Backtest preview HTTP handler.
//!
//! `POST /backtest/preview` — body: `{ portfolioId, proposed: [{symbol, targetWeight}] }`.
//! Returns side-by-side metrics for current vs proposed allocation over the
//! last 30 daily `market_snapshots`. The UI shows it inline on the approval
//! modal so the user has a numeric sanity check before clicking Approve.

use std::collections::HashMap;

use axum::{
    extract::{Json as ExtractJson, State},
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::service::{run_backtest, BacktestResult, DayPrices, WeightedExposure};
use crate::error::{AppError, Result};
use crate::middleware::auth::Claims;
use crate::router::AppState;

const WINDOW_DAYS: i32 = 30;
/// Daily samples needed before the result is trustworthy.
const MIN_OBSERVATIONS: usize = 5;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestRequest {
    pub portfolio_id: Uuid,
    /// Optional override. When omitted, the handler uses the portfolio's
    /// stored `target_weight` (the goal-wizard targets the agent is moving
    /// toward) — useful for showing the "default" backtest without the
    /// caller pre-computing trades.
    #[serde(default)]
    pub proposed: Option<Vec<ProposedWeight>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedWeight {
    pub symbol: String,
    /// Percent in the 0–100 scale (matches goal-wizard and allocation table).
    pub target_weight: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestResponse {
    #[serde(flatten)]
    pub result: BacktestResult,
    /// `true` once `observations >= MIN_OBSERVATIONS`; the UI shows a soft
    /// warning otherwise.
    pub reliable: bool,
}

pub async fn preview(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    ExtractJson(body): ExtractJson<BacktestRequest>,
) -> Result<Json<BacktestResponse>> {
    // 1. Authorize the portfolio belongs to the caller.
    let owns: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM portfolios WHERE id = $1 AND user_id = $2")
            .bind(body.portfolio_id)
            .bind(claims.sub)
            .fetch_optional(&state.db)
            .await?;
    if owns.is_none() {
        return Err(AppError::NotFound(format!(
            "portfolio {}",
            body.portfolio_id
        )));
    }

    // 2. Current + proposed allocations.
    let (current, target) = load_weights(&state, body.portfolio_id).await?;
    let proposed: Vec<WeightedExposure> = match body.proposed {
        Some(rows) => rows
            .into_iter()
            .map(|p| WeightedExposure {
                symbol: p.symbol,
                weight: (p.target_weight / 100.0).clamp(0.0, 1.0),
            })
            .collect(),
        None => target,
    };

    // 3. Daily price series from market_snapshots.
    let series = load_daily_prices(&state, WINDOW_DAYS).await?;

    let result = run_backtest(&series, &current, &proposed);
    let reliable = result.current.observations >= MIN_OBSERVATIONS;

    Ok(Json(BacktestResponse { result, reliable }))
}

async fn load_weights(
    state: &AppState,
    portfolio_id: Uuid,
) -> Result<(Vec<WeightedExposure>, Vec<WeightedExposure>)> {
    let rows: Vec<(String, f64, f64)> = sqlx::query_as(
        "SELECT asset_symbol, current_weight, target_weight
         FROM allocations WHERE portfolio_id = $1",
    )
    .bind(portfolio_id)
    .fetch_all(&state.db)
    .await?;
    let mut current = Vec::with_capacity(rows.len());
    let mut target = Vec::with_capacity(rows.len());
    for (symbol, cur_pct, tgt_pct) in rows {
        current.push(WeightedExposure {
            symbol: symbol.clone(),
            weight: (cur_pct / 100.0).clamp(0.0, 1.0),
        });
        target.push(WeightedExposure {
            symbol,
            weight: (tgt_pct / 100.0).clamp(0.0, 1.0),
        });
    }
    Ok((current, target))
}

async fn load_daily_prices(state: &AppState, window_days: i32) -> Result<Vec<DayPrices>> {
    // One row per snapshot; we de-dupe to the latest snapshot per UTC date
    // so a noisy ticker doesn't oversample any single day.
    let rows: Vec<(serde_json::Value, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT DISTINCT ON (DATE(captured_at))
                assets, captured_at
         FROM market_snapshots
         WHERE captured_at > NOW() - ($1::int || ' days')::interval
         ORDER BY DATE(captured_at) ASC, captured_at DESC",
    )
    .bind(window_days)
    .fetch_all(&state.db)
    .await?;

    let mut series: Vec<DayPrices> = Vec::with_capacity(rows.len());
    for (assets, _at) in rows {
        let mut day: HashMap<String, f64> = HashMap::new();
        if let Some(arr) = assets.as_array() {
            for a in arr {
                let Some(symbol) = a.get("symbol").and_then(|v| v.as_str()) else {
                    continue;
                };
                let Some(price) = a.get("price_usd").and_then(|v| v.as_f64()) else {
                    continue;
                };
                day.insert(symbol.to_string(), price);
            }
        }
        // Stablecoins aren't in market_snapshots; pin them to $1 so the
        // backtest treats USDC/EURC/USYC weight as cash-flat.
        for stable in ["USDC", "EURC", "USYC"] {
            day.entry(stable.to_string()).or_insert(1.0);
        }
        series.push(day);
    }
    Ok(series)
}
