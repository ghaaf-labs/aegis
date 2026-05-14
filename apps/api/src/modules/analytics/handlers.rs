use axum::{extract::State, Extension, Json};

use super::service::{emit, ClientEventBody, EventAccepted};
use crate::middleware::auth::Claims;
use crate::router::AppState;

pub async fn track(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<ClientEventBody>,
) -> crate::error::Result<Json<EventAccepted>> {
    let name = body.event_name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(crate::error::AppError::BadRequest(
            "invalid event_name".into(),
        ));
    }
    emit(&state.db, Some(claims.sub), name, body.properties).await;
    Ok(Json(EventAccepted { accepted: true }))
}
