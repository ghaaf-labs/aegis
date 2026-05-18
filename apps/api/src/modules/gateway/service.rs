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

/// Fetch unified USDC balance for a wallet via Circle's W3S balances endpoint.
/// An empty wallet (no funded tokens yet) returns a zero balance rather than
/// a 5xx — that's the correct semantic for "this user just signed up and
/// hasn't deposited anything." Network or auth errors propagate as Internal.
pub async fn fetch_balance(
    http: &reqwest::Client,
    config: &Config,
    wallet_id: &str,
) -> crate::error::Result<GatewayBalance> {
    if config.circle_mock {
        return Ok(mock_balance(wallet_id));
    }

    #[derive(Deserialize)]
    struct Envelope {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(default, rename = "tokenBalances")]
        token_balances: Vec<TokenBalance>,
    }
    #[derive(Deserialize)]
    struct TokenBalance {
        #[serde(default)]
        amount: String,
        token: Token,
    }
    #[derive(Deserialize)]
    struct Token {
        #[serde(default)]
        symbol: String,
        #[serde(default)]
        blockchain: String,
    }

    let url = format!(
        "{}/v1/w3s/wallets/{wallet_id}/balances",
        config.circle_base_url
    );
    let resp = http
        .get(&url)
        .header("Authorization", format!("Bearer {}", config.circle_api_key))
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("gateway net: {e}")))?;

    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        // Wallet has no funded balances yet — treat as zero rather than 500.
        return Ok(GatewayBalance {
            unified_usdc: 0.0,
            per_chain: HashMap::new(),
            arc_address: None,
            base_address: None,
        });
    }
    let envelope: Envelope = resp
        .error_for_status()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("gateway status: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("gateway decode: {e}")))?;

    let mut per_chain: HashMap<String, f64> = HashMap::new();
    let mut total = 0.0;
    for tb in envelope.data.token_balances {
        if !tb.token.symbol.eq_ignore_ascii_case("USDC") {
            continue;
        }
        let amount: f64 = tb.amount.parse().unwrap_or(0.0);
        let chain_key = match tb.token.blockchain.as_str() {
            "ARC-TESTNET" | "ARC" => "arc".to_string(),
            "BASE-SEPOLIA" | "BASE" => "base".to_string(),
            other if !other.is_empty() => other.to_ascii_lowercase(),
            _ => "unknown".to_string(),
        };
        *per_chain.entry(chain_key).or_insert(0.0) += amount;
        total += amount;
    }

    Ok(GatewayBalance {
        unified_usdc: total,
        per_chain,
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

/// Broadcast a fetched balance over SSE, scoped to a specific user.
pub fn broadcast(sse: &SseSender, user_id: uuid::Uuid, balance: &GatewayBalance) {
    let _ = sse.send(SseEvent::GatewayBalance(SseGatewayBalance {
        user_id,
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
