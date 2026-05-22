use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[allow(dead_code)]
    #[error("bad request: {0}")]
    BadRequest(String),

    #[allow(dead_code)] // re-introduced when business logic surfaces conflicts
    #[error("conflict: {0}")]
    Conflict(String),

    #[error("too many requests: {0}")]
    TooManyRequests(String),

    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),

    /// HTTP 402 — emitted when a tier cap is hit (decisions/month, AUM,
    /// portfolios). UI maps this to an "upgrade required" prompt.
    #[allow(dead_code)]
    #[error("payment required: {0}")]
    PaymentRequired(String),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            AppError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            AppError::Conflict(_) => (StatusCode::CONFLICT, "CONFLICT"),
            AppError::TooManyRequests(_) => (StatusCode::TOO_MANY_REQUESTS, "TOO_MANY_REQUESTS"),
            AppError::ServiceUnavailable(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, "SERVICE_UNAVAILABLE")
            }
            AppError::PaymentRequired(_) => (StatusCode::PAYMENT_REQUIRED, "PAYMENT_REQUIRED"),
            AppError::Database(e) => {
                tracing::error!("database error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR")
            }
            AppError::Internal(e) => {
                tracing::error!("internal error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR")
            }
            AppError::Serde(e) => {
                tracing::error!("serde error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "SERDE_ERROR")
            }
        };

        let body = Json(json!({ "code": code, "message": self.to_string() }));
        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
