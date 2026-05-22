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

/// Bundle returned to the browser after `init_signup` or `init_login`.
/// The browser uses these fields to instantiate `@circle-fin/w3s-pw-web-sdk`
/// and complete the PIN ceremony.
///
/// `challenge_id` is `Some` only for new users (signup); returning users get
/// `None` and the SDK just authenticates the fresh token.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserTokenBundle {
    pub user_token: String,
    pub encryption_key: String,
    pub app_id: String,
    pub challenge_id: Option<String>,
}

/// Response sent after a successful wallet init or login. JWT is set only in
/// an `httpOnly` cookie; the raw app token is intentionally not serialized to
/// the browser because storing it in JS-visible storage makes logout
/// unreliable.
///
/// `wallet` is populated immediately for returning users whose wallet is
/// already provisioned; for fresh signups it's `None` until the browser
/// completes the SDK challenge and `/auth/wallet/status` reports the wallet
/// is ready. `bundle` is present only when the browser must execute a Circle
/// challenge; returning users with an existing wallet do not need short-lived
/// Circle credentials in the response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletAuthResponse {
    #[serde(skip_serializing)]
    pub token: String,
    pub user: WalletUserPublic,
    pub wallet: Option<WalletInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle: Option<UserTokenBundle>,
    pub is_new_user: bool,
}

/// Polled by the browser after the SDK completes the challenge. Returns
/// `Some(wallet)` once Circle has provisioned the addresses, `None` while
/// still pending.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletStatusResponse {
    pub wallet: Option<WalletInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletAuthCodeResponse {
    pub challenge_id: Uuid,
    pub email: String,
    pub expires_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletAuthReadinessResponse {
    pub circle_mock: bool,
    pub email_delivery_configured: bool,
    pub dev_codes_enabled: bool,
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
#[serde(rename_all = "camelCase")]
pub struct InitWalletRequest {
    pub email: String,
    pub challenge_id: Uuid,
    pub code: String,
    /// Optional 8-char user handle of the referrer (matches the `handle`
    /// column on `v_trustability_per_user`). Drives the referral payout
    /// loop in `billing::record_referral`. Honoured only on signup.
    #[serde(default)]
    pub referrer_handle: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WalletAuthIntent {
    Signup,
    Login,
}

impl WalletAuthIntent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Signup => "signup",
            Self::Login => "login",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestWalletAuthCode {
    pub email: String,
    pub intent: WalletAuthIntent,
    #[serde(default)]
    pub referrer_handle: Option<String>,
}
