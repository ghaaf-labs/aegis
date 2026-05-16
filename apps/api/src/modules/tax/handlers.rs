use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::middleware::auth::Claims;
use crate::router::AppState;

use super::export::{export_portfolio, lines_to_csv_1099da};
use super::models::HarvestableLoss;
use super::service::harvestable_losses;
use super::share::{
    create_share_token, list_share_tokens, resolve_share_token, revoke_share_token, ShareTokenRow,
};

pub async fn harvestable(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(portfolio_id): Path<Uuid>,
) -> Result<Json<Vec<HarvestableLoss>>> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM portfolios WHERE id = $1 AND user_id = $2)",
    )
    .bind(portfolio_id)
    .bind(claims.sub)
    .fetch_one(&state.db)
    .await?;
    if !exists {
        return Err(AppError::NotFound(format!("portfolio {portfolio_id}")));
    }
    Ok(Json(
        harvestable_losses(&state, claims.sub, portfolio_id).await?,
    ))
}

// ── 1099-DA CSV export (F-TAX-3) ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportQuery {
    pub portfolio_id: Uuid,
    pub year: Option<i32>,
}

/// `GET /tax/export.csv?portfolioId=...&year=2026` — authed CSV download.
/// Gated by `TAX_EXPORT_V1_ENABLED`. Sets `X-Mock-Excluded` to the count
/// of mock rows that were skipped so the UI can render a provenance line.
pub async fn export_csv(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(q): Query<ExportQuery>,
) -> Result<impl IntoResponse> {
    require_flag(&state)?;
    require_ownership(&state, claims.sub, q.portfolio_id).await?;
    let year = q.year.unwrap_or_else(|| Utc::now().year());
    let export = export_portfolio(&state.db, q.portfolio_id, year).await?;
    let body = lines_to_csv_1099da(&export.lines);
    let filename = format!("aegis_tax_{year}_{}.csv", q.portfolio_id);
    csv_response(&body, &filename, export.mock_lines_excluded_count)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateShareInput {
    pub portfolio_id: Uuid,
    pub year: i32,
    pub ttl_days: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateShareResponse {
    pub token_id: Uuid,
    pub token: String,
    pub share_url: String,
    pub expires_at: DateTime<Utc>,
}

/// `POST /tax/share` — authed; mints a token and returns the public URL.
pub async fn create_share(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(input): Json<CreateShareInput>,
) -> Result<Json<CreateShareResponse>> {
    require_flag(&state)?;
    require_ownership(&state, claims.sub, input.portfolio_id).await?;
    let (token_id, token, expires_at) = create_share_token(
        &state.db,
        claims.sub,
        input.portfolio_id,
        input.year,
        input.ttl_days,
    )
    .await?;
    let share_url = format!(
        "{}/tax/share/{}/export.csv",
        state.config.api_base_url.trim_end_matches('/'),
        token
    );
    Ok(Json(CreateShareResponse {
        token_id,
        token,
        share_url,
        expires_at,
    }))
}

/// `GET /tax/share/:token/export.csv` — public; resolves a token to its
/// originating (user, portfolio, year) and returns the CSV. Read-only.
pub async fn export_via_share(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<impl IntoResponse> {
    require_flag(&state)?;
    let resolved = resolve_share_token(&state.db, &token).await?;
    let (_user_id, portfolio_id, year) =
        resolved.ok_or_else(|| AppError::NotFound("share token".into()))?;
    let export = export_portfolio(&state.db, portfolio_id, year).await?;
    let body = lines_to_csv_1099da(&export.lines);
    let filename = format!("aegis_tax_{year}_{portfolio_id}.csv");
    csv_response(&body, &filename, export.mock_lines_excluded_count)
}

/// `DELETE /tax/share/:token_id` — authed; revokes a token the user owns.
pub async fn revoke_share(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(token_id): Path<Uuid>,
) -> Result<StatusCode> {
    require_flag(&state)?;
    revoke_share_token(&state.db, claims.sub, token_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /tax/shares` — authed; lists the caller's share tokens.
pub async fn list_shares(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<ShareTokenRow>>> {
    require_flag(&state)?;
    let rows = list_share_tokens(&state.db, claims.sub).await?;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryQuery {
    pub portfolio_id: Uuid,
    pub year: Option<i32>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct WalletSummaryRow {
    pub chain: String,
    pub address: String,
    pub lot_count: i64,
    pub last_synced_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxSummary {
    pub year: i32,
    pub wallets: Vec<WalletSummaryRow>,
    pub total_lot_count: i64,
    /// Caveat the UI surfaces verbatim — lot-level chain attribution lands
    /// in a follow-up; for now the user sees both wallets + portfolio-level
    /// totals.
    pub caveat: String,
}

/// `GET /tax/summary?portfolioId=...&year=...` — authed; powers the
/// pre-download wallet provenance card in /settings/tax.
pub async fn summary(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(q): Query<SummaryQuery>,
) -> Result<Json<TaxSummary>> {
    require_flag(&state)?;
    require_ownership(&state, claims.sub, q.portfolio_id).await?;
    let year = q.year.unwrap_or_else(|| Utc::now().year());

    let row: (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT arc_address, base_address FROM users WHERE id = $1",
    )
    .bind(claims.sub)
    .fetch_one(&state.db)
    .await?;
    let (arc, base) = row;

    let counts: (i64, Option<DateTime<Utc>>) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT AS lots, MAX(l.acquired_at) AS last_synced
         FROM cost_basis_lots l
         JOIN allocations a ON a.id = l.allocation_id
         WHERE a.portfolio_id = $1
           AND DATE_PART('year', l.acquired_at) <= $2",
    )
    .bind(q.portfolio_id)
    .bind(year as f64)
    .fetch_one(&state.db)
    .await?;
    let (total, last_synced) = counts;

    let mut wallets = Vec::new();
    if let Some(addr) = arc {
        wallets.push(WalletSummaryRow {
            chain: "arc".into(),
            address: addr,
            lot_count: total,
            last_synced_at: last_synced,
        });
    }
    if let Some(addr) = base {
        wallets.push(WalletSummaryRow {
            chain: "base".into(),
            address: addr,
            lot_count: total,
            last_synced_at: last_synced,
        });
    }

    Ok(Json(TaxSummary {
        year,
        wallets,
        total_lot_count: total,
        caveat: "Lot counts are reported at portfolio level. Per-wallet chain \
                 attribution lands in a future tax-export iteration; both wallet \
                 addresses are listed here so an accountant can match the CSV \
                 rows against on-chain records."
            .into(),
    }))
}

// ── helpers ───────────────────────────────────────────────────────────────

fn require_flag(state: &AppState) -> Result<()> {
    if !state.config.tax_export_v1_enabled {
        return Err(AppError::NotFound("tax export disabled".into()));
    }
    Ok(())
}

async fn require_ownership(state: &AppState, user_id: Uuid, portfolio_id: Uuid) -> Result<()> {
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM portfolios WHERE id = $1 AND user_id = $2)",
    )
    .bind(portfolio_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    if !owned {
        return Err(AppError::NotFound(format!("portfolio {portfolio_id}")));
    }
    Ok(())
}

fn csv_response(
    body: &str,
    filename: &str,
    mock_excluded: i64,
) -> Result<axum::response::Response> {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .map_err(|e| AppError::Internal(anyhow::anyhow!("filename header: {e}")))?,
    );
    headers.insert(
        "X-Mock-Excluded",
        HeaderValue::from_str(&mock_excluded.to_string())
            .map_err(|e| AppError::Internal(anyhow::anyhow!("header: {e}")))?,
    );
    Ok((headers, body.to_string()).into_response())
}
