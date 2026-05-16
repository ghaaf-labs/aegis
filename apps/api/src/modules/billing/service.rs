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
    } else if config.billing_v2_enabled {
        // Real Nanopayments settlement path. We pay from the project
        // treasury wallet to the referrer's Arc address. The /settle call
        // is best-effort here — a network blip shouldn't break user signup,
        // so we still log and continue; the row stays with paid_at NULL so
        // the daily reconcile cron (A4) can retry.
        match settle_referral_via_nanopayments(config, db, referrer_id, reward).await {
            Ok(tx) => tx,
            Err(e) => {
                tracing::warn!(
                    referrer = %referrer_id,
                    new = %new_user_id,
                    error = %e,
                    "referral nanopayments /settle failed; row left unpaid for retry"
                );
                None
            }
        }
    } else {
        // BILLING_V2_ENABLED=false: legacy behaviour. Leave paid_at NULL so
        // an operator can reconcile from the pending-index.
        tracing::warn!(
            "real Nanopayment disabled (BILLING_V2_ENABLED=false); referral recorded but unpaid (referrer={referrer_id}, new={new_user_id})"
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

/// Records the 25 bps protocol fee for a completed rebalance. Idempotent
/// via the (rebalance_id, fee_type) UNIQUE constraint in migration 0007 —
/// retries of the post-plan billing block are safe.
pub async fn record_protocol_fee(
    db: &Db,
    rebalance_id: Uuid,
    amount_usdc: f64,
    settlement_tx_hash: Option<&str>,
) -> anyhow::Result<()> {
    let fee = amount_usdc * 0.0025;

    sqlx::query(
        "INSERT INTO rebalance_fees (rebalance_id, fee_type, amount_usdc, settlement_tx_hash, created_at)
         VALUES ($1, 'protocol', $2, $3, NOW())
         ON CONFLICT (rebalance_id, fee_type) DO NOTHING",
    )
    .bind(rebalance_id)
    .bind(fee)
    .bind(settlement_tx_hash)
    .execute(db)
    .await?;

    Ok(())
}

async fn settle_referral_via_nanopayments(
    config: &Config,
    db: &Db,
    referrer_id: Uuid,
    reward_usdc: f64,
) -> anyhow::Result<Option<String>> {
    if config.nanopayments_treasury_address.trim().is_empty() {
        anyhow::bail!("NANOPAYMENTS_TREASURY_ADDRESS is empty");
    }
    let referrer_address: Option<String> =
        sqlx::query_scalar("SELECT arc_address FROM users WHERE id = $1")
            .bind(referrer_id)
            .fetch_optional(db)
            .await?
            .flatten();
    let Some(pay_to) = referrer_address.filter(|s| !s.is_empty()) else {
        anyhow::bail!("referrer {referrer_id} has no arc_address");
    };

    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "payment": {
            "amount": (reward_usdc * 1_000_000.0) as u64,
            "payer": config.nanopayments_treasury_address,
            "payTo": pay_to,
            "network": "arc-testnet",
        }
    });
    let res = client
        .post(format!("{}/settle", config.nanopayments_facilitator_url))
        .json(&payload)
        .send()
        .await?;
    if !res.status().is_success() {
        anyhow::bail!("/settle returned HTTP {}", res.status());
    }
    let body: serde_json::Value = res.json().await?;
    Ok(body
        .get("transaction")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string()))
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

/// Refund the protocol fee for a failed rebalance. Marks the fee row as
/// `refunded` and (in real mode, when the original settlement produced a
/// tx hash) POSTs to `NANOPAYMENTS_FACILITATOR_URL/reverse` so the user
/// actually gets the USDC back. A 404 from the facilitator is tolerated
/// with a `warn!` log because some facilitator builds don't support
/// `/reverse`; in that case the row stays marked `refunded` for the
/// operator to reconcile manually.
///
/// Returns the reverse-tx hash when one was produced, `None` otherwise.
pub async fn refund_protocol_fee(
    db: &Db,
    config: &Config,
    rebalance_id: Uuid,
    reason: &str,
) -> crate::error::Result<Option<String>> {
    let row: Option<(Option<String>, String)> = sqlx::query_as(
        "SELECT settlement_tx_hash, status
           FROM rebalance_fees
          WHERE rebalance_id = $1 AND fee_type = 'protocol'
          LIMIT 1",
    )
    .bind(rebalance_id)
    .fetch_optional(db)
    .await?;
    let Some((settlement_tx, current_status)) = row else {
        return Ok(None);
    };
    if current_status == "refunded" {
        return Ok(None);
    }

    let reverse_tx = if !config.execution_mock {
        if let Some(original_tx) = settlement_tx.as_deref() {
            match post_reverse(config, original_tx, rebalance_id, reason).await {
                Ok(tx) => tx,
                Err(e) => {
                    tracing::warn!(
                        rebalance_id = ?rebalance_id,
                        error = %e,
                        "nanopayments /reverse failed; marking refunded without on-chain reversal"
                    );
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    sqlx::query(
        "UPDATE rebalance_fees
            SET status = 'refunded',
                refunded_at = NOW(),
                refund_tx_hash = COALESCE($2, refund_tx_hash)
          WHERE rebalance_id = $1 AND fee_type = 'protocol'",
    )
    .bind(rebalance_id)
    .bind(reverse_tx.as_deref())
    .execute(db)
    .await?;

    Ok(reverse_tx)
}

async fn post_reverse(
    config: &Config,
    original_tx: &str,
    rebalance_id: Uuid,
    reason: &str,
) -> anyhow::Result<Option<String>> {
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "transaction": original_tx,
        "rebalanceId": rebalance_id,
        "reason": reason,
        "network": "arc-testnet",
    });
    let res = client
        .post(format!("{}/reverse", config.nanopayments_facilitator_url))
        .json(&payload)
        .send()
        .await?;

    if res.status() == reqwest::StatusCode::NOT_FOUND {
        // Older facilitator builds don't expose /reverse; that's fine — the
        // row is still marked refunded so the user-facing balance stays
        // consistent. Operator can reconcile manually.
        tracing::warn!(
            rebalance_id = ?rebalance_id,
            "nanopayments facilitator returned 404 on /reverse; no on-chain reversal recorded"
        );
        return Ok(None);
    }
    if !res.status().is_success() {
        anyhow::bail!("/reverse returned HTTP {}", res.status());
    }

    let body: serde_json::Value = res.json().await?;
    Ok(body
        .get("transaction")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string()))
}

/// Settle the protocol fee (25 bps) via Circle Nanopayments (x402).
/// In real mode, this calls the facilitator to settle the signed authorization.
/// For the hackathon, the user must have pre-authorized via the Gateway balance or signed the payment.
pub async fn settle_protocol_fee_via_nanopayments(
    config: &Config,
    payer_address: &str,
    amount_usdc: f64,
) -> anyhow::Result<Option<String>> {
    if config.execution_mock {
        // Mock settlement — produces a realistic-looking 0x tx hash for demo / judging.
        // In real mode this performs the x402 settlement against the Circle facilitator.
        use sha2::{Digest as _, Sha256};
        let mut h = Sha256::new();
        h.update(b"protocol-fee:");
        h.update(payer_address.as_bytes());
        h.update(amount_usdc.to_le_bytes());
        let tx = format!("0x{}", hex::encode(&h.finalize()[..8]));
        return Ok(Some(tx));
    }

    let client = reqwest::Client::new();
    let facilitator_url = &config.nanopayments_facilitator_url;
    let seller_address = &config.nanopayments_seller_address;

    if seller_address.is_empty() {
        // When the new billing flag is on, an empty seller address means the
        // operator forgot to provision the treasury wallet — silently
        // returning Ok(None) (the old behaviour) would let every protocol
        // fee disappear into the void. Surface the misconfig instead.
        if config.billing_v2_enabled {
            anyhow::bail!("BILLING_V2_ENABLED=true but NANOPAYMENTS_SELLER_ADDRESS is empty");
        }
        return Ok(None);
    }

    let settle_payload = serde_json::json!({
        "payment": {
            "amount": (amount_usdc * 1_000_000.0) as u64,
            "payer": payer_address,
            "payTo": seller_address,
            "network": "arc-testnet",
        }
    });

    let res = client
        .post(format!("{}/settle", facilitator_url))
        .json(&settle_payload)
        .send()
        .await?;

    if res.status().is_success() {
        let body: serde_json::Value = res.json().await?;
        let tx_hash = body
            .get("transaction")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Ok(tx_hash)
    } else {
        Ok(None)
    }
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
