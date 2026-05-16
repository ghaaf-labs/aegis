use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use aegis_api::{config, db, router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // .env.local wins (gitignored personal overrides — real-exec keys etc.);
    // .env fills the remaining defaults (committed hermetic baseline).
    // Real env vars from shell / k8s secret / CI still beat both because
    // dotenvy never overrides an already-set variable.
    dotenvy::from_filename(".env.local").ok();
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aegis_api=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().compact())
        .init();

    let cfg = config::Config::from_env()?;

    // Explicit second-line check for the billing-v2 invariant. `Config::validate`
    // (called inside from_env) already enforces this, but the call is repeated
    // here so the failure mode is obvious to anyone scanning startup code and
    // so future refactors of `validate` can't silently weaken it.
    if cfg.billing_v2_enabled {
        if cfg.nanopayments_seller_address.trim().is_empty() {
            anyhow::bail!("BILLING_V2_ENABLED=true but NANOPAYMENTS_SELLER_ADDRESS is empty");
        }
        if cfg.nanopayments_treasury_address.trim().is_empty() {
            anyhow::bail!("BILLING_V2_ENABLED=true but NANOPAYMENTS_TREASURY_ADDRESS is empty");
        }
    }

    let db = db::connect(&cfg.database_url).await?;

    sqlx::migrate!("./migrations").run(&db).await?;

    if cfg.aum_stream_enabled {
        aegis_api::modules::billing::aum_stream::spawn(
            db.clone(),
            std::sync::Arc::new(cfg.clone()),
        );
        info!("aum_stream: 24h ticker spawned");
    }

    let app = router::build(db, cfg.clone()).await;

    let addr: SocketAddr = format!("{}:{}", cfg.host, cfg.port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("🛡️  Aegis API listening on http://{addr}");

    axum::serve(listener, app).await?;

    Ok(())
}
