use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("Database error: {0}")]
    Database(#[from] crate::database::DatabaseError),

    #[error("Configuration error: {0}")]
    Config(#[from] crate::config::ConfigError),

    #[error("AI service error: {0}")]
    AI(#[from] AIError),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Internal server error: {0}")]
    Internal(String),
}

#[derive(Debug, thiserror::Error)]
pub enum AIError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Invalid model: {0}")]
    InvalidModel(String),
}

#[derive(Debug, thiserror::Error)]
pub enum AutomationError {
    #[error("Computer action failed: {0}")]
    ActionFailed(String),

    #[error("Invalid coordinates: {0}")]
    InvalidCoordinates(String),

    #[error("File operation failed: {0}")]
    FileOperation(String),

    #[error("Screen capture failed: {0}")]
    ScreenCapture(String),
}

// Convert ServiceError to HTTP responses
impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ServiceError::Validation(msg) => (StatusCode::BAD_REQUEST, msg),
            ServiceError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ServiceError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            ServiceError::Database(e) => {
                tracing::error!("Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            ServiceError::Config(e) => {
                tracing::error!("Configuration error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Configuration error".to_string(),
                )
            }
            ServiceError::AI(e) => {
                tracing::error!("AI service error: {}", e);
                match e {
                    AIError::RateLimit => (
                        StatusCode::TOO_MANY_REQUESTS,
                        "Rate limit exceeded".to_string(),
                    ),
                    AIError::InvalidModel(msg) => (StatusCode::BAD_REQUEST, msg),
                    _ => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "AI service error".to_string(),
                    ),
                }
            }
            ServiceError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        let body = Json(json!({
            "error": error_message
        }));

        (status, body).into_response()
    }
}

// Result type alias for convenience
pub type ServiceResult<T> = Result<T, ServiceError>;

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::*;

    #[test]
    fn test_service_error_response_conversion() {
        let error = ServiceError::Validation("Invalid input".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_not_found_error() {
        let error = ServiceError::NotFound("Task not found".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_unauthorized_error() {
        let error = ServiceError::Unauthorized;
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
