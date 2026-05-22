//! F-TAX-8 — 1099-DA per-wallet export regression test.
//!
//! The 2026 final 1099-DA regulations (`Rev Proc 2024-28`) kill universal
//! aggregation: every CSV row must reference a specific wallet, not be
//! summed across the user's wallets. Aegis's data model encodes "wallet"
//! implicitly via the `tx_hash` (each chain produces its own hash) plus
//! the `src_chain` / `dest_chain` columns on `rebalance_legs`.
//!
//! This integration test pins that the export emits a separate CSV row
//! per leg even when the legs touch different chains — i.e. moving USDC
//! from Base Sepolia (burn) to Arc testnet (mint) produces TWO distinct
//! rows, each with the chain's own `tx_hash` as `leg_ref`. If a future
//! refactor accidentally aggregates across legs (e.g. summing
//! `amount_usdc` into a single row), this test fails.
//!
use aegis_api::modules::tax::export::{export_portfolio, TaxLineKind};
use chrono::TimeZone;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::path::Path;
use uuid::Uuid;

#[tokio::test]
async fn export_emits_separate_rows_per_chain_wallet() -> sqlx::Result<()> {
    let Ok(db_url) = std::env::var("TEST_DATABASE_URL").or_else(|_| std::env::var("DATABASE_URL"))
    else {
        eprintln!("DATABASE_URL not set; skipping per-wallet tax export integration test");
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await?;
    sqlx::migrate::Migrator::new(Path::new("./migrations"))
        .await?
        .run(&pool)
        .await?;
    export_emits_separate_rows_per_chain_wallet_inner(pool).await
}

async fn export_emits_separate_rows_per_chain_wallet_inner(pool: PgPool) -> sqlx::Result<()> {
    // ── Seed: one user with BOTH chain wallets populated ──────────────────
    let user_id = Uuid::new_v4();
    let arc_addr = "0xf22C6d6047eC75c21f5845CEA7F83D740e78aa24"; // sample
    let base_addr = "0x0043D379B27fa9367E02cF90F7A17a37Dc2c7a76"; // sample
    sqlx::query(
        "INSERT INTO users (id, email, risk_tolerance, investment_horizon_months,
                            arc_address, base_address)
         VALUES ($1, $2, 'moderate', 12, $3, $4)",
    )
    .bind(user_id)
    .bind(format!("u-{user_id}@test.aegis"))
    .bind(arc_addr)
    .bind(base_addr)
    .execute(&pool)
    .await?;

    let portfolio_id = Uuid::new_v4();
    sqlx::query("INSERT INTO portfolios (id, user_id, name) VALUES ($1, $2, 'multi-chain')")
        .bind(portfolio_id)
        .bind(user_id)
        .execute(&pool)
        .await?;

    let decision_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO agent_decisions (id, portfolio_id, reasoning, confidence)
         VALUES ($1, $2, 'cross-chain rebalance', 0.9)",
    )
    .bind(decision_id)
    .bind(portfolio_id)
    .execute(&pool)
    .await?;

    let reb_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO rebalances
             (id, portfolio_id, decision_id, status, total_legs, completed_legs)
         VALUES ($1, $2, $3, 'completed', 2, 2)",
    )
    .bind(reb_id)
    .bind(portfolio_id)
    .bind(decision_id)
    .execute(&pool)
    .await?;

    // ── Two confirmed legs, one per chain, each with its own tx_hash ──────
    let when = chrono::Utc.with_ymd_and_hms(2026, 3, 1, 12, 0, 0).unwrap();

    // Leg 0: cross_chain_burn on Base Sepolia (1000 USDC out of Base wallet).
    sqlx::query(
        "INSERT INTO rebalance_legs
             (rebalance_id, leg_index, kind, src_chain, dest_chain,
              src_symbol, dest_symbol, amount_usdc, min_out,
              status, tx_hash, confirmed_at)
         VALUES ($1, 0, 'cross_chain_burn', 'base', 'arc',
                 'USDC', 'USDC', 1000, 1000, 'confirmed', '0xbase_burn', $2)",
    )
    .bind(reb_id)
    .bind(when)
    .execute(&pool)
    .await?;

    // Leg 1: cross_chain_mint on Arc (1000 USDC arriving in Arc wallet).
    sqlx::query(
        "INSERT INTO rebalance_legs
             (rebalance_id, leg_index, kind, src_chain, dest_chain,
              src_symbol, dest_symbol, amount_usdc, min_out,
              status, tx_hash, confirmed_at)
         VALUES ($1, 1, 'cross_chain_mint', 'base', 'arc',
                 'USDC', 'USDC', 1000, 1000, 'confirmed', '0xarc_mint', $2)",
    )
    .bind(reb_id)
    .bind(when)
    .execute(&pool)
    .await?;

    // ── Export + assertions ───────────────────────────────────────────────
    let export = export_portfolio(&pool, portfolio_id, 2026)
        .await
        .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;

    assert_eq!(
        export.lines.len(),
        2,
        "must emit one row per leg even when both legs are part of one cross-chain rebalance"
    );

    // The two rows must reference the two chains' tx_hashes distinctly —
    // this is the wallet-by-wallet contract the 2026 1099-DA regs require.
    let leg_refs: Vec<&str> = export
        .lines
        .iter()
        .filter_map(|l| l.leg_ref.as_deref())
        .collect();
    assert!(
        leg_refs.contains(&"0xbase_burn"),
        "missing burn-side tx_hash row — universal aggregation would drop one"
    );
    assert!(
        leg_refs.contains(&"0xarc_mint"),
        "missing mint-side tx_hash row — universal aggregation would drop one"
    );
    assert_ne!(
        leg_refs[0], leg_refs[1],
        "two rows but same leg_ref — that's aggregated, not per-wallet"
    );

    // Kinds: burn → Acquisition (book the inbound USDC); mint → Disposition
    // (book the swap-out of source-chain USDC). The current export.rs maps
    // them this way; this assertion pins that semantic so a refactor
    // doesn't silently flip it.
    let burn = export
        .lines
        .iter()
        .find(|l| l.leg_ref.as_deref() == Some("0xbase_burn"))
        .expect("burn row");
    assert_eq!(burn.kind, TaxLineKind::Acquisition);

    let mint = export
        .lines
        .iter()
        .find(|l| l.leg_ref.as_deref() == Some("0xarc_mint"))
        .expect("mint row");
    assert_eq!(mint.kind, TaxLineKind::Disposition);

    assert_eq!(
        export.mock_lines_excluded_count, 0,
        "no NULL-tx_hash legs were inserted; nothing should be excluded"
    );

    Ok(())
}
