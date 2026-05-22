use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use tracing::info;
use uuid::Uuid;

use crate::config::Config;
use crate::db::Db;
use crate::error::{AppError, Result};
use crate::modules::analytics;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsycRate {
    pub annualized_yield: f64,
    pub price_usd: f64,
    pub source: &'static str,
    pub fetched_at: DateTime<Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParkResult {
    pub intent: &'static str,
    pub amount_usdc: f64,
    pub executed: bool,
    pub tx_hash: Option<String>,
    pub note: &'static str,
}

/// Hashnote's published USYC yield. Not derivable from on-chain price alone
/// (the oracle reports a cumulative index, not an annualized rate). When
/// the underlying T-Bill yield changes materially, update this constant.
const HASHNOTE_PUBLISHED_YIELD: f64 = 0.0510;

pub async fn rate(http: &reqwest::Client, config: &Config) -> Result<UsycRate> {
    if config.execution_mock || config.usyc_oracle_arc.is_empty() || config.arc_rpc_url.is_empty() {
        return Ok(UsycRate {
            annualized_yield: HASHNOTE_PUBLISHED_YIELD,
            price_usd: 1.00,
            source: "mock",
            fetched_at: Utc::now(),
        });
    }

    match oracle_latest_price(http, &config.arc_rpc_url, &config.usyc_oracle_arc).await {
        Ok(price_usd) => Ok(UsycRate {
            annualized_yield: HASHNOTE_PUBLISHED_YIELD,
            price_usd,
            source: "hashnote-oracle-arc",
            fetched_at: Utc::now(),
        }),
        Err(e) => {
            tracing::warn!(error = %e, "usyc oracle read failed, falling back to published rate");
            Ok(UsycRate {
                annualized_yield: HASHNOTE_PUBLISHED_YIELD,
                price_usd: 1.00,
                source: "mock-fallback",
                fetched_at: Utc::now(),
            })
        }
    }
}

async fn oracle_latest_price(http: &reqwest::Client, rpc_url: &str, oracle: &str) -> Result<f64> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_call",
        "params": [{ "to": oracle, "data": "0xfeaf968c" }, "latest"]
    });
    let resp: serde_json::Value = http
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("oracle rpc send: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("oracle rpc json: {e}")))?;
    let hex_result = resp["result"]
        .as_str()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("oracle: no result field")))?;
    parse_oracle_price(hex_result)
}

/// `latestRoundData()` returns `(uint80 roundId, int256 answer, uint256
/// startedAt, uint256 updatedAt, uint80 answeredInRound)` — five 32-byte
/// words. The price is `answer` at bytes 32..64, scaled by 1e18 (matches
/// the Hashnote Oracle's `decimals() = 18`).
fn parse_oracle_price(hex_str: &str) -> Result<f64> {
    let bytes = hex::decode(hex_str.trim_start_matches("0x"))
        .map_err(|e| AppError::Internal(anyhow::anyhow!("oracle: bad hex: {e}")))?;
    if bytes.len() < 64 {
        return Err(AppError::Internal(anyhow::anyhow!(
            "oracle: short payload ({} bytes)",
            bytes.len()
        )));
    }
    let lo: [u8; 16] = bytes[48..64]
        .try_into()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("oracle: slice")))?;
    Ok(u128::from_be_bytes(lo) as f64 / 1e18)
}

#[allow(dead_code)]
pub async fn park_in_usyc(
    db: &Db,
    config: &Config,
    user_id: Uuid,
    amount_usdc: f64,
) -> Result<ParkResult> {
    info!("treasury: park {amount_usdc:.2} USDC into USYC for user {user_id}");
    analytics::emit(
        db,
        Some(user_id),
        "treasury.park_intent",
        json!({ "amountUsdc": amount_usdc, "asset": "USYC" }),
    )
    .await;

    if config.execution_mock || config.usyc_teller_arc.is_empty() || config.usdc_arc.is_empty() {
        return Ok(ParkResult {
            intent: "park_usyc",
            amount_usdc,
            executed: false,
            tx_hash: None,
            note: "mock — set EXECUTION_MOCK=false and build --features real-usyc",
        });
    }

    #[cfg(not(feature = "real-usyc"))]
    {
        let _ = (db, config, user_id, amount_usdc);
        Err(AppError::Internal(anyhow::anyhow!(
            "real USYC mint requires --features real-usyc"
        )))
    }

    #[cfg(feature = "real-usyc")]
    {
        let tx_hash = usyc_chain::deposit(config, amount_usdc).await?;
        Ok(ParkResult {
            intent: "park_usyc",
            amount_usdc,
            executed: true,
            tx_hash: Some(tx_hash),
            note: "Hashnote Teller deposit on Arc testnet",
        })
    }
}

#[allow(dead_code)]
pub async fn redeem_from_usyc(
    db: &Db,
    config: &Config,
    user_id: Uuid,
    amount_usdc: f64,
) -> Result<ParkResult> {
    info!("treasury: redeem {amount_usdc:.2} USDC-worth of USYC for user {user_id}");
    analytics::emit(
        db,
        Some(user_id),
        "treasury.redeem_intent",
        json!({ "amountUsdc": amount_usdc, "asset": "USYC" }),
    )
    .await;

    if config.execution_mock
        || config.usyc_teller_arc.is_empty()
        || config.usyc_token_arc.is_empty()
    {
        return Ok(ParkResult {
            intent: "redeem_usyc",
            amount_usdc,
            executed: false,
            tx_hash: None,
            note: "mock — set EXECUTION_MOCK=false and build --features real-usyc",
        });
    }

    #[cfg(not(feature = "real-usyc"))]
    {
        let _ = (db, config, user_id, amount_usdc);
        Err(AppError::Internal(anyhow::anyhow!(
            "real USYC redeem requires --features real-usyc"
        )))
    }

    #[cfg(feature = "real-usyc")]
    {
        let tx_hash = usyc_chain::redeem(config, amount_usdc).await?;
        Ok(ParkResult {
            intent: "redeem_usyc",
            amount_usdc,
            executed: true,
            tx_hash: Some(tx_hash),
            note: "Hashnote Teller redeem on Arc testnet",
        })
    }
}

#[cfg(feature = "real-usyc")]
mod usyc_chain {
    use alloy::{
        network::EthereumWallet,
        primitives::{Address, U256},
        providers::ProviderBuilder,
        signers::local::PrivateKeySigner,
        sol,
    };

    use crate::config::Config;
    use crate::error::{AppError, Result};

    sol! {
        #[sol(rpc)]
        interface IERC20 {
            function approve(address spender, uint256 amount) external returns (bool);
        }

        #[sol(rpc)]
        interface IUsycTeller {
            function deposit(uint256 assets, address receiver) external returns (uint256 shares);
            function redeem(uint256 shares, address receiver, address owner) external returns (uint256 assets);
            function convertToShares(uint256 assets) external view returns (uint256 shares);
        }
    }

    pub(super) async fn deposit(config: &Config, amount_usdc: f64) -> Result<String> {
        let (provider, signer_addr, teller, usdc) = build_provider(config)?;
        let amount = (amount_usdc * 1_000_000.0) as u64;

        let usdc_token = IERC20::new(usdc, &provider);
        let _approve = usdc_token
            .approve(teller, U256::from(amount).saturating_mul(U256::from(2u64)))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("USDC approve send: {e}"))?
            .get_receipt()
            .await
            .map_err(|e| anyhow::anyhow!("USDC approve receipt: {e}"))?;

        let teller_c = IUsycTeller::new(teller, &provider);
        let receipt = teller_c
            .deposit(U256::from(amount), signer_addr)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("teller deposit send: {e}"))?
            .get_receipt()
            .await
            .map_err(|e| anyhow::anyhow!("teller deposit receipt: {e}"))?;
        Ok(receipt.transaction_hash.to_string())
    }

    pub(super) async fn redeem(config: &Config, amount_usdc: f64) -> Result<String> {
        let (provider, signer_addr, teller, _) = build_provider(config)?;
        let amount = (amount_usdc * 1_000_000.0) as u64;

        let teller_c = IUsycTeller::new(teller, &provider);
        let shares = teller_c
            .convertToShares(U256::from(amount))
            .call()
            .await
            .map_err(|e| anyhow::anyhow!("convertToShares: {e}"))?;

        let receipt = teller_c
            .redeem(shares, signer_addr, signer_addr)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("teller redeem send: {e}"))?
            .get_receipt()
            .await
            .map_err(|e| anyhow::anyhow!("teller redeem receipt: {e}"))?;
        Ok(receipt.transaction_hash.to_string())
    }

    fn build_provider(
        config: &Config,
    ) -> Result<(impl alloy::providers::Provider, Address, Address, Address)> {
        let signer: PrivateKeySigner = config
            .chain_private_key_arc
            .parse()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("CHAIN_PRIVATE_KEY_ARC: {e}")))?;
        let signer_addr = signer.address();
        let wallet = EthereumWallet::from(signer);
        let rpc_url: reqwest::Url = config
            .arc_rpc_url
            .parse()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("bad arc rpc url: {e}")))?;
        let provider = ProviderBuilder::new().wallet(wallet).connect_http(rpc_url);
        let teller: Address = config
            .usyc_teller_arc
            .parse()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("bad USYC_TELLER_ARC")))?;
        let usdc: Address = config
            .usdc_arc
            .parse()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("bad USDC_ARC")))?;
        Ok((provider, signer_addr, teller, usdc))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_oracle_price_decodes_18_decimal_answer() {
        // Real `latestRoundData()` return from Arc testnet Oracle (2026-05-16):
        //   roundId   = 54
        //   answer    = 1.116277611710661072e18 (1.1163 USYC index)
        //   startedAt = 1770991631
        //   updatedAt = 1770991631
        //   answeredInRound = 54
        let hex = "0x\
            0000000000000000000000000000000000000000000000000000000000000036\
            0000000000000000000000000000000000000000000000000f7e02f6cf9f1a50\
            00000000000000000000000000000000000000000000000000000000698d0e0f\
            00000000000000000000000000000000000000000000000000000000698d0e0f\
            0000000000000000000000000000000000000000000000000000000000000036";
        let price = parse_oracle_price(hex).unwrap();
        assert!(
            (price - 1.1163).abs() < 0.001,
            "expected ~1.1163, got {price}"
        );
    }

    #[test]
    fn parse_oracle_price_rejects_short_payload() {
        assert!(parse_oracle_price("0x00").is_err());
    }
}
