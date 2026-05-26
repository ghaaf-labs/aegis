//! apps/api ↔ `aegis-routing` adapter + routing-engine-driven planner.
//!
//! [`engine_plan`] is the real planning entry point: it calls the routing
//! engine for every symbol delta, translates each `FlowPlan`/`LegDag` into
//! `PlannedLeg`s with **explicit `deps`**, and returns the execution DAG plus
//! any deltas that could not be routed.
//! The heuristic `planner::plan_legs` is kept for staleness-check comparisons
//! (it generates a structurally identical leg list from the same inputs, which
//! the approval gate uses to detect portfolio drift), but execution goes
//! through this module.

mod providers;
mod translate;

pub use providers::liquidity_graph;

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use aegis_routing::{min_cost_flow, Asset as RouteAsset, ChainId, FlowConfig, FlowPlan, ValueUsd};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;

use crate::config::Config;
use crate::domain::chain::ChainKey;
use crate::domain::token;
use crate::modules::rebalance::models::{PlanInput, PlannedLeg, SellSources};
use crate::modules::rebalance::planner::sorted_plan_deltas;
use crate::modules::rebalance::registry::{
    capabilities::RuntimeCapabilities, executable_chain_for_token,
};
use translate::{append_flow_plan, route_and_append};

const ROUTING_DUST_USD: f64 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EngineDeferred {
    pub symbol: String,
    pub side: DeferredSide,
    pub chain: Option<ChainKey>,
    pub amount_usd: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnginePlan {
    pub legs: Vec<PlannedLeg>,
    pub deferred: Vec<EngineDeferred>,
}

/// Map a `ChainKey` to the routing crate's `ChainId` (stable CCTP domain id).
fn chain_id(c: ChainKey) -> ChainId {
    ChainId(c.domain_id())
}

/// Reverse lookup: `ChainId` → `ChainKey`, or `None` for unknown domains.
fn chain_from_id(id: ChainId) -> Option<ChainKey> {
    ChainKey::ALL
        .iter()
        .copied()
        .find(|c| c.domain_id() == id.0)
}

/// Build a routing-crate node for `(symbol, chain)`.
fn asset(symbol: &str, chain: ChainKey) -> RouteAsset {
    RouteAsset::new(chain_id(chain), symbol)
}

fn target_chain_for_symbol(cfg: &Config, symbol: &str) -> ChainKey {
    let caps = RuntimeCapabilities::from_config(cfg);
    if caps.real_mode {
        executable_chain_for_token(&caps, cfg, symbol)
            .unwrap_or_else(|| token::native_chain(symbol))
    } else {
        token::native_chain(symbol)
    }
}

#[derive(Debug, Clone)]
struct SellSelection {
    sources: Vec<(ChainKey, f64)>,
    shortfall_usd: f64,
}

fn sell_sources(input: &PlanInput, symbol: &str, amount_usd: f64) -> SellSelection {
    let sources = match input.sell_sources.get(symbol) {
        None | Some(SellSources::CanonicalFallback) => {
            return SellSelection {
                sources: vec![(token::native_chain(symbol), amount_usd)],
                shortfall_usd: 0.0,
            };
        }
        Some(SellSources::Frozen) => {
            return SellSelection {
                sources: Vec::new(),
                shortfall_usd: amount_usd,
            };
        }
        Some(SellSources::ByChain(values_by_chain)) => values_by_chain,
    };

    let mut rows: Vec<(ChainKey, f64)> = sources
        .iter()
        .filter_map(|(&chain, &value)| {
            (value.is_finite() && value >= ROUTING_DUST_USD).then_some((chain, value))
        })
        .collect();
    rows.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.0.as_str().cmp(b.0.as_str()))
    });

    let mut remaining = amount_usd;
    let mut out = Vec::new();
    for (chain, available) in rows {
        if remaining < ROUTING_DUST_USD {
            break;
        }
        let take = remaining.min(available);
        if take >= ROUTING_DUST_USD {
            out.push((chain, take));
            remaining -= take;
        }
    }
    SellSelection {
        sources: out,
        shortfall_usd: remaining.max(0.0),
    }
}

#[derive(Debug, Clone)]
struct BuyDemand {
    symbol: String,
    target_chain: ChainKey,
    remaining_usd: f64,
}

#[derive(Debug, Clone)]
struct SourceTargetCandidate {
    source_chain: ChainKey,
    demand_idx: usize,
    amount_usd: f64,
    cost_bps: Decimal,
    plan: FlowPlan,
}

fn route_plan_cost_bps(
    graph: &aegis_routing::LiquidityGraph,
    from: &RouteAsset,
    to: &RouteAsset,
    amount_usd: f64,
) -> Option<(Decimal, FlowPlan)> {
    if amount_usd < ROUTING_DUST_USD {
        return None;
    }
    let size = Decimal::from_f64(amount_usd)?;
    let plan = min_cost_flow(graph, from, to, ValueUsd::usd(size), FlowConfig::default()).ok()?;
    if plan.allocations.is_empty() {
        return None;
    }
    let cost_bps = plan.total_cost / size * Decimal::from(10_000);
    Some((cost_bps, plan))
}

fn best_source_target_candidate(
    graph: &aegis_routing::LiquidityGraph,
    usdc_pool: &HashMap<ChainKey, f64>,
    demands: &[BuyDemand],
    failed_pairs: &HashSet<(ChainKey, usize)>,
) -> Option<SourceTargetCandidate> {
    let mut best: Option<SourceTargetCandidate> = None;
    for (&source_chain, &available) in usdc_pool {
        if available < ROUTING_DUST_USD {
            continue;
        }
        for (demand_idx, demand) in demands.iter().enumerate() {
            if failed_pairs.contains(&(source_chain, demand_idx)) {
                continue;
            }
            if demand.remaining_usd < ROUTING_DUST_USD {
                continue;
            }
            let amount_usd = available.min(demand.remaining_usd);
            let from = asset(token::USDC, source_chain);
            let to = asset(&demand.symbol, demand.target_chain);
            let Some((cost_bps, plan)) = route_plan_cost_bps(graph, &from, &to, amount_usd) else {
                continue;
            };
            let candidate = SourceTargetCandidate {
                source_chain,
                demand_idx,
                amount_usd,
                cost_bps,
                plan,
            };
            let replace = best.as_ref().is_none_or(|b| {
                (
                    candidate.cost_bps,
                    candidate.source_chain.as_str(),
                    demands[candidate.demand_idx].symbol.as_str(),
                    candidate.demand_idx,
                ) < (
                    b.cost_bps,
                    b.source_chain.as_str(),
                    demands[b.demand_idx].symbol.as_str(),
                    b.demand_idx,
                )
            });
            if replace {
                best = Some(candidate);
            }
        }
    }
    best
}

/// Routing-engine-driven planner. Produces `PlannedLeg`s with explicit `deps`
/// derived from the `LegDag` the engine compiles. This is the real planning
/// entry point called by the HTTP handler and the executor.
///
/// Sells are processed first (freeing USDC), then buy demands compete for all
/// available USDC sources by route cost. This keeps target selection faithful to
/// the user's allocation while letting the engine choose source chains and split
/// funding across chains when that is cheaper than a single source.
pub fn engine_plan(cfg: &Config, input: &PlanInput) -> EnginePlan {
    let deltas = sorted_plan_deltas(input);
    if deltas.is_empty() {
        return EnginePlan::default();
    }

    let graph = liquidity_graph(cfg);
    let prices = &input.prices;
    let mut all_legs: Vec<PlannedLeg> = Vec::new();
    let mut deferred = Vec::new();
    let mut usdc_pool: HashMap<ChainKey, f64> = input.usdc_per_chain.clone();
    let mut failed_buy_pairs: HashSet<(ChainKey, usize)> = HashSet::new();

    // Sells first: asset → USDC (same chain).
    for d in deltas.iter().filter(|d| d.value_delta_usd < 0.0) {
        let amount = d.value_delta_usd.abs();
        let selection = sell_sources(input, &d.symbol, amount);
        if selection.sources.is_empty() {
            deferred.push(EngineDeferred {
                symbol: d.symbol.clone(),
                side: DeferredSide::Sell,
                chain: None,
                amount_usd: amount,
                reason: format!("No safe sell source is available for {}.", d.symbol),
            });
            continue;
        }
        for (chain, source_amount) in selection.sources {
            let size = Decimal::from_f64(source_amount).unwrap_or_default();
            if route_and_append(
                &graph,
                &asset(&d.symbol, chain),
                &asset(token::USDC, chain),
                size,
                prices,
                &mut all_legs,
            ) {
                *usdc_pool.entry(chain).or_insert(0.0) += source_amount;
            } else {
                tracing::warn!(
                    symbol = %d.symbol,
                    chain = %chain.as_str(),
                    "routing engine: no sell route for {}, skipping delta",
                    d.symbol
                );
                deferred.push(EngineDeferred {
                    symbol: d.symbol.clone(),
                    side: DeferredSide::Sell,
                    chain: Some(chain),
                    amount_usd: source_amount,
                    reason: format!(
                        "No sell route is available for {} on {}.",
                        d.symbol,
                        chain.as_str()
                    ),
                });
            }
        }
        if selection.shortfall_usd >= ROUTING_DUST_USD {
            deferred.push(EngineDeferred {
                symbol: d.symbol.clone(),
                side: DeferredSide::Sell,
                chain: None,
                amount_usd: selection.shortfall_usd,
                reason: format!(
                    "Live wallet holdings could not cover ${:.2} of the requested {} trim.",
                    selection.shortfall_usd, d.symbol
                ),
            });
        }
    }

    let mut demands: Vec<BuyDemand> = deltas
        .iter()
        .filter(|d| d.value_delta_usd > 0.0)
        .map(|d| BuyDemand {
            symbol: d.symbol.clone(),
            target_chain: target_chain_for_symbol(cfg, &d.symbol),
            remaining_usd: d.value_delta_usd,
        })
        .collect();

    while let Some(candidate) =
        best_source_target_candidate(&graph, &usdc_pool, &demands, &failed_buy_pairs)
    {
        let demand = &demands[candidate.demand_idx];
        if append_flow_plan(&graph, &candidate.plan, prices, &mut all_legs) {
            let remaining_pool = (*usdc_pool.entry(candidate.source_chain).or_insert(0.0)
                - candidate.amount_usd)
                .max(0.0);
            usdc_pool.insert(candidate.source_chain, remaining_pool);
            demands[candidate.demand_idx].remaining_usd =
                (demands[candidate.demand_idx].remaining_usd - candidate.amount_usd).max(0.0);
        } else {
            tracing::error!(
                source_chain = %candidate.source_chain.as_str(),
                symbol = %demand.symbol,
                "routing engine found a buy route but could not translate it; skipping only this source-target pair"
            );
            failed_buy_pairs.insert((candidate.source_chain, candidate.demand_idx));
        }
    }

    for demand in &demands {
        if demand.remaining_usd >= ROUTING_DUST_USD {
            deferred.push(EngineDeferred {
                symbol: demand.symbol.clone(),
                side: DeferredSide::Buy,
                chain: Some(demand.target_chain),
                amount_usd: demand.remaining_usd,
                reason: format!(
                    "No route or source liquidity is available for ${:.2} of the {} target.",
                    demand.remaining_usd, demand.symbol
                ),
            });
        }
    }

    // Re-number leg_index to match position (the translate step produces
    // contiguous indices within each allocation, but across allocations the
    // offset is already tracked — verify here for safety).
    for (i, leg) in all_legs.iter_mut().enumerate() {
        leg.leg_index = i as i32;
    }

    EnginePlan {
        legs: all_legs,
        deferred,
    }
}

pub fn engine_plan_legs(cfg: &Config, input: &PlanInput) -> Vec<PlannedLeg> {
    engine_plan(cfg, input).legs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::token::{AddrSource, TOKEN_REGISTRY};
    use crate::modules::rebalance::models::LegKind;
    use rust_decimal::prelude::ToPrimitive;

    /// Fully-configured `Config` so every registry residency resolves.
    fn seeded_config() -> Config {
        let sentinel = "0x1111111111111111111111111111111111111111";
        let mut cfg = crate::config::test_config();
        cfg.usyc_enabled = true;
        cfg.usyc_token_arc = sentinel.into();
        for chain in ChainKey::ALL {
            cfg.chains[chain.index()].usdc = sentinel.into();
        }
        for spec in TOKEN_REGISTRY {
            for res in spec.residencies {
                if matches!(res.addr, AddrSource::Env(_)) {
                    cfg.set_token_address(spec.symbol, res.chain, sentinel);
                    cfg.swap_liquid_tokens
                        .entry(res.chain)
                        .or_default()
                        .push(spec.symbol.to_string());
                }
            }
        }
        cfg
    }

    fn seeded_input(
        symbol: &str,
        value_delta_usd: f64,
        idle_usdc_arc: f64,
        idle_usdc_base: f64,
    ) -> PlanInput {
        use crate::modules::rebalance::models::ChainKey;
        use std::collections::HashMap;
        let mut usdc_per_chain = HashMap::new();
        usdc_per_chain.insert(ChainKey::Arc, idle_usdc_arc);
        usdc_per_chain.insert(ChainKey::Base, idle_usdc_base);
        let mut target_weights = HashMap::new();
        let portfolio_value = value_delta_usd.abs() * 2.0;
        target_weights.insert(symbol.to_string(), 0.5);
        let current_weights = HashMap::new();
        PlanInput {
            portfolio_value_usd: portfolio_value,
            current_weights,
            sell_sources: HashMap::new(),
            target_weights,
            usdc_per_chain,
            drift_threshold: 0.01,
            dust_threshold_usd: 1.0,
            prices: HashMap::new(),
            regime: None,
        }
    }

    #[test]
    fn routing_graph_covers_every_executable_registry_node() {
        let cfg = seeded_config();
        let graph = liquidity_graph(&cfg);
        use crate::domain::token::TOKEN_REGISTRY;
        let mut expected = 0usize;
        let mut covered = 0usize;
        for spec in TOKEN_REGISTRY {
            for chain in spec.supported_chains() {
                if spec.address_for(&cfg, chain).is_some() {
                    expected += 1;
                    if graph.contains(&asset(spec.symbol, chain)) {
                        covered += 1;
                    }
                }
            }
        }
        println!(
            "routing: registry coverage {covered}/{expected} nodes; {} graph nodes, {} edges, graph_id {}",
            graph.node_count(),
            graph.edge_count(),
            &graph.fingerprint_hex()[..16]
        );
        assert!(expected > 0, "registry must yield executable nodes");
        assert_eq!(
            covered, expected,
            "every executable registry node must be in the graph"
        );
    }

    #[test]
    fn routing_graph_fingerprint_is_stable_across_builds() {
        let cfg = seeded_config();
        assert_eq!(
            liquidity_graph(&cfg).fingerprint(),
            liquidity_graph(&cfg).fingerprint(),
            "live graph must be deterministic for the same config"
        );
    }

    #[test]
    fn routing_graph_respects_swap_liquidity_allowlist() {
        let sentinel = "0x1111111111111111111111111111111111111111";
        let mut cfg = crate::config::test_config();
        cfg.chains[ChainKey::Base.index()].usdc = sentinel.into();
        cfg.set_token_address("ETH", ChainKey::Base, sentinel);
        cfg.set_token_address("cbBTC", ChainKey::Base, sentinel);
        cfg.swap_liquid_tokens
            .insert(ChainKey::Base, vec!["ETH".into()]);

        let graph = liquidity_graph(&cfg);

        assert!(graph.contains(&asset("ETH", ChainKey::Base)));
        assert!(
            !graph.contains(&asset("cbBTC", ChainKey::Base)),
            "a configured token address is not enough; the pool must be explicitly liquid"
        );
    }

    #[test]
    fn engine_plan_legs_cross_chain_buy_has_explicit_mint_depends_on_burn() {
        // A buy of ETH (Base-native) when all USDC is on Arc must produce:
        // CrossChainBurn (leg 0, no deps) → CrossChainMint (leg 1, deps=[0])
        // → LocalSwap (leg 2, deps=[1]).
        // The DAG must have EXPLICIT deps — the executor cannot infer them from
        // leg_index ordering alone.
        let cfg = seeded_config();
        let input = seeded_input("ETH", 5_000.0, 5_000.0, 0.0);
        let legs = engine_plan_legs(&cfg, &input);

        assert!(!legs.is_empty(), "cross-chain buy must produce legs");
        println!("engine_plan_legs legs:");
        for l in &legs {
            println!("  [{:?}] kind={:?} deps={:?}", l.leg_index, l.kind, l.deps);
        }

        let mint = legs
            .iter()
            .find(|l| l.kind == LegKind::CrossChainMint)
            .expect("cross-chain buy must include CrossChainMint");
        let burn = legs
            .iter()
            .find(|l| l.kind == LegKind::CrossChainBurn)
            .expect("cross-chain buy must include CrossChainBurn");

        assert!(
            mint.deps.contains(&burn.leg_index),
            "CrossChainMint (leg {}) must explicitly depend on CrossChainBurn (leg {}), got deps={:?}",
            mint.leg_index, burn.leg_index, mint.deps
        );

        let swap = legs
            .iter()
            .find(|l| l.kind == LegKind::LocalSwap)
            .expect("cross-chain buy must include a swap leg");
        assert!(
            swap.deps.contains(&mint.leg_index),
            "LocalSwap (leg {}) must depend on CrossChainMint (leg {}), got deps={:?}",
            swap.leg_index,
            mint.leg_index,
            swap.deps
        );
    }

    #[test]
    fn engine_plan_legs_same_chain_buy_has_no_deps() {
        // Local USDC on Base → ETH on Base should be a single LocalSwap
        // with empty deps (no bridge, no dependency chain).
        let cfg = seeded_config();
        let input = seeded_input("ETH", 5_000.0, 0.0, 5_000.0);
        let legs = engine_plan_legs(&cfg, &input);

        assert!(!legs.is_empty(), "same-chain buy must produce legs");
        assert!(
            legs.iter()
                .all(|l| !matches!(l.kind, LegKind::CrossChainBurn | LegKind::CrossChainMint)),
            "same-chain buy must not produce CCTP legs, got {:?}",
            legs.iter().map(|l| l.kind).collect::<Vec<_>>()
        );
        assert!(
            legs.iter().all(|l| l.deps.is_empty()),
            "same-chain legs must have empty deps"
        );
    }

    #[test]
    fn engine_plan_legs_dag_is_topologically_valid() {
        // For a cross-chain plan, the deps form a valid DAG (no cycles, all
        // referenced leg_index values exist).
        let cfg = seeded_config();
        let input = seeded_input("ETH", 5_000.0, 5_000.0, 0.0);
        let legs = engine_plan_legs(&cfg, &input);

        let indices: std::collections::HashSet<i32> = legs.iter().map(|l| l.leg_index).collect();
        for leg in &legs {
            for &dep in &leg.deps {
                assert!(
                    indices.contains(&dep),
                    "leg {} has dep {} which does not exist in the plan",
                    leg.leg_index,
                    dep
                );
                assert!(
                    dep < leg.leg_index,
                    "dep {} of leg {} must have a lower index (topological order)",
                    dep,
                    leg.leg_index
                );
            }
        }
    }

    #[test]
    fn engine_plan_legs_usyc_buy_from_arc_usdc_has_park_not_swap() {
        // USYC on Arc: USDC(Arc) → USYC(Arc) is a single UsycSubscribe edge →
        // ParkUsyc leg with no deps.
        let cfg = seeded_config();
        let input = seeded_input("USYC", 5_000.0, 5_000.0, 0.0);
        let legs = engine_plan_legs(&cfg, &input);

        assert!(!legs.is_empty(), "USYC buy must produce legs");
        let park = legs.iter().find(|l| l.kind == LegKind::ParkUsyc);
        assert!(
            park.is_some(),
            "USYC buy must produce a ParkUsyc leg, got {:?}",
            legs.iter().map(|l| l.kind).collect::<Vec<_>>()
        );
    }

    #[test]
    fn engine_plan_legs_routes_multiple_sources_to_multiple_demands_by_cost() {
        let cfg = seeded_config();
        let mut usdc_per_chain = HashMap::new();
        usdc_per_chain.insert(ChainKey::Arc, 3_000.0);
        usdc_per_chain.insert(ChainKey::Base, 2_000.0);
        let mut target_weights = HashMap::new();
        target_weights.insert("ETH".to_string(), 0.5);
        target_weights.insert("USYC".to_string(), 0.5);
        let input = PlanInput {
            portfolio_value_usd: 5_000.0,
            current_weights: HashMap::new(),
            sell_sources: HashMap::new(),
            target_weights,
            usdc_per_chain,
            drift_threshold: 0.01,
            dust_threshold_usd: 1.0,
            prices: HashMap::new(),
            regime: None,
        };

        let legs = engine_plan_legs(&cfg, &input);

        let park_usyc: f64 = legs
            .iter()
            .filter(|l| l.kind == LegKind::ParkUsyc && l.dest_symbol.as_deref() == Some("USYC"))
            .map(|l| l.amount_usdc.to_f64().unwrap_or(0.0))
            .sum();
        let buy_eth: f64 = legs
            .iter()
            .filter(|l| l.kind == LegKind::LocalSwap && l.dest_symbol.as_deref() == Some("ETH"))
            .map(|l| l.amount_usdc.to_f64().unwrap_or(0.0))
            .sum();
        let bridged_to_base: f64 = legs
            .iter()
            .filter(|l| {
                l.kind == LegKind::CrossChainBurn
                    && l.src_chain == Some(ChainKey::Arc)
                    && l.dest_chain == Some(ChainKey::Base)
            })
            .map(|l| l.amount_usdc.to_f64().unwrap_or(0.0))
            .sum();

        assert!(
            (park_usyc - 2_500.0).abs() < 0.01,
            "Arc-native USYC demand should use Arc USDC first, got {park_usyc}"
        );
        assert!(
            (buy_eth - 2_500.0).abs() < 1.0,
            "Base-native ETH demand should be fully satisfied, got {buy_eth}"
        );
        assert!(
            (bridged_to_base - 500.0).abs() < 1.0,
            "only Arc surplus after USYC should bridge to Base ETH, got {bridged_to_base}"
        );
    }

    #[test]
    fn engine_plan_legs_sells_from_actual_holding_chain() {
        let cfg = seeded_config();
        let mut current_weights = HashMap::new();
        current_weights.insert("ETH".to_string(), 1.0);
        let mut target_weights = HashMap::new();
        target_weights.insert("ETH".to_string(), 0.0);
        let mut sell_sources = HashMap::new();
        sell_sources.insert(
            "ETH".to_string(),
            SellSources::ByChain(HashMap::from([(ChainKey::ArbSepolia, 1_000.0)])),
        );

        let input = PlanInput {
            portfolio_value_usd: 1_000.0,
            current_weights,
            sell_sources,
            target_weights,
            usdc_per_chain: HashMap::new(),
            drift_threshold: 0.01,
            dust_threshold_usd: 1.0,
            prices: HashMap::new(),
            regime: None,
        };

        let legs = engine_plan_legs(&cfg, &input);

        let sell = legs
            .iter()
            .find(|l| l.kind == LegKind::LocalSwap && l.src_symbol.as_deref() == Some("ETH"))
            .expect("actual-chain ETH holding must produce a sell route");
        assert_eq!(sell.src_chain, Some(ChainKey::ArbSepolia));
        assert_eq!(sell.dest_chain, Some(ChainKey::ArbSepolia));
    }

    #[test]
    fn engine_plan_legs_does_not_fallback_after_sell_source_is_frozen() {
        let cfg = seeded_config();
        let mut current_weights = HashMap::new();
        current_weights.insert("ETH".to_string(), 1.0);
        let mut target_weights = HashMap::new();
        target_weights.insert("ETH".to_string(), 0.0);
        let mut sell_sources = HashMap::new();
        sell_sources.insert("ETH".to_string(), SellSources::Frozen);

        let input = PlanInput {
            portfolio_value_usd: 1_000.0,
            current_weights,
            sell_sources,
            target_weights,
            usdc_per_chain: HashMap::new(),
            drift_threshold: 0.01,
            dust_threshold_usd: 1.0,
            prices: HashMap::new(),
            regime: None,
        };

        let legs = engine_plan_legs(&cfg, &input);

        assert!(
            legs.iter()
                .all(|l| !(l.kind == LegKind::LocalSwap && l.src_symbol.as_deref() == Some("ETH"))),
            "a frozen live-wallet sell source must not fall back to ETH's native chain"
        );
    }
}
