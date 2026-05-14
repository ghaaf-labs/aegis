use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;

use super::service::{estimate, FeeEstimate, PaymasterChain};
use crate::router::AppState;

#[derive(Debug, Deserialize)]
pub struct EstimateQuery {
    #[serde(default = "default_chain")]
    pub chain: PaymasterChain,
    #[serde(default = "default_action")]
    pub action: String,
}

fn default_chain() -> PaymasterChain {
    PaymasterChain::Arc
}

fn default_action() -> String {
    "rebalance".into()
}

pub async fn estimate_fee(
    State(state): State<AppState>,
    Query(q): Query<EstimateQuery>,
) -> crate::error::Result<Json<FeeEstimate>> {
    let est = estimate(&state.config, q.chain, &q.action).await?;
    Ok(Json(est))
}
