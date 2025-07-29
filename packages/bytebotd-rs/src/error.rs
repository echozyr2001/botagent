use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AutomationError {
    #[error("Screenshot capture failed: {0}")]
    ScreenshotFailed(String),

    #[error("Input simulation failed: {0}")]
    InputFailed(String),

    #[error("File operation failed: {0}")]
    FileFailed(String),

    #[error("Application switching failed: {0}")]
    ApplicationFailed(String),

    #[error("Invalid coordinates: x={x}, y={y}")]
    InvalidCoordinates { x: i32, y: i32 },

    #[error("Invalid file path: {0}")]
    InvalidPath(String),

    #[error("File too large: {size} MB exceeds limit of {limit} MB")]
    FileTooLarge { size: u64, limit: u64 },

    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),

    #[error("System error: {0}")]
    System(String),

    #[error("Validation error: {0}")]
    Validation(String),
}

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Automation error: {0}")]
    Automation(#[from] AutomationError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),

    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            ServiceError::Automation(AutomationError::Validation(msg)) => {
                (StatusCode::BAD_REQUEST, msg.clone())
            }
            ServiceError::Automation(AutomationError::InvalidCoordinates { x, y }) => (
                StatusCode::BAD_REQUEST,
                format!("Invalid coordinates: x={x}, y={y}"),
            ),
            ServiceError::Automation(AutomationError::InvalidPath(path)) => (
                StatusCode::BAD_REQUEST,
                format!("Invalid file path: {path}"),
            ),
            ServiceError::Automation(AutomationError::FileTooLarge { size, limit }) => (
                StatusCode::BAD_REQUEST,
                format!("File too large: {size} MB exceeds limit of {limit} MB"),
            ),
            ServiceError::Automation(AutomationError::UnsupportedOperation(op)) => (
                StatusCode::BAD_REQUEST,
                format!("Unsupported operation: {op}"),
            ),
            ServiceError::Serialization(_) => (
                StatusCode::BAD_REQUEST,
                "Invalid request format".to_string(),
            ),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
        };

        let body = Json(json!({
            "error": error_message,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }));

        (status, body).into_response()
    }
}

impl IntoResponse for AutomationError {
    fn into_response(self) -> Response {
        ServiceError::Automation(self).into_response()
    }
}
