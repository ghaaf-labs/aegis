use axum::{
    extract::Request,
    http::{HeaderMap, Method},
    middleware::Next,
    response::Response,
};

use crate::error::AppError;

pub async fn require_request_header(req: Request, next: Next) -> Result<Response, AppError> {
    if is_state_changing(req.method()) && !has_csrf_header(req.headers()) {
        return Err(AppError::Forbidden("csrf_failed".into()));
    }
    Ok(next.run(req).await)
}

fn is_state_changing(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn has_csrf_header(headers: &HeaderMap) -> bool {
    headers
        .get("x-aegis-request")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == "1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn state_changing_methods_need_custom_header() {
        let headers = HeaderMap::new();
        assert!(is_state_changing(&Method::POST));
        assert!(!has_csrf_header(&headers));
    }

    #[test]
    fn custom_header_accepts_only_expected_value() {
        let mut headers = HeaderMap::new();
        headers.insert("x-aegis-request", HeaderValue::from_static("1"));
        assert!(has_csrf_header(&headers));

        headers.insert("x-aegis-request", HeaderValue::from_static("true"));
        assert!(!has_csrf_header(&headers));
    }
}
