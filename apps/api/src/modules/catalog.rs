use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn ensure_reference_data(pool: &PgPool) -> anyhow::Result<()> {
    seed_assets(pool).await?;
    seed_plan_tiers(pool).await?;
    seed_curated_strategies(pool).await?;
    Ok(())
}

async fn seed_assets(pool: &PgPool) -> anyhow::Result<()> {
    for asset in assets() {
        sqlx::query(
            "INSERT INTO assets (symbol, name, coingecko_id)
             VALUES ($1, $2, $3)
             ON CONFLICT (symbol) DO UPDATE SET
               name = EXCLUDED.name,
               coingecko_id = EXCLUDED.coingecko_id",
        )
        .bind(asset.symbol)
        .bind(asset.name)
        .bind(asset.coingecko_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn seed_plan_tiers(pool: &PgPool) -> anyhow::Result<()> {
    for tier in plan_tiers() {
        sqlx::query(
            "INSERT INTO plan_tiers
                (code, monthly_usd, aum_cap_usd, portfolios_cap,
                 decisions_cap_monthly, per_rebalance_bps, aum_annual_bps)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (code) DO UPDATE SET
               monthly_usd = EXCLUDED.monthly_usd,
               aum_cap_usd = EXCLUDED.aum_cap_usd,
               portfolios_cap = EXCLUDED.portfolios_cap,
               decisions_cap_monthly = EXCLUDED.decisions_cap_monthly,
               per_rebalance_bps = EXCLUDED.per_rebalance_bps,
               aum_annual_bps = EXCLUDED.aum_annual_bps",
        )
        .bind(tier.code)
        .bind(tier.monthly_usd)
        .bind(tier.aum_cap_usd)
        .bind(tier.portfolios_cap)
        .bind(tier.decisions_cap_monthly)
        .bind(tier.per_rebalance_bps)
        .bind(tier.aum_annual_bps)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn seed_curated_strategies(pool: &PgPool) -> anyhow::Result<()> {
    for strategy in curated_strategies() {
        sqlx::query(
            "INSERT INTO strategies
               (id, name, description, risk_band, min_horizon_months, target_allocation, is_curated)
             VALUES ($1, $2, $3, $4, $5, $6, TRUE)
             ON CONFLICT (id) DO UPDATE SET
               name = EXCLUDED.name,
               description = EXCLUDED.description,
               risk_band = EXCLUDED.risk_band,
               min_horizon_months = EXCLUDED.min_horizon_months,
               target_allocation = EXCLUDED.target_allocation,
               is_curated = TRUE",
        )
        .bind(strategy.id)
        .bind(strategy.name)
        .bind(strategy.description)
        .bind(strategy.risk_band)
        .bind(strategy.min_horizon_months)
        .bind(&strategy.target_allocation)
        .execute(pool)
        .await?;
    }
    Ok(())
}

struct AssetSeed {
    symbol: &'static str,
    name: &'static str,
    coingecko_id: &'static str,
}

fn assets() -> Vec<AssetSeed> {
    vec![
        AssetSeed {
            symbol: "USDC",
            name: "USD Coin",
            coingecko_id: "usd-coin",
        },
        AssetSeed {
            symbol: "EURC",
            name: "EURC",
            coingecko_id: "euro-coin",
        },
        AssetSeed {
            symbol: "USYC",
            name: "Hashnote USYC",
            coingecko_id: "hashnote-usyc",
        },
        AssetSeed {
            symbol: "BTC",
            name: "Bitcoin",
            coingecko_id: "bitcoin",
        },
        AssetSeed {
            symbol: "ETH",
            name: "Ethereum",
            coingecko_id: "ethereum",
        },
        AssetSeed {
            symbol: "SOL",
            name: "Solana",
            coingecko_id: "solana",
        },
        AssetSeed {
            symbol: "BNB",
            name: "BNB",
            coingecko_id: "binancecoin",
        },
        AssetSeed {
            symbol: "AVAX",
            name: "Avalanche",
            coingecko_id: "avalanche-2",
        },
        AssetSeed {
            symbol: "LINK",
            name: "Chainlink",
            coingecko_id: "chainlink",
        },
        AssetSeed {
            symbol: "UNI",
            name: "Uniswap",
            coingecko_id: "uniswap",
        },
        AssetSeed {
            symbol: "MATIC",
            name: "Polygon",
            coingecko_id: "matic-network",
        },
    ]
}

struct PlanTierSeed {
    code: &'static str,
    monthly_usd: i32,
    aum_cap_usd: Option<i32>,
    portfolios_cap: Option<i32>,
    decisions_cap_monthly: Option<i32>,
    per_rebalance_bps: i32,
    aum_annual_bps: i32,
}

fn plan_tiers() -> Vec<PlanTierSeed> {
    vec![
        PlanTierSeed {
            code: "free",
            monthly_usd: 0,
            aum_cap_usd: Some(5_000),
            portfolios_cap: Some(1),
            decisions_cap_monthly: Some(5),
            per_rebalance_bps: 25,
            aum_annual_bps: 0,
        },
        PlanTierSeed {
            code: "pro",
            monthly_usd: 19,
            aum_cap_usd: None,
            portfolios_cap: Some(1),
            decisions_cap_monthly: Some(240),
            per_rebalance_bps: 15,
            aum_annual_bps: 25,
        },
        PlanTierSeed {
            code: "business",
            monthly_usd: 199,
            aum_cap_usd: None,
            portfolios_cap: Some(1),
            decisions_cap_monthly: None,
            per_rebalance_bps: 10,
            aum_annual_bps: 15,
        },
    ]
}

struct StrategySeed {
    id: Uuid,
    name: &'static str,
    description: &'static str,
    risk_band: &'static str,
    min_horizon_months: i32,
    target_allocation: serde_json::Value,
}

fn curated_strategies() -> Vec<StrategySeed> {
    vec![
        StrategySeed {
            id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            name: "Conservative Treasury",
            description: "Operating-cash treasury with principal preservation as the primary goal. Suited for teams and small businesses that need stable, liquid USDC holdings.",
            risk_band: "low",
            min_horizon_months: 12,
            target_allocation: json!({"USDC": 90, "EURC": 10}),
        },
        StrategySeed {
            id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            name: "Balanced",
            description: "Long-only stablecoin-anchored portfolio with majors exposure. The USDC sleeve provides a stable base; BTC and ETH provide asymmetric upside.",
            risk_band: "medium",
            min_horizon_months: 36,
            target_allocation: json!({"USDC": 50, "BTC": 30, "ETH": 20}),
        },
        StrategySeed {
            id: Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
            name: "Operating Reserve",
            description: "Multi-currency reserve for an internet-native organization with multi-jurisdiction operating expenses. USDC and EURC keep payroll in either denomination.",
            risk_band: "low",
            min_horizon_months: 60,
            target_allocation: json!({"USDC": 80, "EURC": 20}),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{assets, curated_strategies, plan_tiers};

    #[test]
    fn reference_data_covers_current_wallet_tokens_and_catalogs() {
        let symbols = assets()
            .into_iter()
            .map(|asset| asset.symbol)
            .collect::<Vec<_>>();
        assert!(symbols.contains(&"USDC"));
        assert!(symbols.contains(&"EURC"));
        assert!(symbols.contains(&"USYC"));
        assert!(symbols.contains(&"BTC"));
        assert!(symbols.contains(&"ETH"));
        assert!(symbols.contains(&"SOL"));
        assert_eq!(plan_tiers().len(), 3);
        assert_eq!(curated_strategies().len(), 3);
    }
}
