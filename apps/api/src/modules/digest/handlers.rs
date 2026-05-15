use axum::{
    extract::{Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::middleware::auth::Claims;
use crate::router::AppState;

use super::service::{subscribe, unsubscribe_by_token, unsubscribe_by_user};

#[derive(Debug, Deserialize)]
pub struct SubscribeBody {
    pub email: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeResponse {
    pub unsubscribe_token: String,
}

fn looks_like_email(s: &str) -> bool {
    let s = s.trim();
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.')
}

pub async fn create(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<SubscribeBody>,
) -> Result<Json<SubscribeResponse>> {
    let email = body.email.trim();
    if !looks_like_email(email) {
        return Err(AppError::BadRequest("invalid email address".into()));
    }
    let token = subscribe(&state, claims.sub, email).await?;
    Ok(Json(SubscribeResponse {
        unsubscribe_token: token,
    }))
}

pub async fn delete(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<StatusCode> {
    unsubscribe_by_user(&state, claims.sub).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct UnsubscribeQuery {
    pub t: String,
}

pub async fn unsubscribe_public(
    State(state): State<AppState>,
    Query(q): Query<UnsubscribeQuery>,
) -> Result<axum::response::Html<&'static str>> {
    unsubscribe_by_token(&state, &q.t).await?;
    Ok(axum::response::Html(
        "<html><body style=\"font-family:sans-serif;padding:32px\">\
         <h1>Unsubscribed</h1>\
         <p>You won't receive any more Aegis digest emails.</p>\
         </body></html>",
    ))
}
