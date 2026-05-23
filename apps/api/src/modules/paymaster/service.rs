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
        // Both branches return the same stub today — live RPC fee fetch is
        // tracked under F-PAYMASTER-1. Always flag indicative so the UI
        // can render an asterisk and tooltip explaining "not a binding quote".
        is_indicative: true,
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
