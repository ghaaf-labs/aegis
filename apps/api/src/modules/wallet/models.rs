use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WalletUser {
    pub id: Uuid,
    pub email: String,
    pub risk_tolerance: String,
    pub investment_horizon_months: i32,
    pub account_status: String,
    pub custody_model: String,
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

/// Response sent after a successful email-code verify. The opaque session id is
/// set only in an `httpOnly` cookie and intentionally not serialized to the
/// browser.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletAuthResponse {
    #[serde(skip_serializing)]
    pub session_token: String,
    pub status: String,
    pub user: WalletUserPublic,
    pub wallet: Option<WalletInfo>,
    #[serde(skip_serializing)]
    pub is_new_user: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletSessionResponse {
    pub user: WalletUserPublic,
    pub wallet: Option<WalletInfo>,
    pub account_status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletAuthCodeResponse {
    pub challenge_id: Uuid,
    pub email: String,
    pub expires_at: DateTime<Utc>,
    pub resend_in_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletUserPublic {
    pub id: Uuid,
    pub email: String,
    pub risk_tolerance: String,
    pub account_status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartEmailAuthRequest {
    pub email: String,
    #[serde(default)]
    pub referrer_handle: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResendEmailAuthRequest {
    pub challenge_id: Uuid,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyEmailAuthRequest {
    pub challenge_id: Uuid,
    pub code: String,
    #[serde(default)]
    pub consent: Option<EmailAuthConsent>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailAuthConsent {
    #[allow(dead_code)]
    pub tos: bool,
    #[allow(dead_code)]
    pub privacy: bool,
    #[allow(dead_code)]
    pub tos_version: Option<String>,
    #[allow(dead_code)]
    pub privacy_version: Option<String>,
    #[allow(dead_code)]
    pub marketing_opt_in: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn verify_request_is_challenge_scoped() {
        let challenge_id = Uuid::new_v4();
        let request: VerifyEmailAuthRequest = serde_json::from_value(json!({
            "challengeId": challenge_id,
            "code": "123456",
            "consent": {
                "tos": true,
                "privacy": true,
                "tosVersion": "2026-05",
                "privacyVersion": "2026-05",
                "marketingOptIn": false
            }
        }))
        .expect("verify request should not require email");

        assert_eq!(request.challenge_id, challenge_id);
        assert_eq!(request.code, "123456");
        assert!(request.consent.as_ref().is_some_and(|c| c.tos && c.privacy));
    }
}
