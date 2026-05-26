use std::str::FromStr;

use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;

const BPS_DENOMINATOR: u128 = 10_000;

pub fn decimal_str_to_base_units(amount: &str, decimals: u8) -> Option<u128> {
    let amount = Decimal::from_str(amount.trim()).ok()?;
    if amount <= Decimal::ZERO {
        return Some(0);
    }
    amount
        .checked_mul(decimal_scale(decimals)?)?
        .trunc()
        .to_u128()
}

pub fn whole_token_to_base_units(qty: f64, decimals: u8) -> u128 {
    if !qty.is_finite() || qty <= 0.0 {
        return 0;
    }
    Decimal::from_f64(qty)
        .and_then(|amount| amount.checked_mul(decimal_scale(decimals)?))
        .and_then(|scaled| scaled.trunc().to_u128())
        .unwrap_or(0)
}

pub fn usdc_to_base_units(amount_usdc: f64) -> u128 {
    whole_token_to_base_units(amount_usdc, 6)
}

pub fn base_units_to_whole_token(units: u128, decimals: u8) -> f64 {
    let Some(scale) = decimal_scale(decimals) else {
        return 0.0;
    };
    Decimal::from(units)
        .checked_div(scale)
        .and_then(|qty| qty.to_f64())
        .unwrap_or(0.0)
}

pub fn apply_bps_margin(units: u128, margin_bps: u32) -> u128 {
    units.saturating_mul(u128::from(margin_bps)) / BPS_DENOMINATOR
}

fn decimal_scale(decimals: u8) -> Option<Decimal> {
    10_u128.checked_pow(u32::from(decimals)).map(Decimal::from)
}

#[cfg(test)]
mod tests {
    use super::{
        apply_bps_margin, base_units_to_whole_token, decimal_str_to_base_units, usdc_to_base_units,
        whole_token_to_base_units,
    };

    #[test]
    fn decimal_str_to_base_units_preserves_erc20_precision() {
        assert_eq!(
            decimal_str_to_base_units("0.520401419762915672", 18),
            Some(520_401_419_762_915_672)
        );
        assert_eq!(decimal_str_to_base_units("1457.77", 6), Some(1_457_770_000));
    }

    #[test]
    fn bps_margin_uses_integer_flooring() {
        assert_eq!(
            apply_bps_margin(520_401_419_762_915_672, 9_950),
            517_799_412_664_101_093
        );
    }

    #[test]
    fn float_quantity_conversion_is_still_decimal_scaled() {
        assert_eq!(whole_token_to_base_units(0.5, 18), 500_000_000_000_000_000);
        assert_eq!(usdc_to_base_units(1_457.77), 1_457_770_000);
        assert_eq!(base_units_to_whole_token(1_457_770_000, 6), 1_457.77);
    }
}
