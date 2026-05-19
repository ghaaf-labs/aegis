use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::Config;
use crate::error::AppError;
use crate::modules::sse::{GatewayBalance as SseGatewayBalance, SseEvent, SseSender};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayBalance {
    /// Sum of USDC across every chain the user holds a wallet on.
    pub unified_usdc: f64,
    /// Sum of EURC across every chain.
    pub unified_eurc: f64,
    /// USDC per chain — keys are lowercased short names ("arc", "base").
    pub per_chain: HashMap<String, f64>,
    /// EURC per chain — same key set as `per_chain`.
    pub per_chain_eurc: HashMap<String, f64>,
    pub arc_address: Option<String>,
    pub base_address: Option<String>,
}

/// Fetch unified balances for a Circle W3S user. Circle creates one wallet
/// per blockchain, so we list every wallet for the user and aggregate USDC
/// and EURC across chains. Unfunded chains contribute zero rather than
/// raising — fresh signups should see $0 across the board, not a 500.
pub async fn fetch_balance(
    http: &reqwest::Client,
    config: &Config,
    user_id: Uuid,
) -> crate::error::Result<GatewayBalance> {
    if config.circle_mock {
        return Ok(mock_balance(user_id));
    }

    let wallets = list_user_wallets(http, config, user_id).await?;
    let mut balance = GatewayBalance {
        unified_usdc: 0.0,
        unified_eurc: 0.0,
        per_chain: HashMap::new(),
        per_chain_eurc: HashMap::new(),
        arc_address: None,
        base_address: None,
    };

    for w in &wallets {
        match w.blockchain.as_str() {
            "ARC-TESTNET" | "ARC" => balance.arc_address = Some(w.address.clone()),
            "BASE-SEPOLIA" | "BASE" => balance.base_address = Some(w.address.clone()),
            _ => {}
        }
        let chain_key = blockchain_to_key(&w.blockchain);

        let tokens = fetch_wallet_tokens(http, config, &w.id).await?;
        for tb in tokens {
            let amount: f64 = tb.amount.parse().unwrap_or(0.0);
            match tb.token.symbol.to_ascii_uppercase().as_str() {
                "USDC" => {
                    *balance.per_chain.entry(chain_key.clone()).or_insert(0.0) += amount;
                    balance.unified_usdc += amount;
                }
                "EURC" => {
                    *balance
                        .per_chain_eurc
                        .entry(chain_key.clone())
                        .or_insert(0.0) += amount;
                    balance.unified_eurc += amount;
                }
                _ => {} // ignore native gas tokens etc.
            }
        }
    }

    Ok(balance)
}

fn blockchain_to_key(blockchain: &str) -> String {
    match blockchain {
        "ARC-TESTNET" | "ARC" => "arc".into(),
        "BASE-SEPOLIA" | "BASE" => "base".into(),
        other if !other.is_empty() => other.to_ascii_lowercase(),
        _ => "unknown".into(),
    }
}

#[derive(Deserialize)]
struct CircleWallet {
    id: String,
    address: String,
    blockchain: String,
}

#[derive(Deserialize)]
struct TokenBalance {
    amount: String,
    token: TokenMeta,
}

#[derive(Deserialize)]
struct TokenMeta {
    #[serde(default)]
    symbol: String,
}

async fn list_user_wallets(
    http: &reqwest::Client,
    config: &Config,
    user_id: Uuid,
) -> crate::error::Result<Vec<CircleWallet>> {
    #[derive(Deserialize)]
    struct Envelope {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(default)]
        wallets: Vec<CircleWallet>,
    }

    let url = format!("{}/v1/w3s/wallets", config.circle_base_url);
    let envelope: Envelope = http
        .get(&url)
        .query(&[("userId", user_id.to_string())])
        .header("Authorization", format!("Bearer {}", config.circle_api_key))
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("gateway net: {e}")))?
        .error_for_status()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("gateway list_wallets: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("gateway list_wallets decode: {e}")))?;
    Ok(envelope.data.wallets)
}

async fn fetch_wallet_tokens(
    http: &reqwest::Client,
    config: &Config,
    wallet_id: &str,
) -> crate::error::Result<Vec<TokenBalance>> {
    #[derive(Deserialize)]
    struct Envelope {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(default, rename = "tokenBalances")]
        token_balances: Vec<TokenBalance>,
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

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(vec![]);
    }
    let envelope: Envelope = resp
        .error_for_status()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("gateway balances: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("gateway balances decode: {e}")))?;
    Ok(envelope.data.token_balances)
}

fn mock_balance(user_id: Uuid) -> GatewayBalance {
    // Deterministic mock: ~100 USDC + 100 EURC, split 60/40 BASE/ARC.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    user_id.hash(&mut h);
    let wobble = ((h.finish() % 1000) as f64) / 100.0;
    let total_usdc = 100.0 + wobble;
    let base_usdc = total_usdc * 0.6;
    let arc_usdc = total_usdc - base_usdc;
    let mut per_chain = HashMap::new();
    per_chain.insert("arc".into(), arc_usdc);
    per_chain.insert("base".into(), base_usdc);
    let mut per_chain_eurc = HashMap::new();
    per_chain_eurc.insert("arc".into(), 40.0);
    per_chain_eurc.insert("base".into(), 60.0);
    GatewayBalance {
        unified_usdc: total_usdc,
        unified_eurc: 100.0,
        per_chain,
        per_chain_eurc,
        arc_address: None,
        base_address: None,
    }
}

/// Broadcast a fetched balance over SSE, scoped to a specific user.
pub fn broadcast(sse: &SseSender, user_id: uuid::Uuid, balance: &GatewayBalance) {
    let _ = sse.send(SseEvent::GatewayBalance(SseGatewayBalance {
        user_id,
        unified_usdc: balance.unified_usdc,
        unified_eurc: balance.unified_eurc,
        per_chain: balance.per_chain.clone(),
        per_chain_eurc: balance.per_chain_eurc.clone(),
        observed_at: Utc::now(),
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_balance_is_deterministic_per_user() {
        let id = Uuid::new_v4();
        let a = mock_balance(id);
        let b = mock_balance(id);
        assert_eq!(a.unified_usdc, b.unified_usdc);
        assert!(a.unified_usdc > 99.0);
        assert!((a.unified_eurc - 100.0).abs() < 0.01);
    }

    #[test]
    fn mock_balance_sums_per_chain_to_total() {
        let b = mock_balance(Uuid::new_v4());
        let usdc_sum: f64 = b.per_chain.values().sum();
        assert!((b.unified_usdc - usdc_sum).abs() < 1e-6);
        let eurc_sum: f64 = b.per_chain_eurc.values().sum();
        assert!((b.unified_eurc - eurc_sum).abs() < 1e-6);
    }
}
