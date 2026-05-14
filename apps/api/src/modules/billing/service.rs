//! Referral attribution + Nanopayment payout.

use serde::Serialize;
use uuid::Uuid;

use crate::config::Config;
use crate::db::Db;
use crate::error::AppError;
use crate::modules::sse::{SseEvent, SseSender};

/// Default referral reward in USDC. Override via `REFERRAL_REWARD_USDC` env.
pub const DEFAULT_REWARD_USDC: f64 = 0.5;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReferralCreditedPayload {
    pub referrer_user_id: Uuid,
    pub new_user_id: Uuid,
    pub reward_usdc: f64,
    pub tx_hash: Option<String>,
}

/// Record + (mock-)pay a referral. Idempotent — UNIQUE(new_user_id) prevents
/// double payouts even under retry. Returns `Ok(None)` when the handle
/// doesn't resolve to an existing user (the URL was tampered with or the
/// referrer hasn't signed up yet).
pub async fn record_referral(
    db: &Db,
    config: &Config,
    sse: &SseSender,
    referrer_handle: &str,
    new_user_id: Uuid,
) -> crate::error::Result<Option<ReferralCreditedPayload>> {
    let referrer_id = match resolve_handle(db, referrer_handle).await? {
        Some(id) if id != new_user_id => id,
        Some(_) => return Ok(None), // self-referral — silently skip
        None => return Ok(None),
    };

    let reward = config_reward(config);

    let inserted: Option<(Uuid,)> = sqlx::query_as(
        "INSERT INTO referrals (referrer_user_id, new_user_id, reward_usdc)
         VALUES ($1, $2, $3)
         ON CONFLICT (new_user_id) DO NOTHING
         RETURNING id",
    )
    .bind(referrer_id)
    .bind(new_user_id)
    .bind(reward)
    .fetch_optional(db)
    .await?;

    if inserted.is_none() {
        // Already credited.
        return Ok(None);
    }

    // Settle the payout.
    let tx_hash = if config.execution_mock {
        Some(mock_tx_hash(referrer_id, new_user_id))
    } else {
        // Real Nanopayment path — gated TODO. Don't fail the wallet-create
        // call over a missing treasury wallet; instead leave paid_at NULL
        // so an operator can reconcile from the pending-index.
        tracing::warn!(
            "real Nanopayment not implemented; referral recorded but unpaid (referrer={referrer_id}, new={new_user_id})"
        );
        None
    };

    if tx_hash.is_some() {
        sqlx::query("UPDATE referrals SET paid_at = NOW(), tx_hash = $1 WHERE new_user_id = $2")
            .bind(tx_hash.as_deref().unwrap_or(""))
            .bind(new_user_id)
            .execute(db)
            .await?;
    }

    let payload = ReferralCreditedPayload {
        referrer_user_id: referrer_id,
        new_user_id,
        reward_usdc: reward,
        tx_hash: tx_hash.clone(),
    };
    let _ = sse.send(SseEvent::ReferralCredited(payload.clone()));
    Ok(Some(payload))
}

async fn resolve_handle(db: &Db, handle: &str) -> crate::error::Result<Option<Uuid>> {
    if handle.len() < 4 || handle.len() > 64 || !handle.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::BadRequest("invalid referrer handle".into()));
    }
    let row: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE SUBSTRING(md5(id::text), 1, 8) = $1 LIMIT 1",
    )
    .bind(handle)
    .fetch_optional(db)
    .await?;
    Ok(row)
}

fn config_reward(config: &Config) -> f64 {
    let configured = std::env::var("REFERRAL_REWARD_USDC")
        .ok()
        .and_then(|v| v.parse::<f64>().ok());
    let v = configured.unwrap_or(DEFAULT_REWARD_USDC);
    // Sanity clamp: don't accidentally send the treasury wallet a 10 USDC
    // reward via a typo'd env var. 5 USDC is plenty for a hackathon demo.
    let _ = config; // keep signature stable for future Config-derived rewards
    v.clamp(0.01, 5.00)
}

fn mock_tx_hash(referrer: Uuid, new_user: Uuid) -> String {
    // SHA-256 prefix is overkill here; the goal is just a stable per-pair id.
    use sha2::{Digest as _, Sha256};
    let mut h = Sha256::new();
    h.update(b"referral:");
    h.update(referrer.as_bytes());
    h.update(new_user.as_bytes());
    format!("0xmock:{}", hex::encode(&h.finalize()[..8]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_reward_is_in_clamp_range() {
        assert!((0.01..=5.0).contains(&DEFAULT_REWARD_USDC));
    }

    #[test]
    fn mock_tx_hash_is_deterministic_and_prefixed() {
        let r = Uuid::nil();
        let n = Uuid::from_u128(1);
        let a = mock_tx_hash(r, n);
        let b = mock_tx_hash(r, n);
        assert_eq!(a, b);
        assert!(a.starts_with("0xmock:"));
    }
}
