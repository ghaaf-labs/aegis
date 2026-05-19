//! USDC ↔ EURC basis fetched via the platform price provider.
//!
//! Returns `Quote { usdc_usd, eurc_usd }`; the caller divides to get the
//! USDC→EURC mid-market rate. The PriceProvider already runs its own
//! per-ticker cache (3s TTL) so we don't re-cache here — a separate 30s
//! cache used to make EURC/USDC up to 30s stale while every other symbol
//! refreshed at 3s, breaking the consistency consumers expect.
//!
//! On any error the caller should fall back to the prior hardcoded `0.9217`
//! so the agent always has a number.

use crate::modules::prices::{lookup_symbol, PriceProvider};

#[derive(Debug, Clone, Copy)]
pub struct Quote {
    pub usdc_usd: f64,
    pub eurc_usd: f64,
}

pub async fn fetch_quote(provider: &dyn PriceProvider) -> anyhow::Result<Quote> {
    let symbols: Vec<&_> = ["USDC", "EURC"]
        .iter()
        .filter_map(|t| lookup_symbol(t))
        .collect();
    let quotes = provider.fetch_spot(&symbols).await?;
    let usdc = quotes
        .iter()
        .find(|q| q.ticker == "USDC")
        .map(|q| q.price_usd)
        .ok_or_else(|| anyhow::anyhow!("fx: usdc missing from provider response"))?;
    let eurc = quotes
        .iter()
        .find(|q| q.ticker == "EURC")
        .map(|q| q.price_usd)
        .ok_or_else(|| anyhow::anyhow!("fx: eurc missing from provider response"))?;

    Ok(Quote {
        usdc_usd: usdc,
        eurc_usd: eurc,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_division_yields_usdc_per_eurc_rate() {
        let q = Quote {
            usdc_usd: 1.0001,
            eurc_usd: 1.0850,
        };
        let mid = q.usdc_usd / q.eurc_usd;
        assert!((mid - 0.9217).abs() < 0.01, "expected ~0.92, got {mid}");
    }
}
