//! Server-Sent Events module.
//!
//! Exposes `/sse` (see `handler`) and a typed broadcast channel that lives on
//! `AppState`. Every emitter (price ticker, agent service, regime classifier)
//! sends `SseEvent`s; subscribers convert them to `axum::response::sse::Event`
//! frames named by their type.

pub mod events;
pub mod handler;
pub mod ticker;

// Re-export every payload type. `GatewayBalance`, `PriceTick`, and
// `RebalanceStatus` are constructed by emitters in this crate but only
// referenced through `SseEvent` outside; the explicit re-exports keep the
// public surface complete for downstream consumers.
#[allow(unused_imports)]
pub use events::{
    AgentAbstainedPayload, AgentDecisionPayload, AgentToolInvokedPayload, GatewayBalance,
    PegAlertPayload, PriceTick, RebalanceLegPayload, RebalancePlanPayload, RebalanceStatus,
    RegimeFlip, RegimeSignals, SseEvent, TaxHarvestPayload,
};
pub use handler::handler;
pub use ticker::spawn_price_ticker;

use tokio::sync::broadcast;

pub type SseSender = broadcast::Sender<SseEvent>;

/// Reasonable default channel capacity. SSE clients drop slow consumers
/// rather than back-pressuring the publisher; oversize to absorb bursts.
pub const SSE_CHANNEL_CAPACITY: usize = 512;

pub fn new_channel() -> SseSender {
    let (tx, _rx) = broadcast::channel(SSE_CHANNEL_CAPACITY);
    tx
}
