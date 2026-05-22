use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::{error::AppError, middleware::auth::Claims, modules::gateway, router::AppState};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountExportResponse {
    status: &'static str,
    delivery_email: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteAccountRequest {
    confirm: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAccountResponse {
    deletion_requested_at: DateTime<Utc>,
    completes_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct AccountUser {
    id: Uuid,
    email: String,
    wallet_id: Option<String>,
    arc_address: Option<String>,
    base_address: Option<String>,
    deletion_requested_at: Option<DateTime<Utc>>,
}

pub async fn export(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> crate::error::Result<impl IntoResponse> {
    let user = account_user(&state, claims.sub).await?;
    let archive = export_archive(&state, claims.sub).await?;
    send_export_email(&state, &user.email, &archive).await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(AccountExportResponse {
            status: "queued",
            delivery_email: user.email,
        }),
    ))
}

pub async fn delete(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<DeleteAccountRequest>,
) -> crate::error::Result<axum::response::Response> {
    if !body.confirm {
        return Err(AppError::BadRequest("confirm_required".into()));
    }

    let user = account_user(&state, claims.sub).await?;
    if user.deletion_requested_at.is_none() {
        ensure_account_can_close(&state, &user).await?;
    }

    let deletion_requested_at: DateTime<Utc> = sqlx::query_scalar(
        "UPDATE users
         SET deletion_requested_at = COALESCE(deletion_requested_at, NOW())
         WHERE id = $1
         RETURNING deletion_requested_at",
    )
    .bind(user.id)
    .fetch_one(&state.db)
    .await?;

    sqlx::query(
        "UPDATE auth_sessions
         SET revoked_at = COALESCE(revoked_at, NOW())
         WHERE user_id = $1
           AND revoked_at IS NULL",
    )
    .bind(user.id)
    .execute(&state.db)
    .await?;

    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, cleared_session_cookie(&state));
    Ok((
        StatusCode::ACCEPTED,
        headers,
        Json(DeleteAccountResponse {
            deletion_requested_at,
            completes_at: deletion_requested_at + Duration::days(7),
        }),
    )
        .into_response())
}

async fn account_user(state: &AppState, user_id: Uuid) -> crate::error::Result<AccountUser> {
    let user = sqlx::query_as::<_, AccountUser>(
        "SELECT id, email, wallet_id, arc_address, base_address, deletion_requested_at
         FROM users
         WHERE id = $1
           AND anonymized_at IS NULL",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Unauthorized("unknown user".into()))?;
    Ok(user)
}

async fn ensure_account_can_close(
    state: &AppState,
    user: &AccountUser,
) -> crate::error::Result<()> {
    if !has_real_wallet(user) {
        return Ok(());
    }
    if state.config.circle_mock {
        return Err(AppError::ServiceUnavailable(
            "account balance cannot be verified in mock Circle mode".into(),
        ));
    }

    let balance =
        gateway::service::fetch_balance_for_user(&state.db, &state.http, &state.config, user.id)
            .await?;
    if balance_has_funds(balance.unified_usdc, balance.unified_eurc) {
        return Err(AppError::Conflict("funds_present".into()));
    }
    Ok(())
}

fn has_real_wallet(user: &AccountUser) -> bool {
    let Some(wallet_id) = non_empty(user.wallet_id.as_deref()) else {
        return false;
    };
    let Some(arc) = non_empty(user.arc_address.as_deref()) else {
        return false;
    };
    let Some(base) = non_empty(user.base_address.as_deref()) else {
        return false;
    };

    !wallet_id.starts_with("mock_wallet_")
        && !arc.starts_with("0xARC")
        && !base.starts_with("0xBASE")
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}

fn balance_has_funds(usdc: f64, eurc: f64) -> bool {
    usdc.abs() > 0.000001 || eurc.abs() > 0.000001
}

async fn export_archive(state: &AppState, user_id: Uuid) -> crate::error::Result<Value> {
    let archive: SqlJson<Value> = sqlx::query_scalar(
        r#"
        SELECT jsonb_build_object(
          'exportedAt', NOW(),
          'profile', jsonb_build_object(
            'id', u.id,
            'email', u.email,
            'riskTolerance', u.risk_tolerance,
            'investmentHorizonMonths', u.investment_horizon_months,
            'accountStatus', u.account_status,
            'custodyModel', u.custody_model,
            'createdAt', u.created_at
          ),
          'consent', jsonb_build_object(
            'tosVersion', u.tos_version,
            'privacyVersion', u.privacy_version,
            'consentedAt', u.consented_at,
            'marketingOptIn', u.marketing_opt_in
          ),
          'wallet', jsonb_build_object(
            'walletId', u.wallet_id,
            'walletSetId', u.wallet_set_id,
            'arcAddress', u.arc_address,
            'baseAddress', u.base_address
          ),
          'portfolios', COALESCE((
            SELECT jsonb_agg(jsonb_build_object(
              'id', p.id,
              'name', p.name,
              'totalValueUsd', p.total_value_usd,
              'totalPnlUsd', p.total_pnl_usd,
              'totalPnlPct', p.total_pnl_pct,
              'riskScore', p.risk_score,
              'goal', p.goal,
              'createdAt', p.created_at,
              'updatedAt', p.updated_at,
              'allocations', COALESCE((
                SELECT jsonb_agg(jsonb_build_object(
                  'id', a.id,
                  'assetSymbol', a.asset_symbol,
                  'quantity', a.quantity,
                  'targetWeight', a.target_weight,
                  'currentWeight', a.current_weight,
                  'valueUsd', a.value_usd,
                  'updatedAt', a.updated_at
                ) ORDER BY a.asset_symbol)
                FROM allocations a
                WHERE a.portfolio_id = p.id
              ), '[]'::jsonb)
            ) ORDER BY p.created_at)
            FROM portfolios p
            WHERE p.user_id = u.id
          ), '[]'::jsonb),
          'agentDecisions', COALESCE((
            SELECT jsonb_agg(jsonb_build_object(
              'id', d.id,
              'portfolioId', d.portfolio_id,
              'reasoning', d.reasoning,
              'recommendation', d.recommendation,
              'confidence', d.confidence,
              'triggeredBy', d.triggered_by,
              'createdAt', d.created_at
            ) ORDER BY d.created_at)
            FROM agent_decisions d
            JOIN portfolios p ON p.id = d.portfolio_id
            WHERE p.user_id = u.id
          ), '[]'::jsonb),
          'rebalanceEvents', COALESCE((
            SELECT jsonb_agg(jsonb_build_object(
              'id', r.id,
              'portfolioId', r.portfolio_id,
              'agentDecisionId', r.agent_decision_id,
              'status', r.status,
              'trades', r.trades,
              'executedAt', r.executed_at,
              'createdAt', r.created_at
            ) ORDER BY r.created_at)
            FROM rebalance_events r
            JOIN portfolios p ON p.id = r.portfolio_id
            WHERE p.user_id = u.id
          ), '[]'::jsonb)
        )
        FROM users u
        WHERE u.id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    Ok(archive.0)
}

async fn send_export_email(
    state: &AppState,
    email: &str,
    archive: &Value,
) -> crate::error::Result<()> {
    if state.config.resend_api_key.trim().is_empty() || state.config.digest_from.trim().is_empty() {
        return Err(AppError::ServiceUnavailable(
            "account export email is not configured".into(),
        ));
    }

    let archive = serde_json::to_vec_pretty(archive)?;
    let payload = json!({
        "from": state.config.digest_from,
        "to": [email],
        "subject": "Your Aegis data export",
        "html": "<p>Your Aegis data export is attached as JSON.</p><p>If you did not request this, contact support.</p>",
        "attachments": [{
            "filename": "aegis-data-export.json",
            "content": BASE64.encode(archive),
            "content_type": "application/json"
        }]
    });

    state
        .http
        .post("https://api.resend.com/emails")
        .bearer_auth(&state.config.resend_api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("resend export email request: {e}")))?
        .error_for_status()
        .map_err(|e| AppError::ServiceUnavailable(format!("account export email failed: {e}")))?;

    Ok(())
}

fn cleared_session_cookie(state: &AppState) -> HeaderValue {
    let secure = if state.config.session_cookie_secure {
        "; Secure"
    } else {
        ""
    };
    let cleared = format!(
        "{name}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT{secure}",
        name = state.config.session_cookie_name
    );
    HeaderValue::from_str(&cleared).expect("session cookie value is ASCII")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_wallet_detection_rejects_missing_or_mock_fields() {
        let mut user = AccountUser {
            id: Uuid::new_v4(),
            email: "a@example.com".into(),
            wallet_id: Some("wallet-1".into()),
            arc_address: Some("0x1111111111111111111111111111111111111111".into()),
            base_address: Some("0x2222222222222222222222222222222222222222".into()),
            deletion_requested_at: None,
        };
        assert!(has_real_wallet(&user));

        user.wallet_id = Some("mock_wallet_user".into());
        assert!(!has_real_wallet(&user));
    }

    #[test]
    fn tiny_rounding_dust_does_not_block_account_close() {
        assert!(!balance_has_funds(0.0000001, 0.0));
        assert!(balance_has_funds(0.01, 0.0));
        assert!(balance_has_funds(0.0, 0.01));
    }
}
