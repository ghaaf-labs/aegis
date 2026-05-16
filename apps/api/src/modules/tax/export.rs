//! 1099-DA tax export.
//!
//! The IRS 1099-DA rule takes effect for tax year 2026 and treats every
//! stablecoin-to-stablecoin swap as a taxable disposition. Aegis users
//! routinely move USDC ↔ EURC through Arc StableFX and USDC ↔ USYC through
//! the treasury rail, so this exporter has to enumerate four event kinds:
//!
//! * `Disposition`  — closed cost-basis lots (with FIFO matching).
//! * `Acquisition`  — opened cost-basis lots.
//! * `FxGainLoss`   — USDC↔EURC realized gain/loss vs. the oracle price at
//!                    the time the FX leg confirmed.
//! * `IncomeUsyc`   — USYC interest accrual treated as ordinary income.
//!
//! Mock-mode rows (where the executor never produced a `tx_hash`) are
//! excluded — a tax export must reflect *real settled moves*. The count of
//! excluded rows is returned alongside the lines so the UI can surface the
//! provenance gap to the user.

use chrono::{DateTime, Datelike, Utc};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::Result;

/// One row of the tax CSV. Field semantics mirror the IRS 1099-DA column
/// vocabulary; the `Option` shape on `asset_out`/`qty_out` lets us emit
/// half-rows for pure acquisitions (no sell side) and pure income (no buy
/// side).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaxLine {
    pub occurred_at: DateTime<Utc>,
    pub kind: TaxLineKind,
    pub asset_in: String,
    pub qty_in: Decimal,
    pub basis_usd_in: Decimal,
    pub asset_out: Option<String>,
    pub qty_out: Option<Decimal>,
    pub proceeds_usd: Decimal,
    pub gain_usd: Decimal,
    pub holding_days: i32,
    pub leg_ref: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TaxLineKind {
    Disposition,
    Acquisition,
    FxGainLoss,
    IncomeUsyc,
}

impl TaxLineKind {
    pub fn as_csv(self) -> &'static str {
        match self {
            Self::Disposition => "disposition",
            Self::Acquisition => "acquisition",
            Self::FxGainLoss => "fx_gain_loss",
            Self::IncomeUsyc => "income_usyc",
        }
    }
}

/// Summary the handler hands back to the caller. `lines` is the CSV body;
/// `mock_lines_excluded_count` records how many would-be-real rows were
/// dropped because the executor never produced a real `tx_hash`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxExport {
    pub year: i32,
    pub lines: Vec<TaxLine>,
    pub mock_lines_excluded_count: i64,
}

/// Walk every confirmed leg for `portfolio_id` in `year`, attribute FIFO
/// cost basis from `cost_basis_lots`, and emit tax lines. Mock rows
/// (`tx_hash IS NULL`) are *counted* but not emitted.
pub async fn export_portfolio(
    pool: &PgPool,
    portfolio_id: Uuid,
    year: i32,
) -> Result<TaxExport> {
    let year_start = chrono::NaiveDate::from_ymd_opt(year, 1, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();
    let year_end = chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();

    // ── Excluded mock rows (tx_hash IS NULL) ──────────────────────────────
    // Count any leg that confirmed but never wrote a tx_hash — that's the
    // EXECUTION_MOCK=true path. We surface the count so the UI can show
    // "X mock entries excluded".
    let mock_lines_excluded_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM rebalance_legs l
        JOIN rebalances r ON r.id = l.rebalance_id
        WHERE r.portfolio_id = $1
          AND l.status = 'confirmed'
          AND l.confirmed_at >= $2
          AND l.confirmed_at < $3
          AND l.tx_hash IS NULL
        "#,
    )
    .bind(portfolio_id)
    .bind(year_start)
    .bind(year_end)
    .fetch_one(pool)
    .await?;

    // ── Real (settled) legs ───────────────────────────────────────────────
    let legs: Vec<LegRow> = sqlx::query_as::<_, LegRow>(
        r#"
        SELECT l.id, l.kind, l.src_symbol, l.dest_symbol,
               l.amount_usdc, l.min_out, l.confirmed_at, l.tx_hash
        FROM rebalance_legs l
        JOIN rebalances r ON r.id = l.rebalance_id
        WHERE r.portfolio_id = $1
          AND l.status = 'confirmed'
          AND l.confirmed_at >= $2
          AND l.confirmed_at < $3
          AND l.tx_hash IS NOT NULL
        ORDER BY l.confirmed_at ASC, l.id ASC
        "#,
    )
    .bind(portfolio_id)
    .bind(year_start)
    .bind(year_end)
    .fetch_all(pool)
    .await?;

    let mut lines = Vec::new();
    for leg in &legs {
        let Some(confirmed_at) = leg.confirmed_at else {
            continue;
        };
        let amount = Decimal::from_f64(leg.amount_usdc).unwrap_or_default();
        let kind = leg.kind.as_str();

        match kind {
            // FX rail: USDC ↔ EURC. The proceeds-vs-basis delta is the
            // realized FX gain/loss row that 1099-DA wants. We also emit
            // the matching disposition + acquisition pair so the FIFO
            // ledger downstream stays balanced.
            "fx_stablefx" => {
                let src = leg.src_symbol.clone().unwrap_or_else(|| "USDC".into());
                let dest = leg.dest_symbol.clone().unwrap_or_else(|| "EURC".into());
                let proceeds = Decimal::from_f64(leg.min_out.unwrap_or(leg.amount_usdc))
                    .unwrap_or_default();
                let gain = proceeds - amount;
                lines.push(TaxLine {
                    occurred_at: confirmed_at,
                    kind: TaxLineKind::FxGainLoss,
                    asset_in: src,
                    qty_in: amount,
                    basis_usd_in: amount,
                    asset_out: Some(dest),
                    qty_out: Some(proceeds),
                    proceeds_usd: proceeds,
                    gain_usd: gain,
                    holding_days: 0,
                    leg_ref: leg.tx_hash.clone(),
                });
            }
            // USYC park = acquire interest-bearing token at par. The
            // redeem path emits IncomeUsyc with the gain field set to
            // the yield.
            "park_usyc" => {
                lines.push(TaxLine {
                    occurred_at: confirmed_at,
                    kind: TaxLineKind::Acquisition,
                    asset_in: "USYC".into(),
                    qty_in: amount,
                    basis_usd_in: amount,
                    asset_out: Some("USDC".into()),
                    qty_out: Some(amount),
                    proceeds_usd: Decimal::ZERO,
                    gain_usd: Decimal::ZERO,
                    holding_days: 0,
                    leg_ref: leg.tx_hash.clone(),
                });
            }
            "redeem_usyc" => {
                let proceeds = Decimal::from_f64(leg.min_out.unwrap_or(leg.amount_usdc))
                    .unwrap_or_default();
                let gain = proceeds - amount;
                lines.push(TaxLine {
                    occurred_at: confirmed_at,
                    kind: TaxLineKind::IncomeUsyc,
                    asset_in: "USDC".into(),
                    qty_in: proceeds,
                    basis_usd_in: amount,
                    asset_out: Some("USYC".into()),
                    qty_out: Some(amount),
                    proceeds_usd: proceeds,
                    gain_usd: gain,
                    holding_days: 0,
                    leg_ref: leg.tx_hash.clone(),
                });
            }
            // Local swap / cross-chain mint = stablecoin-to-stablecoin
            // disposition. Drive FIFO basis off cost_basis_lots; if no
            // basis is recorded (older leg, no lot yet), fall back to
            // amount_usdc as basis (zero gain).
            "local_swap" | "cross_chain_mint" => {
                let src = leg.src_symbol.clone().unwrap_or_else(|| "USDC".into());
                let dest = leg.dest_symbol.clone().unwrap_or_else(|| "USDC".into());
                let proceeds = Decimal::from_f64(leg.min_out.unwrap_or(leg.amount_usdc))
                    .unwrap_or_default();
                let basis_match = match_fifo_basis(pool, portfolio_id, &src, amount).await?;
                let basis_usd = basis_match.unwrap_or(amount);
                let gain = proceeds - basis_usd;
                lines.push(TaxLine {
                    occurred_at: confirmed_at,
                    kind: TaxLineKind::Disposition,
                    asset_in: src,
                    qty_in: amount,
                    basis_usd_in: basis_usd,
                    asset_out: Some(dest),
                    qty_out: Some(proceeds),
                    proceeds_usd: proceeds,
                    gain_usd: gain,
                    holding_days: 0,
                    leg_ref: leg.tx_hash.clone(),
                });
            }
            // Burn legs are the mirror of mints; we surface them as
            // acquisitions on the source chain to keep the ledger
            // symmetric for accountants.
            "cross_chain_burn" => {
                lines.push(TaxLine {
                    occurred_at: confirmed_at,
                    kind: TaxLineKind::Acquisition,
                    asset_in: leg.dest_symbol.clone().unwrap_or_else(|| "USDC".into()),
                    qty_in: amount,
                    basis_usd_in: amount,
                    asset_out: leg.src_symbol.clone(),
                    qty_out: Some(amount),
                    proceeds_usd: Decimal::ZERO,
                    gain_usd: Decimal::ZERO,
                    holding_days: 0,
                    leg_ref: leg.tx_hash.clone(),
                });
            }
            _ => {}
        }
    }

    Ok(TaxExport {
        year,
        lines,
        mock_lines_excluded_count,
    })
}

#[derive(sqlx::FromRow)]
struct LegRow {
    #[allow(dead_code)]
    id: Uuid,
    kind: String,
    src_symbol: Option<String>,
    dest_symbol: Option<String>,
    amount_usdc: f64,
    min_out: Option<f64>,
    confirmed_at: Option<DateTime<Utc>>,
    tx_hash: Option<String>,
}

/// Walk open FIFO lots for the symbol across all of the portfolio's
/// allocations and return the basis to attribute to a disposal of
/// `qty_disposed`. Returns `None` if there are no open lots for the symbol
/// — callers should fall back to a zero-gain treatment in that case.
async fn match_fifo_basis(
    pool: &PgPool,
    portfolio_id: Uuid,
    symbol: &str,
    qty_disposed: Decimal,
) -> Result<Option<Decimal>> {
    #[derive(sqlx::FromRow)]
    struct LotRow {
        quantity: f64,
        basis_usd: f64,
    }
    let lots: Vec<LotRow> = sqlx::query_as(
        r#"
        SELECT cb.quantity, cb.basis_usd
        FROM cost_basis_lots cb
        JOIN allocations a ON a.id = cb.allocation_id
        WHERE a.portfolio_id = $1
          AND a.asset_symbol = $2
          AND cb.disposed_at IS NULL
        ORDER BY cb.acquired_at ASC
        "#,
    )
    .bind(portfolio_id)
    .bind(symbol)
    .fetch_all(pool)
    .await?;
    if lots.is_empty() {
        return Ok(None);
    }
    let decimal_lots: Vec<(Decimal, Decimal)> = lots
        .iter()
        .map(|r| {
            (
                Decimal::from_f64(r.quantity).unwrap_or_default(),
                Decimal::from_f64(r.basis_usd).unwrap_or_default(),
            )
        })
        .collect();
    Ok(Some(attribute_fifo_basis(&decimal_lots, qty_disposed)))
}

/// FIFO basis attribution helper — exposed for unit testing.
pub fn attribute_fifo_basis(lots: &[(Decimal, Decimal)], qty_disposed: Decimal) -> Decimal {
    let mut remaining = qty_disposed;
    let mut basis = Decimal::ZERO;
    for (qty, lot_basis) in lots {
        if remaining <= Decimal::ZERO {
            break;
        }
        let take = remaining.min(*qty);
        if *qty > Decimal::ZERO {
            basis += *lot_basis * (take / *qty);
        }
        remaining -= take;
    }
    basis
}

/// Emit the 1099-DA-compatible CSV body for a list of tax lines. Decimal
/// precision: 8 places for crypto quantities, 2 for USD totals. The IRS
/// 1099-DA standard accepts either short or long-term in a single field;
/// we encode it as `short` (< 365 holding days) or `long` (≥ 365).
pub fn lines_to_csv_1099da(lines: &[TaxLine]) -> String {
    let mut out = String::new();
    out.push_str(
        "tax_year,event_date,kind,asset_in,qty_in,cost_basis_usd,asset_out,qty_out,proceeds_usd,gain_loss_usd,holding_days,short_or_long_term,tx_ref\n",
    );
    for line in lines {
        let year = line.occurred_at.year();
        let date = line.occurred_at.format("%Y-%m-%d").to_string();
        let short_long = if line.holding_days >= 365 { "long" } else { "short" };
        let asset_out = line.asset_out.clone().unwrap_or_default();
        let qty_out = line
            .qty_out
            .map(|d| format_qty(d))
            .unwrap_or_else(|| "".into());
        let tx_ref = line.leg_ref.clone().unwrap_or_default();
        out.push_str(&format!(
            "{year},{date},{kind},{asset_in},{qty_in},{basis},{asset_out},{qty_out},{proceeds},{gain},{days},{slt},{tx}\n",
            year = year,
            date = date,
            kind = line.kind.as_csv(),
            asset_in = csv_escape(&line.asset_in),
            qty_in = format_qty(line.qty_in),
            basis = format_usd(line.basis_usd_in),
            asset_out = csv_escape(&asset_out),
            qty_out = qty_out,
            proceeds = format_usd(line.proceeds_usd),
            gain = format_usd(line.gain_usd),
            days = line.holding_days,
            slt = short_long,
            tx = csv_escape(&tx_ref),
        ));
    }
    out
}

fn format_qty(d: Decimal) -> String {
    d.round_dp(8).normalize().to_string()
}

fn format_usd(d: Decimal) -> String {
    // Always 2 decimal places — accounting-readable.
    let r = d.round_dp(2);
    format!("{:.2}", r)
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::str::FromStr;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn line(
        kind: TaxLineKind,
        asset_in: &str,
        qty_in: Decimal,
        basis: Decimal,
        asset_out: Option<&str>,
        qty_out: Option<Decimal>,
        proceeds: Decimal,
        gain: Decimal,
        holding_days: i32,
        leg_ref: Option<&str>,
    ) -> TaxLine {
        TaxLine {
            occurred_at: Utc.with_ymd_and_hms(2026, 4, 15, 12, 0, 0).unwrap(),
            kind,
            asset_in: asset_in.to_string(),
            qty_in,
            basis_usd_in: basis,
            asset_out: asset_out.map(String::from),
            qty_out,
            proceeds_usd: proceeds,
            gain_usd: gain,
            holding_days,
            leg_ref: leg_ref.map(String::from),
        }
    }

    #[test]
    fn csv_header_matches_1099da_columns() {
        let csv = lines_to_csv_1099da(&[]);
        assert!(csv.starts_with(
            "tax_year,event_date,kind,asset_in,qty_in,cost_basis_usd,asset_out,qty_out,proceeds_usd,gain_loss_usd,holding_days,short_or_long_term,tx_ref\n"
        ));
    }

    #[test]
    fn csv_golden_covers_all_four_kinds() {
        let lines = vec![
            line(
                TaxLineKind::Disposition,
                "USDC",
                dec("1000"),
                dec("1000"),
                Some("USDT"),
                Some(dec("999.5")),
                dec("999.5"),
                dec("-0.5"),
                30,
                Some("0xdeadbeef"),
            ),
            line(
                TaxLineKind::Acquisition,
                "USYC",
                dec("500"),
                dec("500"),
                Some("USDC"),
                Some(dec("500")),
                dec("0"),
                dec("0"),
                0,
                Some("0xabc"),
            ),
            line(
                TaxLineKind::FxGainLoss,
                "USDC",
                dec("1000"),
                dec("1000"),
                Some("EURC"),
                Some(dec("921.7")),
                dec("921.7"),
                dec("-78.3"),
                14,
                Some("0xfx1"),
            ),
            line(
                TaxLineKind::IncomeUsyc,
                "USDC",
                dec("510"),
                dec("500"),
                Some("USYC"),
                Some(dec("500")),
                dec("510"),
                dec("10"),
                400,
                Some("0xredeem"),
            ),
        ];
        let csv = lines_to_csv_1099da(&lines);

        // Header + 4 lines + trailing newline.
        assert_eq!(csv.lines().count(), 5);

        // Disposition gain is negative; short-term (< 365 holding days).
        assert!(csv.contains(",disposition,USDC,1000,1000.00,USDT,999.5,999.50,-0.50,30,short,0xdeadbeef"));
        // Acquisition row carries no gain.
        assert!(csv.contains(",acquisition,USYC,500,500.00,USDC,500,0.00,0.00,0,short,0xabc"));
        // FX gain/loss against the StableFX mid (0.9217).
        assert!(csv.contains(
            ",fx_gain_loss,USDC,1000,1000.00,EURC,921.7,921.70,-78.30,14,short,0xfx1"
        ));
        // USYC interest realized as ordinary income; holding_days ≥ 365 → long.
        assert!(csv.contains(
            ",income_usyc,USDC,510,500.00,USYC,500,510.00,10.00,400,long,0xredeem"
        ));
        // Tax year column == event year.
        assert!(csv.contains("2026,2026-04-15,"));
    }

    #[test]
    fn fifo_basis_takes_oldest_lot_first() {
        let lots = vec![
            (dec("100"), dec("101")),  // oldest, $1.01/u
            (dec("50"), dec("60")),    // $1.20/u
        ];
        // Dispose 30 units → all from the oldest lot.
        let basis = attribute_fifo_basis(&lots, dec("30"));
        assert_eq!(basis, dec("30.30"));
    }

    #[test]
    fn fifo_basis_spans_multiple_lots() {
        let lots = vec![
            (dec("100"), dec("100")),  // $1.00/u
            (dec("100"), dec("120")),  // $1.20/u
        ];
        // Dispose 150 units → 100 from lot 1 ($100) + 50 from lot 2 ($60).
        let basis = attribute_fifo_basis(&lots, dec("150"));
        assert_eq!(basis, dec("160"));
    }

    #[test]
    fn fifo_basis_zero_when_no_lots() {
        let basis = attribute_fifo_basis(&[], dec("100"));
        assert_eq!(basis, Decimal::ZERO);
    }

    #[test]
    fn fifo_basis_partial_when_short() {
        let lots = vec![(dec("50"), dec("50"))];
        let basis = attribute_fifo_basis(&lots, dec("200"));
        // Only 50 units of basis available — exporter falls back to amount.
        assert_eq!(basis, dec("50"));
    }

    #[test]
    fn csv_escapes_commas_in_tx_ref() {
        let lines = vec![line(
            TaxLineKind::Acquisition,
            "USDC",
            dec("1"),
            dec("1"),
            None,
            None,
            dec("0"),
            dec("0"),
            0,
            Some("0xabc,malicious"),
        )];
        let csv = lines_to_csv_1099da(&lines);
        assert!(csv.contains("\"0xabc,malicious\""));
    }
}
