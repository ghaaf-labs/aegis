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
}

/// Best-effort USDC fee estimate. In mock mode (default for hackathon) the
/// numbers are deterministic by chain so the UI fee preview is stable.
pub async fn estimate(
    config: &Config,
    chain: PaymasterChain,
    action: &str,
) -> crate::error::Result<FeeEstimate> {
    if action.is_empty() || action.len() > 64 {
        return Err(AppError::BadRequest("invalid action".into()));
    }

    let fee_usdc = if config.circle_mock {
        match chain {
            // Arc: sub-cent native gas, paymaster sponsors fully.
            PaymasterChain::Arc => 0.012,
            // Base Sepolia: ERC-4337 paymaster fronts ~$0.10 USDC equivalent.
            PaymasterChain::Base => 0.105,
        }
    } else {
        // Live fee estimate would hit the paymaster's RPC. Out of scope for
        // Sprint 2 — we ship the typed surface and stub the value.
        // Returning the same mock numbers here keeps the contract honest.
        match chain {
            PaymasterChain::Arc => 0.012,
            PaymasterChain::Base => 0.105,
        }
    };

    Ok(FeeEstimate {
        chain: chain.label(),
        action: action.to_string(),
        fee_usdc,
        via: "Circle Paymaster",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config {
            database_url: "x".into(),
            jwt_secret: "x".into(),
            jwt_expiry_hours: 1,
            host: "x".into(),
            port: 0,
            openrouter_api_key: "x".into(),
            openrouter_base_url: "x".into(),
            model_regime: "x".into(),
            model_strategist: "x".into(),
            model_critic: "x".into(),
            model_tax: "x".into(),
            model_commentary: "x".into(),
            openrouter_app_name: "x".into(),
            openrouter_app_url: None,
            coingecko_api_key: None,
            sse_price_tick_secs: 5,
            circle_api_key: "x".into(),
            circle_base_url: "x".into(),
            circle_env: "sandbox".into(),
            circle_mock: true,
            arc_rpc_url: "x".into(),
            base_rpc_url: "x".into(),
            gateway_poll_secs: 10,
            faucet_max_usdc_per_day: 100.0,
            cors_allow_origin: "http://localhost:3000".into(),
            session_cookie_name: "aegis_jwt".into(),
            session_cookie_secure: false,
            cctp_attestation_url: "https://iris-api-sandbox.circle.com".into(),
            cctp_attestation_timeout_secs: 180,
            chain_private_key_arc: String::new(),
            chain_private_key_base: String::new(),
            execution_mock: true,
            scheduler_tick_secs: 300,
            scheduler_cooldown_secs: 1800,
            harvest_threshold_usd: 50.0,
            digest_hour_utc: 8,
            resend_api_key: String::new(),
            digest_from: "x".into(),
            digest_secret: "x".into(),
            public_base_url: "http://localhost:3000".into(),
            api_base_url: "http://localhost:8080".into(),
            // New real-execution + Nanopayments fields (defaults for tests)
            cctp_token_messenger_arc: String::new(),
            cctp_token_messenger_base: String::new(),
            cctp_message_transmitter_arc: String::new(),
            cctp_message_transmitter_base: String::new(),
            rebalance_executor_arc: String::new(),
            rebalance_executor_base: String::new(),
            usdc_arc: String::new(),
            usdc_base: String::new(),
            nanopayments_facilitator_url: "https://gateway-api-testnet.circle.com".into(),
            nanopayments_seller_address: String::new(),
            nanopayments_treasury_address: String::new(),
            billing_v2_enabled: false,
        }
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
