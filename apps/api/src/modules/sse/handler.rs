use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    Extension,
};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

use super::events::SseEvent;
use crate::middleware::auth::Claims;
use crate::router::AppState;

/// SSE endpoint. Each connected client gets its own broadcast receiver.
///
/// **Authentication required.** Public events (price.tick, regime.flip) reach
/// every authenticated subscriber; user-scoped events (agent.decision,
/// wallet.created, gateway.balance) are filtered to the subscriber whose session
/// `sub` matches `audience_user_id()`. Slow clients drop frames rather than
/// back-pressuring publishers.
pub async fn handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.sse.subscribe();
    let subscriber_id = claims.sub;

    let stream = BroadcastStream::new(rx).filter_map(move |res| match res {
        Ok(event) => match event.audience_user_id() {
            None => Some(Ok(to_sse_event(&event))),
            Some(uid) if uid == subscriber_id => Some(Ok(to_sse_event(&event))),
            _ => None,
        },
        // Lag errors mean a slow client missed messages; skip and continue.
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}

fn to_sse_event(event: &SseEvent) -> Event {
    let name = event.event_name();
    match Event::default().event(name).json_data(event) {
        Ok(e) => e,
        // Serialization should be infallible for these types; fall back to a
        // minimal payload rather than panicking the stream task.
        Err(_) => Event::default().event(name).data("{}"),
    }
}
