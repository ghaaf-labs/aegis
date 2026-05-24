/// CCTP V2 finality thresholds. 2000 = standard finality (~13min on Base, free).
/// 1000 = Fast Transfer (sub-30s) but requires a non-zero `maxFee`.
#[cfg_attr(not(any(feature = "real-cctp", test)), allow(dead_code))]
pub(super) const MIN_FINALITY_STANDARD: u32 = 2000;
#[cfg_attr(not(any(feature = "real-cctp", test)), allow(dead_code))]
pub(super) const MIN_FINALITY_FAST: u32 = 1000;

/// One row of Circle's `/v2/burn/USDC/fees/{src}/{dest}` response: the
/// `minimumFee` (bps) charged for a burn at the given `finalityThreshold`.
#[cfg_attr(not(any(feature = "real-cctp", test)), allow(dead_code))]
#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
pub(super) struct CctpFeeEntry {
    #[serde(rename = "finalityThreshold")]
    pub(super) finality_threshold: u32,
    /// bps — may be fractional (e.g. Arb→Base returns `1.3`), so it must
    /// deserialize as a float, not a u32 (a u32 silently fails the whole
    /// response decode and drops the burn to slow standard finality).
    #[serde(rename = "minimumFee")]
    pub(super) minimum_fee: f64,
}

/// The chosen burn parameters: a finality threshold plus the fee (in bps) Circle
/// charges at it. `fee_bps == 0` is the standard, free path.
#[cfg_attr(not(any(feature = "real-cctp", test)), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct BurnFeeChoice {
    pub(super) finality_threshold: u32,
    pub(super) fee_bps: f64,
}

impl BurnFeeChoice {
    #[cfg_attr(not(any(feature = "real-cctp", test)), allow(dead_code))]
    pub(super) const STANDARD: Self = Self {
        finality_threshold: MIN_FINALITY_STANDARD,
        fee_bps: 0.0,
    };
}

/// Select the Fast Transfer threshold + its quoted fee from Circle's fee table.
/// Falls back to the free standard path when no fast entry exists (so the
/// working path is never broken on a fee-table change). Never hardcodes a fee.
#[cfg_attr(not(any(feature = "real-cctp", test)), allow(dead_code))]
pub(super) fn select_burn_fee(entries: &[CctpFeeEntry]) -> BurnFeeChoice {
    entries
        .iter()
        .find(|e| e.finality_threshold == MIN_FINALITY_FAST)
        .map(|e| BurnFeeChoice {
            finality_threshold: MIN_FINALITY_FAST,
            fee_bps: e.minimum_fee,
        })
        .unwrap_or(BurnFeeChoice::STANDARD)
}

/// Compute the absolute on-chain `maxFee` (USDC, 6 decimals) from a burn
/// `amount` and a fee in bps, rounding up so the burn never under-quotes.
#[cfg_attr(not(any(feature = "real-cctp", test)), allow(dead_code))]
pub(super) fn max_fee_for(amount: u128, fee_bps: f64) -> u128 {
    if fee_bps <= 0.0 {
        return 0;
    }
    // amount * fee_bps / 10_000, rounded up so the burn never under-quotes.
    let fee = (amount as f64) * fee_bps / 10_000.0;
    (fee.ceil() as u128).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fee_table_selects_fast_threshold_and_uses_parsed_fee() {
        // Sample shape of Circle's GET /v2/burn/USDC/fees/{src}/{dest} body:
        // one row per finality threshold, fee in bps under `minimumFee`. The fast
        // fee can be FRACTIONAL (Arb→Base really returns 1.3) — a u32 here would
        // fail the whole decode and silently drop to slow standard finality.
        let body = r#"[
            {"finalityThreshold": 2000, "minimumFee": 0},
            {"finalityThreshold": 1000, "minimumFee": 1.3}
        ]"#;
        let entries: Vec<CctpFeeEntry> = serde_json::from_str(body).expect("fee body parses");
        let choice = select_burn_fee(&entries);
        assert_eq!(
            choice.finality_threshold, MIN_FINALITY_FAST,
            "fast threshold must be selected when present"
        );
        assert_eq!(
            choice.fee_bps, 1.3,
            "parsed (fractional) minimumFee must drive the maxFee"
        );

        // 100 USDC (6dp) at 1.3bps = 0.013 USDC = 13_000 base units (rounded up).
        let amount = 100_000_000u128;
        assert_eq!(max_fee_for(amount, choice.fee_bps), 13_000);
    }

    #[test]
    fn fee_table_falls_back_to_standard_when_no_fast_row() {
        let body = r#"[{"finalityThreshold": 2000, "minimumFee": 0}]"#;
        let entries: Vec<CctpFeeEntry> = serde_json::from_str(body).expect("fee body parses");
        let choice = select_burn_fee(&entries);
        assert_eq!(choice, BurnFeeChoice::STANDARD);
        assert_eq!(
            max_fee_for(100_000_000, choice.fee_bps),
            0,
            "standard path is free (maxFee 0)"
        );
    }
}
