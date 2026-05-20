use std::time::Duration;

use chrono::{Datelike, Timelike, Utc};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::router::AppState;

type HmacSha = Hmac<Sha256>;

/// Subscribe a user to the daily digest.
pub async fn subscribe(state: &AppState, user_id: Uuid, email: &str) -> Result<String> {
    let token = mint_token(state, user_id);
    sqlx::query(
        "INSERT INTO digest_subscriptions (user_id, email, unsubscribe_token)
         VALUES ($1, $2, $3)
         ON CONFLICT (user_id) DO UPDATE SET email = EXCLUDED.email",
    )
    .bind(user_id)
    .bind(email)
    .bind(&token)
    .execute(&state.db)
    .await?;
    Ok(token)
}

pub async fn unsubscribe_by_token(state: &AppState, token: &str) -> Result<()> {
    let user_id = verify_token(state, token)?;
    sqlx::query("DELETE FROM digest_subscriptions WHERE user_id = $1")
        .bind(user_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

pub async fn unsubscribe_by_user(state: &AppState, user_id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM digest_subscriptions WHERE user_id = $1")
        .bind(user_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

fn mint_token(state: &AppState, user_id: Uuid) -> String {
    let mut mac = HmacSha::new_from_slice(state.config.digest_secret.as_bytes())
        .expect("HMAC accepts any key size");
    mac.update(user_id.as_bytes());
    let tag = mac.finalize().into_bytes();
    format!("{}.{}", user_id, hex::encode(tag))
}

fn verify_token(state: &AppState, token: &str) -> Result<Uuid> {
    let (uid_part, sig_part) = token
        .split_once('.')
        .ok_or_else(|| AppError::BadRequest("invalid token format".into()))?;
    let user_id =
        Uuid::parse_str(uid_part).map_err(|_| AppError::BadRequest("invalid token uuid".into()))?;
    let expected_sig = hex::decode(sig_part)
        .map_err(|_| AppError::BadRequest("invalid token signature".into()))?;
    let mut mac = HmacSha::new_from_slice(state.config.digest_secret.as_bytes())
        .expect("HMAC accepts any key size");
    mac.update(user_id.as_bytes());
    let want = mac.finalize().into_bytes();
    if want.ct_eq(&expected_sig).unwrap_u8() == 1 {
        Ok(user_id)
    } else {
        Err(AppError::Unauthorized("invalid unsubscribe token".into()))
    }
}

/// Long-running worker. Sleeps directly until the next `DIGEST_HOUR_UTC`
/// rather than polling every minute (avoids 60 wakes/hour for zero work).
pub fn spawn_digest_worker(state: AppState, cancel: CancellationToken) {
    tokio::spawn(async move {
        loop {
            let target_hour = state.config.digest_hour_utc;
            let sleep_for = duration_until_next_hour(Utc::now(), target_hour);
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(sleep_for) => {}
            }
            let now = Utc::now();
            if let Err(e) = send_due(&state, now).await {
                tracing::warn!(error=%e, "digest worker tick failed");
            }
        }
    });
}

/// Compute the duration until the next occurrence of `target_hour` UTC. If
/// we're already past `target_hour` today, jumps to tomorrow. Guarantees a
/// minimum 60-second wait so a clock skew doesn't immediately re-fire.
fn duration_until_next_hour(now: chrono::DateTime<Utc>, target_hour: u32) -> Duration {
    let today_target = now
        .date_naive()
        .and_hms_opt(target_hour, 0, 0)
        .and_then(|t| t.and_local_timezone(Utc).single());
    let mut target = today_target.unwrap_or(now);
    if target <= now {
        target += chrono::Duration::days(1);
    }
    let secs = (target - now).num_seconds().max(60) as u64;
    Duration::from_secs(secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn delay_to_next_hour_jumps_to_tomorrow_when_past_today() {
        // 09:00 UTC, target = 08:00 → should sleep ~23h.
        let now = Utc.with_ymd_and_hms(2026, 5, 14, 9, 0, 0).unwrap();
        let d = duration_until_next_hour(now, 8);
        assert!(d.as_secs() >= 23 * 3600 && d.as_secs() <= 24 * 3600);
    }

    #[test]
    fn delay_to_next_hour_fires_today_when_before_target() {
        // 06:00 UTC, target = 08:00 → should sleep ~2h.
        let now = Utc.with_ymd_and_hms(2026, 5, 14, 6, 0, 0).unwrap();
        let d = duration_until_next_hour(now, 8);
        assert!(d.as_secs() >= 2 * 3600 && d.as_secs() <= 2 * 3600 + 60);
    }

    #[test]
    fn delay_to_next_hour_at_exact_match_jumps_to_tomorrow() {
        let now = Utc.with_ymd_and_hms(2026, 5, 14, 8, 0, 0).unwrap();
        let d = duration_until_next_hour(now, 8);
        assert!(d.as_secs() >= 23 * 3600);
    }
}

#[derive(sqlx::FromRow)]
struct DueSub {
    user_id: Uuid,
    email: String,
    unsubscribe_token: String,
}

async fn send_due(state: &AppState, now: chrono::DateTime<Utc>) -> Result<()> {
    let day_start = now
        .with_hour(0)
        .and_then(|t| t.with_minute(0))
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0))
        .unwrap_or(now);

    let subs: Vec<DueSub> = sqlx::query_as(
        "SELECT user_id, email, unsubscribe_token
         FROM digest_subscriptions
         WHERE last_sent_at IS NULL OR last_sent_at < $1",
    )
    .bind(day_start)
    .fetch_all(&state.db)
    .await?;

    for sub in subs {
        if state.config.resend_api_key.is_empty() {
            tracing::info!(user=%sub.user_id, "digest: skipping (RESEND_API_KEY empty)");
        } else if let Err(e) = send_one(state, &sub).await {
            tracing::warn!(user=%sub.user_id, error=%e, "digest send failed");
            continue;
        }
        sqlx::query("UPDATE digest_subscriptions SET last_sent_at = NOW() WHERE user_id = $1")
            .bind(sub.user_id)
            .execute(&state.db)
            .await?;
    }
    Ok(())
}

async fn send_one(state: &AppState, sub: &DueSub) -> Result<()> {
    let body = render_digest_html(state, sub).await?;
    let payload = serde_json::json!({
        "from": state.config.digest_from,
        "to": [sub.email],
        "subject": "Aegis · daily portfolio digest",
        "html": body,
    });
    let resp = state
        .http
        .post("https://api.resend.com/emails")
        .bearer_auth(&state.config.resend_api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("resend net: {e}")))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::Internal(anyhow::anyhow!(
            "resend status {status}: {text}"
        )));
    }
    Ok(())
}

async fn render_digest_html(state: &AppState, sub: &DueSub) -> Result<String> {
    let portfolio_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM portfolios WHERE user_id = $1")
            .bind(sub.user_id)
            .fetch_one(&state.db)
            .await?;
    let recent_decisions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_decisions d
         JOIN portfolios p ON p.id = d.portfolio_id
         WHERE p.user_id = $1 AND d.created_at > NOW() - INTERVAL '24 hours'",
    )
    .bind(sub.user_id)
    .fetch_one(&state.db)
    .await?;
    // Unsubscribe is a backend GET — the link must hit the API, not the
    // Next.js frontend (which has no equivalent route).
    let unsub_link = format!(
        "{}/digest/unsubscribe?t={}",
        state.config.api_base_url, sub.unsubscribe_token
    );
    let mut h = handlebars::Handlebars::new();
    h.register_escape_fn(handlebars::no_escape);
    let template = include_str!("../../../templates/digest.html.hbs");
    let ctx = serde_json::json!({
        "year": Utc::now().year(),
        "portfolioCount": portfolio_count,
        "recentDecisions": recent_decisions,
        "unsubscribeUrl": unsub_link,
    });
    h.render_template(template, &ctx)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("handlebars: {e}")))
}
