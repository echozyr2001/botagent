use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres, Row};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use shared::types::{
    api::PaginationParams,
    message::{Message, MessageContentBlock},
    task::Role,
};

use super::DatabaseError;

/// Message filtering options for complex queries
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MessageFilter {
    pub task_id: Option<String>,
    pub role: Option<Role>,
    pub user_id: Option<String>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub has_tool_use: Option<bool>,
    pub has_errors: Option<bool>,
}

/// Data transfer object for creating a new message
#[derive(Debug, Clone)]
pub struct CreateMessageDto {
    pub content: Vec<MessageContentBlock>,
    pub role: Role,
    pub task_id: String,
    pub user_id: Option<String>,
    pub summary_id: Option<String>,
}

/// Data transfer object for updating an existing message
#[derive(Debug, Clone)]
pub struct UpdateMessageDto {
    pub content: Option<Vec<MessageContentBlock>>,
    pub summary_id: Option<String>,
}

/// Message repository trait for dependency injection and testing
#[async_trait]
pub trait MessageRepositoryTrait: Send + Sync {
    async fn create(&self, dto: &CreateMessageDto) -> Result<Message, DatabaseError>;
    async fn get_by_id(&self, id: &str) -> Result<Option<Message>, DatabaseError>;
    async fn update(
        &self,
        id: &str,
        dto: &UpdateMessageDto,
    ) -> Result<Option<Message>, DatabaseError>;
    async fn delete(&self, id: &str) -> Result<bool, DatabaseError>;
    async fn list(
        &self,
        filter: &MessageFilter,
        pagination: &PaginationParams,
    ) -> Result<(Vec<Message>, u64), DatabaseError>;
    async fn get_by_task_id(&self, task_id: &str) -> Result<Vec<Message>, DatabaseError>;
    async fn get_by_task_id_paginated(
        &self,
        task_id: &str,
        pagination: &PaginationParams,
    ) -> Result<(Vec<Message>, u64), DatabaseError>;
    async fn delete_by_task_id(&self, task_id: &str) -> Result<u64, DatabaseError>;
    async fn count_by_task_id(&self, task_id: &str) -> Result<u64, DatabaseError>;
    async fn get_latest_by_task_id(
        &self,
        task_id: &str,
        limit: u32,
    ) -> Result<Vec<Message>, DatabaseError>;
}

/// SQLx-based message repository implementation
pub struct MessageRepository {
    pool: Pool<Postgres>,
}

impl MessageRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    /// Build WHERE clause for message filtering
    fn build_filter_clause(
        filter: &MessageFilter,
    ) -> (String, Vec<Box<dyn sqlx::Encode<'_, Postgres> + Send>>) {
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn sqlx::Encode<'_, Postgres> + Send>> = Vec::new();
        let mut param_count = 1;

        if let Some(ref task_id) = filter.task_id {
            conditions.push(format!("\"taskId\" = ${param_count}"));
            params.push(Box::new(task_id.clone()));
            param_count += 1;
        }

        if let Some(role) = filter.role {
            conditions.push(format!("role = ${param_count}"));
            params.push(Box::new(role));
            param_count += 1;
        }

        if let Some(ref user_id) = filter.user_id {
            conditions.push(format!("\"userId\" = ${param_count}"));
            params.push(Box::new(user_id.clone()));
            param_count += 1;
        }

        if let Some(created_after) = filter.created_after {
            conditions.push(format!("\"createdAt\" >= ${param_count}"));
            params.push(Box::new(created_after));
            param_count += 1;
        }

        if let Some(created_before) = filter.created_before {
            conditions.push(format!("\"createdAt\" <= ${param_count}"));
            params.push(Box::new(created_before));
            param_count += 1;
        }

        // Complex JSON queries for content analysis
        if let Some(has_tool_use) = filter.has_tool_use {
            if has_tool_use {
                conditions.push("content::jsonb @> '[{\"type\": \"tool_use\"}]'".to_string());
            } else {
                conditions.push("NOT content::jsonb @> '[{\"type\": \"tool_use\"}]'".to_string());
            }
        }

        if let Some(has_errors) = filter.has_errors {
            if has_errors {
                conditions.push(
                    "content::jsonb @> '[{\"type\": \"tool_result\", \"is_error\": true}]'"
                        .to_string(),
                );
            } else {
                conditions.push(
                    "NOT content::jsonb @> '[{\"type\": \"tool_result\", \"is_error\": true}]'"
                        .to_string(),
                );
            }
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        (where_clause, params)
    }

    /// Validate message content structure
    fn validate_content(content: &[MessageContentBlock]) -> Result<(), DatabaseError> {
        if content.is_empty() {
            return Err(DatabaseError::ValidationError(
                "Message content cannot be empty".to_string(),
            ));
        }

        // Validate each content block
        for block in content {
            match block {
                MessageContentBlock::Text { text } => {
                    if text.trim().is_empty() {
                        return Err(DatabaseError::ValidationError(
                            "Text content blocks cannot be empty".to_string(),
                        ));
                    }
                }
                MessageContentBlock::ToolUse { name, id, .. } => {
                    if name.trim().is_empty() || id.trim().is_empty() {
                        return Err(DatabaseError::ValidationError(
                            "Tool use blocks must have valid name and id".to_string(),
                        ));
                    }
                }
                MessageContentBlock::ToolResult { tool_use_id, .. } => {
                    if tool_use_id.trim().is_empty() {
                        return Err(DatabaseError::ValidationError(
                            "Tool result blocks must have valid tool_use_id".to_string(),
                        ));
                    }
                }
                _ => {} // Other types are valid as-is
            }
        }

        Ok(())
    }
}
#[async_trait]
impl MessageRepositoryTrait for MessageRepository {
    async fn create(&self, dto: &CreateMessageDto) -> Result<Message, DatabaseError> {
        debug!("Creating new message for task: {}", dto.task_id);

        // Validate content structure
        Self::validate_content(&dto.content)?;

        let message_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        // Serialize content blocks to JSON
        let content_json = serde_json::to_value(&dto.content).map_err(|e| {
            DatabaseError::SerializationError(format!("Failed to serialize content: {e}"))
        })?;

        let row = sqlx::query(
            r#"
            INSERT INTO "Message" (
                id, content, role, "createdAt", "updatedAt", 
                "taskId", "summaryId", "userId"
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING 
                id,
                content,
                role,
                "createdAt",
                "updatedAt",
                "taskId",
                "summaryId",
                "userId"
            "#,
        )
        .bind(&message_id)
        .bind(&content_json)
        .bind(dto.role.to_string())
        .bind(now)
        .bind(now)
        .bind(&dto.task_id)
        .bind(&dto.summary_id)
        .bind(&dto.user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to create message: {}", e);
            DatabaseError::QueryError(e)
        })?;

        let message = Message {
            id: row.get("id"),
            content: row.get("content"),
            role: row
                .get::<String, _>("role")
                .parse()
                .map_err(|_| DatabaseError::SerializationError("Invalid role".to_string()))?,
            created_at: row.get("createdAt"),
            updated_at: row.get("updatedAt"),
            task_id: row.get("taskId"),
            summary_id: row.get("summaryId"),
            user_id: row.get("userId"),
        };

        info!("Successfully created message with ID: {}", message.id);
        Ok(message)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Message>, DatabaseError> {
        debug!("Fetching message by ID: {}", id);

        let row = sqlx::query(
            r#"
            SELECT 
                id,
                content,
                role,
                "createdAt",
                "updatedAt",
                "taskId",
                "summaryId",
                "userId"
            FROM "Message"
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to fetch message by ID {}: {}", id, e);
            DatabaseError::QueryError(e)
        })?;

        let message = match row {
            Some(row) => {
                debug!("Found message with ID: {}", id);
                Some(Message {
                    id: row.get("id"),
                    content: row.get("content"),
                    role: row.get::<String, _>("role").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid role".to_string())
                    })?,
                    created_at: row.get("createdAt"),
                    updated_at: row.get("updatedAt"),
                    task_id: row.get("taskId"),
                    summary_id: row.get("summaryId"),
                    user_id: row.get("userId"),
                })
            }
            None => {
                debug!("No message found with ID: {}", id);
                None
            }
        };

        Ok(message)
    }

    async fn update(
        &self,
        id: &str,
        dto: &UpdateMessageDto,
    ) -> Result<Option<Message>, DatabaseError> {
        debug!("Updating message with ID: {}", id);

        // Get current message to preserve existing values
        let current_message = self.get_by_id(id).await?;
        let current_message = match current_message {
            Some(message) => message,
            None => {
                warn!("Attempted to update non-existent message: {}", id);
                return Ok(None);
            }
        };

        let now = Utc::now();
        let content_json = if let Some(ref content) = dto.content {
            Self::validate_content(content)?;
            serde_json::to_value(content).map_err(|e| {
                DatabaseError::SerializationError(format!("Failed to serialize content: {e}"))
            })?
        } else {
            current_message.content
        };

        let summary_id = dto
            .summary_id
            .as_ref()
            .or(current_message.summary_id.as_ref());

        let row = sqlx::query(
            r#"
            UPDATE "Message"
            SET 
                content = $2,
                "summaryId" = $3,
                "updatedAt" = $4
            WHERE id = $1
            RETURNING 
                id,
                content,
                role,
                "createdAt",
                "updatedAt",
                "taskId",
                "summaryId",
                "userId"
            "#,
        )
        .bind(id)
        .bind(&content_json)
        .bind(summary_id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to update message {}: {}", id, e);
            DatabaseError::QueryError(e)
        })?;

        let message = match row {
            Some(row) => {
                info!("Successfully updated message with ID: {}", id);
                Some(Message {
                    id: row.get("id"),
                    content: row.get("content"),
                    role: row.get::<String, _>("role").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid role".to_string())
                    })?,
                    created_at: row.get("createdAt"),
                    updated_at: row.get("updatedAt"),
                    task_id: row.get("taskId"),
                    summary_id: row.get("summaryId"),
                    user_id: row.get("userId"),
                })
            }
            None => None,
        };

        Ok(message)
    }
    async fn delete(&self, id: &str) -> Result<bool, DatabaseError> {
        debug!("Deleting message with ID: {}", id);

        let result = sqlx::query(r#"DELETE FROM "Message" WHERE id = $1"#)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to delete message {}: {}", id, e);
                DatabaseError::QueryError(e)
            })?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!("Successfully deleted message with ID: {}", id);
        } else {
            warn!("No message found to delete with ID: {}", id);
        }

        Ok(deleted)
    }

    async fn list(
        &self,
        filter: &MessageFilter,
        pagination: &PaginationParams,
    ) -> Result<(Vec<Message>, u64), DatabaseError> {
        debug!("Listing messages with filter: {:?}", filter);

        let page = pagination.page.unwrap_or(1);
        let limit = pagination.limit.unwrap_or(20);
        let offset = (page - 1) * limit;

        // Build the WHERE clause for filtering
        let (where_clause, _params) = Self::build_filter_clause(filter);

        // Count total matching records
        let count_query = format!(r#"SELECT COUNT(*) as count FROM "Message" {where_clause}"#);

        let total_count: i64 = sqlx::query(&count_query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to count messages: {}", e);
                DatabaseError::QueryError(e)
            })?
            .get("count");

        // Fetch paginated results
        let data_query = format!(
            r#"
            SELECT 
                id,
                content,
                role,
                "createdAt" as created_at,
                "updatedAt" as updated_at,
                "taskId" as task_id,
                "summaryId" as summary_id,
                "userId" as user_id
            FROM "Message"
            {where_clause}
            ORDER BY "createdAt" DESC
            LIMIT $1 OFFSET $2
            "#
        );

        let rows = sqlx::query(&data_query)
            .bind(limit as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to fetch messages: {}", e);
                DatabaseError::QueryError(e)
            })?;

        let messages: Result<Vec<Message>, DatabaseError> = rows
            .into_iter()
            .map(|row| {
                Ok::<Message, DatabaseError>(Message {
                    id: row.get("id"),
                    content: row.get("content"),
                    role: row.get::<String, _>("role").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid role".to_string())
                    })?,
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                    task_id: row.get("task_id"),
                    summary_id: row.get("summary_id"),
                    user_id: row.get("user_id"),
                })
            })
            .collect();

        let messages = messages?;

        debug!("Found {} messages (total: {})", messages.len(), total_count);
        Ok((messages, total_count as u64))
    }

    async fn get_by_task_id(&self, task_id: &str) -> Result<Vec<Message>, DatabaseError> {
        debug!("Fetching messages for task: {}", task_id);

        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                content,
                role,
                "createdAt",
                "updatedAt",
                "taskId",
                "summaryId",
                "userId"
            FROM "Message"
            WHERE "taskId" = $1
            ORDER BY "createdAt" ASC
            "#,
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to fetch messages for task {}: {}", task_id, e);
            DatabaseError::QueryError(e)
        })?;

        let messages: Result<Vec<Message>, DatabaseError> = rows
            .into_iter()
            .map(|row| {
                Ok::<Message, DatabaseError>(Message {
                    id: row.get("id"),
                    content: row.get("content"),
                    role: row.get::<String, _>("role").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid role".to_string())
                    })?,
                    created_at: row.get("createdAt"),
                    updated_at: row.get("updatedAt"),
                    task_id: row.get("taskId"),
                    summary_id: row.get("summaryId"),
                    user_id: row.get("userId"),
                })
            })
            .collect();

        let messages = messages?;
        debug!("Found {} messages for task {}", messages.len(), task_id);
        Ok(messages)
    }
    async fn get_by_task_id_paginated(
        &self,
        task_id: &str,
        pagination: &PaginationParams,
    ) -> Result<(Vec<Message>, u64), DatabaseError> {
        debug!("Fetching paginated messages for task: {}", task_id);

        let page = pagination.page.unwrap_or(1);
        let limit = pagination.limit.unwrap_or(20);
        let offset = (page - 1) * limit;

        // Count total messages for this task
        let total_count: i64 =
            sqlx::query(r#"SELECT COUNT(*) as count FROM "Message" WHERE "taskId" = $1"#)
                .bind(task_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| {
                    error!("Failed to count messages for task {}: {}", task_id, e);
                    DatabaseError::QueryError(e)
                })?
                .get("count");

        // Fetch paginated results
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                content,
                role,
                "createdAt",
                "updatedAt",
                "taskId",
                "summaryId",
                "userId"
            FROM "Message"
            WHERE "taskId" = $1
            ORDER BY "createdAt" ASC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(task_id)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!(
                "Failed to fetch paginated messages for task {}: {}",
                task_id, e
            );
            DatabaseError::QueryError(e)
        })?;

        let messages: Result<Vec<Message>, DatabaseError> = rows
            .into_iter()
            .map(|row| {
                Ok::<Message, DatabaseError>(Message {
                    id: row.get("id"),
                    content: row.get("content"),
                    role: row.get::<String, _>("role").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid role".to_string())
                    })?,
                    created_at: row.get("createdAt"),
                    updated_at: row.get("updatedAt"),
                    task_id: row.get("taskId"),
                    summary_id: row.get("summaryId"),
                    user_id: row.get("userId"),
                })
            })
            .collect();

        let messages = messages?;
        debug!(
            "Found {} messages for task {} (total: {})",
            messages.len(),
            task_id,
            total_count
        );
        Ok((messages, total_count as u64))
    }

    async fn delete_by_task_id(&self, task_id: &str) -> Result<u64, DatabaseError> {
        debug!("Deleting all messages for task: {}", task_id);

        let result = sqlx::query(r#"DELETE FROM "Message" WHERE "taskId" = $1"#)
            .bind(task_id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to delete messages for task {}: {}", task_id, e);
                DatabaseError::QueryError(e)
            })?;

        let deleted_count = result.rows_affected();
        info!(
            "Successfully deleted {} messages for task {}",
            deleted_count, task_id
        );
        Ok(deleted_count)
    }

    async fn count_by_task_id(&self, task_id: &str) -> Result<u64, DatabaseError> {
        debug!("Counting messages for task: {}", task_id);

        let count: i64 =
            sqlx::query(r#"SELECT COUNT(*) as count FROM "Message" WHERE "taskId" = $1"#)
                .bind(task_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| {
                    error!("Failed to count messages for task {}: {}", task_id, e);
                    DatabaseError::QueryError(e)
                })?
                .get("count");

        debug!("Found {} messages for task {}", count, task_id);
        Ok(count as u64)
    }

    async fn get_latest_by_task_id(
        &self,
        task_id: &str,
        limit: u32,
    ) -> Result<Vec<Message>, DatabaseError> {
        debug!("Fetching latest {} messages for task: {}", limit, task_id);

        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                content,
                role,
                "createdAt",
                "updatedAt",
                "taskId",
                "summaryId",
                "userId"
            FROM "Message"
            WHERE "taskId" = $1
            ORDER BY "createdAt" DESC
            LIMIT $2
            "#,
        )
        .bind(task_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!(
                "Failed to fetch latest messages for task {}: {}",
                task_id, e
            );
            DatabaseError::QueryError(e)
        })?;

        let messages: Result<Vec<Message>, DatabaseError> = rows
            .into_iter()
            .map(|row| {
                Ok::<Message, DatabaseError>(Message {
                    id: row.get("id"),
                    content: row.get("content"),
                    role: row.get::<String, _>("role").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid role".to_string())
                    })?,
                    created_at: row.get("createdAt"),
                    updated_at: row.get("updatedAt"),
                    task_id: row.get("taskId"),
                    summary_id: row.get("summaryId"),
                    user_id: row.get("userId"),
                })
            })
            .collect();

        let messages = messages?;
        debug!(
            "Found {} latest messages for task {}",
            messages.len(),
            task_id
        );
        Ok(messages)
    }
}
