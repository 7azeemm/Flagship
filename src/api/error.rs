use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::fmt;

#[derive(Debug)]
pub enum ApiError {
    DatabaseError(String),
    NotFound(String),
    InvalidQuery(String),
    InternalError(String),
    ServiceUnavailable(String),
    Unauthorized(String),
    Forbidden(String),
    Conflict(String),
    RateLimited,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            ApiError::NotFound(msg) => write!(f, "Not found: {}", msg),
            ApiError::InvalidQuery(msg) => write!(f, "Invalid query: {}", msg),
            ApiError::InternalError(msg) => write!(f, "Internal error: {}", msg),
            ApiError::ServiceUnavailable(msg) => write!(f, "Service unavailable: {}", msg),
            ApiError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            ApiError::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
            ApiError::Conflict(msg) => write!(f, "Conflict: {}", msg),
            ApiError::RateLimited => write!(f, "Rate limited"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_code, message) = match &self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg.clone()),
            ApiError::InvalidQuery(msg) => (StatusCode::BAD_REQUEST, "INVALID_REQUEST", msg.clone()),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg.clone()),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg.clone()),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, "CONFLICT", msg.clone()),
            ApiError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED", "Too many attempts. Please try again later.".to_string()),
            ApiError::DatabaseError(msg) => {
                tracing::error!("Database error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR", "Database operation failed".to_string())
            }
            ApiError::InternalError(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "Internal server error".to_string())
            }
            ApiError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, "SERVICE_UNAVAILABLE", msg.clone()),
        };

        error_response(status, error_code, &message)
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

pub(super) fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(json!({"error": {"code": code, "message": message, "status": status.as_u16()}}))).into_response()
}

// Axum rejects malformed queries, JSON and unsupported methods before calling
// handlers. Keep those errors readable by the same clients as handler errors.
pub async fn json_errors(request: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let response = next.run(request).await;
    let status = response.status();
    if !status.is_client_error() && !status.is_server_error() {
        return response;
    }
    
    if response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .is_some_and(|value| value.to_str().is_ok_and(|value| value.split(';').next() == Some("application/json")))
    {
        return response;
    }
    
    let (mut parts, body) = response.into_parts();
    let message = axum::body::to_bytes(body, 16 * 1024)
        .await
        .ok()
        .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok())
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| status.canonical_reason().unwrap_or("Request failed").into());
    
    let code = match status {
        StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => "INVALID_REQUEST",
        StatusCode::NOT_FOUND => "NOT_FOUND",
        StatusCode::METHOD_NOT_ALLOWED => "METHOD_NOT_ALLOWED",
        StatusCode::PAYLOAD_TOO_LARGE => "PAYLOAD_TOO_LARGE",
        StatusCode::UNSUPPORTED_MEDIA_TYPE => "UNSUPPORTED_MEDIA_TYPE",
        _ => "HTTP_ERROR",
    };
    
    let (_, body) = error_response(status, code, &message).into_parts();
    parts.headers.remove(axum::http::header::CONTENT_LENGTH);
    parts.headers.insert(axum::http::header::CONTENT_TYPE, axum::http::HeaderValue::from_static("application/json"));
    Response::from_parts(parts, body)
}
