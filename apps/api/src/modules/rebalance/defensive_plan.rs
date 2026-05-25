use std::collections::HashMap;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::modules::rebalance::executor::replace_planned_review;
use crate::modules::rebalance::models::{ChainKey, PlanInput, PlannedLeg};
use crate::modules::rebalance::registry::RuntimeCapabilities;
use crate::modules::rebalance::snapshot::RoutableSnapshot;
use crate::router::AppState;

#[derive(Debug, Clone)]
pub struct DefensivePlanRequest {
    pub rule_id: Uuid,
    pub user_id: Uuid,
    pub portfolio_id: Option<Uuid>,
    pub asset: String,
    pub threshold_price: Decimal,
    pub action_kind: String,
    pub target_asset: Option<String>,
}

/// Build and persist a defensive rebalance plan for a peg rule. The risk monitor
/// owns event detection; this module owns rebalance-specific shaping and
/// persistence so peg defense uses the same planning boundary as manual reviews.
pub async fn propose_defensive_plan(
    state: &AppState,
    req: &DefensivePlanRequest,
) -> anyhow::Result<Option<Uuid>> {
    let Some((portfolio_id, total_value_usd, current_weights)) =
        load_defensive_portfolio(state, req).await?
    else {
        return Ok(None);
    };

    let depegged_asset = req.asset.to_uppercase();
    let target_asset = match req.target_asset.clone().map(|t| t.to_uppercase()) {
        Some(t)
            if !t.eq_ignore_ascii_case(&depegged_asset)
                && is_executable_stable(&state.config, &t) =>
        {
            t
        }
        _ => default_defensive_target(&state.config, &depegged_asset),
    };

    let Some((target_weights, depegged_weight)) =
        build_defensive_target(&current_weights, &depegged_asset, &target_asset)
    else {
        tracing::debug!(rule_id=%req.rule_id, %depegged_asset,
            "depegged asset weight < 1%; no defensive plan needed");
        return Ok(None);
    };

    let prices = recent_defensive_prices(state, &current_weights, &target_weights).await;
    let Some(usdc_per_chain) = fetch_usdc_pool(state, req).await else {
        return Ok(None);
    };

    let input = PlanInput {
        portfolio_value_usd: total_value_usd,
        current_weights,
        sell_sources: HashMap::new(),
        target_weights,
        usdc_per_chain,
        drift_threshold: 0.0,
        dust_threshold_usd: 5.0,
        prices,
        regime: Some("risk_off".to_string()),
    };

    let legs = defensive_plan_legs(&state.config, &input);
    if legs.is_empty() {
        tracing::debug!(rule_id=%req.rule_id, "planner produced no legs; no defensive plan persisted");
        return Ok(None);
    }

    let rebalance_id = persist_defensive_plan(
        state,
        portfolio_id,
        req,
        &target_asset,
        depegged_weight,
        &legs,
        &input.prices,
    )
    .await?;
    tracing::info!(
        rule_id=%req.rule_id,
        %rebalance_id,
        legs_count = legs.len(),
        action = %req.action_kind,
        "peg-defense plan persisted"
    );

    Ok(Some(rebalance_id))
}

pub(crate) fn defensive_plan_legs(
    cfg: &crate::config::Config,
    input: &PlanInput,
) -> Vec<PlannedLeg> {
    crate::modules::rebalance::routing::engine_plan(cfg, input).legs
}

async fn load_defensive_portfolio(
    state: &AppState,
    req: &DefensivePlanRequest,
) -> anyhow::Result<Option<(Uuid, f64, HashMap<String, f64>)>> {
    let portfolio_id: Option<Uuid> = match req.portfolio_id {
        Some(pid) => Some(pid),
        None => {
            sqlx::query_scalar(
                "SELECT id FROM portfolios WHERE user_id = $1 ORDER BY updated_at DESC LIMIT 1",
            )
            .bind(req.user_id)
            .fetch_optional(&state.db)
            .await?
        }
    };
    let Some(portfolio_id) = portfolio_id else {
        tracing::warn!(rule_id=%req.rule_id, user_id=%req.user_id, "peg rule has no portfolio; skipping defensive plan");
        return Ok(None);
    };

    let total_value_usd: f64 = sqlx::query_scalar(
        "SELECT total_value_usd::DOUBLE PRECISION FROM portfolios WHERE id = $1",
    )
    .bind(portfolio_id)
    .fetch_one(&state.db)
    .await?;

    let allocations: Vec<(String, f64)> = sqlx::query_as(
        "SELECT asset_symbol, current_weight::DOUBLE PRECISION FROM allocations WHERE portfolio_id = $1",
    )
    .bind(portfolio_id)
    .fetch_all(&state.db)
    .await?;

    let current_weights = allocations
        .into_iter()
        .map(|(sym, w)| (sym, w / 100.0))
        .collect();
    Ok(Some((portfolio_id, total_value_usd, current_weights)))
}

async fn recent_defensive_prices(
    state: &AppState,
    current_weights: &HashMap<String, f64>,
    target_weights: &HashMap<String, f64>,
) -> HashMap<String, f64> {
    let relevant_symbols: Vec<String> = current_weights
        .keys()
        .chain(target_weights.keys())
        .cloned()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    if relevant_symbols.is_empty() {
        return HashMap::new();
    }
    crate::modules::market_data::service::get_historical_prices(
        &state.db,
        &relevant_symbols,
        chrono::Utc::now(),
    )
    .await
    .unwrap_or_default()
}

async fn fetch_usdc_pool(
    state: &AppState,
    req: &DefensivePlanRequest,
) -> Option<HashMap<ChainKey, f64>> {
    match crate::modules::gateway::service::fetch_balance_for_user(
        &state.db,
        &state.http,
        &state.config,
        req.user_id,
    )
    .await
    {
        Ok(balance) => {
            let mut pool: HashMap<ChainKey, f64> = HashMap::new();
            pool.insert(ChainKey::Arc, 0.0);
            pool.insert(ChainKey::Base, 0.0);
            for (chain, amount) in balance.per_chain {
                if let Some(key) = ChainKey::parse(chain.to_lowercase().as_str()) {
                    pool.insert(key, amount);
                }
            }
            Some(pool)
        }
        Err(e) => {
            tracing::warn!(rule_id=%req.rule_id, error=%e, "peg defense: gateway balance unavailable; no defensive plan");
            None
        }
    }
}

async fn persist_defensive_plan(
    state: &AppState,
    portfolio_id: Uuid,
    req: &DefensivePlanRequest,
    target_asset: &str,
    depegged_weight: f64,
    legs: &[PlannedLeg],
    prices: &HashMap<String, f64>,
) -> anyhow::Result<Uuid> {
    let reasoning = format!(
        "Peg-defense: {asset} observed at or below {threshold:.4} for the configured window; \
         shifting {pct}% of portfolio from {asset} into {target}.",
        asset = req.asset.to_uppercase(),
        threshold = req.threshold_price.to_f64().unwrap_or(0.0),
        pct = (depegged_weight * 100.0).round() as i64,
        target = target_asset,
    );
    let decision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO agent_decisions
            (portfolio_id, reasoning, recommendation, confidence, triggered_by,
             model_slug, regime, prompt_tokens, completion_tokens, latency_ms,
             critic_verdict, snapshot, raw_confidence, counterfactual)
         VALUES ($1, $2, $3, 0.9, 'peg_alert',
                 'aegis/rebalance-planner-v1', 'risk_off', 0, 0, 0,
                 $4, $5, 0.9, $6)
         RETURNING id",
    )
    .bind(portfolio_id)
    .bind(&reasoning)
    .bind(serde_json::json!({
        "summary": "Peg-defense rebalance",
        "trades": [],
        "expectedImpact": { "riskDelta": -1.0, "diversificationScore": 0.5 }
    }))
    .bind(None::<serde_json::Value>)
    .bind(serde_json::json!({
        "planner": "deterministic",
        "trigger": "peg_alert",
        "legs": legs.len(),
        "targetAsset": target_asset,
    }))
    .bind(
        "If the peg recovers or route readiness changes, rebuild the review before approving."
            .to_string(),
    )
    .fetch_one(&state.db)
    .await?;

    let rebalance_id = replace_planned_review(state, portfolio_id, decision_id, legs).await?;
    let caps = RuntimeCapabilities::from_config(&state.config);
    let snapshot = RoutableSnapshot::capture_for_plan(&caps, &state.config, prices, legs);
    sqlx::query("UPDATE rebalances SET routable_snapshot_hash = $1 WHERE id = $2")
        .bind(snapshot.hash())
        .bind(rebalance_id)
        .execute(&state.db)
        .await?;
    Ok(rebalance_id)
}

const DEFENSIVE_STABLE_SYMBOLS: &[&str] = &["USDC", "EURC", "USYC"];

pub(crate) fn is_executable_stable(cfg: &crate::config::Config, symbol: &str) -> bool {
    use crate::modules::rebalance::registry::{
        capabilities::RuntimeCapabilities, executable_token_symbols,
    };
    if !DEFENSIVE_STABLE_SYMBOLS
        .iter()
        .any(|s| s.eq_ignore_ascii_case(symbol))
    {
        return false;
    }
    let caps = RuntimeCapabilities::from_config(cfg);
    executable_token_symbols(&caps, cfg)
        .iter()
        .any(|s| s.eq_ignore_ascii_case(symbol))
}

pub(crate) fn default_defensive_target(
    cfg: &crate::config::Config,
    depegged_asset: &str,
) -> String {
    const PREFERENCE: &[&str] = &["USYC", "EURC", "USDC"];
    for candidate in PREFERENCE {
        if candidate.eq_ignore_ascii_case(depegged_asset) {
            continue;
        }
        if is_executable_stable(cfg, candidate) {
            return (*candidate).to_string();
        }
    }
    "USDC".to_string()
}

pub(crate) fn build_defensive_target(
    current_weights: &HashMap<String, f64>,
    depegged_asset: &str,
    target_asset: &str,
) -> Option<(HashMap<String, f64>, f64)> {
    let depegged_weight = *current_weights.get(depegged_asset).unwrap_or(&0.0);
    if depegged_weight < 0.01 {
        return None;
    }
    let mut target_weights = current_weights.clone();
    target_weights.insert(depegged_asset.to_string(), 0.0);
    *target_weights
        .entry(target_asset.to_string())
        .or_insert(0.0) += depegged_weight;
    Some((target_weights, depegged_weight))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peg_rule_proposes_usdc_to_usyc() {
        let mut current = HashMap::new();
        current.insert("USDC".to_string(), 0.80);
        current.insert("BTC".to_string(), 0.20);

        let (target, moved) =
            build_defensive_target(&current, "USDC", "USYC").expect("80% > 1% → defensive plan");
        assert!((moved - 0.80).abs() < 1e-9);
        assert_eq!(target.get("USDC"), Some(&0.0));
        assert_eq!(target.get("USYC"), Some(&0.80));
        assert_eq!(target.get("BTC"), Some(&0.20));
        let total: f64 = target.values().sum();
        assert!((total - 1.0).abs() < 1e-9);
    }

    #[test]
    fn default_defensive_target_avoids_disabled_usyc() {
        let mut cfg = crate::config::test_config();
        cfg.execution_mock = false;
        cfg.circle_mock = false;
        cfg.chains[ChainKey::Arc.index()].private_key = "0xaa".into();
        cfg.chains[ChainKey::Base.index()].private_key = "0xbb".into();
        cfg.chains[ChainKey::Base.index()].usdc =
            "0x036CbD53842c5426634e7929541eC2318f3dCF7e".into();
        cfg.set_token_address(
            "EURC",
            ChainKey::Base,
            "0x60a3E35Cc302bFA44Cb288Bc5a4F316Fdb1adb42",
        );
        cfg.chains[ChainKey::Base.index()].swap_router =
            "0x1111111111111111111111111111111111111111".into();
        cfg.chains[ChainKey::Base.index()].swap_quoter =
            "0x2222222222222222222222222222222222222222".into();
        cfg.swap_liquid_tokens
            .insert(ChainKey::Base, vec!["ETH".into(), "EURC".into()]);

        assert!(!is_executable_stable(&cfg, "USYC"));
        assert_ne!(default_defensive_target(&cfg, "USDC"), "USYC");
        assert_ne!(default_defensive_target(&cfg, "EURC"), "USYC");
        assert!(is_executable_stable(&cfg, "USDC"));
        assert_eq!(default_defensive_target(&cfg, "EURC"), "USDC");
        if cfg!(feature = "real-swap") {
            assert!(is_executable_stable(&cfg, "EURC"));
            assert_eq!(default_defensive_target(&cfg, "USDC"), "EURC");
        }
    }

    #[test]
    fn defensive_plan_legs_use_routing_engine_dag() {
        let mut cfg = crate::config::test_config();
        cfg.execution_mock = false;
        cfg.circle_mock = false;
        cfg.usyc_enabled = true;
        cfg.usyc_token_arc = "0x1111111111111111111111111111111111111111".into();
        cfg.chains[ChainKey::Arc.index()].usdc =
            "0x2222222222222222222222222222222222222222".into();
        cfg.chains[ChainKey::Base.index()].usdc =
            "0x3333333333333333333333333333333333333333".into();
        let mut current_weights = HashMap::new();
        current_weights.insert("USDC".to_string(), 1.0);
        let mut target_weights = HashMap::new();
        target_weights.insert("USDC".to_string(), 0.0);
        target_weights.insert("USYC".to_string(), 1.0);
        let mut usdc_per_chain = HashMap::new();
        usdc_per_chain.insert(ChainKey::Arc, 1_000.0);
        let input = PlanInput {
            portfolio_value_usd: 1_000.0,
            current_weights,
            sell_sources: HashMap::new(),
            target_weights,
            usdc_per_chain,
            drift_threshold: 0.0,
            dust_threshold_usd: 5.0,
            prices: HashMap::new(),
            regime: Some("risk_off".into()),
        };

        let legs = defensive_plan_legs(&cfg, &input);

        assert!(legs.iter().any(|l| l.kind.as_str() == "park_usyc"));
        for leg in &legs {
            for dep in &leg.deps {
                assert!(
                    legs.iter().any(|candidate| candidate.leg_index == *dep),
                    "dependency {dep} must reference a real leg"
                );
            }
        }
    }

    #[test]
    fn peg_rule_no_op_when_depegged_asset_absent() {
        let mut current = HashMap::new();
        current.insert("BTC".to_string(), 1.0);

        assert!(build_defensive_target(&current, "USDC", "USYC").is_none());
    }

    #[test]
    fn peg_rule_no_op_when_depegged_weight_below_one_percent() {
        let mut current = HashMap::new();
        current.insert("USDC".to_string(), 0.005);
        current.insert("BTC".to_string(), 0.995);

        assert!(build_defensive_target(&current, "USDC", "USYC").is_none());
    }

    #[test]
    fn peg_rule_appends_to_existing_target_asset_weight() {
        let mut current = HashMap::new();
        current.insert("USDC".to_string(), 0.50);
        current.insert("USYC".to_string(), 0.20);
        current.insert("BTC".to_string(), 0.30);

        let (target, moved) = build_defensive_target(&current, "USDC", "USYC").expect("50% > 1%");

        assert!((moved - 0.50).abs() < 1e-9);
        assert_eq!(target.get("USDC"), Some(&0.0));
        assert!((target["USYC"] - 0.70).abs() < 1e-9);
        assert_eq!(target.get("BTC"), Some(&0.30));
    }
}
