use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::modules::{
    agent, ai, auth, market_data, portfolio, rebalance,
    sse::{self, SseSender},
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

    // Spawn the price ticker once the channel exists. It runs for the
    // process lifetime; if no clients are subscribed it noops cheaply.
    sse::spawn_price_ticker(http, Arc::new(config), sse_tx);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health
        .route("/health", get(health))
        // Auth
        .route("/auth/register", post(auth::handlers::register))
        .route("/auth/login", post(auth::handlers::login))
        .route("/auth/me", get(auth::handlers::me))
        // Portfolios
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
        // Market data
        .route("/market/snapshot", get(market_data::handlers::snapshot))
        .route("/market/prices", get(market_data::handlers::prices))
        // Agent
        .route(
            "/agent/decisions/:portfolio_id",
            get(agent::handlers::decisions),
        )
        .route("/agent/analyze", post(agent::handlers::analyze))
        // SSE
        .route("/sse", get(sse::handler))
        // Layers
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
