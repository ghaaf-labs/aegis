use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::domain::token::native_chain;
use crate::error::AppError;
use crate::error::Result;
use crate::modules::rebalance::models::{ChainKey, PlanInput};
use crate::modules::rebalance::registry::{executable_token_symbols, RuntimeCapabilities};
use crate::modules::wallet_routes;
use crate::router::AppState;

use super::outcome::DeferredTarget;

const REAL_EXECUTION_CHAIN_USDC_BUFFER: f64 = 2.0;

/// Build the planner input plus the targets that had to be deferred (held as
/// USDC reserve because they have no live route). The deferred list is surfaced
/// by the `create` handler as `PartialDeferred`/`Blocked` so a folded sleeve is
/// shown as intent, never silently dropped (spec §11/§12). Callers that only
/// plan executable legs (auto-pilot, approval re-check) ignore the second field.
pub(super) async fn build_plan_input(
    state: &AppState,
    portfolio_id: Uuid,
) -> Result<(PlanInput, Vec<DeferredTarget>)> {
    // The planner consumes fractions (0-1), but the persisted
    // `current_weight` can lag behind execution. Use confirmed dollar values
    // when present so a just-executed or partially deployed portfolio cannot
    // plan from stale percentages.
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

    let mut target_weights = HashMap::new();
    if let Some(target_obj) = goal.get("targetAllocation").and_then(|v| v.as_object()) {
        for (k, v) in target_obj {
            if let Some(n) = v.as_f64() {
                target_weights.insert(k.clone(), n / 100.0);
            }
        }
    }
    apply_route_preferences_to_targets(&goal, &mut target_weights);
    let deferred = fold_nonexecutable_targets_into_usdc(&state.config, &mut target_weights);
    let wallet_cash_only = real_wallet_cash_only_planning(&state.config);

    let relevant_symbols: Vec<String> = allocations
        .iter()
        .map(|(sym, _, _, _)| sym.clone())
        .chain(target_weights.keys().cloned())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let prices = load_planning_prices(state, &relevant_symbols).await;

    let marked_allocations: Vec<(String, f64, f64)> = allocations
        .into_iter()
        .map(|(sym, weight, value_usd, quantity)| {
            let marked = marked_allocation_value(&sym, value_usd, quantity, &prices);
            (sym, weight, marked)
        })
        .collect();

    let mut usdc_per_chain = load_gateway_pool(state, user_id).await?;
    reserve_real_execution_usdc_buffer(&state.config, &mut usdc_per_chain);
    let idle_usdc: f64 = usdc_per_chain.values().copied().sum();

    // Single valuation authority (extracted + unit-tested): current weights
    // derive from confirmed value (live mark or booked value), never the stale
    // `current_weight` percentage — except the explicit `AllocationBook` fallback
    // (mock/EOA), now a *named mode* rather than an implicit branch. Real Circle
    // wallets are cash-only: positions live in the wallet, not a synthetic book.
    let mode = if wallet_cash_only {
        ValuationMode::WalletCashOnly
    } else {
        ValuationMode::AllocationBook
    };
    let valuation = derive_valuation(mode, &marked_allocations, portfolio_value_usd, idle_usdc);

    if target_weights.is_empty() {
        // Portfolios without a goal fall back to "stay where you are".
        target_weights = valuation.invested_weights.clone();
    }

    // Latest classified regime drives the "let winners run" asymmetric bands.
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

fn real_wallet_cash_only_planning(cfg: &crate::config::Config) -> bool {
    let caps = RuntimeCapabilities::from_config(cfg);
    caps.real_mode && cfg.circle_wallet_exec
}

fn reserve_real_execution_usdc_buffer(
    cfg: &crate::config::Config,
    usdc_per_chain: &mut HashMap<ChainKey, f64>,
) {
    let caps = RuntimeCapabilities::from_config(cfg);
    if !caps.real_mode {
        return;
    }

    for amount in usdc_per_chain.values_mut() {
        if *amount > 0.0 {
            *amount = (*amount - REAL_EXECUTION_CHAIN_USDC_BUFFER).max(0.0);
        }
    }
}

/// Fold any target weight for a non-executable sleeve into USDC before the
/// planner builds legs.
///
/// The agent and the UI still *target* the full designable menu (EURC, cbBTC,
/// …) so the proposal reads as the intended allocation — but the planner must
/// only build legs the executor can actually settle *now*. Without this, a
/// designable-but-not-executable sleeve (e.g. EURC, which has no Uniswap pool
/// on Base Sepolia) becomes a real `USDC→EURC` swap leg that reverts at the AMM.
/// The folded weight stays as USDC (idle reserve); executable sleeves (ETH) are
/// untouched, so a real plan is still built whenever any sleeve is live. When
/// nothing is executable the target reduces to USDC and the plan is simply
/// empty — handled as a graceful no-op upstream, never a reverting leg.
///
/// Mock mode is exempt: every leg settles with a mock receipt, so the full
/// designable target is plannable offline/in CI.
fn fold_nonexecutable_targets_into_usdc(
    cfg: &crate::config::Config,
    target_weights: &mut HashMap<String, f64>,
) -> Vec<DeferredTarget> {
    if target_weights.is_empty() {
        return Vec::new();
    }
    let caps = RuntimeCapabilities::from_config(cfg);
    if !caps.real_mode {
        return Vec::new();
    }
    let executable = executable_token_symbols(&caps, cfg);
    retain_executable_targets(target_weights, &executable)
}

/// Keep USDC and every executable sleeve; move the rest of the weight into USDC
/// and return the folded sleeves as deferred targets (surfaced as intent, not
/// silently dropped). Pure (no config/IO) so the fold rule is unit-testable
/// without a live runtime.
fn retain_executable_targets(
    target_weights: &mut HashMap<String, f64>,
    executable: &[&str],
) -> Vec<DeferredTarget> {
    let mut folded = 0.0;
    let mut deferred = Vec::new();
    target_weights.retain(|symbol, weight| {
        let keep = symbol.eq_ignore_ascii_case("USDC")
            || executable.iter().any(|e| e.eq_ignore_ascii_case(symbol));
        if !keep && weight.is_finite() && *weight > 0.0 {
            folded += *weight;
            deferred.push(DeferredTarget {
                symbol: symbol.clone(),
                target_weight: *weight,
                reason: deferred_reason(symbol),
            });
        }
        keep
    });
    if folded > 0.0 {
        *target_weights.entry("USDC".to_string()).or_insert(0.0) += folded;
    }
    deferred.sort_by(|a, b| {
        a.symbol
            .to_ascii_lowercase()
            .cmp(&b.symbol.to_ascii_lowercase())
    });
    deferred
}

/// Why a sleeve was deferred — truthful and chain-specific so the UI can explain
/// it ("EURC has no live route on Base right now"), not a generic placeholder.
fn deferred_reason(symbol: &str) -> String {
    let chain = native_chain(symbol).as_str();
    format!("No live execution route on {chain} right now; held as USDC reserve until one opens.")
}

pub(super) async fn load_planning_prices(
    state: &AppState,
    symbols: &[String],
) -> HashMap<String, f64> {
    if symbols.is_empty() {
        return HashMap::new();
    }

    let mut prices = crate::modules::market_data::service::fetch_snapshot(state.prices.as_ref())
        .await
        .map(|snapshot| {
            snapshot
                .assets
                .into_iter()
                .map(|asset| (asset.symbol, asset.price_usd))
                .filter(|(_, price)| price.is_finite() && *price > 0.0)
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();

    if prices.len() < symbols.len() {
        if let Ok(history) = crate::modules::market_data::service::get_historical_prices(
            &state.db,
            symbols,
            chrono::Utc::now(),
        )
        .await
        {
            for (symbol, price) in history {
                prices.entry(symbol).or_insert(price);
            }
        }
    }

    prices
}

pub(super) fn marked_allocation_value(
    symbol: &str,
    stored_value_usd: f64,
    quantity: f64,
    prices: &HashMap<String, f64>,
) -> f64 {
    let price = prices
        .get(symbol)
        .copied()
        .or_else(|| stable_planning_price(symbol));
    if quantity > 0.0 && stored_value_usd > 0.0 {
        if let Some(price) = price.filter(|p| p.is_finite() && *p > 0.0) {
            return quantity * price;
        }
    }
    stored_value_usd
}

pub(super) fn stable_planning_price(symbol: &str) -> Option<f64> {
    match symbol {
        "USDC" | "USYC" => Some(1.0),
        _ => None,
    }
}

/// How a portfolio's invested value is established when deriving current weights.
/// Making this a named mode (rather than the old implicit `wallet_cash_only` plus
/// `allocation_value_sum == 0` branch) keeps the stale-percentage path explicit
/// and impossible to reach by accident.
enum ValuationMode {
    /// Real Circle wallet: only Gateway USDC is spendable; the allocation book is
    /// ignored (positions live in the wallet, not a synthetic book). No position
    /// can be sized off a stale percentage here.
    WalletCashOnly,
    /// Mock/EOA: value positions from the allocation book — confirmed marks when
    /// present, else the last stored percentage of total (the documented
    /// offline/CI fallback, used only when no position has a confirmed value).
    AllocationBook,
}

struct Valuation {
    /// Weights over invested value only (pre-idle-USDC dilution). Used as the
    /// "stay where you are" target when the portfolio has no goal.
    invested_weights: HashMap<String, f64>,
    /// Weights over the full planning basis (invested + idle USDC). What the
    /// planner diffs against the target.
    current_weights: HashMap<String, f64>,
    /// Invested value + idle USDC — the planner's `portfolio_value_usd` basis.
    plan_value_usd: f64,
}

/// Turn marked allocations + idle USDC into value-derived current weights.
/// Pure (no IO) so the valuation modes are unit-testable without a live runtime.
/// `marked_allocations` is `(symbol, stale_weight_pct, marked_value_usd)`.
fn derive_valuation(
    mode: ValuationMode,
    marked_allocations: &[(String, f64, f64)],
    portfolio_value_usd: f64,
    idle_usdc: f64,
) -> Valuation {
    let book = matches!(mode, ValuationMode::AllocationBook);
    let allocation_value_sum: f64 = if book {
        marked_allocations.iter().map(|(_, _, v)| v.max(0.0)).sum()
    } else {
        0.0
    };
    let invested_value_usd = if allocation_value_sum > 0.0 {
        allocation_value_sum
    } else if book {
        portfolio_value_usd
    } else {
        0.0
    };
    let mut invested_weights = HashMap::new();
    if book && invested_value_usd > 0.0 {
        for (sym, stale_weight_pct, marked_value) in marked_allocations {
            let confirmed_value = if allocation_value_sum > 0.0 {
                marked_value.max(0.0)
            } else {
                (stale_weight_pct / 100.0) * portfolio_value_usd
            };
            invested_weights.insert(sym.clone(), confirmed_value / invested_value_usd);
        }
    }
    let plan_value_usd = invested_value_usd + idle_usdc;
    let current_weights = if idle_usdc > 0.0 && plan_value_usd > 0.0 {
        invested_weights
            .iter()
            .map(|(sym, weight)| (sym.clone(), (weight * invested_value_usd) / plan_value_usd))
            .collect()
    } else {
        invested_weights.clone()
    };
    Valuation {
        invested_weights,
        current_weights,
        plan_value_usd,
    }
}

pub(super) fn apply_route_preferences_to_targets(
    goal: &serde_json::Value,
    target_weights: &mut HashMap<String, f64>,
) {
    let Some(route_preferences) = goal.get("routePreferences") else {
        return;
    };

    let allowed_tokens = route_preference_set(route_preferences, "tokens");
    if !allowed_tokens.is_empty() {
        target_weights.retain(|symbol, _| {
            symbol == "USDC" || allowed_tokens.contains(&symbol.to_ascii_uppercase())
        });
    }

    let selected_networks = route_preference_set(route_preferences, "networks");
    if selected_networks.is_empty() {
        return;
    }

    let arc_allowed =
        selected_networks.contains(wallet_routes::ARC_TESTNET) || selected_networks.contains("ARC");
    let base_allowed = selected_networks.contains(wallet_routes::BASE_SEPOLIA)
        || selected_networks.contains("BASE");
    target_weights.retain(|symbol, _| {
        // Gate each target by the chain it lands on (registry canonical chain;
        // USYC→Arc, everything else→Base). Replaces the old native-symbol lists.
        match native_chain(symbol) {
            ChainKey::Arc => arc_allowed,
            ChainKey::Base => base_allowed,
            // Other execution chains aren't gated by the Arc/Base toggles.
            _ => true,
        }
    });
}

pub(super) fn route_preference_set(
    route_preferences: &serde_json::Value,
    key: &str,
) -> HashSet<String> {
    let mut values: HashSet<String> = route_preferences
        .get(key)
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .map(|v| v.trim().to_ascii_uppercase())
        .filter(|v| !v.is_empty())
        .collect();
    if values.remove("BTC_ETH_SOL") {
        values.insert("BTC".into());
        values.insert("ETH".into());
        values.insert("SOL".into());
    }
    values
}

/// Lookup unified USDC by chain from Circle Gateway. Real execution fails
/// closed when Gateway is unavailable; mock/demo mode degrades to a zero pool
/// so local review screens can still be exercised.
pub(super) async fn load_gateway_pool(
    state: &AppState,
    user_id: Uuid,
) -> Result<HashMap<ChainKey, f64>> {
    let mut pool: HashMap<ChainKey, f64> = HashMap::new();
    pool.insert(ChainKey::Arc, 0.0);
    pool.insert(ChainKey::Base, 0.0);

    match crate::modules::gateway::service::fetch_balance_for_user(
        &state.db,
        &state.http,
        &state.config,
        user_id,
    )
    .await
    {
        Ok(b) => {
            for (chain, amount) in b.per_chain {
                if let Some(key) = ChainKey::parse(chain.to_lowercase().as_str()) {
                    pool.insert(key, amount);
                }
            }
        }
        Err(e) => {
            if !state.config.execution_mock && !state.config.circle_mock {
                return Err(AppError::Conflict(
                    "Gateway balance is unavailable, so Aegis cannot build a real rebalance plan safely. Retry after Circle Gateway responds."
                        .into(),
                ));
            }
            tracing::warn!(error=%e, ?user_id, "gateway balance fetch failed; mock planner will use zero pool");
        }
    }
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn planning_value_prefers_live_price_and_stable_fallback() {
        let mut prices = HashMap::new();
        prices.insert("BTC".to_string(), 77_000.0);

        assert_eq!(marked_allocation_value("BTC", 840.0, 0.01, &prices), 770.0);
        assert_eq!(
            marked_allocation_value("USYC", 60.0, 60.0, &HashMap::new()),
            60.0
        );
        assert_eq!(
            marked_allocation_value("ETH", 300.0, 0.1, &HashMap::new()),
            300.0
        );
    }

    #[test]
    fn valuation_wallet_cash_only_ignores_allocation_book() {
        // Real Circle wallet: the book is ignored; only idle USDC funds the plan.
        // A phantom row (stale 100%, $0 value) produces no weight.
        let marked = vec![("ETH".to_string(), 100.0, 0.0)];
        let v = derive_valuation(ValuationMode::WalletCashOnly, &marked, 1000.0, 50.0);
        assert!(v.invested_weights.is_empty());
        assert!(
            v.current_weights.is_empty(),
            "no phantom ETH weight from a cash-only wallet"
        );
        assert!((v.plan_value_usd - 50.0).abs() < 1e-9);
    }

    #[test]
    fn valuation_allocation_book_uses_confirmed_value_not_stale_percent() {
        // ETH stale weight says 100% but its confirmed value is $0 (drained); a
        // funded BTC position is the only real value, so ETH weights ~0 (INV-1).
        let marked = vec![
            ("ETH".to_string(), 100.0, 0.0),
            ("BTC".to_string(), 0.0, 600.0),
        ];
        let v = derive_valuation(ValuationMode::AllocationBook, &marked, 600.0, 0.0);
        assert!((v.invested_weights["BTC"] - 1.0).abs() < 1e-9);
        assert!(v.invested_weights["ETH"].abs() < 1e-9);
        assert!((v.plan_value_usd - 600.0).abs() < 1e-9);
    }

    #[test]
    fn valuation_allocation_book_falls_back_to_stale_percent_when_no_confirmed_value() {
        // Documented mock/EOA fallback: no position has a confirmed mark, so
        // weights come from the stored percentages of total.
        let marked = vec![
            ("BTC".to_string(), 60.0, 0.0),
            ("ETH".to_string(), 40.0, 0.0),
        ];
        let v = derive_valuation(ValuationMode::AllocationBook, &marked, 1000.0, 0.0);
        assert!((v.invested_weights["BTC"] - 0.6).abs() < 1e-9);
        assert!((v.invested_weights["ETH"] - 0.4).abs() < 1e-9);
    }

    #[test]
    fn valuation_dilutes_current_weights_with_idle_usdc() {
        // $600 invested BTC + $400 idle USDC ⇒ BTC current weight 0.6 over the
        // full $1000 basis; invested-only weight stays 1.0.
        let marked = vec![("BTC".to_string(), 0.0, 600.0)];
        let v = derive_valuation(ValuationMode::AllocationBook, &marked, 600.0, 400.0);
        assert!((v.invested_weights["BTC"] - 1.0).abs() < 1e-9);
        assert!((v.current_weights["BTC"] - 0.6).abs() < 1e-9);
        assert!((v.plan_value_usd - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn route_preferences_filter_unselected_target_tokens() {
        let goal = json!({
            "targetAllocation": {"USDC": 40, "BTC": 30, "ETH": 20, "USYC": 10},
            "routePreferences": {
                "networks": ["ARC-TESTNET", "BASE-SEPOLIA"],
                "tokens": ["USDC", "USYC"],
                "watchlist": ["BTC_ETH_SOL"]
            }
        });
        let mut targets = HashMap::from([
            ("USDC".to_string(), 0.40),
            ("BTC".to_string(), 0.30),
            ("ETH".to_string(), 0.20),
            ("USYC".to_string(), 0.10),
        ]);

        apply_route_preferences_to_targets(&goal, &mut targets);

        assert!(targets.contains_key("USDC"));
        assert!(targets.contains_key("USYC"));
        assert!(!targets.contains_key("BTC"));
        assert!(!targets.contains_key("ETH"));
    }

    #[test]
    fn route_preferences_filter_targets_by_selected_execution_networks() {
        let goal = json!({
            "routePreferences": {
                "networks": ["ARC-TESTNET"],
                "tokens": ["BTC_ETH_SOL", "USYC", "EURC"]
            }
        });
        let mut targets = HashMap::from([
            ("BTC".to_string(), 0.30),
            ("ETH".to_string(), 0.20),
            ("USYC".to_string(), 0.30),
            ("EURC".to_string(), 0.20),
        ]);

        apply_route_preferences_to_targets(&goal, &mut targets);

        // With only Arc selected, the Base-native sleeves (BTC/ETH and now EURC,
        // which trades on the Base USDC/EURC pool) drop out; only Arc-native
        // USYC survives.
        assert_eq!(
            targets
                .keys()
                .cloned()
                .collect::<std::collections::HashSet<_>>(),
            std::collections::HashSet::from(["USYC".to_string()])
        );
    }

    #[test]
    fn route_preferences_keep_eurc_when_base_selected() {
        // EURC is Base-native now (Base USDC/EURC DEX pool), so selecting Base
        // keeps it even when Arc is not selected.
        let goal = json!({
            "routePreferences": {
                "networks": ["BASE-SEPOLIA"],
                "tokens": ["USYC", "EURC"]
            }
        });
        let mut targets = HashMap::from([("USYC".to_string(), 0.50), ("EURC".to_string(), 0.50)]);

        apply_route_preferences_to_targets(&goal, &mut targets);

        assert!(targets.contains_key("EURC"));
        assert!(!targets.contains_key("USYC"));
    }

    #[test]
    fn fold_moves_nonexecutable_target_weight_into_usdc() {
        // EURC/cbBTC have no executable route on Base Sepolia; their weight must
        // become idle USDC, never a swap leg that reverts at the AMM. ETH stays.
        let mut targets = HashMap::from([
            ("ETH".to_string(), 0.28),
            ("EURC".to_string(), 0.10),
            ("cbBTC".to_string(), 0.12),
            ("USDC".to_string(), 0.50),
        ]);

        let deferred = retain_executable_targets(&mut targets, &["USDC", "ETH"]);

        assert_eq!(targets.get("ETH").copied(), Some(0.28));
        assert!(!targets.contains_key("EURC"));
        assert!(!targets.contains_key("cbBTC"));
        let usdc = targets.get("USDC").copied().unwrap_or_default();
        assert!((usdc - 0.72).abs() < 1e-9, "folded EURC+cbBTC into USDC");
        // The folded sleeves are surfaced as deferred intent (sorted), not dropped.
        let symbols: Vec<&str> = deferred.iter().map(|d| d.symbol.as_str()).collect();
        assert_eq!(symbols, vec!["cbBTC", "EURC"]);
        assert!(deferred.iter().all(|d| !d.reason.is_empty()));
    }

    #[test]
    fn fold_reduces_all_nonexecutable_target_to_usdc() {
        // Everything non-executable ⇒ target collapses to USDC. The planner then
        // produces an empty plan (graceful no-op), not a reverting EURC leg.
        let mut targets = HashMap::from([("EURC".to_string(), 1.0)]);

        retain_executable_targets(&mut targets, &["USDC", "ETH"]);

        assert_eq!(targets, HashMap::from([("USDC".to_string(), 1.0)]));
    }

    #[test]
    fn real_execution_pool_keeps_chain_usdc_buffer() {
        let mut cfg = crate::config::test_config();
        cfg.execution_mock = false;
        cfg.circle_mock = false;
        let mut pool = HashMap::from([
            (ChainKey::Arc, 3.25),
            (ChainKey::Base, 1.50),
            (ChainKey::EthSepolia, 20.0),
        ]);

        reserve_real_execution_usdc_buffer(&cfg, &mut pool);

        assert_eq!(pool.get(&ChainKey::Arc).copied(), Some(1.25));
        assert_eq!(pool.get(&ChainKey::Base).copied(), Some(0.0));
        assert_eq!(pool.get(&ChainKey::EthSepolia).copied(), Some(18.0));
    }

    #[test]
    fn mock_execution_pool_does_not_keep_chain_buffer() {
        let cfg = crate::config::test_config();
        let mut pool = HashMap::from([(ChainKey::Arc, 3.25), (ChainKey::Base, 1.50)]);

        reserve_real_execution_usdc_buffer(&cfg, &mut pool);

        assert_eq!(pool.get(&ChainKey::Arc).copied(), Some(3.25));
        assert_eq!(pool.get(&ChainKey::Base).copied(), Some(1.50));
    }

    #[test]
    fn real_circle_wallet_planning_uses_gateway_cash_only() {
        let mut cfg = crate::config::test_config();
        cfg.execution_mock = false;
        cfg.circle_mock = false;
        cfg.circle_wallet_exec = true;

        assert!(real_wallet_cash_only_planning(&cfg));
    }

    #[test]
    fn eoa_or_mock_planning_may_use_allocation_book() {
        let mut cfg = crate::config::test_config();
        cfg.execution_mock = false;
        cfg.circle_mock = false;
        cfg.circle_wallet_exec = false;
        assert!(!real_wallet_cash_only_planning(&cfg));

        cfg.execution_mock = true;
        cfg.circle_mock = true;
        cfg.circle_wallet_exec = true;
        assert!(!real_wallet_cash_only_planning(&cfg));
    }
}
