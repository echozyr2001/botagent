use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

use super::task::{Role, TaskPriority, TaskStatus, TaskType};

/// File data transfer object for task creation
#[derive(Debug, Clone, Serialize, Deserialize, Validate, PartialEq)]
pub struct TaskFileDto {
    #[validate(length(min = 1, message = "File name cannot be empty"))]
    pub name: String,

    #[validate(length(min = 1, message = "File data cannot be empty"))]
    pub base64: String,

    #[validate(length(min = 1, message = "File type cannot be empty"))]
    pub r#type: String,

    #[validate(range(min = 1, message = "File size must be greater than 0"))]
    pub size: u64,
}

/// Data transfer object for creating a new task
#[derive(Debug, Clone, Serialize, Deserialize, Validate, PartialEq)]
pub struct CreateTaskDto {
    #[validate(length(min = 1, message = "Task description cannot be empty"))]
    pub description: String,

    #[serde(rename = "type")]
    pub task_type: Option<TaskType>,

    #[serde(rename = "scheduledFor")]
    pub scheduled_for: Option<DateTime<Utc>>,

    pub priority: Option<TaskPriority>,

    #[serde(rename = "createdBy")]
    pub created_by: Option<Role>,

    #[serde(rename = "userId")]
    pub user_id: Option<String>,

    /// AI model configuration as JSON
    pub model: Option<serde_json::Value>,

    #[validate]
    pub files: Option<Vec<TaskFileDto>>,
}

/// Data transfer object for updating an existing task
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateTaskDto {
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,

    #[serde(rename = "queuedAt")]
    pub queued_at: Option<DateTime<Utc>>,

    #[serde(rename = "executedAt")]
    pub executed_at: Option<DateTime<Utc>>,

    #[serde(rename = "completedAt")]
    pub completed_at: Option<DateTime<Utc>>,
}

/// Data transfer object for adding a message to a task
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct AddTaskMessageDto {
    #[validate(length(min = 1, message = "Message cannot be empty"))]
    pub message: String,
}

/// Response wrapper for API endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
    pub success: bool,
    pub timestamp: DateTime<Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            data,
            success: true,
            timestamp: Utc::now(),
        }
    }
}

/// Error response for API endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    pub error: String,
    pub success: bool,
    pub timestamp: DateTime<Utc>,
    pub details: Option<serde_json::Value>,
}

impl ApiErrorResponse {
    pub fn new(error: String) -> Self {
        Self {
            error,
            success: false,
            timestamp: Utc::now(),
            details: None,
        }
    }

    pub fn with_details(error: String, details: serde_json::Value) -> Self {
        Self {
            error,
            success: false,
            timestamp: Utc::now(),
            details: Some(details),
        }
    }
}

/// Pagination parameters for list endpoints
#[derive(Debug, Clone, Serialize, Deserialize, Validate, PartialEq)]
pub struct PaginationParams {
    #[validate(range(min = 1, message = "Page must be greater than 0"))]
    pub page: Option<u32>,

    #[validate(range(min = 1, max = 100, message = "Limit must be between 1 and 100"))]
    pub limit: Option<u32>,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: Some(1),
            limit: Some(20),
        }
    }
}

/// Paginated response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub pagination: PaginationInfo,
    pub success: bool,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationInfo {
    pub page: u32,
    pub limit: u32,
    pub total: u64,
    pub pages: u32,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, page: u32, limit: u32, total: u64) -> Self {
        let pages = ((total as f64) / (limit as f64)).ceil() as u32;

        Self {
            data,
            pagination: PaginationInfo {
                page,
                limit,
                total,
                pages,
            },
            success: true,
            timestamp: Utc::now(),
        }
    }
}

/// Task control operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskControlDto {
    pub action: TaskControlAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskControlAction {
    Takeover,
    Resume,
    Cancel,
}

/// Model information for AI service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub provider: String,
    pub name: String,
    pub title: String,
    pub available: bool,
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub version: String,
    pub services: Vec<ServiceHealth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealth {
    pub name: String,
    pub status: String,
    pub details: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use validator::Validate;

    use super::*;

    #[test]
    fn test_create_task_dto_validation() {
        let valid_dto = CreateTaskDto {
            description: "Test task".to_string(),
            task_type: Some(TaskType::Immediate),
            scheduled_for: None,
            priority: Some(TaskPriority::Medium),
            created_by: Some(Role::User),
            user_id: None,
            model: None,
            files: None,
        };

        assert!(valid_dto.validate().is_ok());

        let invalid_dto = CreateTaskDto {
            description: "".to_string(), // Empty description should fail
            task_type: None,
            scheduled_for: None,
            priority: None,
            created_by: None,
            user_id: None,
            model: None,
            files: None,
        };

        assert!(invalid_dto.validate().is_err());
    }

    #[test]
    fn test_task_file_dto_validation() {
        let valid_file = TaskFileDto {
            name: "test.txt".to_string(),
            base64: "dGVzdA==".to_string(),
            r#type: "text/plain".to_string(),
            size: 100,
        };

        assert!(valid_file.validate().is_ok());

        let invalid_file = TaskFileDto {
            name: "".to_string(), // Empty name should fail
            base64: "dGVzdA==".to_string(),
            r#type: "text/plain".to_string(),
            size: 0, // Zero size should fail
        };

        assert!(invalid_file.validate().is_err());
    }

    #[test]
    fn test_pagination_params_validation() {
        let valid_params = PaginationParams {
            page: Some(1),
            limit: Some(20),
        };

        assert!(valid_params.validate().is_ok());

        let invalid_params = PaginationParams {
            page: Some(0),    // Page 0 should fail
            limit: Some(200), // Limit > 100 should fail
        };

        assert!(invalid_params.validate().is_err());
    }

    #[test]
    fn test_api_response_creation() {
        let response = ApiResponse::success("test data");
        assert!(response.success);
        assert_eq!(response.data, "test data");
    }

    #[test]
    fn test_api_error_response_creation() {
        let error_response = ApiErrorResponse::new("Test error".to_string());
        assert!(!error_response.success);
        assert_eq!(error_response.error, "Test error");
        assert!(error_response.details.is_none());

        let error_with_details = ApiErrorResponse::with_details(
            "Test error".to_string(),
            serde_json::json!({"field": "value"}),
        );
        assert!(error_with_details.details.is_some());
    }
}
