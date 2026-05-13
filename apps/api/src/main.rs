use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod config;
mod db;
mod error;
mod middleware;
mod modules;
mod router;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "aegis_api=debug,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer().compact())
        .init();

    let cfg = config::Config::from_env()?;
    let db = db::connect(&cfg.database_url).await?;

    sqlx::migrate!("./migrations").run(&db).await?;

    let app = router::build(db, cfg.clone()).await;

    let addr: SocketAddr = format!("{}:{}", cfg.host, cfg.port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("🛡️  Aegis API listening on http://{addr}");

    axum::serve(listener, app).await?;

    Ok(())
}
