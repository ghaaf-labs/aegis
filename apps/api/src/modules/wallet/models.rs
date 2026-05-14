use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WalletUser {
    pub id: Uuid,
    pub email: String,
    pub risk_tolerance: String,
    pub investment_horizon_months: i32,
    pub wallet_id: Option<String>,
    pub arc_address: Option<String>,
    pub base_address: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// On-the-wire shape mirroring `WalletInfo` in `packages/shared/src/types.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletInfo {
    pub wallet_id: String,
    pub arc_address: String,
    pub base_address: String,
    pub created_at: DateTime<Utc>,
}

/// Response sent after a successful wallet create or login. JWT is also set
/// in an `httpOnly` cookie so SSR can read it; we return the raw token for
/// SPA clients that want to put it in an `Authorization` header.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletAuthResponse {
    pub token: String,
    pub wallet: WalletInfo,
    pub user: WalletUserPublic,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletUserPublic {
    pub id: Uuid,
    pub email: String,
    pub risk_tolerance: String,
}

// ── Request shapes ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RegisterPasskeyRequest {
    pub email: String,
    /// Opaque payload from `navigator.credentials.create()` — passed through
    /// to Circle WaaS which validates the WebAuthn ceremony.
    #[serde(default)]
    pub passkey_attestation: serde_json::Value,
    /// Optional 8-char user handle of the referrer (matches the `handle`
    /// column on `v_trustability_per_user`). Drives the referral payout
    /// loop in `billing::record_referral`.
    #[serde(default, rename = "referrerHandle")]
    pub referrer_handle: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginPasskeyRequest {
    pub email: String,
    /// Opaque payload from `navigator.credentials.get()`.
    #[serde(default)]
    pub passkey_assertion: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct OtpStartRequest {
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct OtpVerifyRequest {
    pub email: String,
    pub code: String,
    /// Referrer handle — same semantics as `RegisterPasskeyRequest`.
    #[serde(default, rename = "referrerHandle")]
    pub referrer_handle: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OtpStartResponse {
    pub email: String,
    pub challenge_id: String,
    /// Seconds before the code expires (Circle defaults to 600).
    pub expires_in: u32,
}
