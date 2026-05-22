use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;
use sqlx::types::Json as SqlJson;
use sqlx::Row;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::auth::Claims,
    modules::{
        gateway,
        wallet::{
            self, models::WalletAuthCodeResponse, provider::MockProvider, service::WalletService,
        },
        wallet_routes,
    },
    router::AppState,
};

const ACCOUNT_EXPORT_RATE_WINDOW_HOURS: i64 = 24;
const ACCOUNT_EXPORT_TTL_HOURS: i64 = 24;
const EXPORT_RATE_LIMIT_SQL: &str = r#"
        SELECT delivered_at
        FROM account_export_jobs
        WHERE user_id = $1
          AND delivered_at IS NOT NULL
          AND delivered_at > $2
        ORDER BY delivered_at ASC
        LIMIT 1
        "#;
const EXPORT_ARCHIVE_SQL: &str = r#"
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
            'walletSetId', u.wallet_set_id,
            'networks', COALESCE((
              SELECT jsonb_agg(jsonb_build_object(
                'blockchain', n.blockchain,
                'walletId', n.circle_wallet_id,
                'address', n.address,
                'accountType', n.account_type,
                'state', n.state
              ) ORDER BY n.blockchain)
              FROM user_wallet_networks n
              WHERE n.user_id = u.id
                AND n.account_type = 'SCA'
                AND n.state = 'LIVE'
                AND (
                  NULLIF($2, '') IS NULL
                  OR n.wallet_set_id = NULLIF($2, '')
                )
            ), '[]'::jsonb)
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
          ), '[]'::jsonb),
          'taxLots', COALESCE((
            SELECT jsonb_agg(jsonb_build_object(
              'id', l.id,
              'allocationId', l.allocation_id,
              'portfolioId', p.id,
              'assetSymbol', a.asset_symbol,
              'acquiredAt', l.acquired_at,
              'quantity', l.quantity,
              'basisUsd', l.basis_usd,
              'disposedAt', l.disposed_at
            ) ORDER BY l.acquired_at)
            FROM cost_basis_lots l
            JOIN allocations a ON a.id = l.allocation_id
            JOIN portfolios p ON p.id = a.portfolio_id
            WHERE p.user_id = u.id
          ), '[]'::jsonb),
          'referrals', COALESCE((
            SELECT jsonb_agg(jsonb_build_object(
              'id', r.id,
              'role', CASE
                WHEN r.referrer_user_id = u.id THEN 'referrer'
                ELSE 'referred'
              END,
              'rewardUsdc', r.reward_usdc,
              'paidAt', r.paid_at,
              'txHash', r.tx_hash,
              'createdAt', r.created_at
            ) ORDER BY r.created_at)
            FROM referrals r
            WHERE r.referrer_user_id = u.id
               OR r.new_user_id = u.id
          ), '[]'::jsonb)
        )
        FROM users u
        WHERE u.id = $1
        "#;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountExportResponse {
    status: &'static str,
    delivery_email: String,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteAccountRequest {
    confirm: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountEmailStartRequest {
    email: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountEmailVerifyRequest {
    challenge_id: Uuid,
    code: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountEmailUpdateResponse {
    email: String,
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
    deletion_requested_at: Option<DateTime<Utc>>,
}

pub async fn export(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> crate::error::Result<impl IntoResponse> {
    let user = account_user(&state, claims.sub).await?;
    ensure_export_rate_limit_available(&state, claims.sub).await?;
    let archive = export_archive(&state, claims.sub).await?;
    let job = create_export_job(&state, claims.sub, archive).await?;
    let download_url = export_download_url(&state, job.id);
    send_export_email(&state, &user.email, &download_url, job.expires_at).await?;
    mark_export_delivered(&state, job.id).await?;
    crate::modules::analytics::service::emit(
        &state.db,
        Some(claims.sub),
        "account.export_requested",
        json!({
            "expiresAt": job.expires_at,
        }),
    )
    .await;

    Ok((
        StatusCode::ACCEPTED,
        Json(AccountExportResponse {
            status: "queued",
            delivery_email: user.email,
            expires_at: job.expires_at,
        }),
    ))
}

pub async fn download_export(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> crate::error::Result<axum::response::Response> {
    let job_id = verify_export_token(&state, &token)?;
    let archive: SqlJson<Value> = sqlx::query_scalar(
        "SELECT archive
         FROM account_export_jobs
         WHERE id = $1
           AND expires_at > NOW()",
    )
    .bind(job_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("account export".into()))?;

    export_archive_download_response(&archive.0)
}

async fn ensure_export_rate_limit_available(
    state: &AppState,
    user_id: Uuid,
) -> crate::error::Result<()> {
    let now = Utc::now();
    let since = now - Duration::hours(ACCOUNT_EXPORT_RATE_WINDOW_HOURS);
    let delivered_at: Option<DateTime<Utc>> = sqlx::query_scalar(EXPORT_RATE_LIMIT_SQL)
        .bind(user_id)
        .bind(since)
        .fetch_optional(&state.db)
        .await?;

    if let Some(delivered_at) = delivered_at {
        let reset_at = delivered_at + Duration::hours(ACCOUNT_EXPORT_RATE_WINDOW_HOURS);
        let retry_after = (reset_at - now).num_seconds().max(1);
        return Err(AppError::TooManyRequests(format!(
            "rate_limited:{retry_after}"
        )));
    }

    Ok(())
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
    crate::modules::analytics::service::emit(
        &state.db,
        Some(user.id),
        "account.delete_requested",
        json!({}),
    )
    .await;

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

pub async fn start_email_update(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<AccountEmailStartRequest>,
) -> crate::error::Result<Json<WalletAuthCodeResponse>> {
    let user = account_user(&state, claims.sub).await?;
    let normalized = body.email.trim().to_ascii_lowercase();
    if normalized == user.email {
        return Err(AppError::BadRequest("email_unchanged".into()));
    }
    ensure_email_available(&state, user.id, &normalized).await?;

    let provider = MockProvider;
    let service = WalletService::new(&state.db, &provider, &state.config, &state.sse);
    let issue = service.request_auth_code(&normalized, None).await?;
    wallet::handlers::deliver_code_issue(&state, issue).await
}

pub async fn verify_email_update(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<AccountEmailVerifyRequest>,
) -> crate::error::Result<Json<AccountEmailUpdateResponse>> {
    let user = account_user(&state, claims.sub).await?;
    let provider = MockProvider;
    let service = WalletService::new(&state.db, &provider, &state.config, &state.sse);
    let verified = service
        .verify_auth_code_for_email_update(user.id, body.challenge_id, &body.code)
        .await?;
    ensure_email_available(&state, user.id, &verified.email).await?;
    let email = sqlx::query_scalar::<_, String>(
        "UPDATE users
         SET email = $2
         WHERE id = $1
           AND NOT EXISTS (
             SELECT 1
             FROM users other
             WHERE other.email = $2
               AND other.id <> $1
               AND other.deletion_requested_at IS NULL
               AND other.anonymized_at IS NULL
           )
         RETURNING email",
    )
    .bind(user.id)
    .bind(&verified.email)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Conflict("email_in_use".into()))?;

    crate::modules::analytics::service::emit(
        &state.db,
        Some(user.id),
        "account.email_updated",
        json!({}),
    )
    .await;

    Ok(Json(AccountEmailUpdateResponse { email }))
}

async fn account_user(state: &AppState, user_id: Uuid) -> crate::error::Result<AccountUser> {
    let user = sqlx::query_as::<_, AccountUser>(
        "SELECT id, email, deletion_requested_at
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

async fn ensure_email_available(
    state: &AppState,
    user_id: Uuid,
    email: &str,
) -> crate::error::Result<()> {
    let in_use: bool = sqlx::query_scalar(
        "SELECT EXISTS (
           SELECT 1
           FROM users
           WHERE email = $1
             AND id <> $2
             AND deletion_requested_at IS NULL
             AND anonymized_at IS NULL
         )",
    )
    .bind(email)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    if in_use {
        return Err(AppError::Conflict("email_in_use".into()));
    }
    Ok(())
}

async fn ensure_account_can_close(
    state: &AppState,
    user: &AccountUser,
) -> crate::error::Result<()> {
    if !wallet_routes::user_has_arc_and_base(&state.db, user.id, &state.config.circle_wallet_set_id)
        .await?
    {
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

fn balance_has_funds(usdc: f64, eurc: f64) -> bool {
    usdc.abs() > 0.000001 || eurc.abs() > 0.000001
}

async fn export_archive(state: &AppState, user_id: Uuid) -> crate::error::Result<Value> {
    let archive: SqlJson<Value> = sqlx::query_scalar(EXPORT_ARCHIVE_SQL)
        .bind(user_id)
        .bind(state.config.circle_wallet_set_id.trim())
        .fetch_one(&state.db)
        .await?;
    Ok(archive.0)
}

#[derive(Debug)]
struct ExportJob {
    id: Uuid,
    expires_at: DateTime<Utc>,
}

async fn create_export_job(
    state: &AppState,
    user_id: Uuid,
    archive: Value,
) -> crate::error::Result<ExportJob> {
    let expires_at = Utc::now() + Duration::hours(ACCOUNT_EXPORT_TTL_HOURS);
    let row = sqlx::query(
        "INSERT INTO account_export_jobs (user_id, archive, expires_at)
         VALUES ($1, $2, $3)
         RETURNING id, expires_at",
    )
    .bind(user_id)
    .bind(SqlJson(archive))
    .bind(expires_at)
    .fetch_one(&state.db)
    .await?;

    Ok(ExportJob {
        id: row.try_get("id")?,
        expires_at: row.try_get("expires_at")?,
    })
}

async fn mark_export_delivered(state: &AppState, job_id: Uuid) -> crate::error::Result<()> {
    sqlx::query("UPDATE account_export_jobs SET delivered_at = NOW() WHERE id = $1")
        .bind(job_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

async fn send_export_email(
    state: &AppState,
    email: &str,
    download_url: &str,
    expires_at: DateTime<Utc>,
) -> crate::error::Result<()> {
    if state.config.resend_api_key.trim().is_empty() || state.config.digest_from.trim().is_empty() {
        return Err(AppError::ServiceUnavailable(
            "account export email is not configured".into(),
        ));
    }

    let html = format!(
        "<p>Your Aegis data export is ready.</p><p><a href=\"{download_url}\">Download your JSON archive</a></p><p>This link expires at {expires_at} UTC. If you did not request this, contact support.</p>"
    );
    let payload = json!({
        "from": state.config.digest_from,
        "to": [email],
        "subject": "Your Aegis data export",
        "html": html
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

fn export_download_url(state: &AppState, job_id: Uuid) -> String {
    export_download_url_from_parts(
        &state.config.api_base_url,
        &state.config.digest_secret,
        job_id,
    )
}

fn export_download_url_from_parts(api_base_url: &str, secret: &str, job_id: Uuid) -> String {
    format!(
        "{}/account/export/{}/download",
        api_base_url.trim_end_matches('/'),
        mint_export_token_with_secret(secret, job_id)
    )
}

fn mint_export_token_with_secret(secret: &str, job_id: Uuid) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key size");
    mac.update(b"account-export:");
    mac.update(job_id.as_bytes());
    let tag = mac.finalize().into_bytes();
    format!("{job_id}.{}", hex::encode(tag))
}

fn verify_export_token(state: &AppState, token: &str) -> crate::error::Result<Uuid> {
    verify_export_token_with_secret(&state.config.digest_secret, token)
}

fn verify_export_token_with_secret(secret: &str, token: &str) -> crate::error::Result<Uuid> {
    let (id_part, sig_part) = token
        .split_once('.')
        .ok_or_else(|| AppError::NotFound("account export".into()))?;
    let job_id =
        Uuid::parse_str(id_part).map_err(|_| AppError::NotFound("account export".into()))?;
    let actual = hex::decode(sig_part).map_err(|_| AppError::NotFound("account export".into()))?;
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key size");
    mac.update(b"account-export:");
    mac.update(job_id.as_bytes());
    let expected = mac.finalize().into_bytes();
    if expected.as_slice().ct_eq(&actual).unwrap_u8() == 1 {
        Ok(job_id)
    } else {
        Err(AppError::NotFound("account export".into()))
    }
}

fn export_archive_download_response(
    archive: &Value,
) -> crate::error::Result<axum::response::Response> {
    let body = serde_json::to_string_pretty(archive)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str("attachment; filename=\"aegis-data-export.json\"")
            .map_err(|e| AppError::Internal(anyhow::anyhow!("filename header: {e}")))?,
    );

    Ok((headers, body).into_response())
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
    fn tiny_rounding_dust_does_not_block_account_close() {
        assert!(!balance_has_funds(0.0000001, 0.0));
        assert!(balance_has_funds(0.01, 0.0));
        assert!(balance_has_funds(0.0, 0.01));
    }

    #[test]
    fn account_export_uses_wallet_network_routes_not_legacy_user_columns() {
        assert!(EXPORT_ARCHIVE_SQL.contains("user_wallet_networks"));
        assert!(EXPORT_ARCHIVE_SQL.contains("'networks'"));
        assert!(!EXPORT_ARCHIVE_SQL.contains("u.wallet_id"));
        assert!(!EXPORT_ARCHIVE_SQL.contains("u.arc_address"));
        assert!(!EXPORT_ARCHIVE_SQL.contains("u.base_address"));
    }

    #[test]
    fn account_export_includes_gdpr_portability_surfaces() {
        assert!(EXPORT_ARCHIVE_SQL.contains("'taxLots'"));
        assert!(EXPORT_ARCHIVE_SQL.contains("cost_basis_lots"));
        assert!(EXPORT_ARCHIVE_SQL.contains("'referrals'"));
        assert!(EXPORT_ARCHIVE_SQL.contains("FROM referrals"));
    }

    #[test]
    fn account_export_rate_limit_counts_only_delivered_jobs() {
        assert!(EXPORT_RATE_LIMIT_SQL.contains("account_export_jobs"));
        assert!(EXPORT_RATE_LIMIT_SQL.contains("delivered_at IS NOT NULL"));
        assert!(EXPORT_RATE_LIMIT_SQL.contains("delivered_at > $2"));
        assert!(!EXPORT_RATE_LIMIT_SQL.contains("auth_rate_limits"));
    }

    #[test]
    fn account_export_token_is_signed_and_tamper_resistant() {
        let job_id = Uuid::new_v4();
        let token = mint_export_token_with_secret("secret-a", job_id);
        assert_eq!(
            verify_export_token_with_secret("secret-a", &token).unwrap(),
            job_id
        );

        assert!(verify_export_token_with_secret("secret-b", &token).is_err());
        assert!(verify_export_token_with_secret("secret-a", &token.replace('.', "-")).is_err());
    }

    #[test]
    fn account_export_download_url_uses_signed_backend_route() {
        let job_id = Uuid::new_v4();
        let url = export_download_url_from_parts("https://api.aegis.local/", "secret-a", job_id);
        let token = url
            .strip_prefix("https://api.aegis.local/account/export/")
            .and_then(|path| path.strip_suffix("/download"))
            .expect("download URL should use account export route");

        assert_eq!(
            verify_export_token_with_secret("secret-a", token).unwrap(),
            job_id
        );
    }

    #[tokio::test]
    async fn account_export_download_response_is_json_attachment() {
        let response = export_archive_download_response(&json!({
            "profile": { "email": "a@example.com" }
        }))
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/json; charset=utf-8"
        );
        assert_eq!(
            response.headers().get(header::CONTENT_DISPOSITION).unwrap(),
            "attachment; filename=\"aegis-data-export.json\""
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = std::str::from_utf8(&body).unwrap();
        assert!(body.contains("\"profile\""));
        assert!(body.contains("a@example.com"));
    }
}
