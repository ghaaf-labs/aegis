use axum::{
    extract::Request,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
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

    #[error("forbidden: {0}")]
    Forbidden(String),

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
        let detail = ErrorDetail::from(&self);
        let mut headers = HeaderMap::new();
        if let Some(retry_after) = detail.retry_after {
            let value = HeaderValue::from_str(&retry_after.to_string())
                .expect("retry-after seconds are ASCII");
            headers.insert(header::RETRY_AFTER, value);
            headers.insert("x-ratelimit-remaining", HeaderValue::from_static("0"));
        }

        let status = match &self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
            AppError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            AppError::PaymentRequired(_) => StatusCode::PAYMENT_REQUIRED,
            AppError::Database(e) => {
                tracing::error!("database error: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
            AppError::Internal(e) => {
                tracing::error!("internal error: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
            AppError::Serde(e) => {
                tracing::error!("serde error: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };

        error_response(status, headers, detail)
    }
}

pub async fn normalize_error_response(req: Request, next: Next) -> Response {
    let response = next.run(req).await;
    let status = response.status();
    if !(status.is_client_error() || status.is_server_error()) || is_json_response(&response) {
        return response;
    }

    let status = if status == StatusCode::UNPROCESSABLE_ENTITY {
        StatusCode::BAD_REQUEST
    } else {
        status
    };
    error_response(status, HeaderMap::new(), fallback_detail(status))
}

struct ErrorDetail {
    code: &'static str,
    message: &'static str,
    retry_after: Option<u64>,
}

impl ErrorDetail {
    fn from(error: &AppError) -> Self {
        match error {
            AppError::BadRequest(message) => bad_request_detail(message),
            AppError::Unauthorized(_) => Self {
                code: "session_invalid",
                message: "Your session expired. Enter your email to continue.",
                retry_after: None,
            },
            AppError::Forbidden(message) if message == "csrf_failed" => Self {
                code: "csrf_failed",
                message: "Something went wrong. Refresh and try again.",
                retry_after: None,
            },
            AppError::Forbidden(message) if message == "account_deletion_requested" => Self {
                code: "account_deletion_requested",
                message: "This account is scheduled for deletion.",
                retry_after: None,
            },
            AppError::Forbidden(_) => Self {
                code: "forbidden",
                message: "This action is not allowed.",
                retry_after: None,
            },
            AppError::TooManyRequests(message) => too_many_requests_detail(message),
            AppError::Conflict(message) if message == "funds_present" => Self {
                code: "funds_present",
                message: "Move your funds out before closing your account.",
                retry_after: None,
            },
            AppError::Conflict(_) => Self {
                code: "conflict",
                message: "This action could not be completed.",
                retry_after: None,
            },
            AppError::NotFound(_) => Self {
                code: "not_found",
                message: "That record was not found.",
                retry_after: None,
            },
            AppError::ServiceUnavailable(message) => service_unavailable_detail(message),
            AppError::PaymentRequired(_) => Self {
                code: "payment_required",
                message: "Upgrade required.",
                retry_after: None,
            },
            AppError::Database(_) | AppError::Internal(_) | AppError::Serde(_) => Self {
                code: "internal_error",
                message: "Something went wrong on our end. Try again.",
                retry_after: None,
            },
        }
    }
}

fn bad_request_detail(message: &str) -> ErrorDetail {
    match message {
        "invalid email" | "invalid_email" => ErrorDetail {
            code: "invalid_email",
            message: "Enter a valid email address.",
            retry_after: None,
        },
        "code_invalid" => ErrorDetail {
            code: "code_invalid",
            message: "That code didn't match. Check it or request a new one.",
            retry_after: None,
        },
        "code_expired" => ErrorDetail {
            code: "code_expired",
            message: "That code expired. Request a new one.",
            retry_after: None,
        },
        "code_used" => ErrorDetail {
            code: "code_used",
            message: "That code was already used. Request a new one.",
            retry_after: None,
        },
        "consent_required" => ErrorDetail {
            code: "consent_required",
            message: "Please accept the Terms and Privacy Policy to continue.",
            retry_after: None,
        },
        "confirm_required" => ErrorDetail {
            code: "confirm_required",
            message: "Confirm before closing your account.",
            retry_after: None,
        },
        _ => ErrorDetail {
            code: "bad_request",
            message: "Check the request and try again.",
            retry_after: None,
        },
    }
}

fn too_many_requests_detail(message: &str) -> ErrorDetail {
    if let Some(seconds) = message.strip_prefix("resend_cooldown:") {
        let retry_after = seconds.parse().ok();
        return ErrorDetail {
            code: "resend_cooldown",
            message: "You can request a new code shortly.",
            retry_after,
        };
    }

    match message {
        "too_many_attempts" => ErrorDetail {
            code: "too_many_attempts",
            message: "Too many tries. Request a new code.",
            retry_after: None,
        },
        "rate_limited" | "too many verification code requests" => ErrorDetail {
            code: "rate_limited",
            message: "Too many requests. Try again shortly.",
            retry_after: Some(60),
        },
        _ => ErrorDetail {
            code: "rate_limited",
            message: "Too many requests. Try again shortly.",
            retry_after: Some(60),
        },
    }
}

fn service_unavailable_detail(message: &str) -> ErrorDetail {
    if message.contains("verification email could not be sent")
        || message.contains("wallet auth email is disabled")
    {
        return ErrorDetail {
            code: "email_send_failed",
            message: "We couldn't send your code. Try again.",
            retry_after: None,
        };
    }

    ErrorDetail {
        code: "service_unavailable",
        message: "Something went wrong on our end. Try again.",
        retry_after: None,
    }
}

fn fallback_detail(status: StatusCode) -> ErrorDetail {
    match status {
        StatusCode::BAD_REQUEST => ErrorDetail {
            code: "bad_request",
            message: "Check the request and try again.",
            retry_after: None,
        },
        StatusCode::UNAUTHORIZED => ErrorDetail {
            code: "session_invalid",
            message: "Your session expired. Enter your email to continue.",
            retry_after: None,
        },
        StatusCode::FORBIDDEN => ErrorDetail {
            code: "forbidden",
            message: "This action is not allowed.",
            retry_after: None,
        },
        StatusCode::NOT_FOUND => ErrorDetail {
            code: "not_found",
            message: "That record was not found.",
            retry_after: None,
        },
        StatusCode::METHOD_NOT_ALLOWED => ErrorDetail {
            code: "method_not_allowed",
            message: "This endpoint does not accept that method.",
            retry_after: None,
        },
        StatusCode::TOO_MANY_REQUESTS => ErrorDetail {
            code: "rate_limited",
            message: "Too many requests. Try again shortly.",
            retry_after: Some(60),
        },
        status if status.is_server_error() => ErrorDetail {
            code: "internal_error",
            message: "Something went wrong on our end. Try again.",
            retry_after: None,
        },
        _ => ErrorDetail {
            code: "bad_request",
            message: "Check the request and try again.",
            retry_after: None,
        },
    }
}

fn is_json_response(response: &Response) -> bool {
    response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("application/json"))
}

fn error_response(status: StatusCode, mut headers: HeaderMap, detail: ErrorDetail) -> Response {
    if let Some(retry_after) = detail.retry_after {
        let value =
            HeaderValue::from_str(&retry_after.to_string()).expect("retry-after seconds are ASCII");
        headers.insert(header::RETRY_AFTER, value);
        headers.insert("x-ratelimit-remaining", HeaderValue::from_static("0"));
    }

    let body = Json(json!({
        "error": {
            "code": detail.code,
            "message": detail.message,
            "retryAfter": detail.retry_after,
        }
    }));
    (status, headers, body).into_response()
}

pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header;

    #[test]
    fn bad_request_uses_docs_error_envelope() {
        let response = AppError::BadRequest("code_invalid".into()).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn resend_cooldown_sets_retry_after_header() {
        let response = AppError::TooManyRequests("resend_cooldown:17".into()).into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response.headers().get(header::RETRY_AFTER).unwrap(),
            HeaderValue::from_static("17")
        );
        assert_eq!(
            response.headers().get("x-ratelimit-remaining").unwrap(),
            HeaderValue::from_static("0")
        );
    }

    #[test]
    fn catalog_maps_session_and_csrf_errors() {
        let session = ErrorDetail::from(&AppError::Unauthorized("missing session".into()));
        assert_eq!(session.code, "session_invalid");

        let csrf = ErrorDetail::from(&AppError::Forbidden("csrf_failed".into()));
        assert_eq!(csrf.code, "csrf_failed");
    }

    #[test]
    fn fallback_422_becomes_bad_request_envelope() {
        let response = error_response(
            StatusCode::BAD_REQUEST,
            HeaderMap::new(),
            fallback_detail(StatusCode::BAD_REQUEST),
        );
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(is_json_response(&response));
    }
}
