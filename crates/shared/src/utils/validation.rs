use regex::Regex;
use validator::Validate;

use crate::types::{
    api::{AddTaskMessageDto, CreateTaskDto, UpdateTaskDto},
    message::{Message, MessageContentBlock},
    task::{Task, TaskType},
    user::File,
};

/// Custom validation error type
#[derive(Debug, thiserror::Error)]
pub enum ValidationErrorType {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[error("Value out of range: {0}")]
    OutOfRange(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),
}

/// Validation result type
pub type ValidationResult<T> = Result<T, ValidationErrorType>;

/// Trait for custom validation logic
pub trait CustomValidate {
    fn custom_validate(&self) -> ValidationResult<()>;
}

impl CustomValidate for CreateTaskDto {
    fn custom_validate(&self) -> ValidationResult<()> {
        // Validate model structure if provided
        if let Some(model) = &self.model {
            if !model.is_object() {
                return Err(ValidationErrorType::InvalidFormat(
                    "Model must be a JSON object".to_string(),
                ));
            }

            let model_obj = model.as_object().unwrap();
            if !model_obj.contains_key("provider") {
                return Err(ValidationErrorType::MissingField("provider".to_string()));
            }
            if !model_obj.contains_key("name") {
                return Err(ValidationErrorType::MissingField("name".to_string()));
            }
        }

        // Validate scheduled task requirements
        if let Some(task_type) = &self.task_type {
            if *task_type == TaskType::Scheduled && self.scheduled_for.is_none() {
                return Err(ValidationErrorType::MissingField(
                    "scheduled_for is required for scheduled tasks".to_string(),
                ));
            }
        }

        Ok(())
    }
}

impl CustomValidate for UpdateTaskDto {
    fn custom_validate(&self) -> ValidationResult<()> {
        // Validate that at least one field is being updated
        if self.status.is_none()
            && self.priority.is_none()
            && self.queued_at.is_none()
            && self.executed_at.is_none()
            && self.completed_at.is_none()
        {
            return Err(ValidationErrorType::InvalidInput(
                "At least one field must be provided for update".to_string(),
            ));
        }

        Ok(())
    }
}

impl CustomValidate for AddTaskMessageDto {
    fn custom_validate(&self) -> ValidationResult<()> {
        // Validate message is not empty (already handled by validator, but double-check)
        if self.message.trim().is_empty() {
            return Err(ValidationErrorType::InvalidInput(
                "Message cannot be empty".to_string(),
            ));
        }

        // Validate message length
        if self.message.len() > 100000 {
            return Err(ValidationErrorType::OutOfRange(
                "Message too long (max 100,000 characters)".to_string(),
            ));
        }

        Ok(())
    }
}

impl CustomValidate for Task {
    fn custom_validate(&self) -> ValidationResult<()> {
        // Use the existing validate_integrity method
        self.validate_integrity()
            .map_err(ValidationErrorType::ValidationFailed)
    }
}

impl CustomValidate for Message {
    fn custom_validate(&self) -> ValidationResult<()> {
        // Use the existing validate_content method
        self.validate_content()
            .map_err(ValidationErrorType::ValidationFailed)
    }
}

impl CustomValidate for File {
    fn custom_validate(&self) -> ValidationResult<()> {
        // Use the existing validate_data method
        self.validate_data()
            .map_err(ValidationErrorType::ValidationFailed)
    }
}

/// Validate a message content block
pub fn validate_content_block(block: &MessageContentBlock) -> ValidationResult<()> {
    match block {
        MessageContentBlock::Text { text } => {
            if text.trim().is_empty() {
                return Err(ValidationErrorType::InvalidInput(
                    "Text content cannot be empty".to_string(),
                ));
            }
            if text.len() > 100000 {
                return Err(ValidationErrorType::OutOfRange(
                    "Text content too long (max 100,000 characters)".to_string(),
                ));
            }
        }
        MessageContentBlock::Image { source } => {
            if source.data.is_empty() {
                return Err(ValidationErrorType::InvalidInput(
                    "Image data cannot be empty".to_string(),
                ));
            }
            if !source.media_type.starts_with("image/") {
                return Err(ValidationErrorType::InvalidFormat(
                    "Invalid image media type".to_string(),
                ));
            }
            // Validate base64 encoding
            use base64::{engine::general_purpose, Engine as _};
            if general_purpose::STANDARD.decode(&source.data).is_err() {
                return Err(ValidationErrorType::InvalidFormat(
                    "Invalid base64 encoding in image data".to_string(),
                ));
            }
        }
        MessageContentBlock::Document { source, .. } => {
            if source.data.is_empty() {
                return Err(ValidationErrorType::InvalidInput(
                    "Document data cannot be empty".to_string(),
                ));
            }
            // Validate base64 encoding
            use base64::{engine::general_purpose, Engine as _};
            if general_purpose::STANDARD.decode(&source.data).is_err() {
                return Err(ValidationErrorType::InvalidFormat(
                    "Invalid base64 encoding in document data".to_string(),
                ));
            }
        }
        MessageContentBlock::ToolUse { name, id, input } => {
            if name.trim().is_empty() {
                return Err(ValidationErrorType::InvalidInput(
                    "Tool name cannot be empty".to_string(),
                ));
            }
            if id.trim().is_empty() {
                return Err(ValidationErrorType::InvalidInput(
                    "Tool use ID cannot be empty".to_string(),
                ));
            }
            if !input.is_object() && !input.is_null() {
                return Err(ValidationErrorType::InvalidFormat(
                    "Tool input must be an object or null".to_string(),
                ));
            }
        }
        MessageContentBlock::ToolResult {
            tool_use_id,
            content,
            ..
        } => {
            if tool_use_id.trim().is_empty() {
                return Err(ValidationErrorType::InvalidInput(
                    "Tool use ID cannot be empty".to_string(),
                ));
            }
            // Recursively validate nested content blocks
            for (i, nested_block) in content.iter().enumerate() {
                if let Err(e) = validate_content_block(nested_block) {
                    return Err(ValidationErrorType::ValidationFailed(format!(
                        "Nested content block {i}: {e}"
                    )));
                }
            }
        }
        MessageContentBlock::Thinking {
            thinking,
            signature,
        } => {
            if thinking.trim().is_empty() {
                return Err(ValidationErrorType::InvalidInput(
                    "Thinking content cannot be empty".to_string(),
                ));
            }
            if signature.trim().is_empty() {
                return Err(ValidationErrorType::InvalidInput(
                    "Thinking signature cannot be empty".to_string(),
                ));
            }
        }
        MessageContentBlock::RedactedThinking { data } => {
            if data.trim().is_empty() {
                return Err(ValidationErrorType::InvalidInput(
                    "Redacted thinking data cannot be empty".to_string(),
                ));
            }
        }
    }
    Ok(())
}

/// Validate email format
pub fn validate_email(email: &str) -> ValidationResult<()> {
    let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
    if !email_regex.is_match(email) {
        return Err(ValidationErrorType::InvalidFormat(
            "Invalid email format".to_string(),
        ));
    }
    Ok(())
}

/// Validate UUID format
pub fn validate_uuid(uuid_str: &str) -> ValidationResult<()> {
    if uuid::Uuid::parse_str(uuid_str).is_err() {
        return Err(ValidationErrorType::InvalidFormat(
            "Invalid UUID format".to_string(),
        ));
    }
    Ok(())
}

/// Validate file path for security
pub fn validate_file_path(path: &str) -> ValidationResult<()> {
    // Prevent path traversal attacks
    if path.contains("..") || path.contains("~") {
        return Err(ValidationErrorType::InvalidInput(
            "Path traversal not allowed".to_string(),
        ));
    }

    // Ensure path is not empty
    if path.trim().is_empty() {
        return Err(ValidationErrorType::InvalidInput(
            "File path cannot be empty".to_string(),
        ));
    }

    // Validate path length
    if path.len() > 4096 {
        return Err(ValidationErrorType::OutOfRange(
            "File path too long (max 4096 characters)".to_string(),
        ));
    }

    Ok(())
}

/// Validate JSON structure
pub fn validate_json_object(
    value: &serde_json::Value,
    required_fields: &[&str],
) -> ValidationResult<()> {
    if !value.is_object() {
        return Err(ValidationErrorType::InvalidFormat(
            "Value must be a JSON object".to_string(),
        ));
    }

    let obj = value.as_object().unwrap();
    for field in required_fields {
        if !obj.contains_key(*field) {
            return Err(ValidationErrorType::MissingField(field.to_string()));
        }
    }

    Ok(())
}

/// Sanitize text input to prevent XSS and other attacks
pub fn sanitize_text_input(input: &str) -> String {
    // Remove null bytes and control characters except newlines and tabs
    input
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t' || *c == '\r')
        .collect::<String>()
        .trim()
        .to_string()
}

/// Validate and sanitize user input
pub fn validate_and_sanitize_input(input: &str, max_length: usize) -> ValidationResult<String> {
    if input.is_empty() {
        return Err(ValidationErrorType::InvalidInput(
            "Input cannot be empty".to_string(),
        ));
    }

    let sanitized = sanitize_text_input(input);

    if sanitized.len() > max_length {
        return Err(ValidationErrorType::OutOfRange(format!(
            "Input too long (max {max_length} characters)"
        )));
    }

    Ok(sanitized)
}

/// Comprehensive validation function that combines validator and custom validation
pub fn validate_with_custom<T>(item: &T) -> ValidationResult<()>
where
    T: Validate + CustomValidate,
{
    // First run standard validator validation
    if let Err(validation_errors) = item.validate() {
        let error_messages: Vec<String> = validation_errors
            .field_errors()
            .iter()
            .flat_map(|(field, errors)| {
                let field = field.to_string();
                errors.iter().map(move |error| {
                    format!(
                        "{}: {}",
                        field,
                        error.message.as_ref().unwrap_or(&"Invalid value".into())
                    )
                })
            })
            .collect();

        return Err(ValidationErrorType::ValidationFailed(
            error_messages.join(", "),
        ));
    }

    // Then run custom validation
    item.custom_validate()
}

/// Batch validation for multiple items
pub fn validate_batch<T>(items: &[T]) -> ValidationResult<()>
where
    T: Validate + CustomValidate,
{
    let mut errors = Vec::new();

    for (index, item) in items.iter().enumerate() {
        if let Err(e) = validate_with_custom(item) {
            errors.push(format!("Item {index}: {e}"));
        }
    }

    if !errors.is_empty() {
        return Err(ValidationErrorType::ValidationFailed(errors.join("; ")));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::types::{api::*, task::TaskType};

    #[test]
    fn test_validate_email() {
        assert!(validate_email("test@example.com").is_ok());
        assert!(validate_email("user.name+tag@domain.co.uk").is_ok());
        assert!(validate_email("invalid-email").is_err());
        assert!(validate_email("@domain.com").is_err());
        assert!(validate_email("user@").is_err());
    }

    #[test]
    fn test_validate_uuid() {
        assert!(validate_uuid("550e8400-e29b-41d4-a716-446655440000").is_ok());
        assert!(validate_uuid("invalid-uuid").is_err());
        assert!(validate_uuid("").is_err());
    }

    #[test]
    fn test_validate_file_path() {
        assert!(validate_file_path("/valid/path/file.txt").is_ok());
        assert!(validate_file_path("relative/path.txt").is_ok());
        assert!(validate_file_path("../../../etc/passwd").is_err());
        assert!(validate_file_path("~/secret").is_err());
        assert!(validate_file_path("").is_err());
    }

    #[test]
    fn test_sanitize_text_input() {
        assert_eq!(sanitize_text_input("  hello world  "), "hello world");
        assert_eq!(
            sanitize_text_input("text\nwith\nnewlines"),
            "text\nwith\nnewlines"
        );
        assert_eq!(
            sanitize_text_input("text\x00with\x01control"),
            "textwithcontrol"
        );
    }

    #[test]
    fn test_create_task_dto_validation() {
        let valid_dto = CreateTaskDto {
            description: "Test task".to_string(),
            task_type: Some(TaskType::Immediate),
            scheduled_for: None,
            priority: None,
            created_by: None,
            user_id: None,
            model: Some(json!({"provider": "anthropic", "name": "claude-3"})),
            files: None,
        };
        assert!(validate_with_custom(&valid_dto).is_ok());

        let invalid_dto = CreateTaskDto {
            description: "Test task".to_string(),
            task_type: Some(TaskType::Scheduled),
            scheduled_for: None, // Missing scheduled_for for scheduled task
            priority: None,
            created_by: None,
            user_id: None,
            model: Some(json!({"provider": "anthropic", "name": "claude-3"})),
            files: None,
        };
        assert!(validate_with_custom(&invalid_dto).is_err());
    }

    #[test]
    fn test_add_task_message_dto_validation() {
        let valid_dto = AddTaskMessageDto {
            message: "Hello world".to_string(),
        };
        assert!(validate_with_custom(&valid_dto).is_ok());

        let empty_dto = AddTaskMessageDto {
            message: "".to_string(),
        };
        assert!(validate_with_custom(&empty_dto).is_err());

        let too_long_dto = AddTaskMessageDto {
            message: "x".repeat(100001),
        };
        assert!(validate_with_custom(&too_long_dto).is_err());
    }
}
