//! Real-shaped demo seed for `/explore`, the leaderboard, and the public diary.
//!
//! Populates 2-3 demo users by driving the REAL service layer in mock-execution
//! mode, so the demo surfaces read genuine seeded DB state instead of
//! hand-authored fixtures. For each user the seed:
//!   1. upserts the user + provisions Circle wallets via the in-process
//!      `MockProvider` (the same path the app uses when `MOCK_CIRCLE=true`),
//!   2. sets the diary public + a per-user risk profile,
//!   3. creates a goal-only portfolio (objective + horizon + risk),
//!   4. runs the real pipeline: allocator (`propose_allocation`) → approve
//!      (`apply_allocation`) → plan (`create_plan`) → execute in mock mode
//!      (`approve_and_execute`).
//!
//! This writes real `agent_decisions` (`kind:"allocation_proposal"` + a planner
//! decision), `allocations`, and `rebalances` (`execution_mode='mock'`), and
//! leaves the 24h outcome compressor able to produce `agent_memory` later.
//!
//! LLM dependency: `propose_allocation` calls the allocator model. If
//! `OPENROUTER_API_KEY` is set, it is called for real; if not, the seed falls
//! back to inserting a representative `allocation_proposal` decision and then
//! applying it — so the seed runs fully offline against just a Postgres.
//!
//! Idempotent-ish: deterministic demo emails mean a re-run upserts the same
//! users + single portfolio rather than duplicating them.
//!
//! Run (mock execution is mandatory):
//!
//! ```bash
//! # Offline — DB only, deterministic fallback allocation:
//! EXECUTION_MOCK=true MOCK_CIRCLE=true \
//!     DATABASE_URL=postgres://... cargo run --bin seed_demo
//!
//! # With a live allocator model:
//! EXECUTION_MOCK=true MOCK_CIRCLE=true OPENROUTER_API_KEY=sk-or-... \
//!     DATABASE_URL=postgres://... cargo run --bin seed_demo
//! ```

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use serde_json::json;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use uuid::Uuid;

use aegis_api::{
    config::Config,
    db,
    modules::{
        agent::{
            models::ProposeAllocationRequest,
            service::{apply_allocation, propose_allocation},
        },
        ai::PromptRegistry,
        rebalance::executor::{approve_and_execute, create_plan},
        rebalance::models::PlannedLeg,
        rebalance::planner::plan_legs,
        sse,
        wallet::models::EmailAuthConsent,
        wallet::{MockProvider, WalletService},
    },
    router::{AppState, AppStateInner},
};

/// A demo persona. `target` is only used by the offline fallback; when an
/// allocator model is configured the agent designs its own target.
struct DemoUser {
    email: &'static str,
    portfolio_name: &'static str,
    objective: &'static str,
    horizon: &'static str,
    risk: &'static str,
    horizon_months: i32,
    /// Representative target weights (percent) for the offline fallback.
    fallback_target: &'static [(&'static str, f64)],
    reasoning: &'static str,
}

const DEMO_USERS: &[DemoUser] = &[
    DemoUser {
        email: "demo.grow@aegis.demo",
        portfolio_name: "Long-horizon growth",
        objective: "grow",
        horizon: "10y",
        risk: "aggressive",
        horizon_months: 120,
        fallback_target: &[("USDC", 35.0), ("BTC", 35.0), ("ETH", 30.0)],
        reasoning: "Long horizon and aggressive tolerance: a majority crypto sleeve (BTC/ETH) with a USDC reserve to fund rebalances and dampen drawdowns.",
    },
    DemoUser {
        email: "demo.balanced@aegis.demo",
        portfolio_name: "Balanced multi-currency",
        objective: "grow",
        horizon: "5y",
        risk: "moderate",
        horizon_months: 60,
        fallback_target: &[("USDC", 50.0), ("BTC", 25.0), ("ETH", 15.0), ("EURC", 10.0)],
        reasoning: "Moderate risk over a 5y horizon: a balanced book with a stable USDC core, a measured BTC/ETH sleeve, and an EURC currency tilt via Arc StableFX.",
    },
    DemoUser {
        email: "demo.preserve@aegis.demo",
        portfolio_name: "Capital preservation",
        objective: "preserve",
        horizon: "3y",
        risk: "conservative",
        horizon_months: 36,
        fallback_target: &[("USDC", 80.0), ("BTC", 12.0), ("ETH", 8.0)],
        reasoning: "Conservative preservation mandate: a dominant USDC reserve with a small BTC/ETH sleeve for upside, keeping realized volatility low.",
    },
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    aegis_api::env::load_env();

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aegis_api=info,seed_demo=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().compact())
        .init();

    let mut cfg = Config::from_env().context("Config::from_env")?;

    // The seed must never touch a real chain or the live Circle API. Force both
    // mocks on regardless of the ambient env so a misconfigured shell can't make
    // the seed mint real transactions or hit Circle.
    if !cfg.execution_mock || !cfg.circle_mock {
        warn!(
            execution_mock = cfg.execution_mock,
            circle_mock = cfg.circle_mock,
            "forcing EXECUTION_MOCK=true + MOCK_CIRCLE=true for the demo seed"
        );
        cfg.execution_mock = true;
        cfg.circle_mock = true;
    }

    let has_llm = !cfg.openrouter_api_key.trim().is_empty();
    info!(
        has_llm,
        "demo seed starting (allocator = {})",
        if has_llm {
            "live model"
        } else {
            "offline fallback"
        }
    );

    let pool = db::connect(&cfg.database_url)
        .await
        .context("connect database")?;
    let http = reqwest::Client::builder()
        .user_agent(concat!("Aegis-SeedDemo/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(240))
        .build()?;
    let sse_tx = sse::new_channel();
    let prompts = Arc::new(PromptRegistry::load().await);
    let prices = aegis_api::modules::prices::build_from_config(http.clone(), &cfg);

    let state: AppState = Arc::new(AppStateInner {
        db: pool.clone(),
        config: cfg.clone(),
        http: http.clone(),
        sse: sse_tx.clone(),
        prompts,
        prices,
    });

    for demo in DEMO_USERS {
        if let Err(e) = seed_user(&state, demo, has_llm).await {
            warn!(email = demo.email, error = %e, "demo user seed failed");
        }
    }

    info!("demo seed complete");
    Ok(())
}

async fn seed_user(state: &AppState, demo: &DemoUser, has_llm: bool) -> anyhow::Result<()> {
    info!(email = demo.email, "seeding demo user");

    // 1. Upsert user + provision/persist the mock wallet via the real service.
    let user_id = ensure_user_and_wallet(state, demo).await?;

    // 2. Per-user risk profile (the allocator + guardrails read these columns).
    sqlx::query(
        "UPDATE users
            SET risk_tolerance = $2,
                investment_horizon_months = $3
          WHERE id = $1",
    )
    .bind(user_id)
    .bind(demo.risk)
    .bind(demo.horizon_months)
    .execute(&state.db)
    .await?;

    // 3. Goal-only portfolio (single-portfolio-per-user; upsert on re-run).
    let portfolio_id = ensure_goal_portfolio(state, user_id, demo).await?;

    // 4. Public diary so /diary/:wallet + the leaderboard surface this user.
    sqlx::query("UPDATE portfolios SET diary_public = TRUE, updated_at = NOW() WHERE id = $1")
        .bind(portfolio_id)
        .execute(&state.db)
        .await?;

    // 5. Allocator (Gate 1): real model when configured, else offline fallback.
    let decision_id = if has_llm {
        match propose_allocation(
            state,
            ProposeAllocationRequest {
                portfolio_id,
                triggered_by: Some("demo_seed".into()),
                risk_override: None,
            },
        )
        .await
        {
            Ok(decision) => decision.id,
            Err(e) => {
                warn!(error = %e, "live allocator failed; using offline fallback proposal");
                insert_fallback_proposal(state, portfolio_id, demo).await?
            }
        }
    } else {
        insert_fallback_proposal(state, portfolio_id, demo).await?
    };

    // 6. Approve the proposal → writes goal.targetAllocation + allocations.
    apply_allocation(state, decision_id, user_id).await?;

    // 7. Build a plan from the approved target + idle mock Gateway USDC, then
    //    execute it in mock mode. This populates `rebalances`
    //    (`execution_mode='mock'`) and confirmed `rebalance_legs`.
    drive_rebalance(state, portfolio_id).await?;

    info!(email = demo.email, %user_id, %portfolio_id, "demo user seeded");
    Ok(())
}

/// Upsert the user and provision + persist their mock wallet through the real
/// `WalletService` (the same code the email-auth path runs). Idempotent: an
/// existing email logs in and re-syncs routes rather than duplicating.
async fn ensure_user_and_wallet(state: &AppState, demo: &DemoUser) -> anyhow::Result<Uuid> {
    let provider = MockProvider;
    let service = WalletService::new(&state.db, &provider, &state.config, &state.sse);
    let consent = demo_consent();
    let response = service
        .init_continue(demo.email, Some(&consent))
        .await
        .context("init_continue (user upsert + mock wallet)")?;
    Ok(response.user.id)
}

/// Current-version consent so `WalletService` provisions immediately instead of
/// short-circuiting on `consent_required`.
fn demo_consent() -> EmailAuthConsent {
    EmailAuthConsent {
        tos: true,
        privacy: true,
        tos_version: Some(aegis_api::modules::wallet::service::CURRENT_TOS_VERSION.to_string()),
        privacy_version: Some(
            aegis_api::modules::wallet::service::CURRENT_PRIVACY_VERSION.to_string(),
        ),
        marketing_opt_in: Some(false),
    }
}

/// Create or update the user's single goal-only portfolio. Mirrors the
/// goal-wizard shape (`objective`/`horizon`/`riskTolerance` + permissive route
/// preferences) so the allocator + planner read genuine inputs. The agent owns
/// `targetAllocation`, so it starts empty.
async fn ensure_goal_portfolio(
    state: &AppState,
    user_id: Uuid,
    demo: &DemoUser,
) -> anyhow::Result<Uuid> {
    let goal = json!({
        "objective": demo.objective,
        "horizon": demo.horizon,
        "riskTolerance": demo.risk,
        "name": demo.portfolio_name,
        "targetAllocation": {},
        "includeUsyc": false,
        "includeEurc": demo.fallback_target.iter().any(|(s, _)| *s == "EURC"),
        "routePreferences": {
            "networks": ["ARC-TESTNET", "BASE-SEPOLIA"],
            "networkWatchlist": ["ETH-SEPOLIA", "ARB-SEPOLIA", "AVAX-FUJI"],
            // Permissive token set so the agent's chosen target survives the
            // planner's route-preference filter for the demo deploy.
            "tokens": ["USDC", "BTC", "ETH", "SOL", "EURC"],
            "watchlist": ["BTC", "ETH", "SOL", "EURC"],
            "createdAt": chrono::Utc::now().to_rfc3339(),
        },
    });

    let mut tx = state.db.begin().await?;
    let existing_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM portfolios
         WHERE user_id = $1
         ORDER BY updated_at DESC, created_at DESC
         LIMIT 1
         FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;

    let portfolio_id = if let Some(id) = existing_id {
        sqlx::query("UPDATE portfolios SET name = $1, goal = $2, updated_at = NOW() WHERE id = $3")
            .bind(demo.portfolio_name)
            .bind(&goal)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        id
    } else {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO portfolios (id, user_id, name, goal) VALUES ($1, $2, $3, $4)")
            .bind(id)
            .bind(user_id)
            .bind(demo.portfolio_name)
            .bind(&goal)
            .execute(&mut *tx)
            .await?;
        id
    };

    // Enforce the single-portfolio invariant on re-run.
    sqlx::query("DELETE FROM portfolios WHERE user_id = $1 AND id <> $2")
        .bind(user_id)
        .bind(portfolio_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(portfolio_id)
}

/// Insert a representative `allocation_proposal` decision directly, matching the
/// columns `propose_allocation` writes. Used when no allocator model is
/// configured so the seed runs offline. `apply_allocation` re-clamps the stored
/// allocation, so this need only be a sensible, executable target map.
async fn insert_fallback_proposal(
    state: &AppState,
    portfolio_id: Uuid,
    demo: &DemoUser,
) -> anyhow::Result<Uuid> {
    let alloc_obj: serde_json::Map<String, serde_json::Value> = demo
        .fallback_target
        .iter()
        .map(|(sym, pct)| (sym.to_string(), json!(pct)))
        .collect();
    let alloc_value = serde_json::Value::Object(alloc_obj.clone());

    let summary = format!(
        "Agent target: {}",
        demo.fallback_target
            .iter()
            .map(|(s, p)| format!("{s} {p:.0}%"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let recommendation = json!({
        "summary": summary,
        "recommendedAllocation": alloc_value,
        "expectedMaxDrawdownPct": null,
    });
    let critic_verdict = json!({
        "verdict": "approve",
        "notes": "Offline demo-seed proposal (no allocator model configured).",
        "confidence": 1.0,
        "clauseIds": [],
    });

    let id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO agent_decisions
           (id, portfolio_id, reasoning, recommendation, confidence,
            triggered_by, model_slug, regime, prompt_tokens, completion_tokens,
            latency_ms, critic_verdict, snapshot, raw_confidence,
            calibrated_confidence, counterfactual, kind, recommended_allocation)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                   $15, $16, $17, $18)
           RETURNING id"#,
    )
    .bind(Uuid::new_v4())
    .bind(portfolio_id)
    .bind(demo.reasoning)
    .bind(&recommendation)
    .bind(0.85_f64)
    .bind("demo_seed")
    .bind("aegis/demo-seed-allocator-v1")
    .bind("neutral")
    .bind(0_i32)
    .bind(0_i32)
    .bind(0_i32)
    .bind(&critic_verdict)
    .bind(json!({}))
    .bind(0.85_f64)
    .bind(0.85_f64)
    .bind(Option::<String>::None)
    .bind("allocation_proposal")
    .bind(&alloc_value)
    .fetch_one(&state.db)
    .await?;
    Ok(id)
}

/// Build a rebalance plan from the approved target + idle mock Gateway USDC and
/// execute it in mock mode. Reuses the real `create_plan` / `approve_and_execute`
/// path, then polls until the rebalance reaches a terminal state. A no-op plan
/// (nothing to deploy) is not an error — the proposal + allocations already give
/// the demo surfaces real state.
async fn drive_rebalance(state: &AppState, portfolio_id: Uuid) -> anyhow::Result<()> {
    let legs = build_plan_legs(state, portfolio_id).await?;
    if legs.is_empty() {
        info!(%portfolio_id, "planner produced no legs (nothing to deploy); skipping execution");
        return Ok(());
    }

    // A mock-mode planner decision tied to the legs the executor will walk.
    let decision_id = insert_mock_rebalance_decision(state, portfolio_id).await?;
    let rebalance_id = create_plan(state, portfolio_id, decision_id, &legs).await?;
    approve_and_execute(state.clone(), rebalance_id).await?;

    // `approve_and_execute` spawns the walk on a background task; poll until the
    // rebalance is terminal so the row is fully populated before the seed exits.
    let started = std::time::Instant::now();
    let timeout = Duration::from_secs(30);
    loop {
        let status: String = sqlx::query_scalar("SELECT status FROM rebalances WHERE id = $1")
            .bind(rebalance_id)
            .fetch_one(&state.db)
            .await?;
        if status == "completed" || status == "failed" {
            info!(%rebalance_id, status, "rebalance reached terminal state");
            break;
        }
        if started.elapsed() >= timeout {
            warn!(%rebalance_id, status, "rebalance still running at poll timeout");
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Ok(())
}

/// Recompute the current set of planner legs the same way the rebalance handler
/// does: read the approved target + confirmed holdings + idle mock Gateway USDC.
async fn build_plan_legs(state: &AppState, portfolio_id: Uuid) -> anyhow::Result<Vec<PlannedLeg>> {
    use std::collections::HashMap;

    use aegis_api::modules::rebalance::models::{ChainKey, PlanInput};

    let (user_id, goal): (Uuid, serde_json::Value) =
        sqlx::query_as("SELECT user_id, goal FROM portfolios WHERE id = $1")
            .bind(portfolio_id)
            .fetch_one(&state.db)
            .await?;

    let mut target_weights: HashMap<String, f64> = HashMap::new();
    if let Some(obj) = goal.get("targetAllocation").and_then(|v| v.as_object()) {
        for (k, v) in obj {
            if let Some(n) = v.as_f64() {
                target_weights.insert(k.clone(), n / 100.0);
            }
        }
    }
    if target_weights.is_empty() {
        return Ok(Vec::new());
    }

    // Fresh demo portfolio: no confirmed positions yet, so current weights are
    // empty and every target weight is funded from idle Gateway USDC.
    let balance = aegis_api::modules::gateway::service::fetch_balance_for_user(
        &state.db,
        &state.http,
        &state.config,
        user_id,
    )
    .await?;
    let mut usdc_per_chain: HashMap<ChainKey, f64> = HashMap::new();
    usdc_per_chain.insert(ChainKey::Arc, 0.0);
    usdc_per_chain.insert(ChainKey::Base, 0.0);
    for (chain, amount) in &balance.per_chain {
        if let Some(key) = ChainKey::parse(chain.to_lowercase().as_str()) {
            usdc_per_chain.insert(key, *amount);
        }
    }
    let idle_usdc: f64 = usdc_per_chain.values().copied().sum();

    let input = PlanInput {
        portfolio_value_usd: idle_usdc,
        current_weights: HashMap::new(),
        target_weights,
        usdc_per_chain,
        drift_threshold: 0.05,
        dust_threshold_usd: 5.0,
        prices: HashMap::new(),
        regime: None,
    };
    Ok(plan_legs(&input))
}

/// A mock-mode planner decision for the demo rebalance. Matches the shape the
/// rebalance handler's mock path writes so downstream readers stay consistent.
async fn insert_mock_rebalance_decision(
    state: &AppState,
    portfolio_id: Uuid,
) -> anyhow::Result<Uuid> {
    let rec = json!({
        "summary": "Demo deploy: allocate idle USDC into the approved target.",
        "trades": [],
        "expectedImpact": { "riskDelta": 0.0, "diversificationScore": 0.5 }
    });
    let id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO agent_decisions
           (id, portfolio_id, reasoning, recommendation, confidence,
            triggered_by, model_slug, regime, prompt_tokens, completion_tokens,
            latency_ms, critic_verdict, snapshot, raw_confidence,
            calibrated_confidence, counterfactual)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
           RETURNING id"#,
    )
    .bind(Uuid::new_v4())
    .bind(portfolio_id)
    .bind("Demo seed rebalance — mock execution mode")
    .bind(&rec)
    .bind(1.0_f64)
    .bind("demo_seed")
    .bind("aegis/demo-seed-rebalance-v1")
    .bind("neutral")
    .bind(0_i32)
    .bind(0_i32)
    .bind(0_i32)
    .bind(serde_json::Value::Null)
    .bind(json!({}))
    .bind(1.0_f64)
    .bind(None::<f64>)
    .bind(None::<String>)
    .fetch_one(&state.db)
    .await?;
    Ok(id)
}
