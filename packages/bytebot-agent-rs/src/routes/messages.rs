use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::get,
    Router,
};
use bytebot_shared_rs::types::{
    api::{AddTaskMessageDto, ApiResponse, PaginatedResponse, PaginationParams},
    message::{Message, MessageContentBlock},
    task::{Role, Task},
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use validator::Validate;

use crate::{
    database::{
        message_repository::{CreateMessageDto, MessageRepositoryTrait},
        task_repository::TaskRepositoryTrait,
    },
    error::{ServiceError, ServiceResult},
    server::AppState,
};

/// Processed message type for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedMessage {
    #[serde(flatten)]
    pub message: Message,
    pub take_over: Option<bool>,
}

/// Grouped messages for chat UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupedMessages {
    pub role: Role,
    pub messages: Vec<ProcessedMessage>,
    pub take_over: Option<bool>,
}

/// Create message-related routes
pub fn create_message_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/tasks/:id/messages",
            get(get_task_messages).post(add_task_message),
        )
        .route("/tasks/:id/messages/raw", get(get_task_raw_messages))
        .route(
            "/tasks/:id/messages/processed",
            get(get_task_processed_messages),
        )
}

/// Get messages for a task with pagination
/// GET /tasks/:id/messages
async fn get_task_messages(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> ServiceResult<Json<PaginatedResponse<Message>>> {
    debug!("Getting messages for task: {}", task_id);

    // Verify task exists
    let task_repo = state.db.task_repository();
    let _task = task_repo
        .get_by_id(&task_id)
        .await
        .map_err(ServiceError::Database)?
        .ok_or_else(|| ServiceError::NotFound(format!("Task with ID {task_id} not found")))?;

    // Parse pagination parameters
    let page = params.get("page").and_then(|p| p.parse().ok()).unwrap_or(1);
    let limit = params
        .get("limit")
        .and_then(|l| l.parse().ok())
        .unwrap_or(10);

    let pagination = PaginationParams {
        page: Some(page),
        limit: Some(limit),
    };

    // Validate pagination
    pagination
        .validate()
        .map_err(|e| ServiceError::Validation(format!("Invalid pagination: {e}")))?;

    // Get messages from repository
    let message_repo = state.db.message_repository();
    let (messages, total) = message_repo
        .get_by_task_id_paginated(&task_id, &pagination)
        .await
        .map_err(ServiceError::Database)?;

    debug!(
        "Found {} messages for task {} (total: {})",
        messages.len(),
        task_id,
        total
    );

    Ok(Json(PaginatedResponse::new(messages, page, limit, total)))
}

/// Add a new message to a task
/// POST /tasks/:id/messages
async fn add_task_message(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Json(dto): Json<AddTaskMessageDto>,
) -> ServiceResult<Json<ApiResponse<Task>>> {
    debug!("Adding message to task: {}", task_id);

    // Validate the DTO
    dto.validate()
        .map_err(|e| ServiceError::Validation(format!("Validation failed: {e}")))?;

    // Verify task exists
    let task_repo = state.db.task_repository();
    let task = task_repo
        .get_by_id(&task_id)
        .await
        .map_err(ServiceError::Database)?
        .ok_or_else(|| ServiceError::NotFound(format!("Task with ID {task_id} not found")))?;

    // Create message content blocks
    let content_blocks = vec![MessageContentBlock::text(dto.message)];

    // Create message
    let message_repo = state.db.message_repository();
    let create_dto = CreateMessageDto {
        content: content_blocks,
        role: Role::User,
        task_id: task_id.clone(),
        user_id: task.user_id.clone(),
        summary_id: None,
    };

    let message = message_repo
        .create(&create_dto)
        .await
        .map_err(ServiceError::Database)?;

    // Emit new message event via WebSocket
    state
        .websocket_gateway
        .emit_new_message(&task_id, &message)
        .await;

    info!("Successfully added message to task: {}", task_id);

    // Return the updated task
    Ok(Json(ApiResponse::success(task)))
}

/// Get raw messages for a task (unprocessed)
/// GET /tasks/:id/messages/raw
async fn get_task_raw_messages(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> ServiceResult<Json<PaginatedResponse<Message>>> {
    debug!("Getting raw messages for task: {}", task_id);

    // Verify task exists
    let task_repo = state.db.task_repository();
    let _task = task_repo
        .get_by_id(&task_id)
        .await
        .map_err(ServiceError::Database)?
        .ok_or_else(|| ServiceError::NotFound(format!("Task with ID {task_id} not found")))?;

    // Parse pagination parameters
    let page = params.get("page").and_then(|p| p.parse().ok()).unwrap_or(1);
    let limit = params
        .get("limit")
        .and_then(|l| l.parse().ok())
        .unwrap_or(10);

    let pagination = PaginationParams {
        page: Some(page),
        limit: Some(limit),
    };

    // Validate pagination
    pagination
        .validate()
        .map_err(|e| ServiceError::Validation(format!("Invalid pagination: {e}")))?;

    // Get raw messages (same as regular messages)
    let message_repo = state.db.message_repository();
    let (messages, total) = message_repo
        .get_by_task_id_paginated(&task_id, &pagination)
        .await
        .map_err(ServiceError::Database)?;

    debug!(
        "Found {} raw messages for task {} (total: {})",
        messages.len(),
        task_id,
        total
    );

    Ok(Json(PaginatedResponse::new(messages, page, limit, total)))
}

/// Get processed and grouped messages for a task (for chat UI)
/// GET /tasks/:id/messages/processed
async fn get_task_processed_messages(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> ServiceResult<Json<ApiResponse<Vec<GroupedMessages>>>> {
    debug!("Getting processed messages for task: {}", task_id);

    // Verify task exists
    let task_repo = state.db.task_repository();
    let _task = task_repo
        .get_by_id(&task_id)
        .await
        .map_err(ServiceError::Database)?
        .ok_or_else(|| ServiceError::NotFound(format!("Task with ID {task_id} not found")))?;

    // Parse pagination parameters
    let page = params.get("page").and_then(|p| p.parse().ok()).unwrap_or(1);
    let limit = params
        .get("limit")
        .and_then(|l| l.parse().ok())
        .unwrap_or(10);

    let pagination = PaginationParams {
        page: Some(page),
        limit: Some(limit),
    };

    // Validate pagination
    pagination
        .validate()
        .map_err(|e| ServiceError::Validation(format!("Invalid pagination: {e}")))?;

    // Get messages from repository
    let message_repo = state.db.message_repository();
    let (messages, _total) = message_repo
        .get_by_task_id_paginated(&task_id, &pagination)
        .await
        .map_err(ServiceError::Database)?;

    // Process and group messages
    let processed_messages = filter_messages(messages)?;
    let grouped_messages = group_back_to_back_messages(processed_messages);

    debug!(
        "Processed {} message groups for task {}",
        grouped_messages.len(),
        task_id
    );

    Ok(Json(ApiResponse::success(grouped_messages)))
}

/// Filter messages for UI display, adding take_over flags where appropriate
fn filter_messages(messages: Vec<Message>) -> Result<Vec<ProcessedMessage>, ServiceError> {
    let mut filtered_messages = Vec::new();

    for message in messages {
        let mut processed_message = ProcessedMessage {
            message: message.clone(),
            take_over: None,
        };

        // Parse content blocks
        let content_blocks: Vec<MessageContentBlock> = message
            .get_content_blocks()
            .map_err(|e| ServiceError::Internal(format!("Failed to parse content blocks: {e}")))?;

        // Process user messages
        if message.role == Role::User {
            // Check if all content blocks are tool results
            let all_tool_results = content_blocks
                .iter()
                .all(|block| matches!(block, MessageContentBlock::ToolResult { .. }));

            // Check if all content blocks are tool use or tool results (take over actions)
            let all_tool_actions = content_blocks.iter().all(|block| {
                matches!(
                    block,
                    MessageContentBlock::ToolResult { .. } | MessageContentBlock::ToolUse { .. }
                )
            });

            if all_tool_results {
                // Pure tool results should be shown as assistant messages
                processed_message.message.role = Role::Assistant;
            } else if all_tool_actions {
                // Computer tool use (take over actions) should be shown as assistant messages with take_over flag
                let tool_use_blocks: Vec<MessageContentBlock> = content_blocks
                    .into_iter()
                    .filter(|block| matches!(block, MessageContentBlock::ToolUse { .. }))
                    .collect();

                if !tool_use_blocks.is_empty() {
                    processed_message
                        .message
                        .set_content_blocks(tool_use_blocks)
                        .map_err(|e| {
                            ServiceError::Internal(format!("Failed to set content blocks: {e}"))
                        })?;
                    processed_message.message.role = Role::Assistant;
                    processed_message.take_over = Some(true);
                }
            }
            // If there are text blocks mixed with tool blocks, keep as user message
        }

        filtered_messages.push(processed_message);
    }

    Ok(filtered_messages)
}

/// Group back-to-back messages from the same role and take_over status
fn group_back_to_back_messages(messages: Vec<ProcessedMessage>) -> Vec<GroupedMessages> {
    let mut grouped_conversation = Vec::new();
    let mut current_group: Option<GroupedMessages> = None;

    for message in messages {
        let role = message.message.role;
        let is_take_over = message.take_over.unwrap_or(false);

        // If this is the first message, role is different, or take_over status is different from the previous group
        if let Some(ref mut group) = current_group {
            if group.role != role || group.take_over.unwrap_or(false) != is_take_over {
                // Save the previous group
                grouped_conversation.push(current_group.take().unwrap());

                // Start a new group
                current_group = Some(GroupedMessages {
                    role,
                    messages: vec![message],
                    take_over: if is_take_over { Some(true) } else { None },
                });
            } else {
                // Same role and take_over status as previous, add to current group
                group.messages.push(message);
            }
        } else {
            // First message - start a new group
            current_group = Some(GroupedMessages {
                role,
                messages: vec![message],
                take_over: if is_take_over { Some(true) } else { None },
            });
        }
    }

    // Add the last group
    if let Some(group) = current_group {
        grouped_conversation.push(group);
    }

    grouped_conversation
}

#[cfg(test)]
mod tests {
    use bytebot_shared_rs::types::message::MessageContentBlock;
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    fn create_test_message(role: Role, content: Vec<MessageContentBlock>) -> Message {
        let mut message = Message::new(content, role, "test-task-id".to_string());
        message.id = Uuid::new_v4().to_string();
        message
    }

    #[test]
    fn test_filter_messages_text_message() {
        let messages = vec![create_test_message(
            Role::User,
            vec![MessageContentBlock::text("Hello world")],
        )];

        let filtered = filter_messages(messages).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message.role, Role::User);
        assert_eq!(filtered[0].take_over, None);
    }

    #[test]
    fn test_filter_messages_tool_result_only() {
        let messages = vec![create_test_message(
            Role::User,
            vec![MessageContentBlock::tool_result(
                "tool-123",
                vec![MessageContentBlock::text("Result")],
            )],
        )];

        let filtered = filter_messages(messages).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message.role, Role::Assistant);
        assert_eq!(filtered[0].take_over, None);
    }

    #[test]
    fn test_filter_messages_tool_use_takeover() {
        let messages = vec![create_test_message(
            Role::User,
            vec![
                MessageContentBlock::tool_use("computer_use", "tool-123", json!({})),
                MessageContentBlock::tool_result(
                    "tool-123",
                    vec![MessageContentBlock::text("Screenshot taken")],
                ),
            ],
        )];

        let filtered = filter_messages(messages).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message.role, Role::Assistant);
        assert_eq!(filtered[0].take_over, Some(true));
    }

    #[test]
    fn test_group_back_to_back_messages() {
        let messages = vec![
            ProcessedMessage {
                message: create_test_message(Role::User, vec![MessageContentBlock::text("Hello")]),
                take_over: None,
            },
            ProcessedMessage {
                message: create_test_message(Role::User, vec![MessageContentBlock::text("World")]),
                take_over: None,
            },
            ProcessedMessage {
                message: create_test_message(
                    Role::Assistant,
                    vec![MessageContentBlock::text("Hi")],
                ),
                take_over: None,
            },
        ];

        let grouped = group_back_to_back_messages(messages);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].role, Role::User);
        assert_eq!(grouped[0].messages.len(), 2);
        assert_eq!(grouped[1].role, Role::Assistant);
        assert_eq!(grouped[1].messages.len(), 1);
    }

    #[test]
    fn test_group_messages_with_takeover() {
        let messages = vec![
            ProcessedMessage {
                message: create_test_message(
                    Role::Assistant,
                    vec![MessageContentBlock::text("Normal")],
                ),
                take_over: None,
            },
            ProcessedMessage {
                message: create_test_message(
                    Role::Assistant,
                    vec![MessageContentBlock::text("Takeover")],
                ),
                take_over: Some(true),
            },
        ];

        let grouped = group_back_to_back_messages(messages);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].take_over, None);
        assert_eq!(grouped[1].take_over, Some(true));
    }
}
