use bytebot_shared_rs::{
    error::ServiceError,
    types::{
        api::{AddTaskMessageDto, CreateTaskDto, TaskFileDto, UpdateTaskDto},
        task::{Role, TaskPriority, TaskStatus, TaskType},
    },
    utils::validation::{validate_email, validate_file_path, validate_uuid, validate_with_custom},
};
use chrono::Utc;
use serde_json::json;
use validator::Validate;

#[test]
fn test_create_task_dto_integration() {
    // Test valid DTO
    let valid_dto = CreateTaskDto {
        description: "Test task description".to_string(),
        task_type: Some(TaskType::Immediate),
        scheduled_for: None,
        priority: Some(TaskPriority::High),
        created_by: Some(Role::User),
        user_id: Some("user-123".to_string()),
        model: Some(json!({
            "provider": "anthropic",
            "name": "claude-3-opus",
            "title": "Claude 3 Opus"
        })),
        files: None,
    };

    assert!(valid_dto.validate().is_ok());
    assert!(validate_with_custom(&valid_dto).is_ok());

    // Test invalid DTO - empty description
    let invalid_dto = CreateTaskDto {
        description: "".to_string(),
        task_type: Some(TaskType::Immediate),
        scheduled_for: None,
        priority: Some(TaskPriority::Medium),
        created_by: Some(Role::User),
        user_id: None,
        model: None,
        files: None,
    };

    assert!(invalid_dto.validate().is_err());

    // Test scheduled task without scheduled_for
    let scheduled_invalid = CreateTaskDto {
        description: "Scheduled task".to_string(),
        task_type: Some(TaskType::Scheduled),
        scheduled_for: None, // Missing required field
        priority: Some(TaskPriority::Medium),
        created_by: Some(Role::User),
        user_id: None,
        model: Some(json!({
            "provider": "anthropic",
            "name": "claude-3"
        })),
        files: None,
    };

    assert!(validate_with_custom(&scheduled_invalid).is_err());
}

#[test]
fn test_update_task_dto_integration() {
    // Test valid update
    let valid_update = UpdateTaskDto {
        status: Some(TaskStatus::Running),
        priority: Some(TaskPriority::High),
        queued_at: Some(Utc::now()),
        executed_at: Some(Utc::now()),
        completed_at: None,
    };

    assert!(valid_update.validate().is_ok());
    assert!(validate_with_custom(&valid_update).is_ok());

    // Test empty update (should fail custom validation)
    let empty_update = UpdateTaskDto {
        status: None,
        priority: None,
        queued_at: None,
        executed_at: None,
        completed_at: None,
    };

    assert!(validate_with_custom(&empty_update).is_err());
}

#[test]
fn test_add_task_message_dto_integration() {
    // Test valid message
    let valid_message = AddTaskMessageDto {
        message: "Hello, this is a test message".to_string(),
    };

    assert!(valid_message.validate().is_ok());
    assert!(validate_with_custom(&valid_message).is_ok());

    // Test empty message
    let empty_message = AddTaskMessageDto {
        message: "".to_string(),
    };

    assert!(empty_message.validate().is_err());

    // Test very long message
    let long_message = AddTaskMessageDto {
        message: "x".repeat(100001),
    };

    assert!(validate_with_custom(&long_message).is_err());
}

#[test]
fn test_task_file_dto_integration() {
    // Test valid file
    let valid_file = TaskFileDto {
        name: "test.txt".to_string(),
        base64: "SGVsbG8gV29ybGQ=".to_string(), // "Hello World" in base64
        r#type: "text/plain".to_string(),
        size: 11,
    };

    assert!(valid_file.validate().is_ok());

    // Test invalid file - empty name
    let invalid_file = TaskFileDto {
        name: "".to_string(),
        base64: "SGVsbG8gV29ybGQ=".to_string(),
        r#type: "text/plain".to_string(),
        size: 11,
    };

    assert!(invalid_file.validate().is_err());

    // Test invalid file - zero size
    let zero_size_file = TaskFileDto {
        name: "test.txt".to_string(),
        base64: "SGVsbG8gV29ybGQ=".to_string(),
        r#type: "text/plain".to_string(),
        size: 0,
    };

    assert!(zero_size_file.validate().is_err());
}

#[test]
fn test_validation_utilities_integration() {
    // Test email validation
    assert!(validate_email("test@example.com").is_ok());
    assert!(validate_email("user.name+tag@domain.co.uk").is_ok());
    assert!(validate_email("invalid-email").is_err());
    assert!(validate_email("@domain.com").is_err());

    // Test UUID validation
    assert!(validate_uuid("550e8400-e29b-41d4-a716-446655440000").is_ok());
    assert!(validate_uuid("invalid-uuid").is_err());
    assert!(validate_uuid("").is_err());

    // Test file path validation
    assert!(validate_file_path("/valid/path/file.txt").is_ok());
    assert!(validate_file_path("relative/path.txt").is_ok());
    assert!(validate_file_path("../../../etc/passwd").is_err());
    assert!(validate_file_path("~/secret").is_err());
    assert!(validate_file_path("").is_err());
}

#[test]
fn test_service_error_conversion() {
    // Test validation error conversion
    let validation_errors = validator::ValidationErrors::new();
    let service_error = ServiceError::from(validation_errors);
    assert!(matches!(service_error, ServiceError::Validation(_)));

    // Test JSON error conversion
    let json_error = serde_json::from_str::<serde_json::Value>("invalid json");
    assert!(json_error.is_err());
    let service_error = ServiceError::from(json_error.unwrap_err());
    assert!(matches!(service_error, ServiceError::Validation(_)));

    // Test UUID error conversion
    let uuid_error = uuid::Uuid::parse_str("invalid-uuid");
    assert!(uuid_error.is_err());
    let service_error = ServiceError::from(uuid_error.unwrap_err());
    assert!(matches!(service_error, ServiceError::Validation(_)));
}

#[test]
fn test_dto_serialization_deserialization() {
    // Test CreateTaskDto serialization/deserialization
    let dto = CreateTaskDto {
        description: "Test task".to_string(),
        task_type: Some(TaskType::Immediate),
        scheduled_for: None,
        priority: Some(TaskPriority::High),
        created_by: Some(Role::User),
        user_id: Some("user-123".to_string()),
        model: Some(json!({
            "provider": "anthropic",
            "name": "claude-3"
        })),
        files: None,
    };

    let json_str = serde_json::to_string(&dto).unwrap();
    let deserialized: CreateTaskDto = serde_json::from_str(&json_str).unwrap();

    assert_eq!(dto.description, deserialized.description);
    assert_eq!(dto.task_type, deserialized.task_type);
    assert_eq!(dto.priority, deserialized.priority);
    assert_eq!(dto.created_by, deserialized.created_by);
    assert_eq!(dto.user_id, deserialized.user_id);

    // Test UpdateTaskDto serialization/deserialization
    let update_dto = UpdateTaskDto {
        status: Some(TaskStatus::Running),
        priority: Some(TaskPriority::High),
        queued_at: Some(Utc::now()),
        executed_at: None,
        completed_at: None,
    };

    let json_str = serde_json::to_string(&update_dto).unwrap();
    let deserialized: UpdateTaskDto = serde_json::from_str(&json_str).unwrap();

    assert_eq!(update_dto.status, deserialized.status);
    assert_eq!(update_dto.priority, deserialized.priority);

    // Test AddTaskMessageDto serialization/deserialization
    let message_dto = AddTaskMessageDto {
        message: "Test message".to_string(),
    };

    let json_str = serde_json::to_string(&message_dto).unwrap();
    let deserialized: AddTaskMessageDto = serde_json::from_str(&json_str).unwrap();

    assert_eq!(message_dto.message, deserialized.message);
}

#[test]
fn test_api_response_structures() {
    use bytebot_shared_rs::types::api::{ApiErrorResponse, ApiResponse, PaginatedResponse};

    // Test ApiResponse
    let response = ApiResponse::success("test data");
    assert!(response.success);
    assert_eq!(response.data, "test data");

    // Test ApiErrorResponse
    let error_response = ApiErrorResponse::new("Test error".to_string());
    assert!(!error_response.success);
    assert_eq!(error_response.error, "Test error");
    assert!(error_response.details.is_none());

    let error_with_details =
        ApiErrorResponse::with_details("Test error".to_string(), json!({"field": "value"}));
    assert!(error_with_details.details.is_some());

    // Test PaginatedResponse
    let data = vec!["item1", "item2", "item3"];
    let paginated = PaginatedResponse::new(data.clone(), 1, 10, 3);
    assert_eq!(paginated.data, data);
    assert_eq!(paginated.pagination.page, 1);
    assert_eq!(paginated.pagination.limit, 10);
    assert_eq!(paginated.pagination.total, 3);
    assert_eq!(paginated.pagination.pages, 1);
    assert!(paginated.success);
}
