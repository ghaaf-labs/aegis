//! Per-user agent memory — read last N decisions + 24h outcomes and
//! compress each to a single line for the strategist prompt's
//! `{{ memory }}` placeholder.

use uuid::Uuid;

use crate::db::Db;

const MEMORY_LIMIT: i64 = 5;
const LINE_CHAR_BUDGET: usize = 100;

/// Compose the `{{ memory }}` block for a portfolio. Empty portfolios get
/// a single placeholder line so the prompt template doesn't read awkwardly.
pub async fn build_memory_block(db: &Db, portfolio_id: Uuid) -> crate::error::Result<String> {
    let rows: Vec<MemoryRow> = sqlx::query_as::<_, MemoryRow>(
        r#"
        SELECT d.created_at::text AS at,
               d.regime,
               d.confidence,
               d.triggered_by,
               d.recommendation,
               m.outcome_24h
        FROM agent_decisions d
        LEFT JOIN agent_memory m ON m.decision_id = d.id
        WHERE d.portfolio_id = $1
        ORDER BY d.created_at DESC
        LIMIT $2
        "#,
    )
    .bind(portfolio_id)
    .bind(MEMORY_LIMIT)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        return Ok("- (no prior decisions yet — this is the first one)".into());
    }

    let mut lines: Vec<String> = Vec::with_capacity(rows.len());
    for row in rows {
        lines.push(format!("- {}", compress(&row)));
    }
    Ok(lines.join("\n"))
}

#[derive(sqlx::FromRow)]
struct MemoryRow {
    at: String,
    regime: Option<String>,
    confidence: f64,
    triggered_by: String,
    recommendation: serde_json::Value,
    outcome_24h: Option<serde_json::Value>,
}

fn compress(row: &MemoryRow) -> String {
    let summary = row
        .recommendation
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("(no summary)");
    let regime = row.regime.as_deref().unwrap_or("?");
    let outcome = row
        .outcome_24h
        .as_ref()
        .and_then(|v| v.get("pnlPct"))
        .and_then(|v| v.as_f64())
        .map(|p| format!(" → 24h {:+.2}%", p))
        .unwrap_or_default();

    let date = row.at.split('T').next().unwrap_or(&row.at);
    let mut line = format!(
        "{date} {regime} {:.0}% [{}] {summary}{outcome}",
        row.confidence * 100.0,
        row.triggered_by
    );
    if line.len() > LINE_CHAR_BUDGET {
        let end = line
            .char_indices()
            .map(|(idx, _)| idx)
            .take_while(|idx| *idx <= LINE_CHAR_BUDGET)
            .last()
            .unwrap_or(0);
        line.truncate(end);
        line.push('…');
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compress_includes_summary_and_regime() {
        let row = MemoryRow {
            at: "2026-05-14T12:00:00Z".into(),
            regime: Some("risk_off".into()),
            confidence: 0.72,
            triggered_by: "user_request".into(),
            recommendation: json!({"summary": "Trim BTC into USYC"}),
            outcome_24h: Some(json!({"pnlPct": 0.45})),
        };
        let line = compress(&row);
        assert!(line.contains("2026-05-14"));
        assert!(line.contains("risk_off"));
        assert!(line.contains("72%"));
        assert!(line.contains("Trim BTC into USYC"));
        assert!(line.contains("+0.45"));
    }

    #[test]
    fn compress_truncates_long_lines() {
        let row = MemoryRow {
            at: "2026-05-14T12:00:00Z".into(),
            regime: Some("neutral".into()),
            confidence: 0.5,
            triggered_by: "scheduled".into(),
            recommendation: json!({ "summary": "x".repeat(200) }),
            outcome_24h: None,
        };
        let line = compress(&row);
        assert!(line.len() <= LINE_CHAR_BUDGET + 4); // include ellipsis
    }
}
