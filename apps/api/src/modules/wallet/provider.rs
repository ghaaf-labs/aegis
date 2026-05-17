//! Circle WaaS provider — pluggable so `MOCK_CIRCLE=true` keeps local dev
//! moving when the sandbox is unreachable.
//!
//! F-WALLET-1 (audited 2026-05-17): the path strings below are centralised in
//! `paths` so the next live smoke can patch them in one place when Circle's
//! W3S product reorg lands. Per the 2026-05-16 smoke, the legacy
//! `/v1/wallets/...` paths return Circle's structured 404; the new W3S
//! product uses `/v1/w3s/...` patterns
//! (`/v1/w3s/users`, `/v1/w3s/user/token`, `/v1/w3s/wallets`, etc.). We
//! leave the legacy strings here as the call shape the rest of the module
//! depends on; flipping them is a 4-line edit in `paths` once the W3S
//! contract is verified against a live `CIRCLE_API_KEY`.

use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::Config;
use crate::error::AppError;
use crate::modules::wallet::models::{OtpStartResponse, WalletInfo};

/// Centralised Circle path strings. Update here when F-WALLET-1 closes.
mod paths {
    /// POST — create a wallet from a WebAuthn attestation.
    pub const WALLETS_CREATE: &str = "/wallets";
    /// POST — authenticate an existing wallet via WebAuthn assertion.
    pub const WALLETS_AUTH: &str = "/wallets/authenticate";
    /// POST — start the email-OTP challenge.
    pub const OTP_START: &str = "/wallets/otp/start";
    /// POST — verify the OTP code and materialise the wallet.
    pub const OTP_VERIFY: &str = "/wallets/otp/verify";
}

#[async_trait]
pub trait WalletProvider: Send + Sync {
    /// Create a new wallet using a WebAuthn credential attestation.
    async fn create_with_passkey(
        &self,
        email: &str,
        passkey_attestation: &serde_json::Value,
    ) -> crate::error::Result<WalletInfo>;

    /// Authenticate an existing wallet using a WebAuthn assertion.
    async fn authenticate_with_passkey(
        &self,
        email: &str,
        passkey_assertion: &serde_json::Value,
    ) -> crate::error::Result<WalletInfo>;

    /// Start an email-OTP challenge. Returns an opaque `challenge_id` to be
    /// echoed back on verify, plus the email it was sent to.
    async fn start_otp(&self, email: &str) -> crate::error::Result<OtpStartResponse>;

    /// Verify the OTP code and return the wallet info (creating the wallet
    /// on first verification, or returning the existing wallet on subsequent
    /// verifications).
    async fn verify_otp(&self, email: &str, code: &str) -> crate::error::Result<WalletInfo>;
}

// ── Live (Circle WaaS) ─────────────────────────────────────────────────────

pub struct CircleProvider<'a> {
    pub http: &'a Client,
    pub config: &'a Config,
}

#[derive(Serialize)]
struct CreateWalletReq<'a> {
    email: &'a str,
    chains: &'a [&'static str],
    auth: &'a serde_json::Value,
}

#[derive(Deserialize)]
struct CircleWalletResp {
    #[serde(rename = "walletId")]
    wallet_id: String,
    #[serde(rename = "addresses")]
    addresses: CircleAddresses,
}

#[derive(Deserialize)]
struct CircleAddresses {
    #[serde(default)]
    arc: Option<String>,
    #[serde(default)]
    base: Option<String>,
}

impl<'a> CircleProvider<'a> {
    pub fn new(http: &'a Client, config: &'a Config) -> Self {
        Self { http, config }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/v1{}", self.config.circle_base_url, path)
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.config.circle_api_key)
    }

    async fn post<TResp: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> crate::error::Result<TResp> {
        self.http
            .post(self.endpoint(path))
            .header("Authorization", self.auth_header())
            .json(body)
            .send()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle network: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle status: {e}")))?
            .json::<TResp>()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("circle decode: {e}")))
    }
}

#[async_trait]
impl WalletProvider for CircleProvider<'_> {
    async fn create_with_passkey(
        &self,
        email: &str,
        passkey_attestation: &serde_json::Value,
    ) -> crate::error::Result<WalletInfo> {
        let resp: CircleWalletResp = self
            .post(
                paths::WALLETS_CREATE,
                &CreateWalletReq {
                    email,
                    chains: &["arc-testnet", "base-sepolia"],
                    auth: passkey_attestation,
                },
            )
            .await?;
        materialize_wallet(resp)
    }

    async fn authenticate_with_passkey(
        &self,
        email: &str,
        passkey_assertion: &serde_json::Value,
    ) -> crate::error::Result<WalletInfo> {
        let resp: CircleWalletResp = self
            .post(
                paths::WALLETS_AUTH,
                &serde_json::json!({
                    "email": email,
                    "assertion": passkey_assertion,
                }),
            )
            .await?;
        materialize_wallet(resp)
    }

    async fn start_otp(&self, email: &str) -> crate::error::Result<OtpStartResponse> {
        #[derive(Deserialize)]
        struct CircleOtpResp {
            #[serde(rename = "challengeId")]
            challenge_id: String,
            #[serde(default = "default_expires_in")]
            expires_in: u32,
        }
        fn default_expires_in() -> u32 {
            600
        }

        let resp: CircleOtpResp = self
            .post(paths::OTP_START, &serde_json::json!({ "email": email }))
            .await?;
        Ok(OtpStartResponse {
            email: email.to_string(),
            challenge_id: resp.challenge_id,
            expires_in: resp.expires_in,
        })
    }

    async fn verify_otp(&self, email: &str, code: &str) -> crate::error::Result<WalletInfo> {
        let resp: CircleWalletResp = self
            .post(
                paths::OTP_VERIFY,
                &serde_json::json!({
                    "email": email,
                    "code": code,
                    "chains": ["arc-testnet", "base-sepolia"],
                }),
            )
            .await?;
        materialize_wallet(resp)
    }
}

fn materialize_wallet(resp: CircleWalletResp) -> crate::error::Result<WalletInfo> {
    let arc = resp
        .addresses
        .arc
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("circle missing arc address")))?;
    let base = resp
        .addresses
        .base
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("circle missing base address")))?;
    Ok(WalletInfo {
        wallet_id: resp.wallet_id,
        arc_address: arc,
        base_address: base,
        created_at: Utc::now(),
    })
}

// ── Mock (offline dev) ─────────────────────────────────────────────────────

pub struct MockProvider;

impl Default for MockProvider {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl WalletProvider for MockProvider {
    async fn create_with_passkey(
        &self,
        email: &str,
        _: &serde_json::Value,
    ) -> crate::error::Result<WalletInfo> {
        Ok(synthetic_wallet(email))
    }

    async fn authenticate_with_passkey(
        &self,
        email: &str,
        _: &serde_json::Value,
    ) -> crate::error::Result<WalletInfo> {
        Ok(synthetic_wallet(email))
    }

    async fn start_otp(&self, email: &str) -> crate::error::Result<OtpStartResponse> {
        Ok(OtpStartResponse {
            email: email.to_string(),
            challenge_id: format!("mock-otp-{}", Uuid::new_v4()),
            expires_in: 600,
        })
    }

    async fn verify_otp(&self, email: &str, _code: &str) -> crate::error::Result<WalletInfo> {
        // In mock mode any 6-digit code passes — caller is expected to bound
        // this behind `Config::circle_mock`.
        Ok(synthetic_wallet(email))
    }
}

fn synthetic_wallet(email: &str) -> WalletInfo {
    // Deterministic synthetic addresses keyed on email so a given dev sees
    // the same wallet across restarts.
    let seed = stable_hash(email);
    WalletInfo {
        wallet_id: format!("mock_wallet_{seed:016x}"),
        arc_address: format!("0xARC{seed:040x}"),
        base_address: format!("0xBASE{seed:040x}"),
        created_at: Utc::now(),
    }
}

fn stable_hash(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_create_returns_addresses_for_email() {
        let p = MockProvider;
        let w = p
            .create_with_passkey("alice@example.com", &serde_json::Value::Null)
            .await
            .unwrap();
        assert!(w.wallet_id.starts_with("mock_wallet_"));
        assert!(w.arc_address.starts_with("0xARC"));
        assert!(w.base_address.starts_with("0xBASE"));
    }

    #[tokio::test]
    async fn mock_create_is_deterministic_per_email() {
        let p = MockProvider;
        let a = p
            .create_with_passkey("a@x", &serde_json::Value::Null)
            .await
            .unwrap();
        let b = p
            .create_with_passkey("a@x", &serde_json::Value::Null)
            .await
            .unwrap();
        assert_eq!(a.wallet_id, b.wallet_id);
        assert_eq!(a.arc_address, b.arc_address);
    }

    #[tokio::test]
    async fn mock_otp_round_trip() {
        let p = MockProvider;
        let start = p.start_otp("alice@example.com").await.unwrap();
        assert!(!start.challenge_id.is_empty());
        let w = p.verify_otp("alice@example.com", "123456").await.unwrap();
        assert!(w.wallet_id.starts_with("mock_wallet_"));
    }
}
