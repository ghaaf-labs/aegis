use serde::Deserialize;
use uuid::Uuid;

use super::models::{AgentDecision, AnalyzeRequest};
use crate::{modules::ai::OpenAiClient, router::AppState};

pub async fn analyze_portfolio(
    state: &AppState,
    req: AnalyzeRequest,
) -> crate::error::Result<AgentDecision> {
    // Fetch portfolio
    let portfolio = sqlx::query_as::<_, crate::modules::portfolio::models::Portfolio>(
        "SELECT * FROM portfolios WHERE id = $1",
    )
    .bind(req.portfolio_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| crate::error::AppError::NotFound(format!("portfolio {}", req.portfolio_id)))?;

    // Fetch allocations
    let allocations = sqlx::query_as::<_, crate::modules::portfolio::models::Allocation>(
        "SELECT * FROM allocations WHERE portfolio_id = $1",
    )
    .bind(req.portfolio_id)
    .fetch_all(&state.db)
    .await?;

    // Fetch market context
    let snapshot = crate::modules::market_data::service::fetch_snapshot(&state.http, &state.config)
        .await
        .map_err(anyhow::Error::from)?;

    // Run risk engine
    let risk = crate::modules::risk_engine::evaluate(&allocations, &snapshot.assets);

    let alloc_summary: Vec<String> = allocations
        .iter()
        .map(|a| {
            format!(
                "  {} {:.4} units (target: {:.1}%, current: {:.1}%, value: ${:.2})",
                a.asset_symbol, a.quantity, a.target_weight, a.current_weight, a.value_usd
            )
        })
        .collect();

    let prompt = format!(
        "You are an AI crypto portfolio manager. Analyze this portfolio and provide a rebalancing recommendation.

Portfolio: {}
Total Value: ${:.2}
Risk Score: {}/100 ({})

Current Allocations:
{}

Market Context:
- Fear & Greed Index: {}
- BTC Dominance: {:.1}%
- Total Market Cap: ${:.0}B

Respond with ONLY valid JSON in this exact structure:
{{
  \"reasoning\": \"<2-3 sentence analysis of the current state and key concern>\",
  \"confidence\": <float 0.0 to 1.0>,
  \"recommendation\": {{
    \"summary\": \"<one line action summary>\",
    \"trades\": [
      {{
        \"symbol\": \"BTC\",
        \"action\": \"buy or sell\",
        \"quantity\": 0.1,
        \"value_usd\": 6000.0,
        \"reason\": \"<one sentence>\"
      }}
    ],
    \"expected_impact\": {{
      \"risk_delta\": <integer, negative means risk decreases>,
      \"diversification_score\": <float 0.0 to 1.0>
    }}
  }}
}}

If no rebalancing is needed, return an empty trades array.",
        portfolio.name,
        portfolio.total_value_usd,
        risk.score,
        risk.summary,
        alloc_summary.join("\n"),
        snapshot.fear_greed_index,
        snapshot.btc_dominance,
        snapshot.total_market_cap_usd / 1e9,
    );

    let ai = OpenAiClient::new(&state.http, &state.config);
    let response = ai.chat(vec![
        crate::modules::ai::Message {
            role: "system".into(),
            content: "You are a professional crypto portfolio manager. Respond with valid JSON only, no markdown.".into(),
        },
        crate::modules::ai::Message {
            role: "user".into(),
            content: prompt,
        },
    ])
    .await
    .map_err(anyhow::Error::from)?;

    #[derive(Deserialize)]
    struct AiResponse {
        reasoning: String,
        confidence: f64,
        recommendation: serde_json::Value,
    }

    let parsed: AiResponse = serde_json::from_str(response.trim()).map_err(|e| {
        crate::error::AppError::Internal(anyhow::anyhow!(
            "failed to parse AI response: {e}\nraw: {response}"
        ))
    })?;

    let decision = sqlx::query_as::<_, AgentDecision>(
        "INSERT INTO agent_decisions (id, portfolio_id, reasoning, recommendation, confidence, triggered_by)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(req.portfolio_id)
    .bind(&parsed.reasoning)
    .bind(&parsed.recommendation)
    .bind(parsed.confidence)
    .bind("user_request")
    .fetch_one(&state.db)
    .await?;

    Ok(decision)
}
