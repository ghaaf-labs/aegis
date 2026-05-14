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

use super::memory;
use super::models::{AgentDecision, AnalyzeRequest};
use crate::config::ModelRoute;
use crate::modules::ai::{Message, OpenRouterClient, PromptKey};
use crate::modules::fx;
use crate::modules::market_data::MarketSnapshot;
use crate::modules::portfolio::models::{Allocation, Portfolio};
use crate::modules::risk_engine::{self, RegimeClassification};
use crate::modules::sse::{
    AgentDecisionPayload, RegimeFlip, RegimeSignals as SseRegimeSignals, SseEvent,
};
use crate::modules::treasury;
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

    // 3b. Personalization signals: per-user memory, USYC rate, EURC basis.
    let memory_block = memory::build_memory_block(&state.db, req.portfolio_id).await?;
    let usyc_rate = treasury::service::rate(&state.http, &state.config)
        .await
        .map(|r| r.annualized_yield)
        .unwrap_or(0.0510);
    let eurc_basis = fx::service::usdc_eurc_basis(&state.http, &state.config)
        .await
        .map(|b| b.mid_rate)
        .unwrap_or(0.92);

    // 4. Strategist proposal.
    let mut strategist_ctx = build_strategist_context(
        &portfolio,
        &allocations,
        &user_profile,
        &snapshot,
        &regime,
        &risk,
    );
    strategist_ctx.insert("memory", memory_block);
    strategist_ctx.insert("usyc_rate", format!("{:.4}", usyc_rate));
    strategist_ctx.insert("usdc_eurc_basis", format!("{:.4}", eurc_basis));
    strategist_ctx.insert("goal_block", format_goal_block(&portfolio.goal));
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
            notes: "(critic output unparsable)".into(),
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
            user_id: portfolio.user_id,
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
    match sqlx::query_scalar::<_, Option<String>>(
        "SELECT regime FROM agent_decisions
         WHERE portfolio_id = $1 AND regime IS NOT NULL
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(portfolio_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(row) => row.flatten(),
        Err(e) => {
            // Don't fail the whole analysis if the lookahead query stumbles;
            // log so the omission is visible and continue with `from: None`.
            warn!("previous_regime query failed: {e}");
            None
        }
    }
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

/// Render the user's goal block for the strategist prompt. Empty goals
/// (legacy portfolios) get a "(no goal set)" line — the strategist still
/// has the rest of the context.
fn format_goal_block(goal: &serde_json::Value) -> String {
    if goal.is_null() || goal == &serde_json::json!({}) {
        return "(no goal set yet — strategist should suggest a starter allocation)".into();
    }
    let name = goal
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("(unnamed)");
    let horizon = goal.get("horizon").and_then(|v| v.as_str()).unwrap_or("?");
    let risk = goal
        .get("riskTolerance")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let monthly = goal
        .get("monthlyContributionUsd")
        .and_then(|v| v.as_f64())
        .map(|v| format!(" · monthly +${:.0}", v))
        .unwrap_or_default();
    let usyc = goal
        .get("includeUsyc")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let eurc = goal
        .get("includeEurc")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let allocations = goal
        .get("targetAllocation")
        .and_then(|v| v.as_object())
        .map(|m| {
            let mut pairs: Vec<String> = m
                .iter()
                .map(|(k, v)| format!("{k} {:.0}%", v.as_f64().unwrap_or(0.0)))
                .collect();
            pairs.sort();
            pairs.join(", ")
        })
        .unwrap_or_default();
    format!(
        "{name} · horizon {horizon} · risk {risk}{monthly} · USYC opt-in: {usyc} · EURC opt-in: {eurc} · targets: {allocations}"
    )
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

    // ── Contract tests: prompts ↔ context builders ────────────────────────
    //
    // The risk in a template-driven prompt system is silent drift: someone
    // adds `{{ new_key }}` to a `.md` template and forgets to populate it
    // from Rust. The model then receives a literal `{{ new_key }}` and the
    // failure mode is "the agent is subtly worse" — not a compile error.
    //
    // These tests render each prompt with the production context builders
    // against realistic inputs and assert no unresolved `{{` remains.

    use crate::modules::ai::{PromptKey, PromptRegistry};
    use crate::modules::market_data::AssetPrice;
    use crate::modules::risk_engine::{MarketRegime, RegimeClassification, RegimeSignals};
    use chrono::Utc;

    fn sample_portfolio() -> Portfolio {
        Portfolio {
            id: Uuid::nil(),
            user_id: Uuid::nil(),
            name: "Retirement".into(),
            total_value_usd: 12_345.67,
            total_pnl_usd: 234.50,
            total_pnl_pct: 1.9,
            risk_score: 50,
            goal: serde_json::json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn sample_allocs() -> Vec<Allocation> {
        vec![
            Allocation {
                id: Uuid::nil(),
                portfolio_id: Uuid::nil(),
                asset_symbol: "BTC".into(),
                quantity: 0.5,
                target_weight: 60.0,
                current_weight: 55.0,
                value_usd: 33_000.0,
            },
            Allocation {
                id: Uuid::nil(),
                portfolio_id: Uuid::nil(),
                asset_symbol: "ETH".into(),
                quantity: 4.0,
                target_weight: 40.0,
                current_weight: 45.0,
                value_usd: 14_000.0,
            },
        ]
    }

    fn sample_snapshot() -> MarketSnapshot {
        MarketSnapshot {
            assets: vec![AssetPrice {
                symbol: "BTC".into(),
                price_usd: 67_000.0,
                change_24h: -3.5,
                change_7d: -1.0,
                market_cap: 1.3e12,
                volume_24h: 4e10,
                updated_at: Utc::now(),
            }],
            fear_greed_index: 28,
            total_market_cap_usd: 2.5e12,
            btc_dominance: 52.4,
            captured_at: Utc::now(),
        }
    }

    fn sample_regime() -> RegimeClassification {
        RegimeClassification {
            regime: MarketRegime::RiskOff,
            confidence: 0.78,
            signals: RegimeSignals {
                btc_vol_30d: 0.6,
                corr_90d: 0.85,
                max_drawdown: 0.18,
                fear_greed: 28,
                btc_dominance: 52.4,
            },
            rationale: "high vol + correlation spike".into(),
        }
    }

    fn sample_user() -> UserProfile {
        UserProfile {
            risk_tolerance: "conservative".into(),
            investment_horizon_months: 60,
        }
    }

    fn sample_risk() -> crate::modules::risk_engine::RiskReport {
        crate::modules::risk_engine::RiskReport {
            score: 65,
            concentration_risk: 0.55,
            volatility_score: 0.7,
            drift_score: 0.05,
            summary: "elevated".into(),
        }
    }

    #[test]
    fn strategist_prompt_renders_without_unresolved_placeholders() {
        let reg = PromptRegistry::embedded();
        let mut ctx = build_strategist_context(
            &sample_portfolio(),
            &sample_allocs(),
            &sample_user(),
            &sample_snapshot(),
            &sample_regime(),
            &sample_risk(),
        );
        // Sprint 2 placeholders: memory, usyc_rate, usdc_eurc_basis, goal_block
        ctx.insert("memory", "- (no prior decisions yet)".into());
        ctx.insert("usyc_rate", "0.0510".into());
        ctx.insert("usdc_eurc_basis", "0.9217".into());
        ctx.insert(
            "goal_block",
            format_goal_block(&serde_json::json!({
                "name": "Retirement",
                "horizon": "5y",
                "riskTolerance": "moderate",
                "targetAllocation": { "BTC": 50, "ETH": 30, "USYC": 20 },
                "includeUsyc": true,
                "includeEurc": false
            })),
        );
        let rendered = reg.render(PromptKey::Strategist, &ctx);
        assert!(
            !rendered.contains("{{"),
            "strategist prompt has unresolved placeholder(s):\n{rendered}"
        );
        // Per-portfolio personalization signal: the user's actual values
        // appear in the rendered prompt.
        assert!(rendered.contains("Retirement"));
        assert!(rendered.contains("conservative"));
        assert!(rendered.contains("60"));
        assert!(rendered.contains("BTC"));
    }

    #[test]
    fn critic_prompt_renders_without_unresolved_placeholders() {
        let reg = PromptRegistry::embedded();
        let proposal = StrategistProposal {
            reasoning: "trim BTC".into(),
            confidence: 0.7,
            recommendation: json!({"summary": "Trim BTC", "trades": [], "expectedImpact": {}}),
        };
        let ctx = build_critic_context(
            &proposal,
            &sample_allocs(),
            &sample_user(),
            &sample_regime(),
            &sample_risk(),
        );
        let rendered = reg.render(PromptKey::Critic, &ctx);
        assert!(
            !rendered.contains("{{"),
            "critic prompt has unresolved placeholder(s):\n{rendered}"
        );
        assert!(rendered.contains("conservative"));
    }

    #[test]
    fn revision_prompt_renders_without_unresolved_placeholders() {
        let reg = PromptRegistry::embedded();
        // Revision context = strategist context + 2 extras.
        let mut ctx = build_strategist_context(
            &sample_portfolio(),
            &sample_allocs(),
            &sample_user(),
            &sample_snapshot(),
            &sample_regime(),
            &sample_risk(),
        );
        ctx.insert("original_proposal_json", "{\"x\":1}".into());
        ctx.insert("critic_verdict_json", "{\"y\":2}".into());
        let rendered = reg.render(PromptKey::Revision, &ctx);
        assert!(
            !rendered.contains("{{"),
            "revision prompt has unresolved placeholder(s):\n{rendered}"
        );
    }

    #[test]
    fn strategist_proposal_round_trips_through_serde_value() {
        // The strategist returns JSON we deserialize into StrategistProposal,
        // then re-serialize into the `recommendation` JSONB column. This test
        // locks the round trip so adding a field doesn't silently break it.
        let raw = r#"{
            "reasoning": "hold steady",
            "confidence": 0.62,
            "recommendation": {
                "summary": "Hold",
                "trades": [
                    {"symbol":"BTC","action":"sell","quantity":0.05,"valueUsd":3000.0,"reason":"reduce concentration"}
                ],
                "expectedImpact": {"riskDelta": -0.04, "diversificationScore": 0.71}
            }
        }"#;
        let p = parse_proposal(raw).unwrap();
        let v = serde_json::to_value(&p.recommendation).unwrap();
        assert_eq!(v["summary"], "Hold");
        assert_eq!(v["trades"][0]["valueUsd"], 3000.0);
        assert_eq!(v["expectedImpact"]["riskDelta"], -0.04);
    }
}
