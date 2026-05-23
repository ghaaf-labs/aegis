//! Idempotently restore public reference catalog rows.
//!
//! Usage:
//!   cargo run --bin seed_curated_strategies

use sqlx::PgPool;
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    aegis_api::env::load_env();
    let url = env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&url).await?;
    aegis_api::modules::catalog::ensure_reference_data(&pool).await?;
    println!("reference catalog ready");
    Ok(())
}
