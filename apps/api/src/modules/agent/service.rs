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

use super::calibration_train;
use super::constitution::{self, ClauseViolation, Tier};
use super::critic as critic_mod;
use super::memory;
use super::models::{AgentDecision, AnalyzeRequest};
use super::tools;
use crate::config::ModelRoute;
use crate::modules::ai::{ChatToolResult, Message, OpenRouterClient, PromptKey};
use crate::modules::fx;
use crate::modules::market_data::MarketSnapshot;
use crate::modules::portfolio::models::{Allocation, Portfolio};
use crate::modules::risk_engine::{self, RegimeClassification};
use crate::modules::sse::{
    AgentAbstainedPayload, AgentDecisionPayload, AgentToolInvokedPayload, RegimeFlip,
    RegimeSignals as SseRegimeSignals, SseEvent,
};
use crate::modules::treasury;
use crate::router::AppState;

const ABSTAIN_CONFIDENCE_THRESHOLD: f64 = 0.5;
/// Wall-clock budget for a single strategist call. Logged but not enforced;
/// keeps the agent honest during demo while we tune model selection.
const STRATEGIST_SLOW_MS: u64 = 10_000;

/// Per-tier model routing + feature toggles. Free runs on the cheap regime
/// slug for the strategist and skips the critic entirely. Pro restores the
/// full Opus + GPT-5 critic pipeline. Business is Pro plus the Constitution
///   counterfactual hooks (A8/A9 read `tier_features` from the persisted
///   decision metadata).
#[derive(Debug, Clone, Copy)]
struct TierModels {
    strategist_route: crate::config::ModelRoute,
    critic_route: crate::config::ModelRoute,
    run_critic: bool,
    constitution: bool,
    counterfactual: bool,
}

fn pick_models(tier: crate::modules::billing::types::Tier) -> TierModels {
    use crate::config::ModelRoute;
    use crate::modules::billing::types::Tier;
    match tier {
        Tier::Free => TierModels {
            // Free runs the cheap classifier slug for both regime + strategist.
            // The slug falls back to its env default if MODEL_REGIME isn't set,
            // so deploys that never configure Free-tier models still work.
            strategist_route: ModelRoute::RegimeClassify,
            critic_route: ModelRoute::CritiqueAgent,
            run_critic: false,
            constitution: false,
            counterfactual: false,
        },
        Tier::Pro => TierModels {
            strategist_route: ModelRoute::RebalanceReason,
            critic_route: ModelRoute::CritiqueAgent,
            run_critic: true,
            constitution: false,
            counterfactual: false,
        },
        Tier::Business => TierModels {
            strategist_route: ModelRoute::RebalanceReason,
            critic_route: ModelRoute::CritiqueAgent,
            run_critic: true,
            constitution: true,
            counterfactual: true,
        },
    }
}

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

    // Tier gate + model routing: resolve once, use for both the cap check
    // and the strategist/critic model selection. When billing v2 is OFF we
    // keep the original Pro-equivalent pipeline so the golden path is
    // untouched.
    let tier = if state.config.billing_v2_enabled {
        let t = crate::middleware::tier::resolve_tier(&state.db, portfolio.user_id).await?;
        crate::middleware::tier::enforce_decision_cap(&state.db, portfolio.user_id, t).await?;
        t
    } else {
        crate::modules::billing::types::Tier::Pro
    };
    let tier_models = pick_models(tier);

    let user_profile = fetch_user_profile(state, portfolio.user_id).await?;

    let snapshot =
        crate::modules::market_data::service::fetch_snapshot(state.prices.as_ref()).await?;

    let ai = OpenRouterClient::new(&state.http, &state.config);

    // 2. Regime classifier — cheap pass that conditions the strategist.
    // Phase 1: pass the DB so we get real 30d vol + 90d correlation from price_history
    let regime =
        risk_engine::classify(&ai, &snapshot, state.prompts.as_ref(), Some(&state.db)).await?;

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
    let eurc_basis = fx::service::usdc_eurc_basis(state.prices.as_ref(), &state.config)
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

    // Wallet awareness: the strategist used to see only `portfolios.total_value_usd`
    // (invested positions) and concluded "portfolio is empty, deposit funds"
    // on every run — even when the user had already funded $100s of USDC + EURC
    // into Circle Gateway. Inject the Gateway balance so the agent knows
    // there's deployable capital and can propose a first-deploy plan.
    let gateway_block = match crate::modules::gateway::service::fetch_balance_for_user(
        &state.db,
        &state.http,
        &state.config,
        portfolio.user_id,
    )
    .await
    {
        Ok(b) => format_gateway_block(&b),
        Err(e) => {
            tracing::debug!(error=%e, "agent: gateway balance fetch failed; strategist sees no wallet info");
            "Wallet balance: unavailable (Gateway lookup failed).".to_string()
        }
    };
    strategist_ctx.insert("wallet_block", gateway_block);
    let harvestable = crate::modules::tax::service::harvestable_losses(
        state,
        portfolio.user_id,
        req.portfolio_id,
    )
    .await
    .unwrap_or_default();
    strategist_ctx.insert(
        "harvestable_losses",
        format_harvestable_losses(&harvestable),
    );
    // Per-user signal: broadcast a tax.harvest.proposed event for any open
    // loss above the configured threshold so the UI surfaces it ahead of the
    // strategist's full reasoning.
    let threshold = state.config.harvest_threshold_usd;
    for loss in &harvestable {
        if loss.unrealized_loss_usd >= threshold {
            let _ = state.sse.send(SseEvent::TaxHarvestProposed(
                crate::modules::sse::TaxHarvestPayload {
                    user_id: portfolio.user_id,
                    portfolio_id: req.portfolio_id,
                    allocation_id: loss.allocation_id,
                    symbol: loss.symbol.clone(),
                    unrealized_loss_usd: loss.unrealized_loss_usd,
                    proposed_at: chrono::Utc::now(),
                },
            ));
        }
    }
    let strategist_prompt = state.prompts.render(PromptKey::Strategist, &strategist_ctx);
    // Tool-aware strategist loop. The model can call `fetch_news`,
    // `fetch_onchain_metric`, `fetch_correlation` up to MAX_TOOL_ITERATIONS-1
    // times; the final iteration forces a JSON proposal output.
    let strategist = run_strategist_with_tools(
        state,
        &ai,
        portfolio.user_id,
        req.portfolio_id,
        &strategist_prompt,
        tier_models.strategist_route,
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
    //
    // Free tier short-circuit: skip the critic entirely (the Haiku strategist
    // already ran; no extra token spend on no-revenue users).
    //
    // Otherwise, the strategist's proposal is run through the versioned
    // constitution YAML (max drawdown, single-asset cap, slippage ceiling,
    // EURC band, USYC floor) BEFORE the LLM critic. Any violation triggers an
    // immediate VETO whose reasoning cites the clause IDs — no LLM call
    // needed. Closes the "prompt-injection bypasses critic" attack class.
    let constitution_violations = if state.config.constitution_enabled && tier_models.run_critic {
        evaluate_constitution(
            &proposal,
            &allocations,
            &portfolio,
            tier_for_user(&user_profile),
        )
    } else {
        Vec::new()
    };

    let verdict = if !tier_models.run_critic {
        CriticOutput {
            demands_revision: false,
            notes: "(critic skipped: Free tier)".into(),
            confidence: 0.0,
            clause_ids: Vec::new(),
            verdict: None,
        }
    } else if !constitution_violations.is_empty() {
        let clause_ids: Vec<String> = constitution_violations
            .iter()
            .map(|v| v.clause_id.clone())
            .collect();
        let notes = format!(
            "Constitution violations: {} — {}",
            clause_ids.join(", "),
            constitution_violations
                .iter()
                .map(|v| v.summary.clone())
                .collect::<Vec<_>>()
                .join("; ")
        );
        CriticOutput {
            demands_revision: true,
            notes,
            confidence: 1.0,
            clause_ids,
            verdict: Some("veto".into()),
        }
    } else {
        let critic_ctx =
            build_critic_context(&proposal, &allocations, &user_profile, &regime, &risk);
        let critic_prompt = state.prompts.render(PromptKey::Critic, &critic_ctx);
        let critic = ai
            .chat(
                tier_models.critic_route,
                vec![
                    Message::system(critic_prompt),
                    Message::user("Render verdict.".to_string()),
                ],
            )
            .await?;
        let mut v = parse_critic(&critic.content).unwrap_or_else(|e| {
            warn!("critic parse failed, treating as approved: {e}");
            CriticOutput {
                demands_revision: false,
                notes: "(critic output unparsable)".into(),
                confidence: 0.0,
                clause_ids: Vec::new(),
                verdict: None,
            }
        });
        v.clause_ids.clear();
        prompt_tokens = prompt_tokens.saturating_add(critic.prompt_tokens);
        completion_tokens = completion_tokens.saturating_add(critic.completion_tokens);
        v
    };

    // 6. Revision (optional).
    if verdict.demands_revision {
        debug!("critic demanded revision: {}", verdict.notes);
        let mut revision_ctx = strategist_ctx.clone();
        revision_ctx.insert("original_proposal_json", json_string(&proposal)?);
        revision_ctx.insert("critic_verdict_json", json_string(&verdict)?);
        let revision_prompt = state.prompts.render(PromptKey::Revision, &revision_ctx);
        let revised = ai
            .chat(
                tier_models.strategist_route,
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
        let _ = state
            .sse
            .send(SseEvent::AgentAbstained(AgentAbstainedPayload {
                user_id: portfolio.user_id,
                portfolio_id: req.portfolio_id,
                confidence: proposal.confidence,
                reason: if proposal.reasoning.is_empty() {
                    "confidence below threshold".to_string()
                } else {
                    proposal.reasoning.clone()
                },
                decided_at: chrono::Utc::now(),
            }));
        "abstain".to_string()
    } else {
        triggered_by
    };

    // 7b. F-CONF-4: apply the strategist calibrator, if one has been fit.
    // Cold start (no calibrations row) ⇒ calibrated == raw; the headline UI
    // number falls back to the raw confidence so behavior is unchanged with
    // the feature flag off.
    let raw_confidence = proposal.confidence;
    let (calibrated_confidence, calibration_id_opt) = if state.config.calibrated_conf_enabled {
        match calibration_train::latest_for(&state.db, calibration_train::TASK_STRATEGIST).await {
            Ok(Some(latest)) => {
                let cal_conf = crate::modules::agent::calibration::apply_scalar(
                    &latest.calibration,
                    "right",
                    raw_confidence,
                );
                (cal_conf, Some(latest.id))
            }
            Ok(None) => {
                tracing::info!(
                    "calibration: no strategist calibration row yet; falling back to raw confidence"
                );
                (raw_confidence, None)
            }
            Err(e) => {
                warn!("calibration lookup failed, using raw confidence: {e:#}");
                (raw_confidence, None)
            }
        }
    } else {
        (raw_confidence, None)
    };

    // 7c. F-CONF-5: optional counterfactual second-pass on the critic.
    let counterfactual_opt = if state.config.calibrated_conf_enabled {
        let cf_prompt = critic_mod::build_prompt(
            &json_string(&proposal).unwrap_or_default(),
            regime.regime.as_str(),
            &verdict.notes,
        );
        match ai
            .chat(
                ModelRoute::CritiqueAgent,
                vec![
                    Message::system(cf_prompt),
                    Message::user("Emit the counterfactual JSON.".to_string()),
                ],
            )
            .await
        {
            Ok(resp) => {
                prompt_tokens = prompt_tokens.saturating_add(resp.prompt_tokens);
                completion_tokens = completion_tokens.saturating_add(resp.completion_tokens);
                match critic_mod::parse(&resp.content) {
                    Ok(out) if !out.is_empty() => Some(out.counterfactual),
                    Ok(_) => None,
                    Err(e) => {
                        warn!("counterfactual parse failed: {e:#}");
                        None
                    }
                }
            }
            Err(e) => {
                warn!("counterfactual call failed: {e:#}");
                None
            }
        }
    } else {
        None
    };

    // 8. Persist with full telemetry.
    let latency_ms = start.elapsed().as_millis() as i32;
    let mut recommendation_value = serde_json::to_value(&proposal.recommendation)?;
    // Inject tier_features into the recommendation JSONB so downstream
    // consumers (A8 calibration, A9 constitution-aware critic) can read what
    // pipeline produced this decision. Camel-case key to match the wire
    // convention; non-breaking add for existing readers.
    if let Some(obj) = recommendation_value.as_object_mut() {
        obj.insert(
            "tierFeatures".to_string(),
            json!({
                "tier": tier.to_string(),
                "constitution": tier_models.constitution,
                "counterfactual": tier_models.counterfactual,
                "criticRan": tier_models.run_critic,
            }),
        );
    }
    let critic_value = serde_json::to_value(&verdict)?;
    let snapshot_value = build_decision_snapshot(&portfolio, &allocations, &snapshot);

    let decision: AgentDecision = sqlx::query_as(
        r#"INSERT INTO agent_decisions
           (id, portfolio_id, reasoning, recommendation, confidence,
            triggered_by, model_slug, regime, prompt_tokens,
            completion_tokens, latency_ms, critic_verdict, snapshot,
            raw_confidence, calibrated_confidence, counterfactual)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
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
    .bind(&snapshot_value)
    .bind(raw_confidence)
    .bind(calibrated_confidence)
    .bind(counterfactual_opt.as_deref())
    .fetch_one(&state.db)
    .await?;

    crate::modules::observability::counters::record_agent_decision();

    // 8b. Increment usage_meters.decisions_count for the current period (A3).
    // Only when billing v2 is on — otherwise the table may be untouched and
    // the UPSERT would create spurious rows for users that won't ever pay.
    if state.config.billing_v2_enabled {
        if let Err(e) = crate::middleware::tier::record_decision(&state.db, portfolio.user_id).await
        {
            warn!(
                "usage_meters bump failed for user {}: {e}",
                portfolio.user_id
            );
        }
    }

    // 8c. F-CONF-4: insert the calibrated_predictions audit row when
    // calibration ran. Best-effort — if this insert fails we still surface
    // the decision (the columns on agent_decisions are the source of truth).
    if state.config.calibrated_conf_enabled {
        if let Err(e) = sqlx::query(
            r#"INSERT INTO calibrated_predictions
               (decision_id, raw_confidence, calibrated_confidence, calibration_id, counterfactual)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(decision.id)
        .bind(raw_confidence)
        .bind(calibrated_confidence)
        .bind(calibration_id_opt)
        .bind(counterfactual_opt.as_deref())
        .execute(&state.db)
        .await
        {
            warn!("calibrated_predictions insert failed: {e:#}");
        }
    }

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
            raw_confidence: decision.raw_confidence,
            calibrated_confidence: decision.calibrated_confidence,
            counterfactual: decision.counterfactual.clone(),
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

/// Render a snapshot of the user's Circle Gateway balance for the strategist.
/// When the user has deployable cash but zero invested, the closing line
/// explicitly tells the strategist to propose a first-deploy plan rather than
/// repeat "deposit funds" indefinitely.
fn format_gateway_block(b: &crate::modules::gateway::service::GatewayBalance) -> String {
    let mut lines = vec![format!(
        "Wallet balance (Circle Gateway, undeployed):\n  Total USDC: {:.2}\n  Total EURC: {:.2}",
        b.unified_usdc, b.unified_eurc
    )];
    for (chain, amt) in &b.per_chain {
        if *amt > 0.0 {
            lines.push(format!("  - {} USDC: {:.2}", chain.to_uppercase(), amt));
        }
    }
    for (chain, amt) in &b.per_chain_eurc {
        if *amt > 0.0 {
            lines.push(format!("  - {} EURC: {:.2}", chain.to_uppercase(), amt));
        }
    }
    let cash_total = b.unified_usdc + b.unified_eurc;
    if cash_total > 5.0 {
        lines.push(
            "Note: deployable capital is already in Gateway. Do not recommend 'deposit funds' — propose how to ALLOCATE this cash into the target weights (a first-deploy plan).".into(),
        );
    }
    lines.join("\n")
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
    let route_preferences = goal
        .get("routePreferences")
        .map(format_route_preferences)
        .unwrap_or_default();
    format!(
        "{name} · horizon {horizon} · risk {risk}{monthly} · USYC opt-in: {usyc} · EURC opt-in: {eurc} · targets: {allocations}{route_preferences}"
    )
}

fn format_route_preferences(route_preferences: &serde_json::Value) -> String {
    let networks = json_string_list(route_preferences, "networks");
    let future_networks = json_string_list(route_preferences, "networkWatchlist");
    let tokens = json_string_list(route_preferences, "tokens");
    let watchlist = json_string_list(route_preferences, "watchlist");
    let networks = if networks.is_empty() {
        "(none)".into()
    } else {
        networks.join(", ")
    };
    let future_networks = if future_networks.is_empty() {
        "(none)".into()
    } else {
        future_networks.join(", ")
    };
    let tokens = if tokens.is_empty() {
        "(none)".into()
    } else {
        tokens.join(", ")
    };
    let watchlist = if watchlist.is_empty() {
        "(none)".into()
    } else {
        watchlist.join(", ")
    };
    format!(
        " · route scope: wallet-ready networks {networks}; wallet-sync queue {future_networks}; target tokens {tokens}; watch {watchlist}"
    )
}

fn json_string_list(value: &serde_json::Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Tool-aware strategist call. Runs up to `MAX_TOOL_ITERATIONS - 1` rounds
/// of tool use; the last iteration is `force_final=true` so the model has to
/// emit a JSON proposal we can parse.
///
/// Aggregates token usage + latency across iterations and reports each tool
/// invocation over SSE as `agent.tool.invoked`.
async fn run_strategist_with_tools(
    state: &AppState,
    ai: &OpenRouterClient<'_>,
    user_id: Uuid,
    portfolio_id: Uuid,
    system_prompt: &str,
    strategist_route: ModelRoute,
) -> crate::error::Result<crate::modules::ai::ChatResponse> {
    use serde_json::json;

    let tool_specs = tools::tool_specs();
    let mut messages: Vec<serde_json::Value> = vec![
        json!({ "role": "system", "content": system_prompt }),
        json!({
            "role": "user",
            "content": "Propose a rebalance, or recommend hold. Use the tools when a signal would change the proposal; do not stack tool calls when you can already answer."
        }),
    ];

    let mut total_prompt = 0u32;
    let mut total_completion = 0u32;
    let mut total_latency = 0u64;
    let mut model_slug = state.config.model_for(strategist_route).to_string();

    for iter in 0..tools::MAX_TOOL_ITERATIONS {
        let is_last = iter == tools::MAX_TOOL_ITERATIONS - 1;
        let result = ai
            .chat_with_tools(strategist_route, &messages, &tool_specs, is_last)
            .await
            .map_err(|e| {
                crate::error::AppError::Internal(anyhow::anyhow!("strategist tool-call: {e}"))
            })?;

        match result {
            ChatToolResult::Final {
                content,
                model_slug: slug,
                prompt_tokens,
                completion_tokens,
                latency_ms,
                cost_usd,
            } => {
                total_prompt = total_prompt.saturating_add(prompt_tokens);
                total_completion = total_completion.saturating_add(completion_tokens);
                total_latency = total_latency.saturating_add(latency_ms);
                model_slug = slug;
                return Ok(crate::modules::ai::ChatResponse {
                    content,
                    model_slug,
                    prompt_tokens: total_prompt,
                    completion_tokens: total_completion,
                    latency_ms: total_latency,
                    cost_usd,
                });
            }
            ChatToolResult::Calls {
                calls,
                assistant_message,
                model_slug: slug,
                prompt_tokens,
                completion_tokens,
                latency_ms,
                cost_usd: _,
            } => {
                total_prompt = total_prompt.saturating_add(prompt_tokens);
                total_completion = total_completion.saturating_add(completion_tokens);
                total_latency = total_latency.saturating_add(latency_ms);
                model_slug = slug;

                // Append the assistant's tool-call message verbatim so the
                // next request has the full call trail.
                messages.push(assistant_message);

                for call in calls.iter() {
                    let call_start = std::time::Instant::now();
                    let payload = tools::dispatch(state, call).await;
                    let call_latency = call_start.elapsed().as_millis() as i32;
                    debug!(
                        tool=%call.name, latency_ms=%call_latency,
                        "agent tool invoked"
                    );
                    let _ = state
                        .sse
                        .send(SseEvent::AgentToolInvoked(AgentToolInvokedPayload {
                            user_id,
                            portfolio_id,
                            tool_name: call.name.clone(),
                            result_preview: truncate_preview(&payload, 200),
                            latency_ms: call_latency,
                            invoked_at: chrono::Utc::now(),
                        }));
                    messages.push(tools::tool_message(&call.id, payload));
                }
            }
        }
    }

    // Shouldn't be reachable — `is_last=true` forces a Final on the last
    // iteration. Belt-and-braces: return a placeholder so the persisted
    // decision still records the work done.
    Ok(crate::modules::ai::ChatResponse {
        content: serde_json::json!({
            "reasoning": "Tool loop exhausted without final proposal.",
            "confidence": 0.0,
            "recommendation": { "summary": "Hold", "trades": [], "expectedImpact": {} }
        })
        .to_string(),
        model_slug,
        prompt_tokens: total_prompt,
        completion_tokens: total_completion,
        cost_usd: None,
        latency_ms: total_latency,
    })
}

fn truncate_preview(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max - 1).collect();
        out.push('…');
        out
    }
}

/// Snapshot the portfolio's holdings + per-asset prices at the moment of the
/// decision. Persisted on `agent_decisions.snapshot` so the outcome compressor
/// can compute *real* 24h deltas (vs. cumulative PnL) and the diary can render
/// a counterfactual ("what if we'd done what we proposed?") instead of the
/// Sprint 3 `realized + 0.5` placeholder.
fn build_decision_snapshot(
    portfolio: &Portfolio,
    allocations: &[Allocation],
    market: &MarketSnapshot,
) -> serde_json::Value {
    let mut price_by_symbol: HashMap<String, f64> = HashMap::with_capacity(market.assets.len());
    for a in &market.assets {
        price_by_symbol.insert(a.symbol.clone(), a.price_usd);
    }

    let holdings: Vec<serde_json::Value> = allocations
        .iter()
        .map(|a| {
            // Prefer the market snapshot price; fall back to value/qty for
            // assets the market data feed doesn't cover (e.g., USDC, USYC).
            let price = price_by_symbol
                .get(&a.asset_symbol)
                .copied()
                .unwrap_or_else(|| {
                    if a.quantity.abs() > f64::EPSILON {
                        a.value_usd / a.quantity
                    } else {
                        0.0
                    }
                });
            json!({
                "symbol": a.asset_symbol,
                "quantity": a.quantity,
                "priceUsd": price,
                "valueUsd": a.value_usd,
            })
        })
        .collect();

    json!({
        "capturedAt": market.captured_at,
        "totalValueUsd": portfolio.total_value_usd,
        "holdings": holdings,
    })
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
    let stripped = crate::modules::ai::strip_json_fences(raw);
    if let Ok(proposal) = serde_json::from_str::<StrategistProposal>(stripped) {
        return Ok(proposal);
    }
    // The LLM occasionally returns malformed JSON (missing comma, unmatched
    // brace, "null" where an object was expected). Surfacing this as a 500
    // breaks the user-facing flow — they click Deploy, see a wall of raw
    // model bytes, and conclude the platform is broken. Fall back to a safe
    // HOLD proposal so the rebalance pipeline can still emit a no-op plan
    // and the user is told to retry once the next strategist call succeeds.
    tracing::warn!(
        raw_len = raw.len(),
        raw_preview = %raw.chars().take(200).collect::<String>(),
        "strategist returned unparsable JSON — using safe HOLD fallback"
    );
    Ok(StrategistProposal {
        reasoning: "Strategist output was unparsable on this pass. Holding the current allocation; retry the action to re-run the agent.".to_string(),
        confidence: 0.4,
        recommendation: default_recommendation(),
    })
}

#[derive(Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct CriticOutput {
    #[serde(default, rename = "demandsRevision", alias = "demands_revision")]
    demands_revision: bool,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    confidence: f32,
    /// Constitution clause IDs cited by this verdict. Empty Vec means either
    /// the constitution check was clean or the feature flag is off. Surfaced
    /// in the UI as the explicit rulebook behind any veto.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    clause_ids: Vec<String>,
    /// Hard-coded verdict label. "veto" is only ever emitted by the
    /// constitution short-circuit; the free-form critic returns "advise"
    /// (with demands_revision) or "approve".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    verdict: Option<String>,
}

/// Map the strategist's structured proposal into the constitution's
/// evaluation shape. The strategist may emit `expectedMaxDrawdownPct`,
/// `allocations`, or `legs` directly on its proposal; whatever it omits is
/// filled in from the user's current allocations (a no-op for that clause).
fn evaluate_constitution(
    strategist: &StrategistProposal,
    current_allocations: &[Allocation],
    portfolio: &Portfolio,
    tier: Tier,
) -> Vec<ClauseViolation> {
    let constitution = match constitution::load() {
        Ok(c) => c,
        Err(e) => {
            warn!("constitution load failed; skipping evaluation: {e}");
            return Vec::new();
        }
    };

    let rec = &strategist.recommendation;

    let expected_max_drawdown_pct = rec
        .get("expectedMaxDrawdownPct")
        .and_then(serde_json::Value::as_f64);

    // Prefer the strategist's own `allocations` field; fall back to current
    // weights so we still surface FX-1 / USYC-1 / RISK-2 violations on the
    // existing portfolio state.
    let allocations: Vec<constitution::ProposalAllocation> = rec
        .get("allocations")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let asset = v.get("asset")?.as_str()?.to_string();
                    let weight = v
                        .get("targetWeightPct")
                        .or_else(|| v.get("target_weight_pct"))
                        .and_then(serde_json::Value::as_f64)?;
                    Some(constitution::ProposalAllocation {
                        asset,
                        target_weight_pct: weight,
                    })
                })
                .collect()
        })
        .unwrap_or_else(|| {
            current_allocations
                .iter()
                .map(|a| constitution::ProposalAllocation {
                    asset: a.asset_symbol.clone(),
                    target_weight_pct: a.target_weight,
                })
                .collect()
        });

    let legs: Vec<constitution::ProposalLeg> = rec
        .get("legs")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|v| constitution::ProposalLeg {
                    slippage_bps: v
                        .get("slippageBps")
                        .or_else(|| v.get("slippage_bps"))
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(0.0),
                })
                .collect()
        })
        .unwrap_or_default();

    let proposal = constitution::Proposal {
        expected_max_drawdown_pct,
        allocations,
        legs,
    };

    constitution::evaluate(constitution, &proposal, tier, portfolio.total_value_usd)
}

/// Derive the user's tier for constitution evaluation. A2's billing schema
/// will land this on `users.tier`; until then we honour an env override
/// (USER_TIER_OVERRIDE) for integration tests and default to Free.
fn tier_for_user(_profile: &UserProfile) -> Tier {
    match std::env::var("USER_TIER_OVERRIDE").as_deref() {
        Ok("business") => Tier::Business,
        Ok("pro") => Tier::Pro,
        _ => Tier::Free,
    }
}

fn parse_critic(raw: &str) -> anyhow::Result<CriticOutput> {
    let stripped = crate::modules::ai::strip_json_fences(raw);
    serde_json::from_str(stripped)
        .map_err(|e| anyhow::anyhow!("invalid critic JSON: {e}; raw: {raw}"))
}

fn json_string<T: serde::Serialize>(value: &T) -> crate::error::Result<String> {
    serde_json::to_string_pretty(value)
        .map_err(|e| crate::error::AppError::Internal(anyhow::anyhow!("serialize: {e}")))
}

/// Render harvestable losses as a human-readable block the strategist can
/// reason over. Empty list collapses to "(none)" so the placeholder still
/// resolves.
fn format_harvestable_losses(losses: &[crate::modules::tax::HarvestableLoss]) -> String {
    if losses.is_empty() {
        return "(none)".to_string();
    }
    let mut out = String::new();
    for l in losses {
        out.push_str(&format!(
            "- {symbol}: ${loss:.2} unrealized loss across {n} open lot(s)\n",
            symbol = l.symbol,
            loss = l.unrealized_loss_usd,
            n = l.lots.len()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_models_free_skips_critic_and_uses_regime_slug() {
        use crate::config::ModelRoute;
        use crate::modules::billing::types::Tier;
        let m = pick_models(Tier::Free);
        assert_eq!(m.strategist_route, ModelRoute::RegimeClassify);
        assert!(!m.run_critic);
        assert!(!m.constitution);
        assert!(!m.counterfactual);
    }

    #[test]
    fn pick_models_pro_runs_full_pipeline_no_extras() {
        use crate::config::ModelRoute;
        use crate::modules::billing::types::Tier;
        let m = pick_models(Tier::Pro);
        assert_eq!(m.strategist_route, ModelRoute::RebalanceReason);
        assert_eq!(m.critic_route, ModelRoute::CritiqueAgent);
        assert!(m.run_critic);
        assert!(!m.constitution);
        assert!(!m.counterfactual);
    }

    #[test]
    fn pick_models_business_enables_constitution_and_counterfactual() {
        use crate::modules::billing::types::Tier;
        let m = pick_models(Tier::Business);
        assert!(m.run_critic);
        assert!(m.constitution);
        assert!(m.counterfactual);
    }

    #[test]
    fn parse_proposal_strips_deepseek_preamble_fence() {
        // DeepSeek-style: prose preamble, fenced JSON, suffix prose.
        let raw = "Here is my proposal:\n\n```json\n{\"reasoning\":\"r\",\"confidence\":0.8}\n```\n\nThanks.";
        let p = parse_proposal(raw).unwrap();
        assert_eq!(p.reasoning, "r");
        assert!((p.confidence - 0.8).abs() < 1e-6);
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
                "includeEurc": false,
                "routePreferences": {
                    "networks": ["ARC-TESTNET", "BASE-SEPOLIA"],
                    "networkWatchlist": ["ETH-SEPOLIA", "ARB-SEPOLIA", "AVAX-FUJI"],
                    "tokens": ["USDC", "BTC", "ETH", "SOL", "USYC", "EURC"],
                    "watchlist": []
                }
            })),
        );
        ctx.insert("harvestable_losses", "(none)".into());
        ctx.insert("wallet_block", "Wallet balance: $0".into());
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
        assert!(rendered.contains("route scope"));
        assert!(rendered.contains("BASE-SEPOLIA"));
        assert!(rendered.contains("AVAX-FUJI"));
        assert!(rendered.contains("USYC"));
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

    // ── F-CON-3: constitution short-circuit ──────────────────────────────
    //
    // The `analyze_portfolio` flow is too big to mock end-to-end without
    // tooling we don't have at this layer (AppState wraps a live PgPool +
    // reqwest client). Instead, these tests exercise the pure
    // `evaluate_constitution` helper that the critic block consults — the
    // bit that decides whether to short-circuit. Combined with the YAML +
    // evaluator tests in `constitution::tests`, the short-circuit decision
    // is fully covered end-to-end at the unit level.

    fn proposal_with(recommendation: serde_json::Value) -> StrategistProposal {
        StrategistProposal {
            reasoning: "test".into(),
            confidence: 0.8,
            recommendation,
        }
    }

    #[test]
    fn evaluate_constitution_returns_empty_for_clean_proposal() {
        let p = proposal_with(json!({
            "expectedMaxDrawdownPct": 0.10,
            "allocations": [
                {"asset": "USDC", "targetWeightPct": 0.50},
                {"asset": "BTC",  "targetWeightPct": 0.30},
                {"asset": "USYC", "targetWeightPct": 0.20}
            ],
            "legs": [{"slippageBps": 20.0}]
        }));
        let portfolio = sample_portfolio();
        let v = evaluate_constitution(&p, &[], &portfolio, Tier::Pro);
        assert!(v.is_empty(), "expected clean but got: {:?}", v);
    }

    #[test]
    fn evaluate_constitution_cites_clause_id_for_oversized_drawdown() {
        let p = proposal_with(json!({
            "expectedMaxDrawdownPct": 0.27,
            "allocations": [{"asset": "USDC", "targetWeightPct": 0.50}]
        }));
        let portfolio = sample_portfolio();
        let v = evaluate_constitution(&p, &[], &portfolio, Tier::Pro);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].clause_id, "RISK-1");
        assert_eq!(v[0].observed, json!(0.27));
    }

    #[test]
    fn evaluate_constitution_fires_risk_2_for_concentrated_position() {
        let p = proposal_with(json!({
            "allocations": [
                {"asset": "BTC",  "targetWeightPct": 0.75},
                {"asset": "USDC", "targetWeightPct": 0.25}
            ]
        }));
        let portfolio = sample_portfolio();
        let v = evaluate_constitution(&p, &[], &portfolio, Tier::Free);
        let ids: Vec<&str> = v.iter().map(|x| x.clause_id.as_str()).collect();
        assert!(ids.contains(&"RISK-2"));
    }

    #[test]
    fn critic_output_serialises_clause_ids_when_present() {
        let v = CriticOutput {
            demands_revision: true,
            notes: "Constitution violations: RISK-1, RISK-2".into(),
            confidence: 1.0,
            clause_ids: vec!["RISK-1".into(), "RISK-2".into()],
            verdict: Some("veto".into()),
        };
        let json = serde_json::to_value(&v).unwrap();
        assert_eq!(json["verdict"], "veto");
        assert_eq!(json["clauseIds"], json!(["RISK-1", "RISK-2"]));
        assert_eq!(json["demandsRevision"], true);
    }

    #[test]
    fn critic_output_omits_clause_ids_when_empty() {
        let v = CriticOutput {
            demands_revision: false,
            notes: "ok".into(),
            confidence: 0.8,
            clause_ids: Vec::new(),
            verdict: None,
        };
        let json = serde_json::to_value(&v).unwrap();
        assert!(json.get("clauseIds").is_none());
        assert!(json.get("verdict").is_none());
    }

    #[test]
    fn tier_for_user_reads_env_override() {
        // SAFETY: env var manipulation is racy across tests; we serialise by
        // writing → reading → restoring inside the same scope and accept the
        // serial-test cost as cheaper than introducing a global lock.
        let prior = std::env::var("USER_TIER_OVERRIDE").ok();
        std::env::set_var("USER_TIER_OVERRIDE", "business");
        let t = tier_for_user(&sample_user());
        assert_eq!(t, Tier::Business);
        std::env::set_var("USER_TIER_OVERRIDE", "pro");
        assert_eq!(tier_for_user(&sample_user()), Tier::Pro);
        std::env::remove_var("USER_TIER_OVERRIDE");
        assert_eq!(tier_for_user(&sample_user()), Tier::Free);
        if let Some(v) = prior {
            std::env::set_var("USER_TIER_OVERRIDE", v);
        }
    }
}
