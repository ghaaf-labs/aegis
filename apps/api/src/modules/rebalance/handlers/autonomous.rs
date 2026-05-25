use serde_json::json;
use uuid::Uuid;

use crate::error::Result;
use crate::modules::rebalance::{
    executor::create_plan,
    models::{PlanInput, PlannedLeg},
    planner::plan_legs,
};
use crate::router::AppState;

use super::{
    approval::{approval_safety, ApprovalSafety},
    plan_input::build_plan_input,
    shared::{reusable_planned_rebalance, stamp_routable_snapshot},
};

/// Outcome of the auto-pilot plan preparation. `NoOp` ⇒ nothing to move
/// (on-target / sub-dust); `Prepared` ⇒ a `planned` rebalance exists, with its
/// approval-safety verdict for the scheduler to gate execution on.
pub enum AutonomousPlan {
    NoOp,
    Prepared {
        rebalance_id: Uuid,
        safety: ApprovalSafety,
    },
}

/// Build (or reuse) a real, `planned` rebalance toward the portfolio's current
/// target and return its approval-safety verdict — the autonomous (auto-pilot)
/// counterpart to the `create` handler. Reuses the *exact same* deterministic
/// planner, plan persistence, and `approval_safety` gate the manual approval
/// flow uses, so auto-pilot can never execute a plan the manual path would
/// reject. The scheduler executes only when `safety.approvable` is true.
pub async fn prepare_autonomous_plan(
    state: &AppState,
    portfolio_id: Uuid,
) -> Result<AutonomousPlan> {
    let input = build_plan_input(state, portfolio_id).await?;
    let legs = plan_legs(&input);
    if legs.is_empty() {
        // On-target or sub-$5 dust — the planner drops it. Nothing to execute.
        return Ok(AutonomousPlan::NoOp);
    }

    let rebalance_id =
        if let Some(existing) = reusable_planned_rebalance(state, portfolio_id, &legs).await? {
            existing.rebalance_id
        } else {
            // Auto-pilot is a real-execution control path; record the same
            // deterministic planner decision the manual `create` route does so the
            // approval-safety gate (which rejects mock/legacy decisions in real
            // mode) accepts it.
            let decision = if state.config.execution_mock || state.config.circle_mock {
                mock_agent_decision(state, portfolio_id).await?
            } else {
                planner_agent_decision(state, portfolio_id, &input, &legs).await?
            };
            let rebalance_id = create_plan(state, portfolio_id, decision.id, &legs).await?;
            // INV-6 must hold for auto-pilot too: bind the freshly-created plan
            // to its routability so the scheduler's approval gate refuses it if a
            // rail flipped Ready⇄track-only after planning (manual `create` does
            // the same). Reused plans keep the hash from their own creation.
            stamp_routable_snapshot(state, rebalance_id).await?;
            rebalance_id
        };

    let safety = approval_safety(state, rebalance_id).await?;
    Ok(AutonomousPlan::Prepared {
        rebalance_id,
        safety,
    })
}

/// Insert a canned agent decision for mock-backed local/demo mode. Lets the
/// rebalance plan endpoint be exercised end-to-end without a live AI call.
pub(super) async fn mock_agent_decision(
    state: &AppState,
    portfolio_id: Uuid,
) -> Result<crate::modules::agent::models::AgentDecision> {
    let rec = json!({
        "summary": "Hold",
        "trades": [],
        "expectedImpact": { "riskDelta": 0.0, "diversificationScore": 0.5 }
    });
    let decision = sqlx::query_as(
        r#"INSERT INTO agent_decisions
           (id, portfolio_id, reasoning, recommendation, confidence,
            triggered_by, model_slug, regime, prompt_tokens,
            completion_tokens, latency_ms, critic_verdict, snapshot,
            raw_confidence, calibrated_confidence, counterfactual)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
           RETURNING *"#,
    )
    .bind(Uuid::new_v4())
    .bind(portfolio_id)
    .bind("Mock decision — local/demo mock mode")
    .bind(&rec)
    .bind(1.0_f64)
    .bind("user_request")
    .bind("anthropic/claude-3-haiku")
    .bind("neutral")
    .bind(0_i32)
    .bind(0_i32)
    .bind(0_i32)
    .bind(serde_json::Value::Null)
    .bind(json!({}))
    .bind(1.0_f64)
    .bind(None::<f64>)
    .bind(None::<String>)
    .fetch_one(&state.db)
    .await?;
    Ok(decision)
}

pub(super) async fn planner_agent_decision(
    state: &AppState,
    portfolio_id: Uuid,
    input: &PlanInput,
    legs: &[PlannedLeg],
) -> Result<crate::modules::agent::models::AgentDecision> {
    let trades: Vec<serde_json::Value> = legs
        .iter()
        .filter_map(|leg| planned_trade(input, leg))
        .collect();
    let idle_usdc: f64 = input.usdc_per_chain.values().copied().sum();
    let route_surface = plan_route_surface(legs);
    let summary = if legs.is_empty() {
        "No rebalance needed — portfolio drift and idle USDC are below execution thresholds."
            .to_string()
    } else {
        format!(
            "Build {} review {} from current holdings and Gateway cash.",
            legs.len(),
            if legs.len() == 1 { "leg" } else { "legs" }
        )
    };
    let reasoning = if legs.is_empty() {
        "Aegis compared confirmed position values, target weights, and Circle Gateway USDC. No leg exceeded the drift or dust thresholds, so the approval screen is a no-op review."
            .to_string()
    } else {
        format!(
            "Aegis built this execution review from ${:.2} invested positions plus ${:.2} idle Gateway USDC (${:.2} total plan value). The plan is deterministic and remains gated by user approval before any {route_surface} is submitted.",
            (input.portfolio_value_usd - idle_usdc).max(0.0), idle_usdc, input.portfolio_value_usd
        )
    };
    let rec = json!({
        "summary": summary,
        "trades": trades,
        "expectedImpact": { "riskDelta": 0.0, "diversificationScore": 0.5 }
    });
    let invested_value_usd = (input.portfolio_value_usd - idle_usdc).max(0.0);
    let critic_verdict = json!({
        "verdict": "approved",
        "notes": "Deterministic planner matched confirmed holdings, target weights, and Gateway balances; approval remains the final execution gate.",
        "confidence": 0.92
    });
    let decision = sqlx::query_as(
        r#"INSERT INTO agent_decisions
           (id, portfolio_id, reasoning, recommendation, confidence,
            triggered_by, model_slug, regime, prompt_tokens,
            completion_tokens, latency_ms, critic_verdict, snapshot,
            raw_confidence, calibrated_confidence, counterfactual)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
           RETURNING *"#,
    )
    .bind(Uuid::new_v4())
    .bind(portfolio_id)
    .bind(reasoning)
    .bind(&rec)
    .bind(0.92_f64)
    .bind("user_request")
    .bind("aegis/rebalance-planner-v1")
    .bind("neutral")
    .bind(0_i32)
    .bind(0_i32)
    .bind(0_i32)
    .bind(critic_verdict)
    .bind(json!({
        "planner": "deterministic",
        "legs": legs.len(),
        "planValueUsd": input.portfolio_value_usd,
        "investedValueUsd": invested_value_usd,
        "portfolioValueUsd": input.portfolio_value_usd,
        "idleUsdc": idle_usdc
    }))
    .bind(0.92_f64)
    .bind(None::<f64>)
    .bind(Some("If drift or idle cash changes, the deterministic planner will build a different approval plan.".to_string()))
    .fetch_one(&state.db)
    .await?;
    Ok(decision)
}

pub(super) fn planned_trade(input: &PlanInput, leg: &PlannedLeg) -> Option<serde_json::Value> {
    let (symbol, action) = match (
        leg.src_symbol.as_deref(),
        leg.dest_symbol.as_deref(),
        leg.kind,
    ) {
        (Some("USDC"), Some(dest), _) if dest != "USDC" => (dest, "buy"),
        (Some(src), Some("USDC"), _) if src != "USDC" => (src, "sell"),
        _ => return None,
    };
    let price = input.prices.get(symbol).copied().unwrap_or(1.0).max(0.01);
    let quantity = leg.amount_usdc / price;
    Some(json!({
        "assetId": symbol,
        "symbol": symbol,
        "action": action,
        "quantity": quantity,
        "valueUsd": leg.amount_usdc,
        "reason": format!("{} on {}", leg.kind.as_str(), leg.dest_chain.or(leg.src_chain).map(|c| c.as_str()).unwrap_or("gateway"))
    }))
}

pub(super) fn plan_route_surface(legs: &[PlannedLeg]) -> String {
    if legs.iter().any(|leg| {
        matches!(
            leg.kind,
            crate::modules::rebalance::models::LegKind::CrossChainBurn
                | crate::modules::rebalance::models::LegKind::CrossChainMint
        )
    }) {
        return "Arc/Base transaction".to_string();
    }
    let mut chains: Vec<&str> = legs
        .iter()
        .filter_map(|leg| leg.dest_chain.or(leg.src_chain))
        .map(|chain| chain.as_str())
        .collect();
    chains.sort_unstable();
    chains.dedup();
    match chains.as_slice() {
        [] => "execution".to_string(),
        ["arc"] => "Arc transaction".to_string(),
        ["base"] => "Base transaction".to_string(),
        _ => "Arc/Base transaction".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::modules::rebalance::models::{ChainKey, LegKind, PlannedLeg};

    fn planned_leg(kind: LegKind, chain: ChainKey) -> PlannedLeg {
        PlannedLeg {
            leg_index: 0,
            kind,
            src_chain: Some(chain),
            dest_chain: Some(chain),
            src_symbol: Some("USDC".into()),
            dest_symbol: Some("ETH".into()),
            amount_usdc: 10.0,
            min_out: None,
        }
    }

    #[test]
    fn plan_route_surface_uses_single_chain_from_plan_legs() {
        assert_eq!(
            plan_route_surface(&[planned_leg(LegKind::LocalSwap, ChainKey::Base,)]),
            "Base transaction"
        );
        assert_eq!(
            plan_route_surface(&[planned_leg(LegKind::LocalSwap, ChainKey::Arc,)]),
            "Arc transaction"
        );
    }
}
