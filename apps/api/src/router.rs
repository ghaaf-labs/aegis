use axum::{
    http::{HeaderValue, Method},
    middleware::from_fn_with_state,
    routing::{get, patch, post},
    Router,
};
use std::sync::Arc;
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};

use crate::middleware::auth::require_auth;
use crate::modules::{
    agent, ai, analytics, backtest, billing, diary, digest, faucet, fx, gateway, market_data,
    observability, paymaster, portfolio, rebalance, risk_engine, scheduler,
    sse::{self, SseSender},
    tax, treasury, trustability, wallet,
};
use crate::{config::Config, db::Db};

pub type AppState = Arc<AppStateInner>;

pub struct AppStateInner {
    pub db: Db,
    pub config: Config,
    pub http: reqwest::Client,
    pub sse: SseSender,
    pub prompts: Arc<ai::PromptRegistry>,
}

pub async fn build(db: Db, config: Config) -> Router {
    let http = reqwest::Client::builder()
        // CoinGecko's free tier 403s anonymous / generic User-Agent strings
        // with the message "please add a descriptive User-Agent". Identify
        // ourselves explicitly — both for CoinGecko's heuristic and for
        // OpenRouter analytics.
        .user_agent(concat!("Aegis/", env!("CARGO_PKG_VERSION")))
        // 240s budget covers the full agent pipeline tail. DeepSeek-v4-pro
        // (strategist + revision) is a reasoning model that emits ~200-400
        // hidden CoT tokens per call, so a single completion can take 30-60s;
        // a strategist-tools-critic-revision pass can run 90-150s total.
        // Faster endpoints (CoinGecko, Circle, Iris) return in < 2s so the
        // extra ceiling is harmless. Reqwest reports streaming timeouts as
        // "error decoding response body", not "timeout" — bake an explicit
        // ceiling so the cause surfaces in the logs.
        .timeout(std::time::Duration::from_secs(240))
        .build()
        .expect("failed to build HTTP client");

    let sse_tx = sse::new_channel();
    let prompts = Arc::new(ai::PromptRegistry::load().await);

    let state = Arc::new(AppStateInner {
        db: db.clone(),
        config: config.clone(),
        http: http.clone(),
        sse: sse_tx.clone(),
        prompts,
    });

    // Realtime background tasks
    sse::spawn_price_ticker(
        http.clone(),
        Arc::new(config.clone()),
        sse_tx.clone(),
        db.clone(),
    );
    gateway::spawn_balance_ticker(db, http, Arc::new(config.clone()), sse_tx);

    // Long-running schedulers (cancelled when the process shuts down).
    let cancel = tokio_util::sync::CancellationToken::new();
    scheduler::spawn_portfolio_scheduler(state.clone(), cancel.clone());
    scheduler::spawn_outcome_compressor(state.clone(), cancel.clone());
    digest::spawn_digest_worker(state.clone(), cancel.clone());
    let _peg_monitor = risk_engine::spawn_peg_monitor(state.clone(), cancel.clone());
    agent::calibration_train::spawn(state.clone(), cancel);

    // CORS — must list specific origin(s) when sending credentials. The
    // wildcard isn't legal alongside `Access-Control-Allow-Credentials: true`.
    let cors = build_cors(&config);

    let authed = Router::new()
        .route("/auth/me", get(wallet::handlers::me))
        .route("/auth/logout", post(wallet::handlers::logout))
        .route("/faucet/usdc", post(faucet::handlers::claim_usdc))
        .route("/gateway/balance", get(gateway::handlers::balance))
        .route("/analytics/event", post(analytics::handlers::track))
        // SSE is authed — events are filtered server-side by audience_user_id.
        .route("/sse", get(sse::handler))
        .route(
            "/portfolios",
            get(portfolio::handlers::list).post(portfolio::handlers::create),
        )
        .route(
            "/portfolios/:id",
            get(portfolio::handlers::get)
                .put(portfolio::handlers::update)
                .delete(portfolio::handlers::delete),
        )
        .route(
            "/portfolios/:id/diary-public",
            get(portfolio::handlers::get_diary_public).patch(portfolio::handlers::set_diary_public),
        )
        .route(
            "/portfolios/:id/rebalance",
            post(rebalance::handlers::trigger),
        )
        .route(
            "/portfolios/:id/rebalance/plan",
            post(rebalance::handlers::create),
        )
        .route(
            "/portfolios/:id/rebalance/history",
            get(rebalance::handlers::history),
        )
        .route("/rebalance/:rebalance_id", get(rebalance::handlers::get))
        .route(
            "/rebalance/:rebalance_id/execute",
            post(rebalance::handlers::execute),
        )
        .route(
            "/tax/harvestable/:portfolio_id",
            get(tax::handlers::harvestable),
        )
        // A10: tax routes — 1099-DA export + accountant share tokens.
        // Gated by config.tax_export_v1_enabled (default false). The
        // public CSV-by-token endpoint is wired outside this authed
        // sub-router below.
        .route("/tax/export.csv", get(tax::handlers::export_csv))
        .route("/tax/shares", get(tax::handlers::list_shares))
        .route("/tax/share", post(tax::handlers::create_share))
        .route(
            "/tax/share/:token_id",
            axum::routing::delete(tax::handlers::revoke_share),
        )
        .route(
            "/digest/subscribe",
            post(digest::handlers::create).delete(digest::handlers::delete),
        )
        .route(
            "/agent/decisions/:portfolio_id",
            get(agent::handlers::decisions),
        )
        .route(
            "/agent/decision/:decision_id",
            get(agent::handlers::decision_by_id),
        )
        .route("/agent/analyze", post(agent::handlers::analyze))
        .route("/backtest/preview", post(backtest::handlers::preview))
        .route("/trustability/me", get(trustability::handlers::me))
        .route("/billing/referrals", get(billing::handlers::list_referrals))
        // F-REG-4 — admin regime-backtest endpoints. Auth-required; also
        // gated by REGIME_BACKTEST_ENABLED inside the handlers.
        .route(
            "/admin/regime/evaluations",
            get(risk_engine::handlers::list_evaluations),
        )
        .route(
            "/admin/regime/backtest",
            post(risk_engine::handlers::kick_off_backtest),
        )
        // F-PEG-4 — peg-defense rules CRUD. Auth-required; gated by
        // PEG_DEFENSE_ENABLED inside the handlers.
        .route(
            "/peg/rules",
            get(risk_engine::handlers::list).post(risk_engine::handlers::create),
        )
        .route(
            "/peg/rules/:id",
            patch(risk_engine::handlers::patch).delete(risk_engine::handlers::delete),
        )
        .route("/peg/rules/:id/pause", post(risk_engine::handlers::pause))
        .route(
            "/peg/rules/:id/unpause",
            post(risk_engine::handlers::unpause),
        )
        // F-TIER-2/5 — subscription + invoice endpoints. Auth-required;
        // gated by BILLING_V2_ENABLED inside the handlers.
        .route(
            "/billing/subscription",
            get(billing::handlers::get_subscription),
        )
        .route(
            "/billing/subscriptions",
            post(billing::handlers::create_subscription),
        )
        .route(
            "/billing/subscriptions/:id",
            patch(billing::handlers::patch_subscription),
        )
        .route("/billing/invoices", get(billing::handlers::list_invoices))
        // F-AUM-5 — admin AUM accrual list + run-once. Gated by
        // AUM_STREAM_ENABLED inside the handlers.
        .route(
            "/admin/billing/accruals",
            get(billing::handlers::list_accruals),
        )
        .route(
            "/admin/billing/accruals/run-once",
            post(billing::handlers::run_accruals_once),
        )
        .route_layer(from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(observability::handlers::metrics))
        // Wallet auth — public (cookies + token set on success).
        .route(
            "/auth/wallet/create",
            post(wallet::handlers::create_passkey),
        )
        .route("/auth/wallet/login", post(wallet::handlers::login_passkey))
        .route("/auth/wallet/otp/start", post(wallet::handlers::start_otp))
        .route(
            "/auth/wallet/otp/verify",
            post(wallet::handlers::verify_otp),
        )
        // Market data — public.
        .route("/market/snapshot", get(market_data::handlers::snapshot))
        .route("/market/prices", get(market_data::handlers::prices))
        // Public rate endpoints — used by /explore and the goal wizard.
        .route(
            "/paymaster/estimate",
            get(paymaster::handlers::estimate_fee),
        )
        .route("/treasury/usyc/rate", get(treasury::handlers::usyc_rate))
        .route("/fx/usdc-eurc", get(fx::handlers::basis))
        // Public pricing catalogue — gated by BILLING_V2_ENABLED inside the handler.
        .route("/billing/tiers", get(billing::handlers::list_tiers))
        // Public constitution model-card — version + clauses, read-only.
        .route(
            "/about/constitution",
            get(agent::constitution_handlers::document),
        )
        // Public leaderboard — anonymous handles, no auth required.
        .route("/leaderboard", get(trustability::handlers::leaderboard))
        // F-REG-4 — public read-only alias for the /about/regime model card.
        .route(
            "/about/regime/latest",
            get(risk_engine::handlers::latest_public),
        )
        // Public diary + share-card data + unsubscribe — no auth.
        .route("/diary/wallet/:wallet", get(diary::handlers::by_wallet))
        .route(
            "/diary/decision/:decision_id",
            get(diary::handlers::by_decision),
        )
        .route(
            "/digest/unsubscribe",
            get(digest::handlers::unsubscribe_public),
        )
        // A10: public tax share — token in path is the auth.
        .route(
            "/tax/share/:token/export.csv",
            get(tax::handlers::export_via_share),
        )
        .merge(authed)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .with_state(state)
}

fn build_cors(config: &Config) -> CorsLayer {
    let layer = CorsLayer::new()
        .allow_credentials(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ]);

    let origins: Vec<HeaderValue> = config
        .cors_allow_origin
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| HeaderValue::from_str(s).ok())
        .collect();

    if origins.is_empty() {
        // No origin configured — fall back to localhost dev origin.
        layer.allow_origin(HeaderValue::from_static("http://localhost:3000"))
    } else {
        layer.allow_origin(origins)
    }
}

async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "service": "aegis-api"
    }))
}
