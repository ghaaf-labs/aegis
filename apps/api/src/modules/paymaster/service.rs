use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::AppError;

/// Chains supported by the paymaster module. Extend in Sprint 3 when we
/// add more.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PaymasterChain {
    Arc,
    Base,
}

impl PaymasterChain {
    pub fn label(self) -> &'static str {
        match self {
            Self::Arc => "Arc",
            Self::Base => "Base",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeEstimate {
    pub chain: &'static str,
    pub action: String,
    pub fee_usdc: f64,
    pub via: &'static str,
    /// True when the value is a deterministic stub rather than a live RPC quote.
    /// The UI surfaces "indicative" alongside the figure so users don't treat
    /// the number as a binding quote.
    pub is_indicative: bool,
}

/// Representative gas units per action, used to size the live fee estimate.
const GAS_UNITS_REBALANCE: u64 = 200_000;
const GAS_UNITS_DEFAULT: u64 = 150_000;
/// Conservative ETH price (USD) used to convert Base ETH-denominated gas into a
/// USDC figure. Base gas is paid in ETH and the exact USDC charge comes from
/// the Circle Paymaster API (premium included), so the Base number stays
/// flagged `is_indicative` until that API is wired (F-PAYMASTER-1). Arc gas is
/// native USDC, so its live estimate is exact.
const ETH_PRICE_USD_HINT: f64 = 3000.0;

fn gas_units_for(action: &str) -> u64 {
    match action {
        "rebalance" => GAS_UNITS_REBALANCE,
        _ => GAS_UNITS_DEFAULT,
    }
}

fn stub_fee(chain: PaymasterChain) -> f64 {
    match chain {
        PaymasterChain::Arc => 0.012,
        PaymasterChain::Base => 0.105,
    }
}

/// Fetch the current `eth_gasPrice` (wei) from a chain RPC via JSON-RPC.
async fn fetch_gas_price_wei(rpc_url: &str) -> Option<u128> {
    if rpc_url.trim().is_empty() {
        return None;
    }
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "eth_gasPrice", "params": []
    });
    let resp = client
        .post(rpc_url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .ok()?;
    let v: serde_json::Value = resp.json().await.ok()?;
    let hex = v.get("result")?.as_str()?;
    u128::from_str_radix(hex.trim_start_matches("0x"), 16).ok()
}

/// Best-effort USDC fee estimate. Mock mode returns deterministic stubs (stable
/// dev preview). In real mode we derive the fee from the chain's live
/// `eth_gasPrice`: Arc gas is native USDC so the figure is exact
/// (`is_indicative = false`); Base gas is ETH so we convert via a price hint
/// and keep the figure `is_indicative` until the Circle Paymaster fee API
/// lands. Any RPC failure or implausible result falls back to the stub.
pub async fn estimate(
    config: &Config,
    chain: PaymasterChain,
    action: &str,
) -> crate::error::Result<FeeEstimate> {
    if action.is_empty() || action.len() > 64 {
        return Err(AppError::BadRequest("invalid action".into()));
    }

    let (fee_usdc, is_indicative) = if config.circle_mock {
        (stub_fee(chain), true)
    } else {
        let (rpc, native_is_usdc) = match chain {
            PaymasterChain::Arc => (config.arc_rpc_url.as_str(), true),
            PaymasterChain::Base => (config.base_rpc_url.as_str(), false),
        };
        let units = gas_units_for(action) as f64;
        match fetch_gas_price_wei(rpc).await {
            Some(price_wei) => {
                let native_cost = (price_wei as f64) * units / 1e18;
                let (fee, indicative) = if native_is_usdc {
                    (native_cost, false) // Arc: native gas IS USDC.
                } else {
                    (native_cost * ETH_PRICE_USD_HINT, true) // Base: ETH→USDC.
                };
                // Sanity guard against decimal/units surprises on a new testnet.
                if fee.is_finite() && fee > 0.0 && fee < 50.0 {
                    (fee, indicative)
                } else {
                    (stub_fee(chain), true)
                }
            }
            None => (stub_fee(chain), true),
        }
    };

    Ok(FeeEstimate {
        chain: chain.label(),
        action: action.to_string(),
        fee_usdc,
        via: "Circle Paymaster",
        is_indicative,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        crate::config::test_config()
    }

    #[tokio::test]
    async fn estimate_arc_is_sub_cent() {
        let e = estimate(&cfg(), PaymasterChain::Arc, "rebalance")
            .await
            .unwrap();
        assert!(e.fee_usdc < 0.05);
        assert_eq!(e.chain, "Arc");
    }

    #[tokio::test]
    async fn estimate_rejects_empty_action() {
        assert!(estimate(&cfg(), PaymasterChain::Arc, "").await.is_err());
    }
}
