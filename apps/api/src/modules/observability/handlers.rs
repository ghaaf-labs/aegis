use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse;

use super::counters::render_prometheus;

pub async fn metrics() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "text/plain; version=0.0.4")],
        render_prometheus(),
    )
}
