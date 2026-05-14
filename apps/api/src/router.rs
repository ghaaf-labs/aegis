use axum::{
    middleware::from_fn_with_state,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::middleware::auth::require_auth;
use crate::modules::{
    agent, ai, analytics, faucet, fx, gateway, market_data, paymaster, portfolio, rebalance,
    sse::{self, SseSender},
    treasury, wallet,
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
        db,
        config: config.clone(),
        http: http.clone(),
        sse: sse_tx.clone(),
        prompts,
    });

    sse::spawn_price_ticker(http, Arc::new(config), sse_tx);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Routes that require an authenticated wallet.
    let authed = Router::new()
        .route("/auth/me", get(wallet::handlers::me))
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
            "/agent/decisions/:portfolio_id",
            get(agent::handlers::decisions),
        )
        .route("/agent/analyze", post(agent::handlers::analyze))
        .route_layer(from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .route("/health", get(health))
        // Wallet auth — public (no JWT required to create or login).
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
        // Market data — public for now (dashboard renders snapshots on landing).
        .route("/market/snapshot", get(market_data::handlers::snapshot))
        .route("/market/prices", get(market_data::handlers::prices))
        // Public rate endpoints — used by /explore and the goal wizard.
        .route(
            "/paymaster/estimate",
            get(paymaster::handlers::estimate_fee),
        )
        .route("/treasury/usyc/rate", get(treasury::handlers::usyc_rate))
        .route("/fx/usdc-eurc", get(fx::handlers::basis))
        // /sse moved to the authed router so user-scoped events can be
        // filtered server-side by audience_user_id.
        .merge(authed)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .with_state(state)
}

async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "service": "aegis-api"
    }))
}
