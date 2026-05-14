use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

use super::events::SseEvent;
use crate::router::AppState;

/// SSE endpoint. Each connected client gets its own broadcast receiver;
/// slow clients drop frames rather than back-pressuring publishers.
pub async fn handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.sse.subscribe();

    let stream = BroadcastStream::new(rx).filter_map(|res| match res {
        Ok(event) => Some(Ok(to_sse_event(&event))),
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
