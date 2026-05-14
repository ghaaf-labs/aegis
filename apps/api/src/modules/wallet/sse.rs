//! SSE payload for `wallet.created`. Kept in the wallet module so the
//! struct can move with the rest of the wallet domain types.

use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletCreatedPayload {
    pub wallet_id: String,
    pub arc_address: String,
    pub base_address: String,
    pub created_at: DateTime<Utc>,
}
