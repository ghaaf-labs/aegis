//! Smoke harness for a real CCTP V2 rebalance, end-to-end, without the HTTP
//! layer or the Circle Wallets API.
//!
//! Loads a pre-seeded user + portfolio from the database (see
//! `scripts/seed-rebalance-smoke.sh`), injects a synthetic Gateway pool
//! `{Base: portfolio_value, Arc: 0}` to force a cross-chain burn-mint pair,
//! drives `executor::approve_and_execute`, then polls until the rebalance
//! reaches a terminal state and prints the leg-level result.
//!
//! Run:
//!
//! ```bash
//! EXECUTION_MOCK=false MOCK_CIRCLE=false BILLING_V2_ENABLED=false \
//!     cargo run --features real-cctp --bin cctp_rebalance_smoke
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use sqlx::Row;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use uuid::Uuid;

use aegis_api::{
    config::Config,
    db,
    modules::{
        ai::PromptRegistry,
        rebalance::{
            executor::{approve_and_execute, create_plan},
            models::{ChainKey, PlanInput},
            planner::plan_legs,
        },
        sse,
    },
    router::AppStateInner,
};

const USER_ID: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
const PORTFOLIO_ID: &str = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    aegis_api::env::load_env();

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aegis_api=info,cctp_rebalance_smoke=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().compact())
        .init();

    let cfg = Config::from_env().context("Config::from_env")?;
    anyhow::ensure!(
        !cfg.execution_mock,
        "smoke must run with EXECUTION_MOCK=false; got true"
    );
    anyhow::ensure!(
        !cfg.circle_mock,
        "smoke must run with MOCK_CIRCLE=false; got true"
    );
    let public_vars = [
        ("CCTP_TOKEN_MESSENGER_BASE", &cfg.cctp_token_messenger_base),
        ("CCTP_TOKEN_MESSENGER_ARC", &cfg.cctp_token_messenger_arc),
        ("USDC_BASE", &cfg.usdc_base),
        ("USDC_ARC", &cfg.usdc_arc),
        ("REBALANCE_EXECUTOR_BASE", &cfg.rebalance_executor_base),
        ("REBALANCE_EXECUTOR_ARC", &cfg.rebalance_executor_arc),
    ];
    println!("--- runtime config ---");
    for (name, val) in public_vars {
        let v = if val.is_empty() {
            "<EMPTY — .env failed to parse?>".to_string()
        } else {
            val.clone()
        };
        println!("  {name:25} = {v}");
    }
    println!(
        "  CHAIN_PRIVATE_KEY_BASE    = {} chars",
        cfg.chain_private_key_base.len()
    );
    println!(
        "  CHAIN_PRIVATE_KEY_ARC     = {} chars",
        cfg.chain_private_key_arc.len()
    );
    println!("---");

    let pool = db::connect(&cfg.database_url).await?;
    let http = reqwest::Client::builder()
        .user_agent(concat!("Aegis-RebalanceSmoke/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(240))
        .build()?;
    let sse_tx = sse::new_channel();
    let prompts = Arc::new(PromptRegistry::load().await);
    let prices = aegis_api::modules::prices::build_from_config(http.clone(), &cfg);
    let state: aegis_api::router::AppState = Arc::new(AppStateInner {
        db: pool.clone(),
        config: cfg.clone(),
        http,
        sse: sse_tx,
        prompts,
        prices,
    });

    let portfolio_id: Uuid = PORTFOLIO_ID.parse()?;
    let user_id: Uuid = USER_ID.parse()?;

    let portfolio_row =
        sqlx::query("SELECT total_value_usd, goal FROM portfolios WHERE id = $1 AND user_id = $2")
            .bind(portfolio_id)
            .bind(user_id)
            .fetch_optional(&pool)
            .await?
            .ok_or_else(|| anyhow!("seed missing — run scripts/seed-rebalance-smoke.sh first"))?;
    let total_value_usd: f64 = portfolio_row.try_get("total_value_usd")?;
    let goal: serde_json::Value = portfolio_row.try_get("goal")?;
    info!(%portfolio_id, total_value_usd, "loaded portfolio");

    let alloc_rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT asset_symbol, current_weight FROM allocations WHERE portfolio_id = $1",
    )
    .bind(portfolio_id)
    .fetch_all(&pool)
    .await?;
    let mut current_weights: HashMap<String, f64> = HashMap::new();
    for (sym, w) in &alloc_rows {
        current_weights.insert(sym.clone(), w / 100.0);
    }

    let mut target_weights: HashMap<String, f64> = HashMap::new();
    if let Some(obj) = goal.get("targetAllocation").and_then(|v| v.as_object()) {
        for (k, v) in obj {
            if let Some(n) = v.as_f64() {
                target_weights.insert(k.clone(), n / 100.0);
            }
        }
    }
    if target_weights.is_empty() {
        anyhow::bail!("goal.targetAllocation empty; seed shape wrong");
    }

    let mut usdc_per_chain: HashMap<ChainKey, f64> = HashMap::new();
    usdc_per_chain.insert(ChainKey::Arc, 0.0);
    usdc_per_chain.insert(ChainKey::Base, total_value_usd);

    let input = PlanInput {
        portfolio_value_usd: total_value_usd,
        current_weights,
        target_weights,
        usdc_per_chain,
        drift_threshold: 0.05,
        dust_threshold_usd: 1.0,
        prices: HashMap::new(),
        regime: None,
    };
    let legs = plan_legs(&input);
    info!(legs_count = legs.len(), "planner emitted legs");
    if legs.is_empty() {
        anyhow::bail!("planner produced zero legs — adjust seed (target_value vs drift/dust)");
    }
    for (i, l) in legs.iter().enumerate() {
        println!(
            "  leg {} → kind={:?} src={:?} dest={:?} amount=${:.4} dest_symbol={:?}",
            i, l.kind, l.src_chain, l.dest_chain, l.amount_usdc, l.dest_symbol
        );
    }

    let decision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO agent_decisions (portfolio_id, reasoning, confidence, triggered_by)
         VALUES ($1, $2, 0.95, 'user_request')
         RETURNING id",
    )
    .bind(portfolio_id)
    .bind("rebalance smoke: cross-chain USDC via CCTP V2")
    .fetch_one(&pool)
    .await?;
    info!(%decision_id, "anchored agent decision");

    let rebalance_id = create_plan(&state, portfolio_id, decision_id, &legs).await?;
    println!();
    println!("rebalance_id = {rebalance_id}");

    approve_and_execute(state.clone(), rebalance_id).await?;
    info!(%rebalance_id, "executor started; polling status until terminal");

    // Outlast the executor's own attestation wait so the in-process tokio
    // task that finishes the rebalance doesn't get dropped when main exits.
    let started = std::time::Instant::now();
    let timeout = Duration::from_secs(cfg.cctp_attestation_timeout_secs + 60);
    loop {
        if started.elapsed() >= timeout {
            warn!("polling timeout — inspect rebalance_legs manually");
            break;
        }
        let row = sqlx::query("SELECT status, failure_reason FROM rebalances WHERE id = $1")
            .bind(rebalance_id)
            .fetch_one(&pool)
            .await?;
        let status: String = row.try_get("status")?;
        let failure: Option<String> = row.try_get("failure_reason")?;
        if status == "completed" || status == "failed" {
            println!();
            println!("FINAL status = {status}");
            if let Some(r) = failure {
                println!("failure_reason = {r}");
            }
            break;
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }

    println!();
    println!("=== rebalance_legs ===");
    let leg_rows = sqlx::query(
        "SELECT leg_index, kind, src_chain, dest_chain, amount_usdc, status,
                tx_hash, cctp_message_hash, failure_reason
         FROM rebalance_legs
         WHERE rebalance_id = $1
         ORDER BY leg_index",
    )
    .bind(rebalance_id)
    .fetch_all(&pool)
    .await?;
    for r in &leg_rows {
        let idx: i32 = r.try_get("leg_index")?;
        let kind: String = r.try_get("kind")?;
        let src: Option<String> = r.try_get("src_chain")?;
        let dest: Option<String> = r.try_get("dest_chain")?;
        let amt: f64 = r.try_get("amount_usdc")?;
        let status: String = r.try_get("status")?;
        let tx: Option<String> = r.try_get("tx_hash")?;
        let msg: Option<String> = r.try_get("cctp_message_hash")?;
        let fail: Option<String> = r.try_get("failure_reason")?;
        println!(
            "  [{idx}] {kind} {} → {} ${amt} status={status} tx={} msg={} fail={}",
            src.as_deref().unwrap_or("-"),
            dest.as_deref().unwrap_or("-"),
            tx.as_deref().unwrap_or("-"),
            msg.as_deref().unwrap_or("-"),
            fail.as_deref().unwrap_or("-"),
        );
    }

    Ok(())
}
