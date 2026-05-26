use std::collections::HashMap;

use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::error::Result;
use crate::modules::rebalance::models::{PlanInput, SellSources};
use crate::modules::rebalance::registry::{rebalanceable_token_symbols, RuntimeCapabilities};
use crate::router::AppState;

use super::outcome::DeferredTarget;

mod reservation_input;
mod target_filters;
mod valuation;
mod wallet_holdings;

use reservation_input::{reserve_real_execution_usdc_buffer, subtract_active_reservations};
use target_filters::{apply_route_preferences_to_targets, fold_nonexecutable_targets_into_usdc};
use valuation::{derive_valuation, load_planning_prices, marked_allocation_value, ValuationMode};
use wallet_holdings::{
    frozen_holdings_value, load_gateway_balance, usdc_pool_from_balance,
    wallet_holding_values_by_chain, wallet_holdings_marked,
};

/// Build the planner input plus the targets that had to be deferred (held as
/// USDC reserve because they have no live route).
pub(super) async fn build_plan_input(
    state: &AppState,
    portfolio_id: Uuid,
) -> Result<(PlanInput, Vec<DeferredTarget>)> {
    let portfolio: (Uuid, f64, serde_json::Value) = sqlx::query_as(
        "SELECT user_id, total_value_usd::DOUBLE PRECISION, goal FROM portfolios WHERE id = $1",
    )
    .bind(portfolio_id)
    .fetch_one(&state.db)
    .await?;
    let user_id = portfolio.0;
    let portfolio_value_usd = portfolio.1;
    let goal = portfolio.2;

    let allocations: Vec<(String, f64, f64, f64)> = sqlx::query_as(
        "SELECT asset_symbol, current_weight::DOUBLE PRECISION,
                value_usd::DOUBLE PRECISION, quantity::DOUBLE PRECISION
         FROM allocations WHERE portfolio_id = $1",
    )
    .bind(portfolio_id)
    .fetch_all(&state.db)
    .await?;

    let mut target_weights = target_weights_from_goal(&goal);
    apply_route_preferences_to_targets(&state.config, &goal, &mut target_weights);
    let deferred = fold_nonexecutable_targets_into_usdc(&state.config, &mut target_weights);

    let caps = RuntimeCapabilities::from_config(&state.config);
    // The rebalanceable set (executable minus tracked-only volatiles) is the base
    // the planner sells from and marks as invested. Tracked volatiles fall through
    // to `frozen_holdings_value` — held, not traded — so a portfolio dominated by
    // un-routable assets still rebalances its stablecoin layer instead of dead-ending.
    let rebalanceable = rebalanceable_token_symbols(&caps, &state.config);
    let real_circle = real_circle_wallet_planning(&state.config);

    let balance = load_gateway_balance(state, user_id).await?;
    let mut usdc_per_chain_decimal = usdc_pool_from_balance(&balance)
        .into_iter()
        .map(|(chain, amount)| (chain, Decimal::from_f64(amount).unwrap_or(Decimal::ZERO)))
        .collect::<HashMap<_, _>>();
    subtract_active_reservations(state, user_id, &mut usdc_per_chain_decimal).await?;
    reserve_real_execution_usdc_buffer(&state.config, &mut usdc_per_chain_decimal);
    let usdc_per_chain = usdc_per_chain_decimal
        .into_iter()
        .map(|(chain, amount)| (chain, amount.to_f64().unwrap_or(0.0)))
        .collect::<HashMap<_, _>>();
    let idle_usdc: f64 = usdc_per_chain.values().copied().sum();

    let prices = load_planning_prices(
        state,
        &relevant_price_symbols(real_circle, &balance, &allocations, &target_weights),
    )
    .await;

    let sell_source_values = if real_circle {
        wallet_holding_values_by_chain(&balance, &prices, &rebalanceable)
    } else {
        HashMap::new()
    };
    let sell_sources = sell_source_values
        .into_iter()
        .map(|(symbol, values)| (symbol, SellSources::by_chain(values)))
        .collect::<HashMap<_, _>>();
    let (mode, marked_allocations, frozen_value_usd) = if real_circle {
        (
            ValuationMode::WalletHoldings,
            wallet_holdings_marked(&sell_sources),
            frozen_holdings_value(&balance, &prices, &rebalanceable),
        )
    } else {
        let marked = allocations
            .into_iter()
            .map(|(sym, weight, value_usd, quantity)| {
                (
                    sym.clone(),
                    weight,
                    marked_allocation_value(&sym, value_usd, quantity, &prices),
                )
            })
            .collect();
        (ValuationMode::AllocationBook, marked, 0.0)
    };
    let valuation = derive_valuation(
        mode,
        &marked_allocations,
        portfolio_value_usd,
        idle_usdc,
        frozen_value_usd,
    );

    if target_weights.is_empty() {
        target_weights = valuation.invested_weights.clone();
    }

    let regime: Option<String> = sqlx::query_scalar(
        "SELECT regime FROM agent_decisions
         WHERE portfolio_id = $1 AND regime IS NOT NULL
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(portfolio_id)
    .fetch_optional(&state.db)
    .await?
    .flatten();

    Ok((
        PlanInput {
            portfolio_value_usd: valuation.plan_value_usd,
            current_weights: valuation.current_weights,
            sell_sources,
            target_weights,
            usdc_per_chain,
            drift_threshold: 0.05,
            dust_threshold_usd: 5.0,
            prices,
            regime,
        },
        deferred,
    ))
}

fn target_weights_from_goal(goal: &serde_json::Value) -> HashMap<String, f64> {
    goal.get("targetAllocation")
        .and_then(|v| v.as_object())
        .into_iter()
        .flatten()
        .filter_map(|(symbol, value)| value.as_f64().map(|n| (symbol.clone(), n / 100.0)))
        .collect()
}

fn relevant_price_symbols(
    real_circle: bool,
    balance: &crate::modules::gateway::service::GatewayBalance,
    allocations: &[(String, f64, f64, f64)],
    target_weights: &HashMap<String, f64>,
) -> Vec<String> {
    let holding_symbols: Vec<String> = if real_circle {
        balance
            .token_balances_by_chain
            .values()
            .flat_map(|tokens| tokens.keys().cloned())
            .collect()
    } else {
        allocations
            .iter()
            .map(|(sym, _, _, _)| sym.clone())
            .collect()
    };
    holding_symbols
        .into_iter()
        .chain(target_weights.keys().cloned())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect()
}

fn real_circle_wallet_planning(cfg: &crate::config::Config) -> bool {
    let caps = RuntimeCapabilities::from_config(cfg);
    caps.real_mode && cfg.circle_wallet_exec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_circle_wallet_planning_uses_gateway_cash_only() {
        let mut cfg = crate::config::test_config();
        cfg.execution_mock = false;
        cfg.circle_mock = false;
        cfg.circle_wallet_exec = true;

        assert!(real_circle_wallet_planning(&cfg));
    }

    #[test]
    fn eoa_or_mock_planning_may_use_allocation_book() {
        let mut cfg = crate::config::test_config();
        cfg.execution_mock = false;
        cfg.circle_mock = false;
        cfg.circle_wallet_exec = false;
        assert!(!real_circle_wallet_planning(&cfg));

        cfg.execution_mock = true;
        cfg.circle_mock = true;
        cfg.circle_wallet_exec = true;
        assert!(!real_circle_wallet_planning(&cfg));
    }
}
