use axum::{extract::State, Json};

use super::{service, AssetPrice, MarketSnapshot};
use crate::router::AppState;

pub async fn snapshot(State(state): State<AppState>) -> crate::error::Result<Json<MarketSnapshot>> {
    let snap = service::fetch_snapshot(&state.http, &state.config)
        .await
        .map_err(anyhow::Error::from)?;
    Ok(Json(snap))
}

pub async fn prices(State(state): State<AppState>) -> crate::error::Result<Json<Vec<AssetPrice>>> {
    let prices = service::fetch_prices(&state.http, &state.config)
        .await
        .map_err(anyhow::Error::from)?;
    Ok(Json(prices))
}
