use std::collections::HashMap;

use aegis_routing::{
    dag::LegDag, min_cost_flow, Asset as RouteAsset, EdgeKind, FlowConfig, ValueUsd,
};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;

use crate::domain::token;
use crate::modules::rebalance::models::{LegKind, PlannedLeg};

use super::chain_from_id;

fn min_out_for(
    dest_symbol: &str,
    amount_usdc: Decimal,
    prices: &HashMap<String, f64>,
) -> Option<Decimal> {
    if dest_symbol.eq_ignore_ascii_case(token::USDC) {
        return None;
    }
    let price = Decimal::from_f64(prices.get(dest_symbol).copied()?)?;
    (price > Decimal::ZERO).then(|| (amount_usdc / price) * Decimal::new(95, 2))
}

fn translate_dag(
    dag: &LegDag,
    prices: &HashMap<String, f64>,
    offset: i32,
) -> Result<Vec<PlannedLeg>, String> {
    let topo = match dag.topological_order() {
        Ok(order) => order,
        Err(e) => return Err(format!("invalid routing leg DAG: {e}")),
    };

    let mut id_to_global: HashMap<usize, i32> = HashMap::new();
    let mut out: Vec<PlannedLeg> = Vec::new();

    for dag_id in topo {
        let leg = &dag.legs[dag_id];
        let mut deps_global: Vec<i32> = Vec::with_capacity(leg.depends_on.len());
        for dep in &leg.depends_on {
            let Some(global) = id_to_global.get(dep) else {
                return Err(format!(
                    "routing leg {dag_id} depends on untranslated DAG leg {dep}"
                ));
            };
            deps_global.push(*global);
        }

        let from_chain = chain_from_id(leg.from.chain);
        let to_chain = chain_from_id(leg.to.chain);
        let from_sym = leg.from.token.as_str().to_string();
        let to_sym = leg.to.token.as_str().to_string();
        let amount = leg.value_in.amount();

        match leg.kind {
            EdgeKind::CctpStandard => {
                let burn_idx = offset + out.len() as i32;
                out.push(PlannedLeg {
                    leg_index: burn_idx,
                    deps: deps_global,
                    kind: LegKind::CrossChainBurn,
                    src_chain: from_chain,
                    dest_chain: to_chain,
                    src_symbol: Some(from_sym.clone()),
                    dest_symbol: Some(to_sym.clone()),
                    amount_usdc: amount,
                    min_out: None,
                });
                let mint_idx = offset + out.len() as i32;
                out.push(PlannedLeg {
                    leg_index: mint_idx,
                    deps: vec![burn_idx],
                    kind: LegKind::CrossChainMint,
                    src_chain: from_chain,
                    dest_chain: to_chain,
                    src_symbol: Some(from_sym),
                    dest_symbol: Some(to_sym),
                    amount_usdc: amount,
                    min_out: None,
                });
                id_to_global.insert(dag_id, mint_idx);
            }
            EdgeKind::AmmSwap => {
                let idx = offset + out.len() as i32;
                let min_out = min_out_for(&to_sym, amount, prices);
                out.push(PlannedLeg {
                    leg_index: idx,
                    deps: deps_global,
                    kind: LegKind::LocalSwap,
                    src_chain: from_chain,
                    dest_chain: to_chain,
                    src_symbol: Some(from_sym),
                    dest_symbol: Some(to_sym),
                    amount_usdc: amount,
                    min_out,
                });
                id_to_global.insert(dag_id, idx);
            }
            EdgeKind::UsycSubscribe => {
                let idx = offset + out.len() as i32;
                let min_out = min_out_for(&to_sym, amount, prices);
                out.push(PlannedLeg {
                    leg_index: idx,
                    deps: deps_global,
                    kind: LegKind::ParkUsyc,
                    src_chain: from_chain,
                    dest_chain: to_chain,
                    src_symbol: Some(from_sym),
                    dest_symbol: Some(to_sym),
                    amount_usdc: amount,
                    min_out,
                });
                id_to_global.insert(dag_id, idx);
            }
            EdgeKind::UsycRedeem => {
                let idx = offset + out.len() as i32;
                out.push(PlannedLeg {
                    leg_index: idx,
                    deps: deps_global,
                    kind: LegKind::RedeemUsyc,
                    src_chain: from_chain,
                    dest_chain: to_chain,
                    src_symbol: Some(from_sym),
                    dest_symbol: Some(to_sym),
                    amount_usdc: amount,
                    min_out: None,
                });
                id_to_global.insert(dag_id, idx);
            }
        }
    }

    Ok(out)
}

pub(super) fn route_and_append(
    graph: &aegis_routing::LiquidityGraph,
    from: &RouteAsset,
    to: &RouteAsset,
    size: Decimal,
    prices: &HashMap<String, f64>,
    out: &mut Vec<PlannedLeg>,
) -> bool {
    let Ok(plan) = min_cost_flow(graph, from, to, ValueUsd::usd(size), FlowConfig::default())
    else {
        return false;
    };
    if plan.allocations.is_empty() {
        return false;
    }
    let offset = out.len() as i32;
    let dag = LegDag::compile(graph, &plan.allocations);
    let new_legs = match translate_dag(&dag, prices, offset) {
        Ok(legs) => legs,
        Err(e) => {
            tracing::error!(error = %e, "routing engine produced an untranslatable leg DAG");
            return false;
        }
    };
    let added = !new_legs.is_empty();
    out.extend(new_legs);
    added
}
