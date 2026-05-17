//! HS-5 smoke: park USDC into Hashnote USYC on Arc testnet.
//!
//! Uses the same pre-seeded user as cctp_rebalance_smoke
//! (`scripts/seed-n6-smoke.sh`). Calls `treasury::service::park_in_usyc`
//! directly — bypasses the HTTP layer and the Circle Wallets API.
//!
//! Run:
//!
//! ```bash
//! EXECUTION_MOCK=false cargo run --features "real-cctp real-usyc" --bin usyc_park_smoke -- --amount 5
//! ```

use std::time::Duration;

use anyhow::{Context, Result};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use uuid::Uuid;

use aegis_api::{config::Config, db, modules::treasury::service::park_in_usyc};

const USER_ID: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";

#[tokio::main]
async fn main() -> Result<()> {
    if let Err(e) = dotenvy::from_filename(".env.local") {
        if !matches!(&e, dotenvy::Error::Io(io) if io.kind() == std::io::ErrorKind::NotFound) {
            eprintln!("[dotenv] .env.local: {e}");
        }
    }
    if let Err(e) = dotenvy::dotenv() {
        if !matches!(&e, dotenvy::Error::Io(io) if io.kind() == std::io::ErrorKind::NotFound) {
            eprintln!("[dotenv] .env: {e}");
        }
    }

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aegis_api=info,usyc_park_smoke=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().compact())
        .init();

    let args: Vec<String> = std::env::args().collect();
    let amount_usdc: f64 = args
        .windows(2)
        .find(|w| w[0] == "--amount")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| "5".into())
        .parse()
        .context("--amount must be a number")?;

    let cfg = Config::from_env().context("Config::from_env")?;
    anyhow::ensure!(
        !cfg.execution_mock,
        "usyc smoke requires EXECUTION_MOCK=false"
    );
    anyhow::ensure!(
        !cfg.usyc_teller_arc.is_empty(),
        "USYC_TELLER_ARC must be set"
    );
    anyhow::ensure!(!cfg.usdc_arc.is_empty(), "USDC_ARC must be set");

    println!("--- runtime config ---");
    println!("  USYC_TELLER_ARC = {}", cfg.usyc_teller_arc);
    println!("  USDC_ARC        = {}", cfg.usdc_arc);
    println!("  ARC_RPC_URL set = {}", !cfg.arc_rpc_url.is_empty());
    println!("---");

    let pool = db::connect(&cfg.database_url).await?;
    let user_id: Uuid = USER_ID.parse()?;
    info!(%user_id, amount_usdc, "parking USDC into USYC");

    let started = std::time::Instant::now();
    let result = park_in_usyc(&pool, &cfg, user_id, amount_usdc)
        .await
        .context("park_in_usyc")?;
    let elapsed = started.elapsed();

    println!();
    println!("=== USYC park result ===");
    println!("  intent      = {}", result.intent);
    println!("  amount_usdc = {}", result.amount_usdc);
    println!("  executed    = {}", result.executed);
    println!(
        "  tx_hash     = {}",
        result.tx_hash.as_deref().unwrap_or("(none)")
    );
    println!("  note        = {}", result.note);
    println!("  wall_clock  = {:?}", elapsed);

    // Drain analytics emitter task so the park_intent event lands before
    // exit. Small sleep is fine; the emitter is fire-and-forget.
    tokio::time::sleep(Duration::from_millis(250)).await;

    Ok(())
}
