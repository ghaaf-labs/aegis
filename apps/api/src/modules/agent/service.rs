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
use super::models::{AgentDecision, AnalyzeRequest, ProposeAllocationRequest};
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
    // Route-execution awareness: the strategist must only propose moving funds
    // into tokens that can actually execute. Track-only tokens (disabled USYC,
    // KYB-gated EURC, volatiles without a live swap route) may be discussed but
    // not traded — the registry would otherwise block them at approval/execute.
    strategist_ctx.insert(
        "route_capabilities",
        format_route_capabilities(&state.config),
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
            kind: decision.kind.clone(),
            recommended_allocation: decision.recommended_allocation.clone(),
        }));

    Ok(decision)
}

// ── Agent-decided allocation (the headline) ─────────────────────────────────

/// Structural + risk guardrails the clamp enforces. Phase 1 extends this with
/// regime/vol/correlation tilts; Phase 0 keeps the invariant set: a per-asset
/// cap on volatile sleeves (≤ the constitution's RISK-2 60%) plus a stable +
/// yield reserve floor derived from the user's risk tolerance.
#[derive(Debug, Clone, Copy)]
struct Guardrails {
    /// Max weight (%) for any single non-stable asset (≤ RISK-2 60%).
    single_asset_cap: f64,
    /// Minimum combined weight (%) for the stable/yield reserve sleeve.
    stable_floor: f64,
    /// Max combined weight (%) across ALL non-stable assets. This is the
    /// correlation-diversification guardrail: cbBTC/WETH/cbETH move together,
    /// so the whole crypto sleeve is bounded, not just each leg.
    volatile_cluster_cap: f64,
}

/// Regime/vol-aware guardrails. Base caps/floors come from risk tolerance;
/// `risk_off` raises the stable+yield floor and trims volatile caps, `risk_on`
/// loosens them, and high BTC realized vol scales the volatile sleeve down.
fn derive_guardrails(risk_tolerance: &str, regime: &str, btc_vol_30d: f64) -> Guardrails {
    let (mut single_asset_cap, mut stable_floor, mut volatile_cluster_cap): (f64, f64, f64) =
        match risk_tolerance.to_lowercase().as_str() {
            "aggressive" => (60.0, 5.0, 90.0),
            "moderate" => (45.0, 20.0, 70.0),
            // conservative (also the safe default for unknown values)
            _ => (25.0, 50.0, 45.0),
        };
    match regime.to_lowercase().as_str() {
        "risk_off" => {
            stable_floor = (stable_floor + 20.0).min(80.0);
            volatile_cluster_cap = (volatile_cluster_cap - 20.0).max(10.0);
            single_asset_cap = single_asset_cap.min(40.0);
        }
        "risk_on" => {
            stable_floor = (stable_floor - 5.0).max(5.0);
            volatile_cluster_cap = (volatile_cluster_cap + 10.0).min(95.0);
        }
        _ => {}
    }
    // High realized vol → trim the volatile sleeve further.
    if btc_vol_30d > 0.8 {
        volatile_cluster_cap = (volatile_cluster_cap - 15.0).max(10.0);
        single_asset_cap = (single_asset_cap - 10.0).max(10.0);
    }
    // RISK-2 hard ceiling regardless of inputs.
    single_asset_cap = single_asset_cap.min(60.0);
    Guardrails {
        single_asset_cap,
        stable_floor,
        volatile_cluster_cap,
    }
}

const STABLE_SYMBOLS: &[&str] = &["USDC", "sUSDS", "USYC", "EURC", "aUSDC"];

fn is_stable_symbol(sym: &str) -> bool {
    STABLE_SYMBOLS.iter().any(|s| s.eq_ignore_ascii_case(sym))
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// Deterministic safety net: turn a raw LLM allocation map into a valid target.
/// Drops non-executable tokens (USDC always allowed) and non-positive weights,
/// caps any single non-stable asset at the guardrail cap, enforces the
/// stable/yield reserve floor, normalizes to sum 100, and sweeps the rounding
/// residual into USDC. Always returns a non-empty map summing to ~100 (USDC-only
/// in the worst case) — so a bad/refused LLM output can never produce an invalid
/// or over-concentrated target.
fn clamp_allocation(
    raw: &serde_json::Map<String, serde_json::Value>,
    executable: &[&str],
    guardrails: Guardrails,
) -> std::collections::BTreeMap<String, f64> {
    use std::collections::BTreeMap;
    let usdc_only =
        || -> BTreeMap<String, f64> { std::iter::once(("USDC".to_string(), 100.0)).collect() };
    let exec_ok = |sym: &str| {
        sym.eq_ignore_ascii_case("USDC") || executable.iter().any(|e| e.eq_ignore_ascii_case(sym))
    };

    // 1. Keep executable, positive weights (canonicalize USDC casing).
    let mut weights: BTreeMap<String, f64> = BTreeMap::new();
    for (sym, v) in raw {
        let w = v.as_f64().unwrap_or(0.0);
        if w <= 0.0 || !exec_ok(sym) {
            continue;
        }
        let key = if sym.eq_ignore_ascii_case("USDC") {
            "USDC".to_string()
        } else {
            sym.clone()
        };
        *weights.entry(key).or_insert(0.0) += w;
    }
    let total: f64 = weights.values().sum();
    if total <= 0.0 {
        return usdc_only();
    }

    // 2. Normalize to sum 100.
    for w in weights.values_mut() {
        *w = (*w / total) * 100.0;
    }

    // 3. Cap each non-stable asset; route the excess into USDC (a stable, so
    //    this can never re-violate a cap).
    let mut excess = 0.0;
    for (k, w) in weights.iter_mut() {
        if !is_stable_symbol(k) && *w > guardrails.single_asset_cap {
            excess += *w - guardrails.single_asset_cap;
            *w = guardrails.single_asset_cap;
        }
    }
    if excess > 0.0 {
        *weights.entry("USDC".to_string()).or_insert(0.0) += excess;
    }

    // 3b. Cap the combined non-stable (correlated) cluster; route the excess
    //     into USDC. Scaling down can't re-violate the single-asset cap.
    let cluster_sum: f64 = weights
        .iter()
        .filter(|(k, _)| !is_stable_symbol(k))
        .map(|(_, w)| *w)
        .sum();
    if cluster_sum > guardrails.volatile_cluster_cap && cluster_sum > 0.0 {
        let scale = guardrails.volatile_cluster_cap / cluster_sum;
        let mut moved = 0.0;
        for (k, w) in weights.iter_mut() {
            if !is_stable_symbol(k) {
                let nw = *w * scale;
                moved += *w - nw;
                *w = nw;
            }
        }
        *weights.entry("USDC".to_string()).or_insert(0.0) += moved;
    }

    // 4. Enforce the stable/yield reserve floor by scaling non-stables DOWN
    //    proportionally (never up — so caps still hold) and topping up USDC.
    let stable_sum: f64 = weights
        .iter()
        .filter(|(k, _)| is_stable_symbol(k))
        .map(|(_, w)| *w)
        .sum();
    if stable_sum < guardrails.stable_floor {
        let nonstable_sum: f64 = weights
            .iter()
            .filter(|(k, _)| !is_stable_symbol(k))
            .map(|(_, w)| *w)
            .sum();
        let deficit = (guardrails.stable_floor - stable_sum).min(nonstable_sum);
        if nonstable_sum > 0.0 && deficit > 0.0 {
            let scale = (nonstable_sum - deficit) / nonstable_sum;
            for (k, w) in weights.iter_mut() {
                if !is_stable_symbol(k) {
                    *w *= scale;
                }
            }
            *weights.entry("USDC".to_string()).or_insert(0.0) += deficit;
        }
    }

    // 5. Round to 0.01 and sweep the residual into USDC so the map sums to 100.
    for w in weights.values_mut() {
        *w = round2(*w);
    }
    let sum: f64 = weights.values().sum();
    let residual = round2(100.0 - sum);
    if residual.abs() >= 0.01 {
        let usdc = weights.entry("USDC".to_string()).or_insert(0.0);
        *usdc = round2((*usdc + residual).max(0.0));
    }
    weights.retain(|_, w| *w > 0.0);
    if weights.is_empty() {
        return usdc_only();
    }
    weights
}

/// Read the user's high-level objective from the goal JSONB (set at onboarding).
fn goal_objective(goal: &serde_json::Value) -> String {
    goal.get("objective")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("grow")
        .to_string()
}

/// Run the allocator: the agent designs a full target allocation from the
/// user's objective/horizon/risk + market regime, deterministically clamped to
/// a valid, executable, non-over-concentrated target. Persists an
/// `allocation_proposal` decision and broadcasts it over SSE. The user approves
/// it via [`apply_allocation`].
pub async fn propose_allocation(
    state: &AppState,
    req: ProposeAllocationRequest,
) -> crate::error::Result<AgentDecision> {
    use crate::modules::rebalance::registry::{
        capabilities::RuntimeCapabilities, executable_token_symbols,
    };
    let start = Instant::now();
    let triggered_by = req
        .triggered_by
        .clone()
        .unwrap_or_else(|| "allocation_proposal".to_string());

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

    let mut user_profile = fetch_user_profile(state, portfolio.user_id).await?;
    // The Gate-1 risk dial re-proposes at a different risk level without
    // mutating the stored goal/profile.
    if let Some(r) = req.risk_override.as_deref() {
        let r = r.trim().to_lowercase();
        if matches!(r.as_str(), "conservative" | "moderate" | "aggressive") {
            user_profile.risk_tolerance = r;
        }
    }

    let snapshot =
        crate::modules::market_data::service::fetch_snapshot(state.prices.as_ref()).await?;
    let ai = OpenRouterClient::new(&state.http, &state.config);

    let regime =
        risk_engine::classify(&ai, &snapshot, state.prompts.as_ref(), Some(&state.db)).await?;
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

    let risk = risk_engine::evaluate(&allocations, &snapshot.assets);
    let memory_block = memory::build_memory_block(&state.db, req.portfolio_id).await?;
    let usyc_rate = treasury::service::rate(&state.http, &state.config)
        .await
        .map(|r| r.annualized_yield)
        .unwrap_or(0.0510);
    let eurc_basis = fx::service::usdc_eurc_basis(state.prices.as_ref(), &state.config)
        .await
        .map(|b| b.mid_rate)
        .unwrap_or(0.92);

    let mut ctx = build_strategist_context(
        &portfolio,
        &allocations,
        &user_profile,
        &snapshot,
        &regime,
        &risk,
    );
    ctx.insert("memory", memory_block);
    ctx.insert("usyc_rate", format!("{:.4}", usyc_rate));
    ctx.insert("usdc_eurc_basis", format!("{:.4}", eurc_basis));
    ctx.insert("goal_block", format_goal_block(&portfolio.goal));
    ctx.insert("objective", goal_objective(&portfolio.goal));
    let gateway_block = match crate::modules::gateway::service::fetch_balance_for_user(
        &state.db,
        &state.http,
        &state.config,
        portfolio.user_id,
    )
    .await
    {
        Ok(b) => format_gateway_block(&b),
        Err(_) => "Wallet balance: unavailable (Gateway lookup failed).".to_string(),
    };
    ctx.insert("wallet_block", gateway_block);
    ctx.insert(
        "route_capabilities",
        format_route_capabilities(&state.config),
    );
    // Tax-loss harvesting (RFB #4): make the allocator aware of positions
    // sitting at an unrealized loss so it can prefer trimming them when moving
    // toward the new target. The realized harvest is surfaced for approval via
    // the existing rebalance plan (Gate 2) — nothing executes unapproved.
    let harvestable = crate::modules::tax::service::harvestable_losses(
        state,
        portfolio.user_id,
        req.portfolio_id,
    )
    .await
    .unwrap_or_default();
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
    ctx.insert(
        "harvestable_losses",
        format_harvestable_losses(&harvestable),
    );

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

    let caps = RuntimeCapabilities::from_config(&state.config);
    let executable = executable_token_symbols(&caps, &state.config);
    let guardrails = derive_guardrails(
        &user_profile.risk_tolerance,
        regime.regime.as_str(),
        regime.signals.btc_vol_30d,
    );

    // Conservative fallback instead of abstain: a low-confidence or empty
    // proposal falls back to the current target (re-clamped) or USDC-only,
    // never a no-op — the user always gets something to approve.
    let low_conf = parsed.confidence < ABSTAIN_CONFIDENCE_THRESHOLD;
    let raw_map = parsed.recommended_allocation.clone().unwrap_or_default();
    let clamped = if low_conf || raw_map.is_empty() {
        match portfolio
            .goal
            .get("targetAllocation")
            .and_then(|v| v.as_object())
        {
            Some(obj) if !obj.is_empty() => clamp_allocation(obj, &executable, guardrails),
            _ => std::iter::once(("USDC".to_string(), 100.0)).collect(),
        }
    } else {
        clamp_allocation(&raw_map, &executable, guardrails)
    };

    let reasoning = if low_conf {
        format!(
            "Low allocator confidence ({:.2}); proposing a conservative reserve allocation. {}",
            parsed.confidence, parsed.reasoning
        )
        .trim()
        .to_string()
    } else {
        parsed.reasoning.clone()
    };
    let confidence = parsed.confidence.clamp(0.0, 1.0);

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
            "expectedMaxDrawdownPct": parsed.expected_max_drawdown_pct,
        }),
        recommended_allocation: Some(alloc_obj.clone()),
        expected_max_drawdown_pct: parsed.expected_max_drawdown_pct,
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
        "expectedMaxDrawdownPct": parsed.expected_max_drawdown_pct,
    });
    let critic_value = serde_json::to_value(&verdict)?;
    let snapshot_value = build_decision_snapshot(&portfolio, &allocations, &snapshot);
    let latency_ms = start.elapsed().as_millis() as i32;

    let decision: AgentDecision = sqlx::query_as(
        r#"INSERT INTO agent_decisions
           (id, portfolio_id, reasoning, recommendation, confidence,
            triggered_by, model_slug, regime, prompt_tokens, completion_tokens,
            latency_ms, critic_verdict, snapshot, raw_confidence,
            calibrated_confidence, counterfactual, kind, recommended_allocation)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                   $15, $16, $17, $18)
           RETURNING *"#,
    )
    .bind(Uuid::new_v4())
    .bind(req.portfolio_id)
    .bind(&reasoning)
    .bind(&recommendation_value)
    .bind(confidence)
    .bind(&triggered_by)
    .bind(&resp.model_slug)
    .bind(regime.regime.as_str())
    .bind(resp.prompt_tokens as i32)
    .bind(resp.completion_tokens as i32)
    .bind(latency_ms)
    .bind(&critic_value)
    .bind(&snapshot_value)
    .bind(confidence)
    .bind(confidence)
    .bind(Option::<String>::None)
    .bind("allocation_proposal")
    .bind(&alloc_value)
    .fetch_one(&state.db)
    .await?;

    crate::modules::observability::counters::record_agent_decision();

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
            kind: decision.kind.clone(),
            recommended_allocation: decision.recommended_allocation.clone(),
        }));

    Ok(decision)
}

/// Row shape for the allocation-proposal ownership/state lookup in
/// [`apply_allocation`]: (portfolio_id, kind, recommended_allocation, applied_at).
type AllocationDecisionRow = (
    Uuid,
    Option<String>,
    Option<serde_json::Value>,
    Option<chrono::DateTime<chrono::Utc>>,
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
    use crate::modules::rebalance::registry::{
        capabilities::RuntimeCapabilities, executable_token_symbols,
    };

    let row: Option<AllocationDecisionRow> = sqlx::query_as(
        r#"SELECT d.portfolio_id, d.kind, d.recommended_allocation, d.allocation_applied_at
           FROM agent_decisions d
           JOIN portfolios p ON p.id = d.portfolio_id
           WHERE d.id = $1 AND p.user_id = $2"#,
    )
    .bind(decision_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    let (portfolio_id, kind, rec_alloc, applied_at) =
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
        return Ok(p);
    }

    // Re-clamp the stored allocation (defense in depth — never trust a stored
    // value blindly) against the current executable set + user guardrails.
    let raw = rec_alloc
        .as_ref()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    let user_profile = fetch_user_profile(state, user_id).await?;
    let caps = RuntimeCapabilities::from_config(&state.config);
    let executable = executable_token_symbols(&caps, &state.config);
    // Re-clamp with risk-only baseline guardrails (structural defense-in-depth;
    // the regime/vol tilts were already applied at propose time).
    let clamped = clamp_allocation(
        &raw,
        &executable,
        derive_guardrails(&user_profile.risk_tolerance, "neutral", 0.0),
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

    sqlx::query("UPDATE agent_decisions SET allocation_applied_at = NOW() WHERE id = $1")
        .bind(decision_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    let p = sqlx::query_as::<_, Portfolio>("SELECT * FROM portfolios WHERE id = $1")
        .bind(portfolio_id)
        .fetch_one(&state.db)
        .await?;
    Ok(p)
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

/// Render the route-execution capability block for the strategist prompt:
/// which tokens can actually be traded vs. which are price-tracked only.
fn format_route_capabilities(cfg: &crate::config::Config) -> String {
    use crate::modules::rebalance::registry::{
        capabilities::RuntimeCapabilities, executable_token_symbols, tokens::TOKEN_REGISTRY,
    };
    let caps = RuntimeCapabilities::from_config(cfg);
    let executable = executable_token_symbols(&caps, cfg);
    let tracked: Vec<&str> = TOKEN_REGISTRY
        .iter()
        .map(|s| s.symbol)
        .filter(|s| !executable.contains(s))
        .collect();
    format!(
        "- **Executable now** (you MAY propose buying/parking/selling these): {}\n\
         - **Track-only** (price-tracked but NOT executable — do NOT propose trades into these; mention as context only): {}",
        executable.join(", "),
        if tracked.is_empty() { "none".to_string() } else { tracked.join(", ") },
    )
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
        " · route scope: wallet-ready networks {networks}; rebalance execution rails ARC-TESTNET, BASE-SEPOLIA; wallet-sync queue {future_networks}; target tokens {tokens}; watch {watchlist}"
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
    fn clamp_allocation_enforces_cap_floor_executable_and_sum() {
        let raw = json!({ "USDC": 10.0, "cbBTC": 90.0, "SOL": 50.0 })
            .as_object()
            .unwrap()
            .clone();
        let executable = ["USDC", "cbBTC", "WETH"];
        let g = derive_guardrails("moderate", "neutral", 0.0); // cap 45, floor 20
        let out = clamp_allocation(&raw, &executable, g);

        // SOL is not executable → dropped.
        assert!(!out.contains_key("SOL"));
        // cbBTC respects the cap even after normalization.
        assert!(out.get("cbBTC").copied().unwrap_or(0.0) <= g.single_asset_cap + 0.05);
        // Sums to ~100.
        let sum: f64 = out.values().sum();
        assert!((sum - 100.0).abs() < 0.05, "sum={sum}");
        // Stable/yield floor is met.
        let stable: f64 = out
            .iter()
            .filter(|(k, _)| is_stable_symbol(k))
            .map(|(_, w)| *w)
            .sum();
        assert!(stable >= g.stable_floor - 0.05, "stable={stable}");
    }

    #[test]
    fn clamp_allocation_empty_or_garbage_falls_back_to_usdc() {
        let empty = serde_json::Map::new();
        let out = clamp_allocation(
            &empty,
            &["USDC"],
            derive_guardrails("conservative", "neutral", 0.0),
        );
        assert_eq!(out.get("USDC").copied(), Some(100.0));

        // All non-executable → still a valid USDC-only target.
        let garbage = json!({ "DOGE": 100.0 }).as_object().unwrap().clone();
        let out = clamp_allocation(
            &garbage,
            &["USDC", "cbBTC"],
            derive_guardrails("aggressive", "neutral", 0.0),
        );
        assert_eq!(out.get("USDC").copied(), Some(100.0));
    }

    #[test]
    fn derive_guardrails_tightens_in_risk_off_and_high_vol() {
        let base = derive_guardrails("aggressive", "neutral", 0.0);
        let off = derive_guardrails("aggressive", "risk_off", 0.0);
        assert!(off.stable_floor > base.stable_floor);
        assert!(off.volatile_cluster_cap < base.volatile_cluster_cap);
        let high_vol = derive_guardrails("aggressive", "neutral", 1.2);
        assert!(high_vol.volatile_cluster_cap < base.volatile_cluster_cap);
        // RISK-2 hard ceiling always holds.
        assert!(base.single_asset_cap <= 60.0);
    }

    #[test]
    fn clamp_allocation_caps_volatile_cluster() {
        // conservative + risk_off → a low cluster cap; an all-crypto raw is
        // heavily trimmed back into the stable reserve.
        let raw = json!({ "cbBTC": 50.0, "WETH": 50.0 })
            .as_object()
            .unwrap()
            .clone();
        let g = derive_guardrails("conservative", "risk_off", 0.0);
        let out = clamp_allocation(&raw, &["USDC", "cbBTC", "WETH"], g);
        let cluster: f64 = out
            .iter()
            .filter(|(k, _)| !is_stable_symbol(k))
            .map(|(_, w)| *w)
            .sum();
        assert!(
            cluster <= g.volatile_cluster_cap + 0.1,
            "cluster={cluster} cap={}",
            g.volatile_cluster_cap
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
