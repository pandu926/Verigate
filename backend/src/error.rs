use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Application error type with appropriate HTTP status code mapping.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Terminal 3 auth error: {0}")]
    T3Auth(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Invalid state transition from {current_state}")]
    InvalidTransition {
        current_state: String,
        allowed: Vec<String>,
    },

    #[error("AI service error: {0}")]
    AiError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "database error"),
            AppError::Config(_) => (StatusCode::INTERNAL_SERVER_ERROR, "configuration error"),
            AppError::NotFound(msg) => return not_found_response(msg),
            AppError::Validation(msg) => return validation_response(msg),
            AppError::T3Auth(_) => (StatusCode::SERVICE_UNAVAILABLE, "authentication service error"),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
            AppError::Unauthorized(msg) => return auth_error_response(StatusCode::UNAUTHORIZED, msg),
            AppError::Forbidden(msg) => return auth_error_response(StatusCode::FORBIDDEN, msg),
            AppError::InvalidTransition { current_state, allowed } => {
                return invalid_transition_response(current_state, allowed);
            }
            AppError::AiError(_) => (StatusCode::BAD_GATEWAY, "AI service error"),
        };

        let body = json!({
            "error": message,
            "status": status.as_u16(),
        });

        (status, axum::Json(body)).into_response()
    }
}

fn not_found_response(msg: &str) -> Response {
    let body = json!({
        "error": msg,
        "status": 404,
    });
    (StatusCode::NOT_FOUND, axum::Json(body)).into_response()
}

fn validation_response(msg: &str) -> Response {
    let body = json!({
        "error": msg,
        "status": 422,
    });
    (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(body)).into_response()
}

fn invalid_transition_response(current_state: &str, allowed: &[String]) -> Response {
    let body = json!({
        "error": format!("Invalid state transition from '{current_state}'"),
        "data": {
            "current_state": current_state,
            "allowed_transitions": allowed,
        },
        "status": 409,
    });
    (StatusCode::CONFLICT, axum::Json(body)).into_response()
}

fn auth_error_response(status: StatusCode, msg: &str) -> Response {
    let body = json!({
        "data": null,
        "error": msg,
        "meta": { "status": status.as_u16() },
    });
    (status, axum::Json(body)).into_response()
}
