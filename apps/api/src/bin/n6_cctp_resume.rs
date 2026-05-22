//! Resume a CCTP V2 mint after a burn has already landed.
//!
//! Use when `cctp_rebalance_smoke` ran the burn but the process was
//! interrupted before the 13-min Standard finality cleared. Polls
//! iris-api until the attestation lands, calls `receiveMessage` on the
//! destination chain, then patches the matching `rebalance_legs` row.
//!
//! Run:
//!
//! ```bash
//! EXECUTION_MOCK=false MOCK_CIRCLE=false CCTP_ATTESTATION_TIMEOUT_SECS=1200 \
//!     cargo run --features real-cctp --bin n6_cctp_resume -- \
//!     --burn-tx 0x... --src base --dest arc --rebalance-id <uuid>
//! ```

use std::time::Duration;

use anyhow::{Context, Result};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use uuid::Uuid;

use aegis_api::{
    config::Config,
    db,
    modules::rebalance::{cross_chain::CctpClient, models::ChainKey},
};

#[tokio::main]
async fn main() -> Result<()> {
    aegis_api::env::load_env();

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aegis_api=info,n6_cctp_resume=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().compact())
        .init();

    let args: Vec<String> = std::env::args().collect();
    let get = |flag: &str| -> Option<String> {
        args.windows(2).find(|w| w[0] == flag).map(|w| w[1].clone())
    };
    let burn_tx = get("--burn-tx").context("--burn-tx required")?;
    let src = get("--src").context("--src required")?;
    let dest = get("--dest").context("--dest required")?;
    let rebalance_id: Uuid = get("--rebalance-id")
        .context("--rebalance-id required")?
        .parse()?;
    let src_key = ChainKey::parse(&src).context("bad --src (arc|base)")?;
    let dest_key = ChainKey::parse(&dest).context("bad --dest (arc|base)")?;

    let cfg = Config::from_env().context("Config::from_env")?;
    anyhow::ensure!(!cfg.execution_mock, "resume requires EXECUTION_MOCK=false");

    let pool = db::connect(&cfg.database_url).await?;
    let http = reqwest::Client::builder()
        .user_agent(concat!("Aegis-CctpResume/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(cfg.cctp_attestation_timeout_secs + 60))
        .build()?;

    let client = CctpClient::new(&http, &cfg);

    // Idempotency guard: re-running with the same args after the mint has
    // already landed would re-submit receiveMessage on-chain. CCTP V2's
    // usedNonces check would revert (no double-mint risk), but the operator
    // would see a confusing "execution reverted" error and waste gas on a
    // no-op tx. Short-circuit cleanly when the mint leg is already confirmed.
    let existing_mint: Option<(String, String)> = sqlx::query_as(
        "SELECT status, COALESCE(tx_hash, '')
         FROM rebalance_legs
         WHERE rebalance_id = $1 AND kind = 'cross_chain_mint'",
    )
    .bind(rebalance_id)
    .fetch_optional(&pool)
    .await?;
    if let Some((status, tx)) = &existing_mint {
        if status == "confirmed" && !tx.is_empty() {
            info!(mint_tx = %tx, "mint leg already confirmed — nothing to do");
            println!();
            println!("MINT_TX = {} (already confirmed)", tx);
            return Ok(());
        }
    }

    info!(burn_tx = %burn_tx, src = %src, "polling iris-api for attestation");
    let attestation = client
        .wait_for_attestation(src_key.domain_id(), &burn_tx)
        .await
        .context("wait_for_attestation")?;
    info!("attestation landed; submitting receiveMessage on destination");

    let mint = client
        .receive_message(dest_key, &attestation)
        .await
        .context("receive_message")?;

    println!();
    println!("MINT_TX = {}", mint.tx_hash);

    let rows = sqlx::query(
        "UPDATE rebalance_legs
            SET tx_hash = $1, status = 'confirmed', updated_at = NOW()
            WHERE rebalance_id = $2 AND kind = 'cross_chain_mint'
              AND status != 'confirmed'
            RETURNING leg_index",
    )
    .bind(&mint.tx_hash)
    .bind(rebalance_id)
    .fetch_all(&pool)
    .await?;
    info!(rows_updated = rows.len(), "patched cross_chain_mint leg");

    Ok(())
}
