use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::time::Duration;
use tokio::time::interval;

use crate::router::AppState;

pub async fn handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut tick = interval(Duration::from_secs(5));

    loop {
        tokio::select! {
            _ = tick.tick() => {
                let msg = json!({
                    "type": "price_update",
                    "payload": {
                        "btc": 67420.0 + (rand_jitter() * 100.0),
                        "eth": 3521.0 + (rand_jitter() * 20.0),
                    },
                    "timestamp": Utc::now().to_rfc3339()
                });

                if sender.send(Message::Text(msg.to_string().into())).await.is_err() {
                    break;
                }
            }
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Echo ping/pong
                        if text.contains("ping") {
                            let _ = sender.send(Message::Text(
                                json!({ "type": "pong", "timestamp": Utc::now().to_rfc3339() }).to_string().into()
                            )).await;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

fn rand_jitter() -> f64 {
    (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as f64
        / 1_000_000_000.0)
        * 2.0
        - 1.0
}
