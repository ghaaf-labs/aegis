use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::AppError;
use crate::modules::sse::{GatewayBalance as SseGatewayBalance, SseEvent, SseSender};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayBalance {
    pub unified_usdc: f64,
    pub per_chain: HashMap<String, f64>,
    pub arc_address: Option<String>,
    pub base_address: Option<String>,
}

/// Fetch unified USDC balance for a wallet. Mock returns deterministic
/// numbers keyed on the wallet_id so the demo stays stable.
pub async fn fetch_balance(
    http: &reqwest::Client,
    config: &Config,
    wallet_id: &str,
) -> crate::error::Result<GatewayBalance> {
    if config.circle_mock {
        return Ok(mock_balance(wallet_id));
    }

    #[derive(Deserialize)]
    struct CircleGatewayResp {
        #[serde(rename = "unifiedUsdc")]
        unified_usdc: f64,
        #[serde(rename = "perChain")]
        per_chain: HashMap<String, f64>,
    }
    let resp: CircleGatewayResp = http
        .get(format!(
            "{}/v1/gateway/wallets/{wallet_id}/balance",
            config.circle_base_url
        ))
        .header("Authorization", format!("Bearer {}", config.circle_api_key))
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("gateway net: {e}")))?
        .error_for_status()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("gateway status: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("gateway decode: {e}")))?;

    Ok(GatewayBalance {
        unified_usdc: resp.unified_usdc,
        per_chain: resp.per_chain,
        arc_address: None,
        base_address: None,
    })
}

fn mock_balance(wallet_id: &str) -> GatewayBalance {
    // Deterministic mock: 100 USDC after first faucet claim, split 60/40
    // across Arc and Base. Adds a small wobble per wallet_id so different
    // demo wallets show different numbers.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    wallet_id.hash(&mut h);
    let wobble = ((h.finish() % 1000) as f64) / 100.0; // 0..10
    let total = 100.0 + wobble;
    let arc = total * 0.6;
    let base = total - arc;
    let mut per = HashMap::new();
    per.insert("arc".into(), arc);
    per.insert("base".into(), base);
    GatewayBalance {
        unified_usdc: total,
        per_chain: per,
        arc_address: None,
        base_address: None,
    }
}

/// Broadcast a fetched balance over SSE.
pub fn broadcast(sse: &SseSender, balance: &GatewayBalance) {
    let _ = sse.send(SseEvent::GatewayBalance(SseGatewayBalance {
        unified_usdc: balance.unified_usdc,
        per_chain: balance.per_chain.clone(),
        observed_at: Utc::now(),
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_balance_is_deterministic_per_wallet() {
        let a = mock_balance("wallet-a");
        let b = mock_balance("wallet-a");
        assert_eq!(a.unified_usdc, b.unified_usdc);
        let c = mock_balance("wallet-b");
        assert!((a.unified_usdc - c.unified_usdc).abs() > 0.0);
    }

    #[test]
    fn mock_balance_sums_perfectly() {
        let b = mock_balance("wallet-z");
        let sum: f64 = b.per_chain.values().sum();
        assert!((b.unified_usdc - sum).abs() < 1e-6);
    }
}
