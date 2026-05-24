//! Shared decision scaffolding for the agent service.
//!
//! Both entry points — [`analyze_portfolio`](super::service::analyze_portfolio)
//! (strategist → critic → revision) and
//! [`propose_allocation`](super::service::propose_allocation) (allocator →
//! clamp → constitution) — open with the same prefix: load the portfolio +
//! allocations + user profile, classify the regime (and broadcast the flip),
//! evaluate risk, pull the per-user memory / USYC rate / EURC basis, and
//! assemble the strategist/gateway prompt context. That work lives here in one
//! typed [`DecisionContext`] so the two flows can keep only their divergent
//! tails inline.

use std::collections::HashMap;

use uuid::Uuid;

use super::memory;
use crate::modules::ai::OpenRouterClient;
use crate::modules::fx;
use crate::modules::market_data::MarketSnapshot;
use crate::modules::portfolio::models::{Allocation, Portfolio};
use crate::modules::risk_engine::{self, RegimeClassification};
use crate::modules::sse::{RegimeFlip, RegimeSignals as SseRegimeSignals, SseEvent};
use crate::modules::treasury;
use crate::router::AppState;
use tracing::warn;

/// Everything both decision entry points need before they diverge: the loaded
/// portfolio/allocations/user, the classified regime + risk report, the market
/// snapshot, and the fully-assembled strategist prompt context (memory, USYC
/// rate, EURC basis, goal block, wallet/gateway block, route capabilities,
/// harvestable-loss block — all already inserted).
pub(super) struct DecisionContext {
    pub(super) portfolio: Portfolio,
    pub(super) allocations: Vec<Allocation>,
    pub(super) user_profile: UserProfile,
    pub(super) snapshot: MarketSnapshot,
    pub(super) regime: RegimeClassification,
    pub(super) risk: crate::modules::risk_engine::RiskReport,
    pub(super) strategist_ctx: HashMap<&'static str, String>,
    /// The tier the decision-cap gate resolved, when `enforce_cap` was set
    /// (the strategist path). `None` for the allocator path, which doesn't
    /// meter decisions.
    pub(super) tier: Option<crate::modules::billing::types::Tier>,
}

/// Load + classify + assemble the shared decision context for `portfolio_id`.
///
/// `risk_override` re-proposes at a different risk level without mutating the
/// stored profile (the Gate-1 risk dial in `propose_allocation`); the strategist
/// path passes `None`. `enforce_cap` runs the tier resolution + decision-cap
/// gate immediately after the portfolio loads (before any LLM call or SSE
/// broadcast) — the strategist path sets it; the allocator path does not.
///
/// Side effects mirror the original inline prefix exactly: the regime flip is
/// broadcast over SSE, and a `TaxHarvestProposed` event is emitted for every
/// open loss above the configured threshold.
pub(super) async fn build_decision_context(
    state: &AppState,
    portfolio_id: Uuid,
    risk_override: Option<&str>,
    enforce_cap: bool,
) -> crate::error::Result<DecisionContext> {
    // 1. Fetch portfolio + allocations + user (for risk tolerance + horizon).
    let portfolio: Portfolio = sqlx::query_as("SELECT * FROM portfolios WHERE id = $1")
        .bind(portfolio_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound(format!("portfolio {portfolio_id}")))?;

    let allocations: Vec<Allocation> = sqlx::query_as(
        "SELECT * FROM allocations WHERE portfolio_id = $1 ORDER BY current_weight DESC",
    )
    .bind(portfolio_id)
    .fetch_all(&state.db)
    .await?;

    // Tier gate + model routing: resolve once, use for both the cap check
    // and the strategist/critic model selection. When billing v2 is OFF we
    // keep the original Pro-equivalent pipeline so the golden path is
    // untouched. Runs before the regime LLM call + SSE so a capped user fails
    // fast (same ordering as the original inline prefix).
    let tier = if enforce_cap {
        let t = if state.config.billing_v2_enabled {
            let t = crate::middleware::tier::resolve_tier(&state.db, portfolio.user_id).await?;
            crate::middleware::tier::enforce_decision_cap(&state.db, portfolio.user_id, t).await?;
            t
        } else {
            crate::modules::billing::types::Tier::Pro
        };
        Some(t)
    } else {
        None
    };

    let mut user_profile = fetch_user_profile(state, portfolio.user_id).await?;
    // The Gate-1 risk dial re-proposes at a different risk level without
    // mutating the stored goal/profile.
    if let Some(r) = risk_override {
        let r = r.trim().to_lowercase();
        if matches!(r.as_str(), "conservative" | "moderate" | "aggressive") {
            user_profile.risk_tolerance = r;
        }
    }

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
        from: previous_regime(state, portfolio_id).await,
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
    let memory_block = memory::build_memory_block(&state.db, portfolio_id).await?;
    let usyc_rate = treasury::service::rate(&state.http, &state.config)
        .await
        .map(|r| r.annualized_yield)
        .unwrap_or(0.0510);
    let eurc_basis = fx::service::usdc_eurc_basis(state.prices.as_ref(), &state.config)
        .await
        .map(|b| b.mid_rate)
        .unwrap_or(0.92);

    // 4. Strategist proposal context.
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
    // Route-execution awareness: the strategist must only propose moving funds
    // into tokens that can actually execute. Track-only tokens (disabled USYC,
    // KYB-gated EURC, volatiles without a live swap route) may be discussed but
    // not traded — the registry would otherwise block them at approval/execute.
    strategist_ctx.insert(
        "route_capabilities",
        format_route_capabilities(&state.config),
    );

    let harvestable =
        crate::modules::tax::service::harvestable_losses(state, portfolio.user_id, portfolio_id)
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
                    portfolio_id,
                    allocation_id: loss.allocation_id,
                    symbol: loss.symbol.clone(),
                    unrealized_loss_usd: loss.unrealized_loss_usd,
                    proposed_at: chrono::Utc::now(),
                },
            ));
        }
    }

    Ok(DecisionContext {
        portfolio,
        allocations,
        user_profile,
        snapshot,
        regime,
        risk,
        strategist_ctx,
        tier,
    })
}

// ── User profile ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub(super) struct UserProfile {
    pub(super) risk_tolerance: String,
    pub(super) investment_horizon_months: i32,
}

pub(super) async fn fetch_user_profile(
    state: &AppState,
    user_id: Uuid,
) -> crate::error::Result<UserProfile> {
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

pub(super) async fn previous_regime(state: &AppState, portfolio_id: Uuid) -> Option<String> {
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

// ── Context builders ───────────────────────────────────────────────────────

/// Render the route-execution capability block for the strategist prompt:
/// which tokens can actually be traded vs. which are price-tracked only.
pub(super) fn format_route_capabilities(cfg: &crate::config::Config) -> String {
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

pub(super) fn build_strategist_context(
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
pub(super) fn format_goal_block(goal: &serde_json::Value) -> String {
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

/// Snapshot the portfolio's holdings + per-asset prices at the moment of the
/// decision. Persisted on `agent_decisions.snapshot` so the outcome compressor
/// can compute *real* 24h deltas (vs. cumulative PnL) and the diary can render
/// a counterfactual ("what if we'd done what we proposed?") instead of the
/// Sprint 3 `realized + 0.5` placeholder.
pub(super) fn build_decision_snapshot(
    portfolio: &Portfolio,
    allocations: &[Allocation],
    market: &MarketSnapshot,
) -> serde_json::Value {
    use serde_json::json;
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

pub(super) fn format_allocations(allocations: &[Allocation]) -> String {
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

/// Render harvestable losses as a human-readable block the strategist can
/// reason over. Empty list collapses to "(none)" so the placeholder still
/// resolves.
pub(super) fn format_harvestable_losses(losses: &[crate::modules::tax::HarvestableLoss]) -> String {
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
