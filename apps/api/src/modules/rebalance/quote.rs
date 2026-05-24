//! Quote validation — a real execution leg may only proceed against a fresh,
//! self-consistent quote. The same `validate()` gate covers AMM swaps (real
//! Uniswap quotes) and the trivial 1:1 CCTP USDC bridge, so no leg can execute
//! on a stale, mismatched, or zero-`min_out` quote.

use chrono::{DateTime, Duration, Utc};

use super::models::ChainKey;

/// Max age of a quote from issue to use. Quotes older than this must be refreshed.
pub const MAX_QUOTE_TTL_SECS: i64 = 60;
/// Max tolerated slippage. Anything looser is rejected as unsafe.
pub const MAX_SLIPPAGE_BPS: u32 = 100;
/// The deadline must sit at least this far in the future to leave room for
/// inclusion.
pub const MIN_DEADLINE_SLACK_SECS: i64 = 15;

/// A priced, time-bounded swap/bridge quote bound to an exact (token, chain)
/// pair. `amount_in` / `min_out` are in the respective tokens' base units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedQuote {
    pub quote_id: uuid::Uuid,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub src_token: String,
    pub dest_token: String,
    pub src_chain: ChainKey,
    pub dest_chain: ChainKey,
    pub amount_in: u128,
    pub min_out: u128,
    /// Quoted base-unit amount of the *non-USDC* asset this leg moves, taken
    /// straight from the on-chain quoter — the real pool's exchange rate, not a
    /// mainnet spot price. Buy (USDC→token): expected destination token output.
    /// Sell (token→USDC): expected source token spent. `0` for a pure USDC↔USDC
    /// bridge (no asset leg). The executor records holdings from this so
    /// `allocations.quantity` matches what actually landed on-chain rather than
    /// `amount_usdc / mainnet_price`.
    pub expected_asset_units: u128,
    pub slippage_bps: u32,
    /// Unix timestamp (seconds) embedded in the on-chain call.
    pub deadline: u64,
    pub provider: String,
    /// The Uniswap-V3 pool fee tier (e.g. 500 / 3000 / 10000) this quote was
    /// priced against, chosen by best-execution tier selection. `None` for
    /// quotes with no V3 fee tier (the 1:1 CCTP bridge, Trader Joe LB). The
    /// executor MUST submit the swap on this exact tier — pricing on the best
    /// pool but executing on another would miss `min_out` or fill worse.
    pub fee_tier: Option<u32>,
}

impl ValidatedQuote {
    /// A 1:1 quote for a pure USDC CCTP bridge (no swap on either side). The
    /// uniform gate still applies so freshness/deadline are enforced.
    pub fn cctp_one_to_one(
        src_chain: ChainKey,
        dest_chain: ChainKey,
        amount_in: u128,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            quote_id: uuid::Uuid::new_v4(),
            issued_at: now,
            expires_at: now + Duration::seconds(MAX_QUOTE_TTL_SECS),
            src_token: "USDC".into(),
            dest_token: "USDC".into(),
            src_chain,
            dest_chain,
            amount_in,
            min_out: amount_in,
            // A pure USDC bridge moves no non-USDC asset, so there is no asset
            // quantity to record from the quote.
            expected_asset_units: 0,
            slippage_bps: 0,
            deadline: (now + Duration::seconds(600)).timestamp() as u64,
            provider: "cctp-1to1".into(),
            // A pure USDC bridge is not an AMM swap — no fee tier.
            fee_tier: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteError {
    Expired,
    NotYetValid,
    TtlTooLong,
    TokenMismatch,
    ChainMismatch,
    MinOutZero,
    SlippageTooHigh,
    DeadlinePassed,
}

impl QuoteError {
    pub fn detail(self) -> &'static str {
        match self {
            QuoteError::Expired => "quote expired; refresh before executing",
            QuoteError::NotYetValid => "quote issue time is in the future",
            QuoteError::TtlTooLong => "quote validity window exceeds the freshness limit",
            QuoteError::TokenMismatch => "quote token does not match the leg",
            QuoteError::ChainMismatch => "quote chain does not match the leg",
            QuoteError::MinOutZero => "quote min_out must be greater than zero",
            QuoteError::SlippageTooHigh => "quote slippage exceeds the safety cap",
            QuoteError::DeadlinePassed => "quote deadline is too soon or already passed",
        }
    }
}

/// What the leg expects the quote to satisfy.
#[derive(Debug, Clone, Copy)]
pub struct QuoteExpectation<'a> {
    pub src_token: &'a str,
    pub dest_token: &'a str,
    pub src_chain: ChainKey,
    pub dest_chain: ChainKey,
}

/// Validate a quote against the leg's expectation and the current time.
pub fn validate(
    q: &ValidatedQuote,
    expect: QuoteExpectation<'_>,
    now: DateTime<Utc>,
) -> Result<(), QuoteError> {
    if q.issued_at > now {
        return Err(QuoteError::NotYetValid);
    }
    if now >= q.expires_at {
        return Err(QuoteError::Expired);
    }
    if (q.expires_at - q.issued_at) > Duration::seconds(MAX_QUOTE_TTL_SECS) {
        return Err(QuoteError::TtlTooLong);
    }
    if q.deadline <= (now + Duration::seconds(MIN_DEADLINE_SLACK_SECS)).timestamp() as u64 {
        return Err(QuoteError::DeadlinePassed);
    }
    if q.min_out == 0 {
        return Err(QuoteError::MinOutZero);
    }
    if q.slippage_bps > MAX_SLIPPAGE_BPS {
        return Err(QuoteError::SlippageTooHigh);
    }
    if q.src_token != expect.src_token || q.dest_token != expect.dest_token {
        return Err(QuoteError::TokenMismatch);
    }
    if q.src_chain != expect.src_chain || q.dest_chain != expect.dest_chain {
        return Err(QuoteError::ChainMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_quote(now: DateTime<Utc>) -> ValidatedQuote {
        ValidatedQuote {
            quote_id: uuid::Uuid::new_v4(),
            issued_at: now,
            expires_at: now + Duration::seconds(30),
            src_token: "USDC".into(),
            dest_token: "ETH".into(),
            src_chain: ChainKey::Base,
            dest_chain: ChainKey::Base,
            amount_in: 1_000_000,
            min_out: 500,
            expected_asset_units: 510,
            slippage_bps: 50,
            deadline: (now + Duration::seconds(300)).timestamp() as u64,
            provider: "uniswap-v3".into(),
            fee_tier: Some(3000),
        }
    }

    fn expect() -> QuoteExpectation<'static> {
        QuoteExpectation {
            src_token: "USDC",
            dest_token: "ETH",
            src_chain: ChainKey::Base,
            dest_chain: ChainKey::Base,
        }
    }

    #[test]
    fn fresh_quote_validates() {
        let now = Utc::now();
        assert_eq!(validate(&base_quote(now), expect(), now), Ok(()));
    }

    #[test]
    fn expired_quote_rejected() {
        let now = Utc::now();
        let q = base_quote(now - Duration::seconds(31));
        assert_eq!(validate(&q, expect(), now), Err(QuoteError::Expired));
    }

    #[test]
    fn ttl_too_long_rejected() {
        let now = Utc::now();
        let mut q = base_quote(now);
        q.expires_at = now + Duration::seconds(MAX_QUOTE_TTL_SECS + 5);
        assert_eq!(validate(&q, expect(), now), Err(QuoteError::TtlTooLong));
    }

    #[test]
    fn token_mismatch_rejected() {
        let now = Utc::now();
        let mut q = base_quote(now);
        q.dest_token = "BTC".into();
        assert_eq!(validate(&q, expect(), now), Err(QuoteError::TokenMismatch));
    }

    #[test]
    fn chain_mismatch_rejected() {
        let now = Utc::now();
        let mut q = base_quote(now);
        q.src_chain = ChainKey::Arc;
        assert_eq!(validate(&q, expect(), now), Err(QuoteError::ChainMismatch));
    }

    #[test]
    fn zero_min_out_rejected() {
        let now = Utc::now();
        let mut q = base_quote(now);
        q.min_out = 0;
        assert_eq!(validate(&q, expect(), now), Err(QuoteError::MinOutZero));
    }

    #[test]
    fn high_slippage_rejected() {
        let now = Utc::now();
        let mut q = base_quote(now);
        q.slippage_bps = MAX_SLIPPAGE_BPS + 1;
        assert_eq!(
            validate(&q, expect(), now),
            Err(QuoteError::SlippageTooHigh)
        );
    }

    #[test]
    fn deadline_too_soon_rejected() {
        let now = Utc::now();
        let mut q = base_quote(now);
        q.deadline = (now + Duration::seconds(MIN_DEADLINE_SLACK_SECS - 1)).timestamp() as u64;
        assert_eq!(validate(&q, expect(), now), Err(QuoteError::DeadlinePassed));
    }

    #[test]
    fn cctp_one_to_one_is_valid() {
        let now = Utc::now();
        let q = ValidatedQuote::cctp_one_to_one(ChainKey::Arc, ChainKey::Base, 40_000_000, now);
        let exp = QuoteExpectation {
            src_token: "USDC",
            dest_token: "USDC",
            src_chain: ChainKey::Arc,
            dest_chain: ChainKey::Base,
        };
        assert_eq!(validate(&q, exp, now), Ok(()));
    }
}
