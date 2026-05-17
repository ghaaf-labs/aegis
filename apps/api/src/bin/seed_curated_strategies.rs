//! SM-1 — seed the three curated strategies for the marketplace MVP.
//!
//! Idempotent: each row has a stable UUID and the upsert uses ON CONFLICT
//! DO UPDATE so re-running the binary is safe. Designed to be invoked
//! during deploy bootstrap so a fresh DB always has the curated set.
//!
//! Usage:
//!   cargo run --bin seed_curated_strategies

use serde_json::json;
use sqlx::PgPool;
use std::env;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::from_filename(".env.local").ok();
    dotenvy::dotenv().ok();
    let url = env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&url).await?;

    let rows = curated();
    for r in &rows {
        sqlx::query(
            "INSERT INTO strategies \
               (id, name, description, risk_band, min_horizon_months, target_allocation, is_curated) \
             VALUES ($1, $2, $3, $4, $5, $6, TRUE) \
             ON CONFLICT (id) DO UPDATE SET \
               name = EXCLUDED.name, \
               description = EXCLUDED.description, \
               risk_band = EXCLUDED.risk_band, \
               min_horizon_months = EXCLUDED.min_horizon_months, \
               target_allocation = EXCLUDED.target_allocation, \
               is_curated = TRUE",
        )
        .bind(r.id)
        .bind(r.name)
        .bind(r.description)
        .bind(r.risk_band)
        .bind(r.min_horizon_months)
        .bind(&r.target_allocation)
        .execute(&pool)
        .await?;
        println!("upserted: {} ({})", r.name, r.id);
    }
    Ok(())
}

struct Row {
    id: Uuid,
    name: &'static str,
    description: &'static str,
    risk_band: &'static str,
    min_horizon_months: i32,
    target_allocation: serde_json::Value,
}

fn curated() -> Vec<Row> {
    vec![
        Row {
            // Stable UUIDs so the binary is idempotent.
            id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            name: "Conservative Treasury",
            description: "Operating-cash treasury with a yield-bearing T-Bill sleeve. \
                Suited for DAOs and SMBs that need principal preservation but \
                want USYC's ~5% yield on idle USDC.",
            risk_band: "low",
            min_horizon_months: 12,
            target_allocation: json!({"USDC": 70, "USYC": 20, "EURC": 10}),
        },
        Row {
            id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            name: "Balanced",
            description: "Long-only stablecoin-anchored portfolio with majors exposure. \
                Half the book sits in income-generating USDC + USYC; the other \
                half rides BTC + ETH for asymmetric upside.",
            risk_band: "medium",
            min_horizon_months: 36,
            target_allocation: json!({"USDC": 40, "BTC": 30, "ETH": 20, "USYC": 10}),
        },
        Row {
            id: Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
            name: "DAO Reserve",
            description: "Multi-currency reserve for an internet-native organization with \
                multi-jurisdiction operating expenses. USDC + EURC keeps payroll \
                in either denomination; USYC carries the yield sleeve.",
            risk_band: "low",
            min_horizon_months: 60,
            target_allocation: json!({"USDC": 60, "USYC": 20, "EURC": 20}),
        },
    ]
}
