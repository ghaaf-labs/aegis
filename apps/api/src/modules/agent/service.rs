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
use std::time::{Duration, Instant};

use rust_decimal::prelude::ToPrimitive;
use serde::Deserialize;
use serde_json::json;
use tracing::{debug, warn};
use uuid::Uuid;

use super::allocation::{
    clamp_allocation, derive_guardrails, finalize_allocation, goal_objective, round2,
    FinalizedAllocation, RawAllocation,
};
use super::calibration_train;
use super::constitution::{self, ClauseViolation, Tier};
use super::critic as critic_mod;
use super::decision_context::{
    build_decision_context, build_decision_snapshot, fetch_user_profile, format_allocations,
    DecisionContext, UserProfile,
};
use super::models::{AgentDecision, AnalyzeRequest, ProposeAllocationRequest};
use super::tools;
use crate::config::ModelRoute;
use crate::modules::ai::{ChatToolResult, Message, OpenRouterClient, PromptKey};
use crate::modules::portfolio::models::{Allocation, Portfolio};
use crate::modules::risk_engine::RegimeClassification;
use crate::modules::sse::{
    AgentAbstainedPayload, AgentDecisionPayload, AgentToolInvokedPayload, SseEvent,
};
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
    // Synchronous (run-to-completion) entry for the scheduler. The HTTP route
    // uses `enqueue_analysis` + a spawned `run_analysis_job` so the request
    // returns immediately (see agent::handlers::analyze).
    let (queued, is_new) = enqueue_analysis(state, &req).await?;
    if is_new {
        return run_analysis_job(state, queued.id, &req).await;
    }
    await_decision_ready(state, queued.id).await
}

/// Enqueue a rebalance-analysis job: insert a `queued` placeholder (kind
/// `rebalance`) and return it, deduping to any in-flight analysis for the
/// portfolio. `is_new = false` ⇒ a job is already running; don't start another.
pub async fn enqueue_analysis(
    state: &AppState,
    req: &AnalyzeRequest,
) -> crate::error::Result<(AgentDecision, bool)> {
    let triggered_by = req
        .triggered_by
        .clone()
        .unwrap_or_else(|| "user_request".to_string());
    let inserted: Option<AgentDecision> = sqlx::query_as(
        r#"INSERT INTO agent_decisions
               (id, portfolio_id, reasoning, triggered_by, kind, status)
           VALUES ($1, $2, '', $3, 'rebalance', 'queued')
           ON CONFLICT (portfolio_id, kind) WHERE status IN ('queued', 'running')
           DO NOTHING
           RETURNING *"#,
    )
    .bind(Uuid::new_v4())
    .bind(req.portfolio_id)
    .bind(&triggered_by)
    .fetch_optional(&state.db)
    .await?;
    if let Some(row) = inserted {
        return Ok((row, true));
    }
    let existing: AgentDecision = sqlx::query_as(
        r#"SELECT * FROM agent_decisions
           WHERE portfolio_id = $1 AND kind = 'rebalance'
                 AND status IN ('queued', 'running')
           ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(req.portfolio_id)
    .fetch_one(&state.db)
    .await?;
    Ok((existing, false))
}

/// Run a queued analysis job to completion (semaphore-bounded); flips the row to
/// `ready`, or `failed` so the client recovers via a retry.
pub async fn run_analysis_job(
    state: &AppState,
    decision_id: Uuid,
    req: &AnalyzeRequest,
) -> crate::error::Result<AgentDecision> {
    let _permit = state
        .inference_permits
        .acquire()
        .await
        .map_err(|e| anyhow::anyhow!("inference semaphore closed: {e}"))?;
    sqlx::query(
        "UPDATE agent_decisions SET status = 'running', started_at = now() WHERE id = $1 AND status = 'queued'",
    )
    .bind(decision_id)
    .execute(&state.db)
    .await?;
    match run_analysis_pipeline(state, decision_id, req).await {
        Ok(decision) => Ok(decision),
        Err(e) => {
            let msg = e.to_string();
            let _ = sqlx::query(
                "UPDATE agent_decisions SET status = 'failed', error = $2, completed_at = now() WHERE id = $1",
            )
            .bind(decision_id)
            .bind(&msg)
            .execute(&state.db)
            .await;
            tracing::warn!(decision_id = %decision_id, error = %msg, "analysis job failed");
            Err(e)
        }
    }
}

/// The strategist → critic → (optional) revision pipeline proper. Finalizes the
/// queued row via `persist_and_broadcast_decision`.
async fn run_analysis_pipeline(
    state: &AppState,
    decision_id: Uuid,
    req: &AnalyzeRequest,
) -> crate::error::Result<AgentDecision> {
    let start = Instant::now();
    let triggered_by = req
        .triggered_by
        .clone()
        .unwrap_or_else(|| "user_request".to_string());

    // Shared scaffolding: load + classify + assemble the strategist context.
    // `enforce_cap` runs the tier resolution + decision-cap gate inside the
    // helper, right after the portfolio loads (before any LLM call or SSE) —
    // matching the original ordering so a capped user fails fast.
    let ctx = build_decision_context(state, req.portfolio_id, None, true).await?;
    let tier = ctx
        .tier
        .unwrap_or(crate::modules::billing::types::Tier::Pro);
    let tier_models = pick_models(tier);

    let ai = OpenRouterClient::new(&state.http, &state.config);

    // Strategist → critic → single revision.
    let StrategistRun {
        mut proposal,
        verdict,
        model_slug,
        mut prompt_tokens,
        mut completion_tokens,
    } = run_strategist_with_critic(state, &ai, req, &ctx, tier_models).await?;

    // Revision (optional).
    if verdict.demands_revision {
        debug!("critic demanded revision: {}", verdict.notes);
        let mut revision_ctx = ctx.strategist_ctx.clone();
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

    // Decide on triggered_by — abstain if the strategist isn't confident.
    let final_triggered_by = if proposal.confidence < ABSTAIN_CONFIDENCE_THRESHOLD {
        let _ = state
            .sse
            .send(SseEvent::AgentAbstained(AgentAbstainedPayload {
                user_id: ctx.portfolio.user_id,
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

    // Calibration + optional counterfactual second pass.
    let calibration = apply_calibration_and_counterfactual(
        state,
        &ai,
        &proposal,
        &verdict,
        ctx.regime.regime.as_str(),
        &mut prompt_tokens,
        &mut completion_tokens,
    )
    .await;

    let latency_ms = start.elapsed().as_millis() as i32;
    persist_and_broadcast_decision(
        state,
        &ctx,
        PersistArgs {
            decision_id,
            proposal: &proposal,
            verdict: &verdict,
            final_triggered_by: &final_triggered_by,
            model_slug: &model_slug,
            tier,
            tier_models,
            prompt_tokens,
            completion_tokens,
            latency_ms,
            calibration,
        },
    )
    .await
}

/// Outcome of the strategist → critic loop (pre-revision): the parsed proposal,
/// the critic verdict, the slug + token usage to roll into the persisted row.
struct StrategistRun {
    proposal: StrategistProposal,
    verdict: CriticOutput,
    model_slug: String,
    prompt_tokens: u32,
    completion_tokens: u32,
}

/// Run the tool-aware strategist, then the critic pass. Free tier skips the
/// critic entirely; otherwise the proposal is checked against the constitution
/// YAML (immediate VETO citing clause IDs) before the LLM critic runs.
async fn run_strategist_with_critic(
    state: &AppState,
    ai: &OpenRouterClient<'_>,
    req: &AnalyzeRequest,
    ctx: &DecisionContext,
    tier_models: TierModels,
) -> crate::error::Result<StrategistRun> {
    let strategist_prompt = state
        .prompts
        .render(PromptKey::Strategist, &ctx.strategist_ctx);
    // Tool-aware strategist loop. The model can call `fetch_news`,
    // `fetch_onchain_metric`, `fetch_correlation` up to MAX_TOOL_ITERATIONS-1
    // times; the final iteration forces a JSON proposal output.
    let strategist = run_strategist_with_tools(
        state,
        ai,
        ctx.portfolio.user_id,
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
    let proposal = parse_proposal(&strategist.content)?;
    let mut prompt_tokens = strategist.prompt_tokens;
    let mut completion_tokens = strategist.completion_tokens;

    // Critic pass — adversarial review.
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
            &ctx.allocations,
            &ctx.portfolio,
            tier_for_user(&ctx.user_profile),
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
        let critic_ctx = build_critic_context(
            &proposal,
            &ctx.allocations,
            &ctx.user_profile,
            &ctx.regime,
            &ctx.risk,
        );
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

    Ok(StrategistRun {
        proposal,
        verdict,
        model_slug: strategist.model_slug,
        prompt_tokens,
        completion_tokens,
    })
}

/// Calibration + counterfactual artifacts threaded into the persisted decision.
struct CalibrationOutcome {
    raw_confidence: f64,
    calibrated_confidence: f64,
    calibration_id: Option<Uuid>,
    counterfactual: Option<String>,
}

/// F-CONF-4/5: apply the strategist calibrator (cold start ⇒ calibrated == raw)
/// and, when calibration is enabled, run an optional counterfactual second-pass
/// on the critic. Token usage from the extra call accumulates into the
/// caller's counters.
async fn apply_calibration_and_counterfactual(
    state: &AppState,
    ai: &OpenRouterClient<'_>,
    proposal: &StrategistProposal,
    verdict: &CriticOutput,
    regime: &str,
    prompt_tokens: &mut u32,
    completion_tokens: &mut u32,
) -> CalibrationOutcome {
    // F-CONF-4: apply the strategist calibrator, if one has been fit.
    // Cold start (no calibrations row) ⇒ calibrated == raw; the headline UI
    // number falls back to the raw confidence so behavior is unchanged with
    // the feature flag off.
    let raw_confidence = proposal.confidence;
    let (calibrated_confidence, calibration_id) = if state.config.calibrated_conf_enabled {
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

    // F-CONF-5: optional counterfactual second-pass on the critic.
    let counterfactual = if state.config.calibrated_conf_enabled {
        let cf_prompt = critic_mod::build_prompt(
            &json_string(proposal).unwrap_or_default(),
            regime,
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
                *prompt_tokens = prompt_tokens.saturating_add(resp.prompt_tokens);
                *completion_tokens = completion_tokens.saturating_add(resp.completion_tokens);
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

    CalibrationOutcome {
        raw_confidence,
        calibrated_confidence,
        calibration_id,
        counterfactual,
    }
}

/// Inputs for persisting + broadcasting a `rebalance` decision. Grouped into a
/// struct so the helper stays under the argument-count threshold.
struct PersistArgs<'a> {
    /// The queued row to finalize (flip to `ready` with the decision). Its
    /// `portfolio_id` is already set, so the persist step only updates content.
    decision_id: Uuid,
    proposal: &'a StrategistProposal,
    verdict: &'a CriticOutput,
    final_triggered_by: &'a str,
    model_slug: &'a str,
    tier: crate::modules::billing::types::Tier,
    tier_models: TierModels,
    prompt_tokens: u32,
    completion_tokens: u32,
    latency_ms: i32,
    calibration: CalibrationOutcome,
}

/// Persist the rebalance decision with full telemetry, bump usage meters /
/// calibration audit rows, and broadcast the final `agent.decision` over SSE.
async fn persist_and_broadcast_decision(
    state: &AppState,
    ctx: &DecisionContext,
    args: PersistArgs<'_>,
) -> crate::error::Result<AgentDecision> {
    let PersistArgs {
        decision_id,
        proposal,
        verdict,
        final_triggered_by,
        model_slug,
        tier,
        tier_models,
        prompt_tokens,
        completion_tokens,
        latency_ms,
        calibration,
    } = args;
    let CalibrationOutcome {
        raw_confidence,
        calibrated_confidence,
        calibration_id,
        counterfactual,
    } = calibration;

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
    let critic_value = serde_json::to_value(verdict)?;
    let snapshot_value = build_decision_snapshot(&ctx.portfolio, &ctx.allocations, &ctx.snapshot);

    // Flip the queued row to `ready` with the full decision (the async
    // analyze pipeline pre-inserted it; see `enqueue_analysis`).
    let decision: AgentDecision = sqlx::query_as(
        r#"UPDATE agent_decisions SET
               reasoning = $2, recommendation = $3, confidence = $4, triggered_by = $5,
               model_slug = $6, regime = $7, prompt_tokens = $8, completion_tokens = $9,
               latency_ms = $10, critic_verdict = $11, snapshot = $12, raw_confidence = $13,
               calibrated_confidence = $14, counterfactual = $15,
               status = 'ready', completed_at = now()
           WHERE id = $1
           RETURNING *"#,
    )
    .bind(decision_id)
    .bind(&proposal.reasoning)
    .bind(&recommendation_value)
    .bind(proposal.confidence)
    .bind(final_triggered_by)
    .bind(model_slug)
    .bind(ctx.regime.regime.as_str())
    .bind(prompt_tokens as i32)
    .bind(completion_tokens as i32)
    .bind(latency_ms)
    .bind(&critic_value)
    .bind(&snapshot_value)
    .bind(raw_confidence)
    .bind(calibrated_confidence)
    .bind(counterfactual.as_deref())
    .fetch_one(&state.db)
    .await?;

    crate::modules::observability::counters::record_agent_decision();

    // Increment usage_meters.decisions_count for the current period (A3).
    // Only when billing v2 is on — otherwise the table may be untouched and
    // the UPSERT would create spurious rows for users that won't ever pay.
    if state.config.billing_v2_enabled {
        if let Err(e) =
            crate::middleware::tier::record_decision(&state.db, ctx.portfolio.user_id).await
        {
            warn!(
                "usage_meters bump failed for user {}: {e}",
                ctx.portfolio.user_id
            );
        }
    }

    // F-CONF-4: insert the calibrated_predictions audit row when
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
        .bind(calibration_id)
        .bind(counterfactual.as_deref())
        .execute(&state.db)
        .await
        {
            warn!("calibrated_predictions insert failed: {e:#}");
        }
    }

    broadcast_agent_decision(state, ctx.portfolio.user_id, &decision);

    Ok(decision)
}

pub(crate) fn broadcast_agent_decision(state: &AppState, user_id: Uuid, decision: &AgentDecision) {
    let _ = state
        .sse
        .send(SseEvent::AgentDecision(AgentDecisionPayload {
            id: decision.id,
            portfolio_id: decision.portfolio_id,
            user_id,
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
            kind: decision.kind.clone(),
            recommended_allocation: decision.recommended_allocation.clone(),
            allocation_applied_at: decision.allocation_applied_at,
        }));
}

// ── Agent-decided allocation (the headline) ─────────────────────────────────

/// Run the allocator: the agent designs a full target allocation from the
/// user's objective/horizon/risk + market regime, deterministically clamped to
/// a valid, executable, non-over-concentrated target. Persists an
/// `allocation_proposal` decision and broadcasts it over SSE. The user approves
/// it via [`apply_allocation`].
pub async fn propose_allocation(
    state: &AppState,
    req: ProposeAllocationRequest,
) -> crate::error::Result<AgentDecision> {
    // Synchronous (run-to-completion) entry, used by the auto-pilot / scheduler
    // which already run off the request path. The user-facing HTTP route instead
    // calls `enqueue_allocation` + a spawned `run_allocation_job` so the request
    // returns immediately (see agent::handlers::propose_allocation).
    let (queued, is_new) = enqueue_allocation(state, &req).await?;
    if is_new {
        return run_allocation_job(state, queued.id, &req).await;
    }
    // A job for this portfolio is already in flight (a concurrent request / tab);
    // wait for it to finish rather than starting a duplicate model call.
    await_decision_ready(state, queued.id).await
}

/// Enqueue an allocation-proposal job: insert a `queued` placeholder row and
/// return it. The partial unique index `agent_decisions_one_inflight_per_*`
/// guarantees at most one in-flight job per portfolio, so a double-submit
/// (React StrictMode, double-click, retry-while-running) dedupes onto the
/// existing row. Returns `(row, is_new)` — `is_new = false` means an in-flight
/// job already existed and the caller must NOT start a second one. The
/// placeholder stays out of the decision list (Gate-1 never opens on it) until
/// the worker flips it to `ready`.
pub async fn enqueue_allocation(
    state: &AppState,
    req: &ProposeAllocationRequest,
) -> crate::error::Result<(AgentDecision, bool)> {
    let triggered_by = req
        .triggered_by
        .clone()
        .unwrap_or_else(|| "allocation_proposal".to_string());

    let inserted: Option<AgentDecision> = sqlx::query_as(
        r#"INSERT INTO agent_decisions
               (id, portfolio_id, reasoning, triggered_by, kind, status)
           VALUES ($1, $2, '', $3, 'allocation_proposal', 'queued')
           ON CONFLICT (portfolio_id, kind) WHERE status IN ('queued', 'running')
           DO NOTHING
           RETURNING *"#,
    )
    .bind(Uuid::new_v4())
    .bind(req.portfolio_id)
    .bind(&triggered_by)
    .fetch_optional(&state.db)
    .await?;

    if let Some(row) = inserted {
        return Ok((row, true));
    }

    let existing: AgentDecision = sqlx::query_as(
        r#"SELECT * FROM agent_decisions
           WHERE portfolio_id = $1 AND kind = 'allocation_proposal'
                 AND status IN ('queued', 'running')
           ORDER BY created_at DESC
           LIMIT 1"#,
    )
    .bind(req.portfolio_id)
    .fetch_one(&state.db)
    .await?;
    Ok((existing, false))
}

/// Run a queued allocation job to completion: mark it `running`, run the
/// pipeline, then flip the row to `ready` (persisting the full decision) or
/// `failed` (so the client recovers via a retry instead of hanging on an SSE
/// event that never arrives). Bounded by the inference semaphore so an
/// onboarding burst can't open unbounded OpenRouter calls.
pub async fn run_allocation_job(
    state: &AppState,
    decision_id: Uuid,
    req: &ProposeAllocationRequest,
) -> crate::error::Result<AgentDecision> {
    let _permit = state
        .inference_permits
        .acquire()
        .await
        .map_err(|e| anyhow::anyhow!("inference semaphore closed: {e}"))?;

    sqlx::query(
        "UPDATE agent_decisions SET status = 'running', started_at = now() WHERE id = $1 AND status = 'queued'",
    )
    .bind(decision_id)
    .execute(&state.db)
    .await?;

    match run_allocation_pipeline(state, decision_id, req).await {
        Ok(decision) => Ok(decision),
        Err(e) => {
            let msg = e.to_string();
            // Best-effort failure mark; the boot reconciler is the backstop.
            let _ = sqlx::query(
                "UPDATE agent_decisions SET status = 'failed', error = $2, completed_at = now() WHERE id = $1",
            )
            .bind(decision_id)
            .bind(&msg)
            .execute(&state.db)
            .await;
            tracing::warn!(decision_id = %decision_id, error = %msg, "allocation job failed");
            Err(e)
        }
    }
}

/// The allocation pipeline proper: classify regime, call the allocator, finalize
/// (the deterministic clamp keeps the persisted target honest regardless of
/// model), constitution-check, then flip the queued row to `ready` and broadcast
/// `agent.decision`.
async fn run_allocation_pipeline(
    state: &AppState,
    decision_id: Uuid,
    req: &ProposeAllocationRequest,
) -> crate::error::Result<AgentDecision> {
    use crate::modules::rebalance::registry::allocation_target_symbols;
    let start = Instant::now();

    // Shared scaffolding: load + classify + assemble context. The Gate-1 risk
    // dial re-proposes at a different risk level via `risk_override` without
    // mutating the stored profile. The allocator-specific `objective` key is
    // added below — the strategist path never carries it.
    let DecisionContext {
        portfolio,
        allocations,
        user_profile,
        snapshot,
        regime,
        strategist_ctx: mut ctx,
        ..
    } = build_decision_context(state, req.portfolio_id, req.risk_override.as_deref(), false)
        .await?;
    ctx.insert("objective", goal_objective(&portfolio.goal));

    let ai = OpenRouterClient::new(&state.http, &state.config);
    let prompt = state.prompts.render(PromptKey::Allocator, &ctx);
    let resp = ai
        .chat(
            ModelRoute::RebalanceReason,
            vec![
                Message::system(prompt),
                Message::user("Design the target allocation as JSON.".to_string()),
            ],
        )
        .await?;
    let parsed = parse_proposal(&resp.content)?;

    let guardrails = derive_guardrails(
        &user_profile.risk_tolerance,
        regime.regime.as_str(),
        regime.signals.btc_vol_30d,
    );
    let target_universe = allocation_target_symbols(&state.config);
    let raw_map = parsed.recommended_allocation.clone().unwrap_or_default();
    let confidence = parsed.confidence.clamp(0.0, 1.0);

    // Reconcile the raw model output into a consistent, persist-ready target:
    // clamp against the designable target universe, annotate the reasoning with
    // any risk adjustments, and keep the drawdown consistent with the final mix.
    // Execution readiness is checked later by the approval safety gate; do not
    // silently convert a valid target into a 100% USDC no-op just because a rail
    // is temporarily not ready.
    let FinalizedAllocation {
        allocation: clamped,
        reasoning,
        expected_max_drawdown_pct,
        adjustments,
    } = finalize_allocation(
        RawAllocation {
            weights: &raw_map,
            reasoning: &parsed.reasoning,
            confidence: parsed.confidence,
            expected_max_drawdown_pct: parsed.expected_max_drawdown_pct,
        },
        portfolio
            .goal
            .get("targetAllocation")
            .and_then(|v| v.as_object()),
        &target_universe,
        guardrails,
    );
    if !adjustments.is_empty() {
        tracing::info!(
            portfolio_id = %req.portfolio_id,
            adjustments = ?adjustments,
            "allocator proposal clamped to risk limits; reasoning annotated"
        );
    }

    let alloc_obj: serde_json::Map<String, serde_json::Value> = clamped
        .iter()
        .map(|(k, v)| (k.clone(), json!(round2(*v))))
        .collect();
    let alloc_value = serde_json::Value::Object(alloc_obj.clone());

    // Constitution check (unconditional) over the clamped allocation. The clamp
    // already enforces RISK-2 (≤60%), so this is a verification + audit surface;
    // surfaced in the verdict rather than forcing a revision.
    let constitution_proposal = StrategistProposal {
        reasoning: reasoning.clone(),
        confidence,
        recommendation: json!({
            "allocations": clamped
                .iter()
                .map(|(k, v)| json!({ "asset": k, "targetWeightPct": v }))
                .collect::<Vec<_>>(),
            "expectedMaxDrawdownPct": expected_max_drawdown_pct,
        }),
        recommended_allocation: Some(alloc_obj.clone()),
        expected_max_drawdown_pct,
    };
    let violations = evaluate_constitution(
        &constitution_proposal,
        &allocations,
        &portfolio,
        tier_for_user(&user_profile),
    );
    let verdict = if violations.is_empty() {
        CriticOutput {
            demands_revision: false,
            notes: "Constitution clean (allocation clamped to policy).".into(),
            confidence: 1.0,
            clause_ids: Vec::new(),
            verdict: Some("approve".into()),
        }
    } else {
        let ids: Vec<String> = violations.iter().map(|v| v.clause_id.clone()).collect();
        CriticOutput {
            demands_revision: false,
            notes: format!("Constitution advisories after clamp: {}", ids.join(", ")),
            confidence: 1.0,
            clause_ids: ids,
            verdict: Some("advise".into()),
        }
    };

    let summary = format!(
        "Agent target: {}",
        clamped
            .iter()
            .map(|(k, v)| format!("{k} {v:.0}%"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let recommendation_value = json!({
        "summary": summary,
        "recommendedAllocation": alloc_value,
        "expectedMaxDrawdownPct": expected_max_drawdown_pct,
    });
    let critic_value = serde_json::to_value(&verdict)?;
    let snapshot_value = build_decision_snapshot(&portfolio, &allocations, &snapshot);
    let latency_ms = start.elapsed().as_millis() as i32;

    // Flip the queued row to `ready` with the full decision in one statement.
    let decision: AgentDecision = sqlx::query_as(
        r#"UPDATE agent_decisions SET
               reasoning = $2, recommendation = $3, confidence = $4, model_slug = $5,
               regime = $6, prompt_tokens = $7, completion_tokens = $8, latency_ms = $9,
               critic_verdict = $10, snapshot = $11, raw_confidence = $12,
               calibrated_confidence = $13, recommended_allocation = $14,
               status = 'ready', completed_at = now()
           WHERE id = $1
           RETURNING *"#,
    )
    .bind(decision_id)
    .bind(&reasoning)
    .bind(&recommendation_value)
    .bind(confidence)
    .bind(&resp.model_slug)
    .bind(regime.regime.as_str())
    .bind(resp.prompt_tokens as i32)
    .bind(resp.completion_tokens as i32)
    .bind(latency_ms)
    .bind(&critic_value)
    .bind(&snapshot_value)
    .bind(confidence)
    .bind(confidence)
    .bind(&alloc_value)
    .fetch_one(&state.db)
    .await?;

    crate::modules::observability::counters::record_agent_decision();

    broadcast_agent_decision(state, portfolio.user_id, &decision);

    Ok(decision)
}

/// Poll a decision row until it leaves the in-flight states, returning the
/// terminal row. Used when a blocking caller (auto-pilot) deduped onto an
/// already-running job; bounded so a wedged worker can't hang it forever.
pub(crate) async fn await_decision_ready(
    state: &AppState,
    decision_id: Uuid,
) -> crate::error::Result<AgentDecision> {
    let deadline =
        Instant::now() + Duration::from_secs(state.config.openrouter_attempt_timeout_secs * 3 + 30);
    loop {
        let row: AgentDecision = sqlx::query_as("SELECT * FROM agent_decisions WHERE id = $1")
            .bind(decision_id)
            .fetch_one(&state.db)
            .await?;
        match row.status.as_deref() {
            // `None` covers legacy rows written before migration 0004.
            Some("ready") | None => return Ok(row),
            Some("failed") => {
                return Err(anyhow::anyhow!(row
                    .error
                    .clone()
                    .unwrap_or_else(|| "allocation job failed".to_string()))
                .into())
            }
            _ if Instant::now() >= deadline => {
                return Err(anyhow::anyhow!("allocation job did not complete in time").into())
            }
            _ => tokio::time::sleep(Duration::from_millis(750)).await,
        }
    }
}

/// Mark inference jobs orphaned by a process restart as `failed`. A single API
/// replica means anything still `queued`/`running` at boot was abandoned when
/// the previous process exited; the client then recovers via a retry instead of
/// waiting on an SSE event that will never arrive. Run once at startup.
pub async fn reconcile_orphaned_inference(state: &AppState) -> crate::error::Result<u64> {
    let res = sqlx::query(
        "UPDATE agent_decisions SET status = 'failed', error = 'interrupted by restart', completed_at = now() WHERE status IN ('queued', 'running')",
    )
    .execute(&state.db)
    .await?;
    let n = res.rows_affected();
    if n > 0 {
        tracing::warn!(
            count = n,
            "reconciled orphaned inference jobs to failed at boot"
        );
    }
    Ok(n)
}

/// Row shape for the allocation-proposal ownership/state lookup in
/// [`apply_allocation`]: (portfolio_id, kind, recommended_allocation, applied_at).
type AllocationDecisionRow = (
    Uuid,
    Option<String>,
    Option<serde_json::Value>,
    Option<chrono::DateTime<chrono::Utc>>,
    // The regime the proposal was clamped under (so apply re-clamps identically).
    Option<String>,
);

/// Apply an approved allocation proposal: write the agent's target into
/// `portfolios.goal.targetAllocation` and seed `allocations.target_weight`.
/// This is the only path (besides onboarding's empty create) that writes the
/// target — the user owns this decision via Gate-1 approval. Idempotent: a
/// proposal already applied just returns the current portfolio.
pub async fn apply_allocation(
    state: &AppState,
    decision_id: Uuid,
    user_id: Uuid,
) -> crate::error::Result<Portfolio> {
    Ok(apply_allocation_once(state, decision_id, user_id)
        .await?
        .portfolio)
}

pub(crate) struct AllocationApplyOutcome {
    pub portfolio: Portfolio,
    pub newly_applied: bool,
}

pub(crate) async fn apply_allocation_once(
    state: &AppState,
    decision_id: Uuid,
    user_id: Uuid,
) -> crate::error::Result<AllocationApplyOutcome> {
    use crate::modules::rebalance::registry::allocation_target_symbols;

    let row: Option<AllocationDecisionRow> = sqlx::query_as(
        r#"SELECT d.portfolio_id, d.kind, d.recommended_allocation,
                  d.allocation_applied_at, d.regime
           FROM agent_decisions d
           JOIN portfolios p ON p.id = d.portfolio_id
           WHERE d.id = $1 AND p.user_id = $2"#,
    )
    .bind(decision_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    let (portfolio_id, kind, rec_alloc, applied_at, decision_regime) =
        row.ok_or_else(|| crate::error::AppError::NotFound(format!("decision {decision_id}")))?;

    if kind.as_deref() != Some("allocation_proposal") {
        return Err(crate::error::AppError::BadRequest(
            "decision is not an allocation proposal".into(),
        ));
    }

    // Idempotent: already applied → return the current portfolio unchanged.
    if applied_at.is_some() {
        let p = sqlx::query_as::<_, Portfolio>("SELECT * FROM portfolios WHERE id = $1")
            .bind(portfolio_id)
            .fetch_one(&state.db)
            .await?;
        return Ok(AllocationApplyOutcome {
            portfolio: p,
            newly_applied: false,
        });
    }

    // Re-clamp the stored allocation against the designable target universe +
    // user guardrails. This is defense in depth for older/stale proposals while
    // preserving the user's approved target; execution blockers are surfaced by
    // the Gate-2 route safety check instead of hidden by rewriting to USDC.
    let raw = rec_alloc
        .as_ref()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    let user_profile = fetch_user_profile(state, user_id).await?;
    let target_universe = allocation_target_symbols(&state.config);
    // Use the *decision's own regime* so the guardrails match the ones the
    // proposal was clamped under — otherwise a regime-tilted proposal (e.g. a
    // wider risk_on single-asset cap) would be silently re-tightened under a
    // neutral baseline, making the adopted target diverge from the one the
    // user/auto-pilot saw.
    let clamped = clamp_allocation(
        &raw,
        &target_universe,
        derive_guardrails(
            &user_profile.risk_tolerance,
            decision_regime.as_deref().unwrap_or("neutral"),
            0.0,
        ),
    );

    let target_obj: serde_json::Map<String, serde_json::Value> = clamped
        .iter()
        .map(|(k, v)| (k.clone(), json!(round2(*v))))
        .collect();
    let target_symbols: Vec<String> = clamped.keys().cloned().collect();

    let mut tx = state.db.begin().await?;

    // Merge targetAllocation into the existing goal JSONB (preserve other keys).
    sqlx::query(
        "UPDATE portfolios
            SET goal = jsonb_set(
                    COALESCE(goal, '{}'::jsonb),
                    '{targetAllocation}',
                    $2::jsonb,
                    true
                ),
                updated_at = NOW()
          WHERE id = $1",
    )
    .bind(portfolio_id)
    .bind(serde_json::Value::Object(target_obj))
    .execute(&mut *tx)
    .await?;

    // Sync routePreferences.tokens to the agent's allocated symbols. The planner
    // filters the target allocation by routePreferences.tokens; the onboarding
    // wizard defaults that to ["USDC"], so without this an agent-chosen token
    // (e.g. ETH) is silently dropped and the rebalance plans nothing. Approving
    // the agent's allocation = enabling its tokens. Merge with `||` so other
    // routePreferences keys (networks, watchlist) are preserved.
    sqlx::query(
        "UPDATE portfolios
            SET goal = jsonb_set(
                    COALESCE(goal, '{}'::jsonb),
                    '{routePreferences}',
                    COALESCE(goal->'routePreferences', '{}'::jsonb)
                        || jsonb_build_object('tokens', $2::jsonb),
                    true
                )
          WHERE id = $1",
    )
    .bind(portfolio_id)
    .bind(json!(target_symbols))
    .execute(&mut *tx)
    .await?;

    // Seed allocations.target_weight (mirrors portfolio::handlers::create).
    for (symbol, weight) in &clamped {
        sqlx::query(
            "INSERT INTO allocations
                (id, portfolio_id, asset_symbol, quantity, target_weight, current_weight, value_usd)
             VALUES ($1, $2, $3, 0, $4, 0, 0)
             ON CONFLICT (portfolio_id, asset_symbol) DO UPDATE
                SET target_weight = EXCLUDED.target_weight,
                    updated_at = NOW()",
        )
        .bind(Uuid::new_v4())
        .bind(portfolio_id)
        .bind(symbol)
        .bind(*weight)
        .execute(&mut *tx)
        .await?;
    }
    // Zero out targets no longer in the allocation, then prune empty rows.
    sqlx::query(
        "UPDATE allocations SET target_weight = 0, updated_at = NOW()
         WHERE portfolio_id = $1 AND asset_symbol <> ALL($2)",
    )
    .bind(portfolio_id)
    .bind(&target_symbols)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM allocations
         WHERE portfolio_id = $1 AND target_weight = 0 AND quantity = 0
           AND value_usd = 0 AND asset_symbol <> ALL($2)",
    )
    .bind(portfolio_id)
    .bind(&target_symbols)
    .execute(&mut *tx)
    .await?;

    let apply_claim = sqlx::query(
        "UPDATE agent_decisions
         SET allocation_applied_at = NOW()
         WHERE id = $1 AND allocation_applied_at IS NULL",
    )
    .bind(decision_id)
    .execute(&mut *tx)
    .await?;
    let newly_applied = apply_claim.rows_affected() > 0;

    tx.commit().await?;

    let p = sqlx::query_as::<_, Portfolio>("SELECT * FROM portfolios WHERE id = $1")
        .bind(portfolio_id)
        .fetch_one(&state.db)
        .await?;
    let decision: AgentDecision = sqlx::query_as("SELECT * FROM agent_decisions WHERE id = $1")
        .bind(decision_id)
        .fetch_one(&state.db)
        .await?;
    if newly_applied {
        broadcast_agent_decision(state, user_id, &decision);
    }
    Ok(AllocationApplyOutcome {
        portfolio: p,
        newly_applied,
    })
}

// ── Context builders ───────────────────────────────────────────────────────

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

// ── Strategist response shape ──────────────────────────────────────────────

#[derive(Deserialize, serde::Serialize, Debug, Clone)]
struct StrategistProposal {
    #[serde(default)]
    reasoning: String,
    #[serde(default)]
    confidence: f64,
    #[serde(default = "default_recommendation")]
    recommendation: serde_json::Value,
    /// Allocator output (`PromptKey::Allocator`): the proposed target weights
    /// {symbol: pct}. Absent on `rebalance`/strategist proposals.
    #[serde(default, rename = "recommendedAllocation")]
    recommended_allocation: Option<serde_json::Map<String, serde_json::Value>>,
    /// Allocator's expected max drawdown for the proposed mix (percent).
    #[serde(default, rename = "expectedMaxDrawdownPct")]
    expected_max_drawdown_pct: Option<f64>,
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
        recommended_allocation: None,
        expected_max_drawdown_pct: None,
    })
}

#[derive(Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CriticOutput {
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

impl CriticOutput {
    /// True when the verdict cites no violated constitution clauses. The
    /// autonomous (auto-pilot) path uses this to refuse to adopt/execute a
    /// flagged allocation.
    pub(crate) fn constitution_clean(&self) -> bool {
        self.clause_ids.is_empty()
    }
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

    constitution::evaluate(
        constitution,
        &proposal,
        tier,
        portfolio.total_value_usd.to_f64().unwrap_or(0.0),
    )
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

#[cfg(test)]
mod tests {
    use super::super::decision_context::{
        build_strategist_context, format_goal_block, format_route_capabilities,
    };
    use super::*;
    use crate::modules::market_data::MarketSnapshot;

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
        use rust_decimal_macros::dec;
        let allocs = vec![Allocation {
            id: Uuid::nil(),
            portfolio_id: Uuid::nil(),
            asset_symbol: "BTC".into(),
            quantity: dec!(0.5),
            target_weight: 50.0,
            current_weight: 55.0,
            value_usd: dec!(30000.0),
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
        use rust_decimal_macros::dec;
        Portfolio {
            id: Uuid::nil(),
            user_id: Uuid::nil(),
            name: "Retirement".into(),
            total_value_usd: dec!(12345.67),
            total_pnl_usd: dec!(234.50),
            total_pnl_pct: 1.9,
            risk_score: 50,
            goal: serde_json::json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn sample_allocs() -> Vec<Allocation> {
        use rust_decimal_macros::dec;
        vec![
            Allocation {
                id: Uuid::nil(),
                portfolio_id: Uuid::nil(),
                asset_symbol: "BTC".into(),
                quantity: dec!(0.5),
                target_weight: 60.0,
                current_weight: 55.0,
                value_usd: dec!(33000.0),
            },
            Allocation {
                id: Uuid::nil(),
                portfolio_id: Uuid::nil(),
                asset_symbol: "ETH".into(),
                quantity: dec!(4.0),
                target_weight: 40.0,
                current_weight: 45.0,
                value_usd: dec!(14000.0),
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
        ctx.insert(
            "route_capabilities",
            format_route_capabilities(&crate::config::test_config()),
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
            recommended_allocation: None,
            expected_max_drawdown_pct: None,
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
    fn allocator_prompt_renders_without_unresolved_placeholders() {
        let reg = PromptRegistry::embedded();
        let mut ctx = build_strategist_context(
            &sample_portfolio(),
            &sample_allocs(),
            &sample_user(),
            &sample_snapshot(),
            &sample_regime(),
            &sample_risk(),
        );
        // Extra inserts the allocator path supplies at runtime.
        ctx.insert("memory", "(none)".into());
        ctx.insert("usyc_rate", "0.0510".into());
        ctx.insert("usdc_eurc_basis", "0.92".into());
        ctx.insert("goal_block", "grow · horizon 5y · risk moderate".into());
        ctx.insert("objective", "grow".into());
        ctx.insert("wallet_block", "Total USDC: 1000.00".into());
        ctx.insert("route_capabilities", "Executable now: USDC, cbBTC".into());
        ctx.insert("harvestable_losses", "(none)".into());
        let rendered = reg.render(PromptKey::Allocator, &ctx);
        assert!(
            !rendered.contains("{{"),
            "allocator prompt has unresolved placeholder(s):\n{rendered}"
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
            recommended_allocation: None,
            expected_max_drawdown_pct: None,
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
