use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;
use tracing::{debug, error, warn};

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
        let (status, error_message, error_code) = match &self {
            ServiceError::Automation(AutomationError::Validation(msg)) => {
                (StatusCode::BAD_REQUEST, msg.clone(), "VALIDATION_ERROR")
            }
            ServiceError::Automation(AutomationError::InvalidCoordinates { x, y }) => (
                StatusCode::BAD_REQUEST,
                format!("Invalid coordinates: x={x}, y={y}"),
                "INVALID_COORDINATES"
            ),
            ServiceError::Automation(AutomationError::InvalidPath(path)) => (
                StatusCode::BAD_REQUEST,
                format!("Invalid file path: {path}"),
                "INVALID_PATH"
            ),
            ServiceError::Automation(AutomationError::FileTooLarge { size, limit }) => (
                StatusCode::BAD_REQUEST,
                format!("File too large: {size} MB exceeds limit of {limit} MB"),
                "FILE_TOO_LARGE"
            ),
            ServiceError::Automation(AutomationError::UnsupportedOperation(op)) => (
                StatusCode::BAD_REQUEST,
                format!("Unsupported operation: {op}"),
                "UNSUPPORTED_OPERATION"
            ),
            ServiceError::Automation(AutomationError::ScreenshotFailed(msg)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Screenshot failed: {msg}"),
                "SCREENSHOT_FAILED"
            ),
            ServiceError::Automation(AutomationError::InputFailed(msg)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Input simulation failed: {msg}"),
                "INPUT_FAILED"
            ),
            ServiceError::Automation(AutomationError::FileFailed(msg)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("File operation failed: {msg}"),
                "FILE_OPERATION_FAILED"
            ),
            ServiceError::Automation(AutomationError::ApplicationFailed(msg)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Application switching failed: {msg}"),
                "APPLICATION_FAILED"
            ),
            ServiceError::Automation(AutomationError::System(msg)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("System error: {msg}"),
                "SYSTEM_ERROR"
            ),
            ServiceError::Serialization(e) => (
                StatusCode::BAD_REQUEST,
                format!("Invalid request format: {e}"),
                "SERIALIZATION_ERROR"
            ),
            ServiceError::Io(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("IO error: {e}"),
                "IO_ERROR"
            ),
            ServiceError::Image(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Image processing error: {e}"),
                "IMAGE_ERROR"
            ),
            ServiceError::Base64(e) => (
                StatusCode::BAD_REQUEST,
                format!("Base64 decode error: {e}"),
                "BASE64_ERROR"
            ),
            ServiceError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Internal server error: {msg}"),
                "INTERNAL_ERROR"
            ),
        };

        // Log error details for debugging
        match status {
            StatusCode::INTERNAL_SERVER_ERROR => {
                error!("Internal server error [{}]: {}", error_code, error_message);
            }
            StatusCode::BAD_REQUEST => {
                warn!("Bad request [{}]: {}", error_code, error_message);
            }
            _ => {
                debug!("Request error [{}]: {}", error_code, error_message);
            }
        }

        let body = Json(json!({
            "success": false,
            "error": {
                "message": error_message,
                "code": error_code,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }
        }));

        (status, body).into_response()
    }
}

impl IntoResponse for AutomationError {
    fn into_response(self) -> Response {
        ServiceError::Automation(self).into_response()
    }
}
