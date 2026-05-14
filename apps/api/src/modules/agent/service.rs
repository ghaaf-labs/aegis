//! Agent service — orchestrates regime → strategist → critic → revision.
//!
//! Every step is observable: regime is classified before reasoning, the
//! strategist proposal is critiqued, and (if the critic objects) the
//! strategist revises once. The final decision is persisted with full
//! telemetry (model slug, token usage, latency, critic verdict) and pushed
//! over SSE as `agent.decision`.
//!
//! Trade specificity-to-portfolio is enforced by the prompt template — every
//! field in the strategist prompt is portfolio-specific (goal, allocations,
//! PnL, risk tolerance, horizon). The agent is instructed to cite the user's
//! symbols, never generic crypto trades.

use std::collections::HashMap;
use std::time::Instant;

use serde::Deserialize;
use serde_json::json;
use tracing::{debug, warn};
use uuid::Uuid;

use super::models::{AgentDecision, AnalyzeRequest};
use crate::config::ModelRoute;
use crate::modules::ai::{Message, OpenRouterClient, PromptKey};
use crate::modules::market_data::MarketSnapshot;
use crate::modules::portfolio::models::{Allocation, Portfolio};
use crate::modules::risk_engine::{self, RegimeClassification};
use crate::modules::sse::{
    AgentDecisionPayload, RegimeFlip, RegimeSignals as SseRegimeSignals, SseEvent,
};
use crate::router::AppState;

const ABSTAIN_CONFIDENCE_THRESHOLD: f64 = 0.5;
/// Wall-clock budget for a single strategist call. Logged but not enforced;
/// keeps the agent honest during demo while we tune model selection.
const STRATEGIST_SLOW_MS: u64 = 10_000;

pub async fn analyze_portfolio(
    state: &AppState,
    req: AnalyzeRequest,
) -> crate::error::Result<AgentDecision> {
    let start = Instant::now();
    let triggered_by = req
        .triggered_by
        .clone()
        .unwrap_or_else(|| "user_request".to_string());

    // 1. Fetch portfolio + allocations + user (for risk tolerance + horizon).
    let portfolio: Portfolio = sqlx::query_as("SELECT * FROM portfolios WHERE id = $1")
        .bind(req.portfolio_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| {
            crate::error::AppError::NotFound(format!("portfolio {}", req.portfolio_id))
        })?;

    let allocations: Vec<Allocation> = sqlx::query_as(
        "SELECT * FROM allocations WHERE portfolio_id = $1 ORDER BY current_weight DESC",
    )
    .bind(req.portfolio_id)
    .fetch_all(&state.db)
    .await?;

    let user_profile = fetch_user_profile(state, portfolio.user_id).await?;

    let snapshot =
        crate::modules::market_data::service::fetch_snapshot(&state.http, &state.config).await?;

    let ai = OpenRouterClient::new(&state.http, &state.config);

    // 2. Regime classifier — cheap pass that conditions the strategist.
    let regime = risk_engine::classify(&ai, &snapshot, state.prompts.as_ref()).await?;

    // Broadcast the regime read immediately so the UI can react before the
    // strategist call completes (sub-second feedback even when Opus is slow).
    let _ = state.sse.send(SseEvent::RegimeFlip(RegimeFlip {
        from: previous_regime(state, req.portfolio_id).await,
        to: regime.regime.as_str().to_string(),
        confidence: regime.confidence,
        signals: SseRegimeSignals {
            btc_vol_30d: regime.signals.btc_vol_30d,
            corr_90d: regime.signals.corr_90d,
            max_drawdown: regime.signals.max_drawdown,
        },
        classified_at: chrono::Utc::now(),
    }));

    // 3. Risk engine — concentration + vol + drift; orthogonal to regime.
    let risk = risk_engine::evaluate(&allocations, &snapshot.assets);

    // 4. Strategist proposal.
    let strategist_ctx = build_strategist_context(
        &portfolio,
        &allocations,
        &user_profile,
        &snapshot,
        &regime,
        &risk,
    );
    let strategist_prompt = state.prompts.render(PromptKey::Strategist, &strategist_ctx);
    let strategist = ai
        .chat(
            ModelRoute::RebalanceReason,
            vec![
                Message::system(strategist_prompt),
                Message::user("Propose a rebalance, or recommend hold."),
            ],
        )
        .await?;
    if strategist.was_slow(STRATEGIST_SLOW_MS) {
        warn!(
            "strategist call slow: {}ms (budget {STRATEGIST_SLOW_MS}ms) model={}",
            strategist.latency_ms, strategist.model_slug
        );
    }
    let mut proposal = parse_proposal(&strategist.content)?;
    let mut prompt_tokens = strategist.prompt_tokens;
    let mut completion_tokens = strategist.completion_tokens;

    // 5. Critic pass — adversarial review.
    let critic_ctx = build_critic_context(&proposal, &allocations, &user_profile, &regime, &risk);
    let critic_prompt = state.prompts.render(PromptKey::Critic, &critic_ctx);
    let critic = ai
        .chat(
            ModelRoute::CritiqueAgent,
            vec![
                Message::system(critic_prompt),
                Message::user("Render verdict.".to_string()),
            ],
        )
        .await?;
    let verdict = parse_critic(&critic.content).unwrap_or_else(|e| {
        warn!("critic parse failed, treating as approved: {e}");
        CriticOutput {
            demands_revision: false,
            notes: "(critic output unparseable)".into(),
            confidence: 0.0,
        }
    });
    prompt_tokens = prompt_tokens.saturating_add(critic.prompt_tokens);
    completion_tokens = completion_tokens.saturating_add(critic.completion_tokens);

    // 6. Revision (optional).
    if verdict.demands_revision {
        debug!("critic demanded revision: {}", verdict.notes);
        let mut revision_ctx = strategist_ctx.clone();
        revision_ctx.insert("original_proposal_json", json_string(&proposal)?);
        revision_ctx.insert("critic_verdict_json", json_string(&verdict)?);
        let revision_prompt = state.prompts.render(PromptKey::Revision, &revision_ctx);
        let revised = ai
            .chat(
                ModelRoute::RebalanceReason,
                vec![
                    Message::system(revision_prompt),
                    Message::user("Provide the revised proposal.".to_string()),
                ],
            )
            .await?;
        proposal = parse_proposal(&revised.content)?;
        prompt_tokens = prompt_tokens.saturating_add(revised.prompt_tokens);
        completion_tokens = completion_tokens.saturating_add(revised.completion_tokens);
    }

    // 7. Decide on triggered_by — abstain if the strategist isn't confident.
    let final_triggered_by = if proposal.confidence < ABSTAIN_CONFIDENCE_THRESHOLD {
        "abstain".to_string()
    } else {
        triggered_by
    };

    // 8. Persist with full telemetry.
    let latency_ms = start.elapsed().as_millis() as i32;
    let recommendation_value = serde_json::to_value(&proposal.recommendation)?;
    let critic_value = serde_json::to_value(&verdict)?;

    let decision: AgentDecision = sqlx::query_as(
        r#"INSERT INTO agent_decisions
           (id, portfolio_id, reasoning, recommendation, confidence,
            triggered_by, model_slug, regime, prompt_tokens,
            completion_tokens, latency_ms, critic_verdict)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
           RETURNING *"#,
    )
    .bind(Uuid::new_v4())
    .bind(req.portfolio_id)
    .bind(&proposal.reasoning)
    .bind(&recommendation_value)
    .bind(proposal.confidence)
    .bind(&final_triggered_by)
    .bind(&strategist.model_slug)
    .bind(regime.regime.as_str())
    .bind(prompt_tokens as i32)
    .bind(completion_tokens as i32)
    .bind(latency_ms)
    .bind(&critic_value)
    .fetch_one(&state.db)
    .await?;

    // 9. Broadcast the final decision over SSE.
    let _ = state
        .sse
        .send(SseEvent::AgentDecision(AgentDecisionPayload {
            id: decision.id,
            portfolio_id: decision.portfolio_id,
            reasoning: decision.reasoning.clone(),
            recommendation: decision.recommendation.clone(),
            confidence: decision.confidence,
            triggered_by: decision.triggered_by.clone(),
            created_at: decision.created_at,
            model_slug: decision.model_slug.clone(),
            regime: decision.regime.clone(),
            prompt_tokens: decision.prompt_tokens,
            completion_tokens: decision.completion_tokens,
            latency_ms: decision.latency_ms,
            critic_verdict: decision.critic_verdict.clone(),
        }));

    Ok(decision)
}

// ── Context builders ───────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
struct UserProfile {
    risk_tolerance: String,
    investment_horizon_months: i32,
}

async fn fetch_user_profile(state: &AppState, user_id: Uuid) -> crate::error::Result<UserProfile> {
    let profile = sqlx::query_as::<_, UserProfile>(
        "SELECT risk_tolerance, investment_horizon_months FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(UserProfile {
        risk_tolerance: "moderate".into(),
        investment_horizon_months: 12,
    });
    Ok(profile)
}

async fn previous_regime(state: &AppState, portfolio_id: Uuid) -> Option<String> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT regime FROM agent_decisions
         WHERE portfolio_id = $1 AND regime IS NOT NULL
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(portfolio_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .flatten()
}

fn build_strategist_context(
    portfolio: &Portfolio,
    allocations: &[Allocation],
    user: &UserProfile,
    snapshot: &MarketSnapshot,
    regime: &RegimeClassification,
    risk: &crate::modules::risk_engine::RiskReport,
) -> HashMap<&'static str, String> {
    let mut ctx = HashMap::new();
    ctx.insert("portfolio_name", portfolio.name.clone());
    ctx.insert(
        "total_value_usd",
        format!("{:.2}", portfolio.total_value_usd),
    );
    ctx.insert("pnl_usd", format!("{:.2}", portfolio.total_pnl_usd));
    ctx.insert("pnl_pct", format!("{:.2}", portfolio.total_pnl_pct));
    ctx.insert("risk_tolerance", user.risk_tolerance.clone());
    ctx.insert("horizon_months", user.investment_horizon_months.to_string());
    ctx.insert("allocations_table", format_allocations(allocations));

    ctx.insert("regime", regime.regime.as_str().into());
    ctx.insert("regime_confidence", format!("{:.2}", regime.confidence));
    ctx.insert("btc_vol_30d", format!("{:.4}", regime.signals.btc_vol_30d));
    ctx.insert("corr_90d", format!("{:.4}", regime.signals.corr_90d));
    ctx.insert(
        "max_drawdown",
        format!("{:.4}", regime.signals.max_drawdown),
    );
    ctx.insert("fear_greed", snapshot.fear_greed_index.to_string());
    ctx.insert("btc_dominance", format!("{:.2}", snapshot.btc_dominance));
    ctx.insert(
        "concentration_risk",
        format!("{:.3}", risk.concentration_risk),
    );
    ctx.insert("volatility_score", format!("{:.3}", risk.volatility_score));
    ctx.insert("drift_score", format!("{:.3}", risk.drift_score));
    ctx
}

fn build_critic_context(
    proposal: &StrategistProposal,
    allocations: &[Allocation],
    user: &UserProfile,
    regime: &RegimeClassification,
    risk: &crate::modules::risk_engine::RiskReport,
) -> HashMap<&'static str, String> {
    let mut ctx = HashMap::new();
    ctx.insert(
        "proposal_json",
        serde_json::to_string_pretty(proposal).unwrap_or_default(),
    );
    ctx.insert("regime", regime.regime.as_str().into());
    ctx.insert("regime_confidence", format!("{:.2}", regime.confidence));
    ctx.insert(
        "concentration_risk",
        format!("{:.3}", risk.concentration_risk),
    );
    ctx.insert("volatility_score", format!("{:.3}", risk.volatility_score));
    ctx.insert("drift_score", format!("{:.3}", risk.drift_score));
    ctx.insert("allocations_table", format_allocations(allocations));
    ctx.insert("risk_tolerance", user.risk_tolerance.clone());
    ctx.insert("horizon_months", user.investment_horizon_months.to_string());
    ctx
}

fn format_allocations(allocations: &[Allocation]) -> String {
    if allocations.is_empty() {
        return "(empty portfolio)".into();
    }
    let mut rows = vec!["| Symbol | Qty | Target % | Current % | Value USD |".to_string()];
    rows.push("|---|---|---|---|---|".into());
    for a in allocations {
        rows.push(format!(
            "| {} | {:.4} | {:.2} | {:.2} | {:.2} |",
            a.asset_symbol, a.quantity, a.target_weight, a.current_weight, a.value_usd
        ));
    }
    rows.join("\n")
}

// ── Strategist response shape ──────────────────────────────────────────────

#[derive(Deserialize, serde::Serialize, Debug, Clone)]
struct StrategistProposal {
    #[serde(default)]
    reasoning: String,
    #[serde(default)]
    confidence: f64,
    #[serde(default = "default_recommendation")]
    recommendation: serde_json::Value,
}

fn default_recommendation() -> serde_json::Value {
    json!({ "summary": "Hold", "trades": [], "expectedImpact": { "riskDelta": 0.0, "diversificationScore": 0.0 } })
}

fn parse_proposal(raw: &str) -> crate::error::Result<StrategistProposal> {
    let stripped = strip_fences(raw);
    serde_json::from_str(stripped).map_err(|e| {
        crate::error::AppError::Internal(anyhow::anyhow!(
            "failed to parse strategist proposal: {e}\nraw: {raw}"
        ))
    })
}

#[derive(Deserialize, serde::Serialize, Debug, Clone)]
struct CriticOutput {
    #[serde(default, rename = "demandsRevision", alias = "demands_revision")]
    demands_revision: bool,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    confidence: f32,
}

fn parse_critic(raw: &str) -> anyhow::Result<CriticOutput> {
    let stripped = strip_fences(raw);
    serde_json::from_str(stripped)
        .map_err(|e| anyhow::anyhow!("invalid critic JSON: {e}; raw: {raw}"))
}

fn strip_fences(raw: &str) -> &str {
    let t = raw.trim();
    t.strip_prefix("```json")
        .or_else(|| t.strip_prefix("```"))
        .unwrap_or(t)
        .trim_end_matches("```")
        .trim()
}

fn json_string<T: serde::Serialize>(value: &T) -> crate::error::Result<String> {
    serde_json::to_string_pretty(value)
        .map_err(|e| crate::error::AppError::Internal(anyhow::anyhow!("serialize: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_fences_handles_json_fence() {
        assert_eq!(strip_fences("```json\n{\"a\":1}\n```"), "{\"a\":1}");
    }

    #[test]
    fn strip_fences_handles_bare_fence() {
        assert_eq!(strip_fences("```\n{\"a\":1}\n```"), "{\"a\":1}");
    }

    #[test]
    fn strip_fences_passes_plain_json() {
        assert_eq!(strip_fences("{\"a\":1}"), "{\"a\":1}");
    }

    #[test]
    fn parse_proposal_fills_defaults() {
        let p = parse_proposal(r#"{"reasoning":"r","confidence":0.7}"#).unwrap();
        assert_eq!(p.reasoning, "r");
        assert!(p.recommendation.is_object());
    }

    #[test]
    fn parse_critic_supports_camel_and_snake() {
        let camel: CriticOutput =
            serde_json::from_str(r#"{"demandsRevision":true,"notes":"x","confidence":0.4}"#)
                .unwrap();
        assert!(camel.demands_revision);
        let snake: CriticOutput =
            serde_json::from_str(r#"{"demands_revision":false,"notes":"y","confidence":0.9}"#)
                .unwrap();
        assert!(!snake.demands_revision);
    }

    #[test]
    fn format_allocations_renders_table() {
        let allocs = vec![Allocation {
            id: Uuid::nil(),
            portfolio_id: Uuid::nil(),
            asset_symbol: "BTC".into(),
            quantity: 0.5,
            target_weight: 50.0,
            current_weight: 55.0,
            value_usd: 30000.0,
        }];
        let table = format_allocations(&allocs);
        assert!(table.contains("BTC"));
        assert!(table.contains("55.00"));
    }

    #[test]
    fn format_allocations_handles_empty() {
        assert_eq!(format_allocations(&[]), "(empty portfolio)");
    }
}
