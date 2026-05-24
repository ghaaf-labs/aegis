use serde::Serialize;

use crate::config::Config;
use crate::error::AppError;
use crate::modules::rebalance::models::ChainKey;

/// Chain a gas estimate is requested for. The paymaster speaks `ChainKey`
/// directly so OP/Arb/Eth/Avax map to the live native-gas path rather than a
/// `_ => Base` catch-all. The handler deserializes this from the `chain` query
/// param; `ChainKey`'s `snake_case` serde form keeps `arc`/`base` (and the new
/// chains' `eth_sepolia`, …) on the wire.
pub type PaymasterChain = ChainKey;

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
/// Conservative ETH price (USD) used to convert ETH-denominated gas into a USDC
/// figure on every non-Arc EVM chain. Their gas is paid in the native coin and
/// the exact USDC charge comes from the Circle Paymaster API (premium
/// included), so the figure stays flagged `is_indicative` until that API is
/// wired (F-PAYMASTER-1). Arc gas is native USDC, so its live estimate is exact.
const ETH_PRICE_USD_HINT: f64 = 3000.0;

fn gas_units_for(action: &str) -> u64 {
    match action {
        "rebalance" => GAS_UNITS_REBALANCE,
        _ => GAS_UNITS_DEFAULT,
    }
}

/// Human label for the chain in the fee estimate response.
fn chain_label(chain: ChainKey) -> &'static str {
    match chain {
        ChainKey::Arc => "Arc",
        ChainKey::Base => "Base",
        ChainKey::EthSepolia => "Ethereum Sepolia",
        ChainKey::ArbSepolia => "Arbitrum Sepolia",
        ChainKey::AvaxFuji => "Avalanche Fuji",
        ChainKey::OpSepolia => "OP Sepolia",
    }
}

/// True when the chain pays gas in native USDC (Arc) rather than a volatile
/// native coin. Only Arc does today; everything else converts via the price
/// hint and stays `is_indicative`.
fn native_gas_is_usdc(chain: ChainKey) -> bool {
    matches!(chain, ChainKey::Arc)
}

/// JSON-RPC URL used for the live `eth_gasPrice` probe on `chain`.
fn rpc_for_gas(cfg: &Config, chain: ChainKey) -> &str {
    &cfg.chain(chain).rpc_url
}

/// Deterministic per-chain stub used in mock mode and as the RPC fallback.
fn stub_fee(chain: ChainKey) -> f64 {
    match chain {
        ChainKey::Arc => 0.012,
        ChainKey::Base => 0.105,
        // Other EVM testnets pay ETH-style gas; use the Base-class stub until a
        // live quote (or the Circle Paymaster fee API) replaces it.
        ChainKey::EthSepolia | ChainKey::ArbSepolia | ChainKey::AvaxFuji | ChainKey::OpSepolia => {
            0.105
        }
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
        let rpc = rpc_for_gas(config, chain);
        let native_is_usdc = native_gas_is_usdc(chain);
        let units = gas_units_for(action) as f64;
        match fetch_gas_price_wei(rpc).await {
            Some(price_wei) => {
                let native_cost = (price_wei as f64) * units / 1e18;
                let (fee, indicative) = if native_is_usdc {
                    (native_cost, false) // Arc: native gas IS USDC.
                } else {
                    (native_cost * ETH_PRICE_USD_HINT, true) // ETH-style → USDC.
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
        chain: chain_label(chain),
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

    #[tokio::test]
    async fn estimate_base_uses_eth_style_stub_in_mock() {
        // Mock mode → deterministic stub; Base is ETH-style gas, not Arc-native.
        let e = estimate(&cfg(), PaymasterChain::Base, "rebalance")
            .await
            .unwrap();
        assert_eq!(e.chain, "Base");
        assert_eq!(e.fee_usdc, stub_fee(ChainKey::Base));
    }

    #[tokio::test]
    async fn estimate_new_chains_resolve_per_chain_labels() {
        // Each new chain estimates via its own ChainKey arm, not a `_ => Base`
        // fallback — verified through the distinct labels.
        for (chain, label) in [
            (ChainKey::OpSepolia, "OP Sepolia"),
            (ChainKey::ArbSepolia, "Arbitrum Sepolia"),
            (ChainKey::EthSepolia, "Ethereum Sepolia"),
            (ChainKey::AvaxFuji, "Avalanche Fuji"),
        ] {
            let e = estimate(&cfg(), chain, "rebalance").await.unwrap();
            assert_eq!(e.chain, label);
        }
    }

    #[test]
    fn only_arc_pays_native_usdc_gas() {
        assert!(native_gas_is_usdc(ChainKey::Arc));
        for c in [
            ChainKey::Base,
            ChainKey::EthSepolia,
            ChainKey::ArbSepolia,
            ChainKey::AvaxFuji,
            ChainKey::OpSepolia,
        ] {
            assert!(!native_gas_is_usdc(c), "{c:?} must not pay USDC-native gas");
        }
    }

    #[test]
    fn rpc_for_gas_reads_per_chain_url() {
        let mut c = cfg();
        c.chains[ChainKey::OpSepolia.index()].rpc_url = "https://op.example".into();
        assert_eq!(rpc_for_gas(&c, ChainKey::OpSepolia), "https://op.example");
        assert_eq!(
            rpc_for_gas(&c, ChainKey::Arc),
            c.chain(ChainKey::Arc).rpc_url.as_str()
        );
    }
}
