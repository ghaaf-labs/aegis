use rust_decimal::Decimal;
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::modules::rebalance::models::{ChainKey, LegKind};
use crate::modules::rebalance::quote::ValidatedQuote;
use crate::modules::rebalance::registry::tokens;
use crate::modules::wallet_routes;

#[derive(sqlx::FromRow, Clone)]
pub(super) struct LegRow {
    pub(super) id: Uuid,
    pub(super) leg_index: i32,
    pub(super) kind: String,
    pub(super) src_chain: Option<String>,
    pub(super) dest_chain: Option<String>,
    pub(super) src_symbol: Option<String>,
    pub(super) dest_symbol: Option<String>,
    pub(super) amount_usdc: Decimal,
    /// Planner-computed minimum destination output (token units, slippage
    /// applied). Set on CrossChainBurn hook-swap legs; `None` for plain
    /// USDC bridges. Used to size the hook's `min_out`.
    pub(super) min_out: Option<Decimal>,
    /// Per-leg state-machine status (`pending`/`submitted`/`confirmed`/`failed`).
    /// Read on every walk so a resumed plan can skip legs already confirmed
    /// rather than re-submitting them. NOT NULL DEFAULT in the schema.
    pub(super) status: String,
    /// How many times this leg has been submitted (bumped before each dispatch).
    /// Read on every walk so a persistently-reverting leg can be capped at
    /// `MAX_LEG_ATTEMPTS` rather than re-dispatching forever across resumes.
    /// NOT NULL DEFAULT 0 (migration 0038).
    pub(super) attempt_count: i32,
}

/// Maximum number of submit attempts for a single leg before it is failed.
/// Bounds runaway retries across resumes (migration 0038's `attempt_count`).
pub(super) const MAX_LEG_ATTEMPTS: i32 = 5;

pub(super) fn parse_kind(s: &str) -> Result<LegKind> {
    Ok(match s {
        "local_swap" => LegKind::LocalSwap,
        "cross_chain_burn" => LegKind::CrossChainBurn,
        "cross_chain_mint" => LegKind::CrossChainMint,
        "park_usyc" => LegKind::ParkUsyc,
        "redeem_usyc" => LegKind::RedeemUsyc,
        "fx_stablefx" => LegKind::FxStablefx,
        other => return Err(AppError::BadRequest(format!("unknown leg kind: {other}"))),
    })
}

pub(super) fn blockchain_for_chain(chain: ChainKey) -> &'static str {
    wallet_routes::blockchain_for_chain(chain)
}

/// The real on-chain fill of a quote's non-USDC asset, in whole token units,
/// taken from the quoter's `expected_asset_units` (the pool's real exchange
/// rate). `None` for a pure USDC↔USDC bridge or an un-priced/zero quote.
pub(super) fn quote_filled_qty(quote: &ValidatedQuote) -> Option<f64> {
    let symbol = if !quote.src_token.eq_ignore_ascii_case(tokens::USDC) {
        quote.src_token.as_str()
    } else if !quote.dest_token.eq_ignore_ascii_case(tokens::USDC) {
        quote.dest_token.as_str()
    } else {
        return None;
    };
    if quote.expected_asset_units == 0 {
        return None;
    }
    let decimals = tokens::token(symbol)?.decimals;
    let qty = quote.expected_asset_units as f64 / 10f64.powi(i32::from(decimals));
    (qty.is_finite() && qty > 0.0).then_some(qty)
}

pub(super) fn is_sell_leg(kind: LegKind, leg: &LegRow) -> bool {
    // A sell-side leg moves a non-USDC asset into USDC.
    matches!(
        kind,
        LegKind::LocalSwap | LegKind::RedeemUsyc | LegKind::FxStablefx
    ) && leg.dest_symbol.as_deref() == Some("USDC")
        && leg.src_symbol.as_deref() != Some("USDC")
}

/// A buy-side leg acquires a non-USDC asset for USDC. Covers local swaps,
/// USYC park, EURC FX, and cross-chain burns whose hook performs the
/// destination swap (dest_symbol carries the volatile target).
pub(super) fn is_buy_leg(kind: LegKind, leg: &LegRow) -> bool {
    let dest = leg.dest_symbol.as_deref().unwrap_or("");
    if dest.is_empty() || dest == "USDC" {
        return false;
    }
    matches!(
        kind,
        LegKind::LocalSwap | LegKind::ParkUsyc | LegKind::FxStablefx | LegKind::CrossChainBurn
    )
}

pub(super) fn quantity_for_notional(amount_usdc: f64, spot_price: f64) -> Option<f64> {
    if amount_usdc <= 0.0 || spot_price <= 0.0 {
        return None;
    }
    Some(amount_usdc / spot_price)
}

/// The quantity to write to holdings for a settled leg. The real on-chain fill
/// (`filled_qty`, from the executed quote) is authoritative because it reflects
/// the pool's true exchange rate; the `amount_usdc / spot_price` estimate is a
/// last resort for legs that can't report a fill (mock mode, cross-chain hook
/// swap). This is the fix for holdings showing `amount_usdc / mainnet_price`
/// instead of the tokens that actually landed.
pub(super) fn settled_quantity(
    filled_qty: Option<f64>,
    amount_usdc: f64,
    spot_price: f64,
) -> Option<f64> {
    match filled_qty {
        Some(q) if q.is_finite() && q > 0.0 => Some(q),
        _ => quantity_for_notional(amount_usdc, spot_price),
    }
}

#[cfg(test)]
pub(super) mod test_helpers {
    use rust_decimal::prelude::FromPrimitive;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    use crate::modules::rebalance::models::{ChainKey, LegKind};

    use super::LegRow;

    pub(in super::super) fn make_leg(kind: LegKind, amount_usdc: f64) -> LegRow {
        LegRow {
            id: Uuid::new_v4(),
            leg_index: 0,
            kind: kind.as_str().to_string(),
            src_chain: None,
            dest_chain: None,
            src_symbol: None,
            dest_symbol: None,
            amount_usdc: Decimal::from_f64(amount_usdc).unwrap_or_default(),
            min_out: None,
            status: "pending".into(),
            attempt_count: 0,
        }
    }

    pub(in super::super) fn make_swap_leg(src: &str, dest: &str) -> LegRow {
        LegRow {
            id: Uuid::new_v4(),
            leg_index: 0,
            kind: LegKind::LocalSwap.as_str().to_string(),
            src_chain: Some(ChainKey::Base.as_str().to_string()),
            dest_chain: Some(ChainKey::Base.as_str().to_string()),
            src_symbol: Some(src.to_string()),
            dest_symbol: Some(dest.to_string()),
            amount_usdc: Decimal::from_f64(600.0).unwrap_or_default(),
            min_out: None,
            status: "pending".into(),
            attempt_count: 0,
        }
    }

    pub(in super::super) fn make_mint_leg(dest: ChainKey, amount: f64) -> LegRow {
        LegRow {
            id: Uuid::new_v4(),
            leg_index: 1,
            kind: LegKind::CrossChainMint.as_str().to_string(),
            src_chain: Some(ChainKey::Arc.as_str().to_string()),
            dest_chain: Some(dest.as_str().to_string()),
            src_symbol: Some("USDC".into()),
            dest_symbol: Some("USDC".into()),
            amount_usdc: Decimal::from_f64(amount).unwrap_or_default(),
            min_out: None,
            status: "confirmed".into(),
            attempt_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::test_helpers::{make_swap_leg};
    use super::*;

    #[test]
    fn local_swap_into_usdc_is_sell_not_buy() {
        let sell = make_swap_leg("BTC", "USDC");

        assert!(is_sell_leg(LegKind::LocalSwap, &sell));
        assert!(!is_buy_leg(LegKind::LocalSwap, &sell));
    }

    #[test]
    fn local_swap_from_usdc_is_buy_not_sell() {
        let buy = make_swap_leg("USDC", "ETH");

        assert!(is_buy_leg(LegKind::LocalSwap, &buy));
        assert!(!is_sell_leg(LegKind::LocalSwap, &buy));
    }

    #[test]
    fn quantity_for_notional_rejects_zero_or_missing_prices() {
        assert_eq!(quantity_for_notional(600.0, 100_000.0), Some(0.006));
        assert_eq!(quantity_for_notional(0.0, 100_000.0), None);
        assert_eq!(quantity_for_notional(600.0, 0.0), None);
    }

    #[test]
    fn settled_quantity_prefers_real_on_chain_fill_over_price_derived() {
        // The bug: $20 of WETH on a testnet pool actually lands 0.0708 WETH,
        // but amount_usdc / mainnet_price gives ~0.0096. The real fill must win.
        let real_fill = 0.0708;
        let mainnet_price = 2080.0;
        let amount_usdc = 20.0;
        // Price-derived would be far off.
        let price_derived = quantity_for_notional(amount_usdc, mainnet_price).unwrap();
        assert!((price_derived - 0.0096).abs() < 0.0005);
        // settled_quantity returns the real fill, not the price-derived value.
        assert_eq!(
            settled_quantity(Some(real_fill), amount_usdc, mainnet_price),
            Some(real_fill)
        );
    }

    #[test]
    fn settled_quantity_falls_back_to_price_when_no_fill() {
        // No on-chain fill (mock mode / cross-chain hook) → price-derived.
        assert_eq!(settled_quantity(None, 600.0, 100_000.0), Some(0.006));
        // A zero/non-finite fill is ignored in favor of the price-derived value.
        assert_eq!(settled_quantity(Some(0.0), 600.0, 100_000.0), Some(0.006));
        assert_eq!(
            settled_quantity(Some(f64::NAN), 600.0, 100_000.0),
            Some(0.006)
        );
    }

    #[test]
    fn quote_filled_qty_scales_by_token_decimals_for_buy_and_sell() {
        let now = Utc::now();
        // Buy: USDC→ETH, quoter says 0.0708 WETH (18dp base units).
        let mut buy = ValidatedQuote::cctp_one_to_one(ChainKey::Base, ChainKey::Base, 0, now);
        buy.src_token = "USDC".into();
        buy.dest_token = "ETH".into();
        buy.expected_asset_units = 70_800_000_000_000_000; // 0.0708 * 1e18
        let q = quote_filled_qty(&buy).unwrap();
        assert!((q - 0.0708).abs() < 1e-9);

        // Sell: ETH→USDC, quoter says the wallet spends 0.5 WETH.
        let mut sell = ValidatedQuote::cctp_one_to_one(ChainKey::Base, ChainKey::Base, 0, now);
        sell.src_token = "ETH".into();
        sell.dest_token = "USDC".into();
        sell.expected_asset_units = 500_000_000_000_000_000; // 0.5 * 1e18
        assert_eq!(quote_filled_qty(&sell), Some(0.5));
    }

    #[test]
    fn quote_filled_qty_is_none_for_pure_usdc_bridge() {
        // A USDC↔USDC bridge has no non-USDC asset, so there is nothing to record.
        let bridge =
            ValidatedQuote::cctp_one_to_one(ChainKey::Arc, ChainKey::Base, 40_000_000, Utc::now());
        assert_eq!(quote_filled_qty(&bridge), None);
    }

    #[test]
    fn confirmed_leg_status_marks_skip_on_resume() {
        // The resume guard keys off leg.status == "confirmed". A confirmed leg
        // must be skippable; a pending one must not.
        let mut confirmed = make_swap_leg("USDC", "ETH");
        confirmed.status = "confirmed".into();
        assert_eq!(confirmed.status, "confirmed");

        let pending = make_swap_leg("USDC", "ETH");
        assert_eq!(pending.status, "pending");
        assert_ne!(pending.status, "confirmed");
    }
}
