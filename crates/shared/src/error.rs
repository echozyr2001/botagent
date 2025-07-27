use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;
use validator::ValidationErrors;

use crate::types::api::ApiErrorResponse;

/// Main service error type that encompasses all possible errors
#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("AI service error: {0}")]
    AI(#[from] AIError),

    #[error("Automation error: {0}")]
    Automation(#[from] AutomationError),

    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    #[error("Authentication error: {0}")]
    Authentication(#[from] AuthenticationError),

    #[error("Authorization error: {0}")]
    Authorization(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    #[error("External service error: {0}")]
    ExternalService(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

/// Database-related errors
#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Query failed: {0}")]
    Query(String),

    #[error("Transaction failed: {0}")]
    Transaction(String),

    #[error("Migration failed: {0}")]
    Migration(String),

    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Pool error: {0}")]
    Pool(String),
}

/// AI service-related errors
#[derive(Debug, Error)]
pub enum AIError {
    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Rate limit exceeded for provider {provider}: {message}")]
    RateLimit { provider: String, message: String },

    #[error("Model not available: {model}")]
    ModelNotAvailable { model: String },

    #[error("Invalid model configuration: {0}")]
    InvalidModelConfig(String),

    #[error("Message format error: {0}")]
    MessageFormat(String),

    #[error("Token limit exceeded: {0}")]
    TokenLimit(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Provider unavailable: {provider}")]
    ProviderUnavailable { provider: String },
}

/// Desktop automation-related errors
#[derive(Debug, Error)]
pub enum AutomationError {
    #[error("Screen capture failed: {0}")]
    ScreenCapture(String),

    #[error("Mouse operation failed: {0}")]
    Mouse(String),

    #[error("Keyboard operation failed: {0}")]
    Keyboard(String),

    #[error("File operation failed: {0}")]
    FileOperation(String),

    #[error("Application switching failed: {0}")]
    ApplicationSwitch(String),

    #[error("Invalid coordinates: x={x}, y={y}")]
    InvalidCoordinates { x: i32, y: i32 },

    #[error("Invalid action parameters: {0}")]
    InvalidParameters(String),

    #[error("System permission denied: {0}")]
    PermissionDenied(String),

    #[error("Platform not supported: {0}")]
    PlatformNotSupported(String),

    #[error("Timeout during automation: {0}")]
    Timeout(String),

    #[error("Display not available: {0}")]
    DisplayNotAvailable(String),
}

/// Input validation errors
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Field validation failed: {0}")]
    Field(String),

    #[error("Multiple validation errors: {0}")]
    Multiple(String),

    #[error("JSON parsing error: {0}")]
    Json(String),

    #[error("UUID parsing error: {0}")]
    Uuid(String),

    #[error("Date parsing error: {0}")]
    Date(String),

    #[error("Required field missing: {field}")]
    Required { field: String },

    #[error("Invalid enum value: {value} for field {field}")]
    InvalidEnum { field: String, value: String },
}

/// Authentication and authorization errors
#[derive(Debug, Error)]
pub enum AuthenticationError {
    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Token expired")]
    TokenExpired,

    #[error("Invalid token: {0}")]
    InvalidToken(String),

    #[error("Missing token")]
    MissingToken,

    #[error("Session expired")]
    SessionExpired,

    #[error("User not found")]
    UserNotFound,

    #[error("Account locked")]
    AccountLocked,

    #[error("Password validation failed: {0}")]
    PasswordValidation(String),
}

// Conversion implementations for external error types

impl From<sqlx::Error> for ServiceError {
    fn from(err: sqlx::Error) -> Self {
        let db_error = match err {
            sqlx::Error::Database(db_err) => {
                if db_err.is_unique_violation() || db_err.is_foreign_key_violation() {
                    DatabaseError::ConstraintViolation(db_err.to_string())
                } else {
                    DatabaseError::Query(db_err.to_string())
                }
            }
            sqlx::Error::PoolTimedOut => {
                DatabaseError::Pool("Connection pool timed out".to_string())
            }
            sqlx::Error::PoolClosed => DatabaseError::Pool("Connection pool closed".to_string()),
            sqlx::Error::RowNotFound => {
                return ServiceError::NotFound("Resource not found".to_string())
            }
            _ => DatabaseError::Query(err.to_string()),
        };
        ServiceError::Database(db_error)
    }
}

impl From<reqwest::Error> for ServiceError {
    fn from(err: reqwest::Error) -> Self {
        let ai_error = if err.is_timeout() {
            AIError::Timeout(err.to_string())
        } else if err.is_connect() {
            AIError::Http(format!("Connection failed: {err}"))
        } else {
            AIError::Http(err.to_string())
        };
        ServiceError::AI(ai_error)
    }
}

impl From<serde_json::Error> for ServiceError {
    fn from(err: serde_json::Error) -> Self {
        ServiceError::Validation(ValidationError::Json(err.to_string()))
    }
}

impl From<uuid::Error> for ServiceError {
    fn from(err: uuid::Error) -> Self {
        ServiceError::Validation(ValidationError::Uuid(err.to_string()))
    }
}

impl From<ValidationErrors> for ServiceError {
    fn from(err: ValidationErrors) -> Self {
        let error_messages: Vec<String> = err
            .field_errors()
            .iter()
            .flat_map(|(field, errors)| {
                errors.iter().map(move |error| {
                    format!(
                        "{}: {}",
                        field,
                        error.message.as_ref().unwrap_or(&"Invalid value".into())
                    )
                })
            })
            .collect();

        ServiceError::Validation(ValidationError::Multiple(error_messages.join(", ")))
    }
}

impl From<jsonwebtoken::errors::Error> for ServiceError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        let auth_error = match err.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthenticationError::TokenExpired,
            jsonwebtoken::errors::ErrorKind::InvalidToken => {
                AuthenticationError::InvalidToken(err.to_string())
            }
            _ => AuthenticationError::InvalidToken(err.to_string()),
        };
        ServiceError::Authentication(auth_error)
    }
}

// HTTP response conversion
impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        let (status, error_message, details) = match &self {
            ServiceError::Validation(validation_err) => {
                let details = match validation_err {
                    ValidationError::Multiple(msg) => {
                        Some(json!({ "validation_errors": msg.split(", ").collect::<Vec<_>>() }))
                    }
                    ValidationError::Required { field } => Some(json!({ "missing_field": field })),
                    ValidationError::InvalidEnum { field, value } => {
                        Some(json!({ "field": field, "invalid_value": value }))
                    }
                    _ => None,
                };
                (StatusCode::BAD_REQUEST, self.to_string(), details)
            }
            ServiceError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone(), None),
            ServiceError::Authentication(_) => (StatusCode::UNAUTHORIZED, self.to_string(), None),
            ServiceError::Authorization(msg) => (StatusCode::FORBIDDEN, msg.clone(), None),
            ServiceError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone(), None),
            ServiceError::RateLimit(msg) => (StatusCode::TOO_MANY_REQUESTS, msg.clone(), None),
            ServiceError::AI(ai_err) => match ai_err {
                AIError::RateLimit { provider, message } => {
                    let details = Some(json!({ "provider": provider, "message": message }));
                    (StatusCode::TOO_MANY_REQUESTS, self.to_string(), details)
                }
                AIError::Authentication(_) => (StatusCode::UNAUTHORIZED, self.to_string(), None),
                AIError::ModelNotAvailable { model } => {
                    let details = Some(json!({ "model": model }));
                    (StatusCode::BAD_REQUEST, self.to_string(), details)
                }
                _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string(), None),
            },
            ServiceError::Automation(auto_err) => match auto_err {
                AutomationError::InvalidCoordinates { x, y } => {
                    let details = Some(json!({ "coordinates": { "x": x, "y": y } }));
                    (StatusCode::BAD_REQUEST, self.to_string(), details)
                }
                AutomationError::PermissionDenied(_) => {
                    (StatusCode::FORBIDDEN, self.to_string(), None)
                }
                AutomationError::PlatformNotSupported(_) => {
                    (StatusCode::NOT_IMPLEMENTED, self.to_string(), None)
                }
                _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string(), None),
            },
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
                None,
            ),
        };

        let error_response = if let Some(details) = details {
            ApiErrorResponse::with_details(error_message, details)
        } else {
            ApiErrorResponse::new(error_message)
        };

        (status, Json(error_response)).into_response()
    }
}

// Result type aliases for convenience
pub type ServiceResult<T> = Result<T, ServiceError>;
pub type DatabaseResult<T> = Result<T, DatabaseError>;
pub type AIResult<T> = Result<T, AIError>;
pub type AutomationResult<T> = Result<T, AutomationError>;
pub type ValidationResult<T> = Result<T, ValidationError>;
pub type AuthResult<T> = Result<T, AuthenticationError>;

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::*;

    #[test]
    fn test_service_error_display() {
        let error = ServiceError::NotFound("Task not found".to_string());
        assert_eq!(error.to_string(), "Not found: Task not found");
    }

    #[test]
    fn test_database_error_conversion() {
        let db_error = DatabaseError::Query("SELECT failed".to_string());
        let service_error = ServiceError::Database(db_error);
        assert!(matches!(service_error, ServiceError::Database(_)));
    }

    #[test]
    fn test_ai_error_conversion() {
        let ai_error = AIError::RateLimit {
            provider: "anthropic".to_string(),
            message: "Rate limit exceeded".to_string(),
        };
        let service_error = ServiceError::AI(ai_error);
        assert!(matches!(service_error, ServiceError::AI(_)));
    }

    #[test]
    fn test_automation_error_conversion() {
        let auto_error = AutomationError::InvalidCoordinates { x: -1, y: -1 };
        let service_error = ServiceError::Automation(auto_error);
        assert!(matches!(service_error, ServiceError::Automation(_)));
    }

    #[test]
    fn test_validation_error_conversion() {
        let validation_error = ValidationError::Required {
            field: "description".to_string(),
        };
        let service_error = ServiceError::Validation(validation_error);
        assert!(matches!(service_error, ServiceError::Validation(_)));
    }

    #[test]
    fn test_http_response_status_codes() {
        let test_cases = vec![
            (
                ServiceError::NotFound("test".to_string()),
                StatusCode::NOT_FOUND,
            ),
            (
                ServiceError::Authorization("test".to_string()),
                StatusCode::FORBIDDEN,
            ),
            (
                ServiceError::Conflict("test".to_string()),
                StatusCode::CONFLICT,
            ),
            (
                ServiceError::RateLimit("test".to_string()),
                StatusCode::TOO_MANY_REQUESTS,
            ),
        ];

        for (error, expected_status) in test_cases {
            let response = error.into_response();
            assert_eq!(response.status(), expected_status);
        }
    }

    #[test]
    fn test_validation_error_details() {
        let validation_error = ValidationError::Required {
            field: "description".to_string(),
        };
        let service_error = ServiceError::Validation(validation_error);
        let response = service_error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_ai_error_with_details() {
        let ai_error = AIError::RateLimit {
            provider: "anthropic".to_string(),
            message: "Rate limit exceeded".to_string(),
        };
        let service_error = ServiceError::AI(ai_error);
        let response = service_error.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_automation_error_with_coordinates() {
        let auto_error = AutomationError::InvalidCoordinates { x: -1, y: -1 };
        let service_error = ServiceError::Automation(auto_error);
        let response = service_error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
