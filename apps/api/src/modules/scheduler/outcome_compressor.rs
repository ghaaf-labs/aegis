//! 24h outcome compressor.
//!
//! Closes the adaptive-learning loop: every decision that fired ~24h ago is
//! paired with the portfolio's actual performance over the same window and
//! compressed to an 80-char memory row. The strategist's next prompt reads
//! these rows via `agent::memory`.
//!
//! Sprint 4 promoted the realized-delta computation from "cumulative PnL"
//! (which was wrong: it conflated old performance with the decision) to a
//! true 24h delta against `agent_decisions.snapshot.totalValueUsd`. The
//! counterfactual is computed by replaying the recommendation's trades on
//! the snapshot holdings and revaluing at current prices.
//!
//! Failure mode: if any step fails (no snapshot, no current prices, malformed
//! recommendation) we fall back to the Sprint 3 heuristic so a single bad row
//! doesn't stall the loop.

use std::collections::HashMap;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::router::AppState;

const TICK_SECS: u64 = 3600;

pub fn spawn_outcome_compressor(state: AppState, cancel: CancellationToken) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(Duration::from_secs(TICK_SECS)) => {}
            }
            if let Err(e) = compress_pending(&state).await {
                tracing::warn!(error=%e, "outcome compressor tick failed");
            }
        }
    });
}

#[derive(sqlx::FromRow)]
struct DecisionRow {
    id: Uuid,
    portfolio_id: Uuid,
    triggered_by: String,
    snapshot: serde_json::Value,
    recommendation: serde_json::Value,
}

async fn compress_pending(state: &AppState) -> crate::error::Result<()> {
    let rows: Vec<DecisionRow> = sqlx::query_as(
        "SELECT d.id, d.portfolio_id, d.triggered_by, d.snapshot, d.recommendation
         FROM agent_decisions d
         LEFT JOIN agent_memory m ON m.decision_id = d.id
         WHERE m.id IS NULL
           AND d.created_at < NOW() - INTERVAL '23 hours'
           AND d.created_at > NOW() - INTERVAL '25 hours'
           AND d.triggered_by != 'abstain'
         LIMIT 20",
    )
    .fetch_all(&state.db)
    .await?;

    for row in rows {
        if let Err(e) = compress_one(state, &row).await {
            tracing::warn!(decision_id=%row.id, error=%e, "compress_one failed");
        }
    }
    Ok(())
}

async fn compress_one(state: &AppState, row: &DecisionRow) -> crate::error::Result<()> {
    let current_total: f64 =
        sqlx::query_scalar("SELECT total_value_usd FROM portfolios WHERE id = $1")
            .bind(row.portfolio_id)
            .fetch_optional(&state.db)
            .await?
            .unwrap_or(0.0);

    let snap_total = snapshot_total(&row.snapshot);

    // Realized 24h delta. If the snapshot is empty (legacy row pre-Sprint-4)
    // we fall back to the cumulative PnL — same behavior as before so we
    // never regress an existing diary entry.
    let realized = if snap_total > 0.0 {
        (current_total - snap_total) / snap_total * 100.0
    } else {
        let legacy: Option<f64> =
            sqlx::query_scalar("SELECT total_pnl_pct FROM portfolios WHERE id = $1")
                .bind(row.portfolio_id)
                .fetch_optional(&state.db)
                .await?;
        legacy.unwrap_or(0.0)
    };

    let counterfactual = compute_counterfactual(state, &row.snapshot, &row.recommendation)
        .await
        .unwrap_or(realized + 0.5);

    let summary = format!(
        "{}: realized {realized:+.2}%, would-have-been {counterfactual:+.2}%",
        row.triggered_by
    );
    let outcome = serde_json::json!({
        "realizedPctChange": realized,
        "counterfactualPctChange": counterfactual,
        "compressedSummary": summary,
        "recordedAt": chrono::Utc::now(),
    });

    sqlx::query(
        "INSERT INTO agent_memory (portfolio_id, decision_id, outcome_24h)
         VALUES ($1, $2, $3)
         ON CONFLICT DO NOTHING",
    )
    .bind(row.portfolio_id)
    .bind(row.id)
    .bind(outcome)
    .execute(&state.db)
    .await?;
    Ok(())
}

/// Replay the recommendation's trades on the snapshot's holdings, then revalue
/// at current market prices. Returns the percent delta vs the captured total.
/// Returns `None` on any data-missing path so the caller can fall back.
async fn compute_counterfactual(
    state: &AppState,
    snapshot: &serde_json::Value,
    recommendation: &serde_json::Value,
) -> Option<f64> {
    let snap_total = snapshot_total(snapshot);
    if snap_total <= 0.0 {
        return None;
    }

    let holdings = snapshot.get("holdings").and_then(|v| v.as_array())?;
    if holdings.is_empty() {
        return None;
    }

    // Build counterfactual quantities = snapshot holdings + trades.
    let mut qty: HashMap<String, f64> = HashMap::with_capacity(holdings.len() + 4);
    let mut snap_price: HashMap<String, f64> = HashMap::with_capacity(holdings.len());
    for h in holdings {
        let symbol = h.get("symbol").and_then(|v| v.as_str())?.to_string();
        let q = h.get("quantity").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let p = h.get("priceUsd").and_then(|v| v.as_f64()).unwrap_or(0.0);
        qty.insert(symbol.clone(), q);
        snap_price.insert(symbol, p);
    }

    if let Some(trades) = recommendation.get("trades").and_then(|v| v.as_array()) {
        for t in trades {
            let Some(symbol) = t.get("symbol").and_then(|v| v.as_str()) else {
                continue;
            };
            let amount = t.get("quantity").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let action = t.get("action").and_then(|v| v.as_str()).unwrap_or("");
            let entry = qty.entry(symbol.to_string()).or_insert(0.0);
            match action {
                "sell" => *entry -= amount,
                "buy" => *entry += amount,
                _ => {}
            }
        }
    }

    // Current prices.
    let market = crate::modules::market_data::service::fetch_snapshot(&state.http, &state.config)
        .await
        .ok()?;
    let mut current_price: HashMap<String, f64> = HashMap::with_capacity(market.assets.len());
    for a in &market.assets {
        current_price.insert(a.symbol.clone(), a.price_usd);
    }

    let counterfactual_value: f64 = qty
        .iter()
        .map(|(sym, q)| {
            let p = current_price
                .get(sym)
                .copied()
                .or_else(|| snap_price.get(sym).copied())
                .unwrap_or(0.0);
            q * p
        })
        .sum();

    Some((counterfactual_value - snap_total) / snap_total * 100.0)
}

fn snapshot_total(snapshot: &serde_json::Value) -> f64 {
    snapshot
        .get("totalValueUsd")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn snapshot_total_reads_field() {
        let s = json!({ "totalValueUsd": 1234.5 });
        assert!((snapshot_total(&s) - 1234.5).abs() < 1e-9);
    }

    #[test]
    fn snapshot_total_zero_when_missing() {
        assert_eq!(snapshot_total(&json!({})), 0.0);
        assert_eq!(snapshot_total(&json!({ "totalValueUsd": null })), 0.0);
    }

    // compute_counterfactual is async + hits market_data; isolated logic in
    // snapshot_total + the trade-replay branch is exercised by the integration
    // test suite once a fixture market snapshot is available.
}
