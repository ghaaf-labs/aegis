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

/// Response sent after a successful wallet init or login. JWT is also set in
/// an `httpOnly` cookie so SSR can read it; we return the raw token for SPA
/// clients that prefer `Authorization` headers.
///
/// `wallet` is populated immediately for returning users whose wallet is
/// already provisioned; for fresh signups it's `None` until the browser
/// completes the SDK challenge and `/auth/wallet/status` reports the wallet
/// is ready.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletAuthResponse {
    pub token: String,
    pub user: WalletUserPublic,
    pub wallet: Option<WalletInfo>,
    pub bundle: UserTokenBundle,
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
    /// Optional 8-char user handle of the referrer (matches the `handle`
    /// column on `v_trustability_per_user`). Drives the referral payout
    /// loop in `billing::record_referral`. Honoured only on signup.
    #[serde(default)]
    pub referrer_handle: Option<String>,
}
