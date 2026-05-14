use axum::{
    http::{HeaderValue, Method},
    middleware::from_fn_with_state,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};

use crate::middleware::auth::require_auth;
use crate::modules::{
    agent, ai, analytics, diary, digest, faucet, fx, gateway, market_data, paymaster, portfolio,
    rebalance, scheduler,
    sse::{self, SseSender},
    tax, treasury, wallet,
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
        .timeout(std::time::Duration::from_secs(30))
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
    sse::spawn_price_ticker(http.clone(), Arc::new(config.clone()), sse_tx.clone());
    gateway::spawn_balance_ticker(db, http, Arc::new(config.clone()), sse_tx);

    // Long-running schedulers (cancelled when the process shuts down).
    let cancel = tokio_util::sync::CancellationToken::new();
    scheduler::spawn_portfolio_scheduler(state.clone(), cancel.clone());
    scheduler::spawn_outcome_compressor(state.clone(), cancel.clone());
    digest::spawn_digest_worker(state.clone(), cancel);

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
        .route(
            "/digest/subscribe",
            post(digest::handlers::create).delete(digest::handlers::delete),
        )
        .route(
            "/agent/decisions/:portfolio_id",
            get(agent::handlers::decisions),
        )
        .route("/agent/analyze", post(agent::handlers::analyze))
        .route_layer(from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .route("/health", get(health))
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
