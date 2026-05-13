use sqlx::{postgres::PgPoolOptions, PgPool};

pub type Db = PgPool;

pub async fn connect(url: &str) -> anyhow::Result<Db> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .min_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(url)
        .await?;

    tracing::info!("connected to postgres");
    Ok(pool)
}
