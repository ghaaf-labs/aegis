//! Live route-quality assessment for real execution.
//!
//! The deterministic planner can propose target deltas, but a review is only
//! safe if the current wallet balance and live venue quote make the route
//! executable at a sane price. This module owns that decision as structured
//! data so handlers, approval checks, and the future provider-backed planner all
//! consume the same result.

use uuid::Uuid;

use crate::domain::units::{apply_bps_margin, base_units_to_whole_token};
use crate::error::Result;
use crate::modules::rebalance::models::{ChainKey, LegKind, PlanInput, PlannedLeg};
use crate::modules::rebalance::registry::{route::RouteLeg, tokens, RuntimeCapabilities};
use crate::router::AppState;
use rust_decimal::prelude::ToPrimitive;

const MAX_QUOTER_PRICE_GAP_BPS: f64 = 25.0;
const LIVE_TOKEN_SPEND_MARGIN_BPS: u32 = 9_950;
const USDC_DECIMALS: u8 = 6;

fn leg_amount_usd(leg: &PlannedLeg) -> f64 {
    leg.amount_usdc.to_f64().unwrap_or(0.0)
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouteBlock {
    pub leg_index: i32,
    pub side: RouteBlockSide,
    pub symbol: String,
    pub chain: Option<ChainKey>,
    pub amount_usd: f64,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteBlockSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RouteAssessment {
    Safe,
    Blocked(RouteBlock),
}

impl RouteAssessment {
    pub fn block_message(self) -> Option<String> {
        match self {
            Self::Safe => None,
            Self::Blocked(block) => Some(block.message),
        }
    }
}

/// Return every unsafe live local swap route in a plan. This lets the caller
/// freeze only the unsafe buy/trim legs, re-run the planner, and keep
/// independent safe buy/bridge legs instead of blocking the entire review.
pub async fn live_route_blocks(
    state: &AppState,
    user_id: Uuid,
    input: &PlanInput,
    legs: &[PlannedLeg],
) -> Result<Vec<RouteBlock>> {
    let caps = RuntimeCapabilities::from_config(&state.config);
    if !caps.real_mode || !state.config.circle_wallet_exec {
        return Ok(Vec::new());
    }

    let mut blocks = Vec::new();
    for leg in legs {
        let assessment = if is_local_sell(leg) {
            assess_live_sell_leg(state, user_id, input, leg).await?
        } else if is_local_buy(leg) {
            assess_live_buy_leg(state, input, leg).await?
        } else {
            RouteAssessment::Safe
        };
        match assessment {
            RouteAssessment::Safe => {}
            RouteAssessment::Blocked(block) => blocks.push(block),
        }
    }
    Ok(blocks)
}

pub async fn assess_live_sell_leg(
    state: &AppState,
    user_id: Uuid,
    input: &PlanInput,
    leg: &PlannedLeg,
) -> Result<RouteAssessment> {
    if !is_local_sell(leg) {
        return Ok(RouteAssessment::Safe);
    }

    let Some(route_leg) = RouteLeg::from_parts(
        leg.kind.as_str(),
        leg.src_chain.map(|c| c.as_str().to_string()),
        leg.dest_chain.map(|c| c.as_str().to_string()),
        leg.src_symbol.clone(),
        leg.dest_symbol.clone(),
        leg_amount_usd(leg),
    ) else {
        return Ok(RouteAssessment::Blocked(RouteBlock {
            leg_index: leg.leg_index,
            side: RouteBlockSide::Sell,
            symbol: leg.src_symbol.clone().unwrap_or_default(),
            chain: leg.src_chain.or(leg.dest_chain),
            amount_usd: leg_amount_usd(leg),
            message: "A sell leg could not be parsed into an executable route.".into(),
        }));
    };
    let Some(ctx) = live_sell_context(state, user_id, input, leg).await? else {
        return Ok(RouteAssessment::Blocked(RouteBlock {
            leg_index: leg.leg_index,
            side: RouteBlockSide::Sell,
            symbol: leg.src_symbol.clone().unwrap_or_default(),
            chain: leg.src_chain.or(leg.dest_chain),
            amount_usd: leg_amount_usd(leg),
            message: "A sell leg is missing token, chain, price, or registry metadata.".into(),
        }));
    };
    let exact_output = crate::modules::rebalance::adapters::swap::quote(
        &state.config,
        &route_leg,
        chrono::Utc::now(),
    )
    .await;

    let (price_gap, balance_gap, spend_units) = match exact_output {
        Ok(quote) => {
            let price_gap = sell_quote_price_gap(input, leg, &quote);
            let balance_gap = sell_quote_balance_gap_from(&ctx, &quote);
            if price_gap.is_none() && balance_gap.is_none() {
                return Ok(RouteAssessment::Safe);
            }
            let spend_units = if balance_gap.is_some() {
                ctx.spendable_units.min(quote.amount_in)
            } else {
                quote.expected_asset_units.min(ctx.spendable_units)
            };
            (price_gap, balance_gap, spend_units)
        }
        Err(err) => (
            Some(format!(
                "No exact-output quote succeeded for the planned sale: {err}."
            )),
            None,
            ctx.spendable_units,
        ),
    };

    let exact_input = if spend_units > 0 {
        crate::modules::rebalance::adapters::swap::quote_sell_exact_input_units(
            &state.config,
            ctx.chain,
            &ctx.symbol,
            spend_units,
            chrono::Utc::now(),
        )
        .await
        .ok()
    } else {
        None
    };

    Ok(RouteAssessment::Blocked(RouteBlock {
        leg_index: leg.leg_index,
        side: RouteBlockSide::Sell,
        symbol: ctx.symbol.clone(),
        chain: Some(ctx.chain),
        amount_usd: leg_amount_usd(leg),
        message: sell_route_block_message(leg, &ctx, price_gap, balance_gap, exact_input.as_ref()),
    }))
}

pub async fn assess_live_buy_leg(
    state: &AppState,
    input: &PlanInput,
    leg: &PlannedLeg,
) -> Result<RouteAssessment> {
    if !is_local_buy(leg) {
        return Ok(RouteAssessment::Safe);
    }

    let Some(route_leg) = RouteLeg::from_parts(
        leg.kind.as_str(),
        leg.src_chain.map(|c| c.as_str().to_string()),
        leg.dest_chain.map(|c| c.as_str().to_string()),
        leg.src_symbol.clone(),
        leg.dest_symbol.clone(),
        leg_amount_usd(leg),
    ) else {
        return Ok(RouteAssessment::Blocked(RouteBlock {
            leg_index: leg.leg_index,
            side: RouteBlockSide::Buy,
            symbol: leg.dest_symbol.clone().unwrap_or_default(),
            chain: leg.dest_chain.or(leg.src_chain),
            amount_usd: leg_amount_usd(leg),
            message: "A buy leg could not be parsed into an executable route.".into(),
        }));
    };
    let Some(ctx) = live_buy_context(input, leg) else {
        return Ok(RouteAssessment::Blocked(RouteBlock {
            leg_index: leg.leg_index,
            side: RouteBlockSide::Buy,
            symbol: leg.dest_symbol.clone().unwrap_or_default(),
            chain: leg.dest_chain.or(leg.src_chain),
            amount_usd: leg_amount_usd(leg),
            message: "A buy leg is missing token, chain, price, or registry metadata.".into(),
        }));
    };

    let quote = match crate::modules::rebalance::adapters::swap::quote(
        &state.config,
        &route_leg,
        chrono::Utc::now(),
    )
    .await
    {
        Ok(quote) => quote,
        Err(err) => {
            return Ok(RouteAssessment::Blocked(RouteBlock {
                leg_index: leg.leg_index,
                side: RouteBlockSide::Buy,
                symbol: ctx.symbol.clone(),
                chain: Some(ctx.chain),
                amount_usd: leg_amount_usd(leg),
                message: format!(
                    "No safe live {} buy route is available on {} for the planned ${:.2} spend. Exact-input quote failed: {err}.",
                    ctx.symbol,
                    ctx.chain.as_str(),
                    leg_amount_usd(leg),
                ),
            }));
        }
    };

    if let Some(message) = buy_quote_price_gap(leg, &ctx, &quote) {
        return Ok(RouteAssessment::Blocked(RouteBlock {
            leg_index: leg.leg_index,
            side: RouteBlockSide::Buy,
            symbol: ctx.symbol,
            chain: Some(ctx.chain),
            amount_usd: leg_amount_usd(leg),
            message,
        }));
    }

    Ok(RouteAssessment::Safe)
}

fn is_local_sell(leg: &PlannedLeg) -> bool {
    leg.kind == LegKind::LocalSwap
        && leg.dest_symbol.as_deref() == Some(tokens::USDC)
        && leg.src_symbol.as_deref() != Some(tokens::USDC)
}

fn is_local_buy(leg: &PlannedLeg) -> bool {
    leg.kind == LegKind::LocalSwap
        && leg.src_symbol.as_deref() == Some(tokens::USDC)
        && leg.dest_symbol.as_deref() != Some(tokens::USDC)
}

struct LiveBuyContext {
    symbol: String,
    chain: ChainKey,
    decimals: u8,
    market_price: f64,
}

fn live_buy_context(input: &PlanInput, leg: &PlannedLeg) -> Option<LiveBuyContext> {
    let symbol = leg.dest_symbol.as_deref()?;
    let chain = leg.dest_chain.or(leg.src_chain)?;
    let spec = tokens::token(symbol)?;
    let market_price = input.prices.get(symbol).copied().filter(|p| *p > 0.0)?;
    Some(LiveBuyContext {
        symbol: symbol.to_string(),
        chain,
        decimals: spec.decimals,
        market_price,
    })
}

fn buy_quote_price_gap(
    leg: &PlannedLeg,
    ctx: &LiveBuyContext,
    quote: &crate::modules::rebalance::quote::ValidatedQuote,
) -> Option<String> {
    let expected_qty = base_units_to_whole_token(quote.expected_asset_units, ctx.decimals);
    if expected_qty <= 0.0 || ctx.market_price <= 0.0 {
        return None;
    }
    let quoted_price = leg_amount_usd(leg) / expected_qty;
    let gap_bps = ((quoted_price - ctx.market_price).abs() / ctx.market_price) * 10_000.0;
    if gap_bps <= MAX_QUOTER_PRICE_GAP_BPS {
        return None;
    }
    let fee = quote
        .fee_tier
        .map(|tier| format!(" fee tier {tier}"))
        .unwrap_or_default();
    Some(format!(
        "No safe live USDC→{} route is available on {} for the planned ${:.2} spend. Best {}{} quote implies ${quoted_price:.2}/{} versus market ${:.2} (gap {:.0} bps, limit {MAX_QUOTER_PRICE_GAP_BPS:.0} bps).",
        ctx.symbol,
        ctx.chain.as_str(),
        leg_amount_usd(leg),
        quote.provider,
        fee,
        ctx.symbol,
        ctx.market_price,
        gap_bps,
    ))
}

fn sell_quote_price_gap(
    input: &PlanInput,
    leg: &PlannedLeg,
    quote: &crate::modules::rebalance::quote::ValidatedQuote,
) -> Option<String> {
    let symbol = leg.src_symbol.as_deref()?;
    let spec = tokens::token(symbol)?;
    let expected_qty = base_units_to_whole_token(quote.expected_asset_units, spec.decimals);
    let market_price = input.prices.get(symbol).copied()?;
    if expected_qty <= 0.0 || market_price <= 0.0 {
        return None;
    }
    let quoted_price = leg_amount_usd(leg) / expected_qty;
    let gap_bps = ((quoted_price - market_price).abs() / market_price) * 10_000.0;
    if gap_bps <= MAX_QUOTER_PRICE_GAP_BPS {
        return None;
    }
    Some(format!(
        "Live {} swap pricing is too far from the portfolio mark to execute safely. Market mark is ${market_price:.2}; on-chain quote is ${quoted_price:.2} (gap {gap_bps:.0} bps, limit {MAX_QUOTER_PRICE_GAP_BPS:.0} bps).",
        symbol
    ))
}

struct LiveSellContext {
    symbol: String,
    chain: ChainKey,
    decimals: u8,
    market_price: f64,
    live_units: u128,
    spendable_units: u128,
}

async fn live_sell_context(
    state: &AppState,
    user_id: Uuid,
    input: &PlanInput,
    leg: &PlannedLeg,
) -> Result<Option<LiveSellContext>> {
    let Some(symbol) = leg.src_symbol.as_deref() else {
        return Ok(None);
    };
    let Some(chain) = leg.src_chain.or(leg.dest_chain) else {
        return Ok(None);
    };
    let Some(spec) = tokens::token(symbol) else {
        return Ok(None);
    };
    let Some(market_price) = input.prices.get(symbol).copied().filter(|p| *p > 0.0) else {
        return Ok(None);
    };
    let live_units = crate::modules::gateway::service::fetch_chain_token_balance_units(
        &state.http,
        &state.config,
        &state.db,
        user_id,
        chain,
        symbol,
        spec.decimals,
    )
    .await?;
    let spendable_units = apply_bps_margin(live_units, LIVE_TOKEN_SPEND_MARGIN_BPS);
    Ok(Some(LiveSellContext {
        symbol: symbol.to_string(),
        chain,
        decimals: spec.decimals,
        market_price,
        live_units,
        spendable_units,
    }))
}

fn sell_quote_balance_gap_from(
    ctx: &LiveSellContext,
    quote: &crate::modules::rebalance::quote::ValidatedQuote,
) -> Option<String> {
    if quote.amount_in <= ctx.spendable_units {
        return None;
    }
    let needed = base_units_to_whole_token(quote.amount_in, ctx.decimals);
    let spendable = base_units_to_whole_token(ctx.spendable_units, ctx.decimals);
    Some(format!(
        "Live {} balance on {} cannot fund the on-chain sell quote. Quote needs {:.8} {}; spendable wallet balance is {:.8}. Aegis will not create a review that Circle would reject.",
        ctx.symbol,
        ctx.chain.as_str(),
        needed,
        ctx.symbol,
        spendable
    ))
}

fn sell_route_block_message(
    leg: &PlannedLeg,
    ctx: &LiveSellContext,
    price_gap: Option<String>,
    balance_gap: Option<String>,
    exact_input: Option<&crate::modules::rebalance::adapters::swap::ExactInputSellQuote>,
) -> String {
    let mut parts = vec![format!(
        "No safe live {}→USDC route is available on {} for the planned ${:.2} sale. Aegis will not create a review that would execute at a bad pool price or fail in Circle estimation.",
        ctx.symbol,
        ctx.chain.as_str(),
        leg_amount_usd(leg),
    )];

    if let Some(reason) = price_gap {
        parts.push(reason);
    }
    if let Some(reason) = balance_gap {
        parts.push(reason);
    }

    if let Some(q) = exact_input {
        let qty = base_units_to_whole_token(q.quote.expected_asset_units, ctx.decimals);
        let out = base_units_to_whole_token(q.expected_usdc_units, USDC_DECIMALS);
        if qty > 0.0 && out > 0.0 {
            let implied = out / qty;
            let gap_bps = ((implied - ctx.market_price).abs() / ctx.market_price) * 10_000.0;
            let fee = q
                .quote
                .fee_tier
                .map(|tier| format!(" fee tier {tier}"))
                .unwrap_or_default();
            parts.push(format!(
                "Best exact-input check: selling {:.8} {} via {}{} returns ${:.2}, implied ${:.2}/{} (gap {:.0} bps from the market mark).",
                qty,
                ctx.symbol,
                q.quote.provider,
                fee,
                out,
                implied,
                ctx.symbol,
                gap_bps,
            ));
        }
    } else {
        let spendable = base_units_to_whole_token(ctx.spendable_units, ctx.decimals);
        let live = base_units_to_whole_token(ctx.live_units, ctx.decimals);
        parts.push(format!(
            "No exact-input fallback quote succeeded for the spendable wallet balance ({spendable:.8} of {live:.8} {}).",
            ctx.symbol
        ));
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::Utc;

    use crate::modules::rebalance::{
        adapters::swap::ExactInputSellQuote,
        models::{decimal_usd, ChainKey, LegKind, PlanInput, PlannedLeg},
        quote::ValidatedQuote,
    };

    use super::{
        buy_quote_price_gap, sell_quote_price_gap, sell_route_block_message, LiveBuyContext,
        LiveSellContext,
    };

    fn sell_leg(amount_usdc: f64) -> PlannedLeg {
        PlannedLeg {
            leg_index: 0,
            deps: vec![],
            kind: LegKind::LocalSwap,
            src_chain: Some(ChainKey::Base),
            dest_chain: Some(ChainKey::Base),
            src_symbol: Some("ETH".into()),
            dest_symbol: Some("USDC".into()),
            amount_usdc: decimal_usd(amount_usdc),
            min_out: None,
        }
    }

    fn input(price: f64) -> PlanInput {
        let mut prices = HashMap::new();
        prices.insert("ETH".into(), price);
        PlanInput {
            portfolio_value_usd: 1_000.0,
            current_weights: HashMap::new(),
            sell_sources: HashMap::new(),
            target_weights: HashMap::new(),
            usdc_per_chain: HashMap::new(),
            drift_threshold: 0.05,
            dust_threshold_usd: 5.0,
            prices,
            regime: None,
        }
    }

    #[test]
    fn sell_quote_price_gap_blocks_pool_price_far_from_market_mark() {
        let leg = sell_leg(878.799_357);
        let mut quote = ValidatedQuote::cctp_one_to_one(
            ChainKey::Base,
            ChainKey::Base,
            878_799_357,
            Utc::now(),
        );
        quote.src_token = "ETH".into();
        quote.dest_token = "USDC".into();
        quote.expected_asset_units = 2_224_483_629_811_495_917;

        let blocker = sell_quote_price_gap(&input(2_118.0), &leg, &quote)
            .expect("large price gap must block");
        assert!(blocker.contains("too far from the portfolio mark"));
    }

    #[test]
    fn sell_quote_price_gap_allows_quote_close_to_market_mark() {
        let leg = sell_leg(1_000.0);
        let mut quote = ValidatedQuote::cctp_one_to_one(
            ChainKey::Base,
            ChainKey::Base,
            1_000_000_000,
            Utc::now(),
        );
        quote.src_token = "ETH".into();
        quote.dest_token = "USDC".into();
        quote.expected_asset_units = 500_000_000_000_000_000;

        assert!(sell_quote_price_gap(&input(2_000.0), &leg, &quote).is_none());
    }

    #[test]
    fn sell_route_block_message_includes_best_exact_input_route() {
        let leg = sell_leg(878.799_357);
        let ctx = LiveSellContext {
            symbol: "ETH".into(),
            chain: ChainKey::Base,
            decimals: 18,
            market_price: 2_119.65,
            live_units: 520_401_419_762_915_672,
            spendable_units: crate::domain::units::apply_bps_margin(520_401_419_762_915_672, 9_950),
        };
        let mut quote = ValidatedQuote::cctp_one_to_one(
            ChainKey::Base,
            ChainKey::Base,
            520_401_419_762_915_672,
            Utc::now(),
        );
        quote.src_token = "ETH".into();
        quote.dest_token = "USDC".into();
        quote.amount_in = 520_401_419_762_915_672;
        quote.min_out = 383_573_174;
        quote.expected_asset_units = 520_401_419_762_915_672;
        quote.provider = "uniswap-v3".into();
        quote.fee_tier = Some(3000);
        let exact = ExactInputSellQuote {
            quote,
            expected_usdc_units: 385_500_678,
        };

        let message = sell_route_block_message(
            &leg,
            &ctx,
            Some("price gap".into()),
            Some("balance gap".into()),
            Some(&exact),
        );

        assert!(message.contains("No safe live ETH"));
        assert!(message.contains("Best exact-input check"));
        assert!(message.contains("fee tier 3000"));
        assert!(message.contains("$385.50"));
    }

    #[test]
    fn buy_quote_price_gap_blocks_pool_price_far_from_market_mark() {
        let leg = PlannedLeg {
            leg_index: 0,
            deps: vec![],
            kind: LegKind::LocalSwap,
            src_chain: Some(ChainKey::Base),
            dest_chain: Some(ChainKey::Base),
            src_symbol: Some("USDC".into()),
            dest_symbol: Some("ETH".into()),
            amount_usdc: decimal_usd(400.0),
            min_out: None,
        };
        let ctx = LiveBuyContext {
            symbol: "ETH".into(),
            chain: ChainKey::Base,
            decimals: 18,
            market_price: 2_000.0,
        };
        let mut quote = ValidatedQuote::cctp_one_to_one(
            ChainKey::Base,
            ChainKey::Base,
            400_000_000,
            Utc::now(),
        );
        quote.src_token = "USDC".into();
        quote.dest_token = "ETH".into();
        quote.amount_in = 400_000_000;
        quote.expected_asset_units = 1_000_000_000_000_000_000;
        quote.provider = "uniswap-v3".into();
        quote.fee_tier = Some(3000);

        let blocker = buy_quote_price_gap(&leg, &ctx, &quote).expect("large gap must block");
        assert!(blocker.contains("No safe live USDC"));
        assert!(blocker.contains("fee tier 3000"));
    }

    #[test]
    fn buy_quote_price_gap_allows_quote_close_to_market_mark() {
        let leg = PlannedLeg {
            leg_index: 0,
            deps: vec![],
            kind: LegKind::LocalSwap,
            src_chain: Some(ChainKey::Base),
            dest_chain: Some(ChainKey::Base),
            src_symbol: Some("USDC".into()),
            dest_symbol: Some("ETH".into()),
            amount_usdc: decimal_usd(1_000.0),
            min_out: None,
        };
        let ctx = LiveBuyContext {
            symbol: "ETH".into(),
            chain: ChainKey::Base,
            decimals: 18,
            market_price: 2_000.0,
        };
        let mut quote = ValidatedQuote::cctp_one_to_one(
            ChainKey::Base,
            ChainKey::Base,
            1_000_000_000,
            Utc::now(),
        );
        quote.src_token = "USDC".into();
        quote.dest_token = "ETH".into();
        quote.amount_in = 1_000_000_000;
        quote.expected_asset_units = 500_000_000_000_000_000;

        assert!(buy_quote_price_gap(&leg, &ctx, &quote).is_none());
    }
}
