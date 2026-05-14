//! SSE payload for `wallet.created`. Kept in the wallet module so the
//! struct can move with the rest of the wallet domain types.

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletCreatedPayload {
    /// Audience filter — `/sse` only forwards this event to the matching user.
    pub user_id: Uuid,
    pub wallet_id: String,
    pub arc_address: String,
    pub base_address: String,
    pub created_at: DateTime<Utc>,
}
