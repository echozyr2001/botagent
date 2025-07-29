use std::collections::HashMap;

use async_trait::async_trait;
use bytebot_shared_rs::types::{
    api::{CreateTaskDto, PaginationParams, UpdateTaskDto},
    task::{Role, Task, TaskPriority, TaskStatus, TaskType},
};
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres, Row};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::DatabaseError;

/// Task filtering options for complex queries
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TaskFilter {
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub task_type: Option<TaskType>,
    pub user_id: Option<String>,
    pub created_by: Option<Role>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub scheduled_after: Option<DateTime<Utc>>,
    pub scheduled_before: Option<DateTime<Utc>>,
}

/// Task repository trait for dependency injection and testing
#[async_trait]
pub trait TaskRepositoryTrait: Send + Sync {
    async fn create(&self, dto: &CreateTaskDto) -> Result<Task, DatabaseError>;
    async fn get_by_id(&self, id: &str) -> Result<Option<Task>, DatabaseError>;
    async fn update(&self, id: &str, dto: &UpdateTaskDto) -> Result<Option<Task>, DatabaseError>;
    async fn delete(&self, id: &str) -> Result<bool, DatabaseError>;
    async fn list(
        &self,
        filter: &TaskFilter,
        pagination: &PaginationParams,
    ) -> Result<(Vec<Task>, u64), DatabaseError>;
    async fn update_status(
        &self,
        id: &str,
        status: TaskStatus,
    ) -> Result<Option<Task>, DatabaseError>;
    async fn get_tasks_by_status(&self, status: TaskStatus) -> Result<Vec<Task>, DatabaseError>;
    async fn get_scheduled_tasks(&self, before: DateTime<Utc>) -> Result<Vec<Task>, DatabaseError>;
    async fn count_by_status(&self) -> Result<HashMap<TaskStatus, u64>, DatabaseError>;
}

/// SQLx-based task repository implementation
pub struct TaskRepository {
    pool: Pool<Postgres>,
}

impl TaskRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    /// Validate task status transitions
    fn validate_status_transition(
        current: TaskStatus,
        new: TaskStatus,
    ) -> Result<(), DatabaseError> {
        use TaskStatus::*;

        let valid_transitions = match current {
            Pending => vec![Running, Cancelled],
            Running => vec![NeedsHelp, NeedsReview, Completed, Failed, Cancelled],
            NeedsHelp => vec![Running, Cancelled],
            NeedsReview => vec![Running, Completed, Cancelled],
            Completed | Failed | Cancelled => vec![], // Terminal states
        };

        if valid_transitions.contains(&new) {
            Ok(())
        } else {
            Err(DatabaseError::InvalidStatusTransition {
                from: current,
                to: new,
            })
        }
    }

    /// Build WHERE clause for task filtering
    fn build_filter_clause(
        filter: &TaskFilter,
    ) -> (String, Vec<Box<dyn sqlx::Encode<'_, Postgres> + Send>>) {
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn sqlx::Encode<'_, Postgres> + Send>> = Vec::new();
        let mut param_count = 1;

        if let Some(status) = filter.status {
            conditions.push(format!("status = ${param_count}"));
            params.push(Box::new(status));
            param_count += 1;
        }

        if let Some(priority) = filter.priority {
            conditions.push(format!("priority = ${param_count}"));
            params.push(Box::new(priority));
            param_count += 1;
        }

        if let Some(task_type) = filter.task_type {
            conditions.push(format!("type = ${param_count}"));
            params.push(Box::new(task_type));
            param_count += 1;
        }

        if let Some(ref user_id) = filter.user_id {
            conditions.push(format!("\"userId\" = ${param_count}"));
            params.push(Box::new(user_id.clone()));
            param_count += 1;
        }

        if let Some(created_by) = filter.created_by {
            conditions.push(format!("\"createdBy\" = ${param_count}"));
            params.push(Box::new(created_by));
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

        if let Some(scheduled_after) = filter.scheduled_after {
            conditions.push(format!("\"scheduledFor\" >= ${param_count}"));
            params.push(Box::new(scheduled_after));
            param_count += 1;
        }

        if let Some(scheduled_before) = filter.scheduled_before {
            conditions.push(format!("\"scheduledFor\" <= ${param_count}"));
            params.push(Box::new(scheduled_before));
            param_count += 1;
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        (where_clause, params)
    }
}

#[async_trait]
impl TaskRepositoryTrait for TaskRepository {
    async fn create(&self, dto: &CreateTaskDto) -> Result<Task, DatabaseError> {
        debug!("Creating new task with description: {}", dto.description);

        let task_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let task_type = dto.task_type.unwrap_or_default();
        let priority = dto.priority.unwrap_or_default();
        let created_by = dto.created_by.unwrap_or(Role::User);
        let model = dto.model.clone().unwrap_or_else(|| {
            serde_json::json!({
                "provider": "anthropic",
                "name": "claude-3-sonnet-20240229",
                "title": "Claude 3 Sonnet"
            })
        });

        // Validate scheduled tasks have scheduled_for timestamp
        if task_type == TaskType::Scheduled && dto.scheduled_for.is_none() {
            return Err(DatabaseError::ValidationError(
                "Scheduled tasks must have scheduled_for timestamp".to_string(),
            ));
        }

        let row = sqlx::query(
            r#"
            INSERT INTO "Task" (
                id, description, type, status, priority, control, 
                "createdAt", "createdBy", "scheduledFor", "updatedAt", 
                model, "userId"
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING 
                id,
                description,
                type,
                status,
                priority,
                control,
                "createdAt",
                "createdBy",
                "scheduledFor",
                "updatedAt",
                "executedAt",
                "completedAt",
                "queuedAt",
                error,
                result,
                model,
                "userId"
            "#,
        )
        .bind(&task_id)
        .bind(&dto.description)
        .bind(task_type.to_string())
        .bind(TaskStatus::Pending.to_string())
        .bind(priority.to_string())
        .bind(Role::Assistant.to_string())
        .bind(now)
        .bind(created_by.to_string())
        .bind(dto.scheduled_for)
        .bind(now)
        .bind(&model)
        .bind(&dto.user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to create task: {}", e);
            DatabaseError::QueryError(e)
        })?;

        let task =
            Task {
                id: row.get("id"),
                description: row.get("description"),
                task_type: row.get::<String, _>("type").parse().map_err(|_| {
                    DatabaseError::SerializationError("Invalid task type".to_string())
                })?,
                status: row
                    .get::<String, _>("status")
                    .parse()
                    .map_err(|_| DatabaseError::SerializationError("Invalid status".to_string()))?,
                priority: row.get::<String, _>("priority").parse().map_err(|_| {
                    DatabaseError::SerializationError("Invalid priority".to_string())
                })?,
                control: row.get::<String, _>("control").parse().map_err(|_| {
                    DatabaseError::SerializationError("Invalid control".to_string())
                })?,
                created_at: row.get("createdAt"),
                created_by: row.get::<String, _>("createdBy").parse().map_err(|_| {
                    DatabaseError::SerializationError("Invalid created_by".to_string())
                })?,
                scheduled_for: row.get("scheduledFor"),
                updated_at: row.get("updatedAt"),
                executed_at: row.get("executedAt"),
                completed_at: row.get("completedAt"),
                queued_at: row.get("queuedAt"),
                error: row.get("error"),
                result: row.get("result"),
                model: row.get("model"),
                user_id: row.get("userId"),
            };

        info!("Successfully created task with ID: {}", task.id);
        Ok(task)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Task>, DatabaseError> {
        debug!("Fetching task by ID: {}", id);

        let row = sqlx::query(
            r#"
            SELECT 
                id,
                description,
                type,
                status,
                priority,
                control,
                "createdAt",
                "createdBy",
                "scheduledFor",
                "updatedAt",
                "executedAt",
                "completedAt",
                "queuedAt",
                error,
                result,
                model,
                "userId"
            FROM "Task"
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to fetch task by ID {}: {}", id, e);
            DatabaseError::QueryError(e)
        })?;

        let task = match row {
            Some(row) => {
                debug!("Found task with ID: {}", id);
                Some(Task {
                    id: row.get("id"),
                    description: row.get("description"),
                    task_type: row.get::<String, _>("type").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid task type".to_string())
                    })?,
                    status: row.get::<String, _>("status").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid status".to_string())
                    })?,
                    priority: row.get::<String, _>("priority").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid priority".to_string())
                    })?,
                    control: row.get::<String, _>("control").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid control".to_string())
                    })?,
                    created_at: row.get("createdAt"),
                    created_by: row.get::<String, _>("createdBy").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid created_by".to_string())
                    })?,
                    scheduled_for: row.get("scheduledFor"),
                    updated_at: row.get("updatedAt"),
                    executed_at: row.get("executedAt"),
                    completed_at: row.get("completedAt"),
                    queued_at: row.get("queuedAt"),
                    error: row.get("error"),
                    result: row.get("result"),
                    model: row.get("model"),
                    user_id: row.get("userId"),
                })
            }
            None => {
                debug!("No task found with ID: {}", id);
                None
            }
        };

        Ok(task)
    }

    async fn update(&self, id: &str, dto: &UpdateTaskDto) -> Result<Option<Task>, DatabaseError> {
        debug!("Updating task with ID: {}", id);

        // First, get the current task to validate status transitions
        let current_task = self.get_by_id(id).await?;
        let current_task = match current_task {
            Some(task) => task,
            None => {
                warn!("Attempted to update non-existent task: {}", id);
                return Ok(None);
            }
        };

        // Validate status transition if status is being updated
        if let Some(new_status) = dto.status {
            Self::validate_status_transition(current_task.status, new_status)?;
        }

        let now = Utc::now();
        let status = dto.status.unwrap_or(current_task.status);
        let priority = dto.priority.unwrap_or(current_task.priority);

        let row = sqlx::query(
            r#"
            UPDATE "Task"
            SET 
                status = $2,
                priority = $3,
                "queuedAt" = $4,
                "executedAt" = $5,
                "completedAt" = $6,
                "updatedAt" = $7
            WHERE id = $1
            RETURNING 
                id,
                description,
                type,
                status,
                priority,
                control,
                "createdAt",
                "createdBy",
                "scheduledFor",
                "updatedAt",
                "executedAt",
                "completedAt",
                "queuedAt",
                error,
                result,
                model,
                "userId"
            "#,
        )
        .bind(id)
        .bind(status.to_string())
        .bind(priority.to_string())
        .bind(dto.queued_at)
        .bind(dto.executed_at)
        .bind(dto.completed_at)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to update task {}: {}", id, e);
            DatabaseError::QueryError(e)
        })?;

        let task = match row {
            Some(row) => {
                info!("Successfully updated task with ID: {}", id);
                Some(Task {
                    id: row.get("id"),
                    description: row.get("description"),
                    task_type: row.get::<String, _>("type").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid task type".to_string())
                    })?,
                    status: row.get::<String, _>("status").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid status".to_string())
                    })?,
                    priority: row.get::<String, _>("priority").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid priority".to_string())
                    })?,
                    control: row.get::<String, _>("control").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid control".to_string())
                    })?,
                    created_at: row.get("createdAt"),
                    created_by: row.get::<String, _>("createdBy").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid created_by".to_string())
                    })?,
                    scheduled_for: row.get("scheduledFor"),
                    updated_at: row.get("updatedAt"),
                    executed_at: row.get("executedAt"),
                    completed_at: row.get("completedAt"),
                    queued_at: row.get("queuedAt"),
                    error: row.get("error"),
                    result: row.get("result"),
                    model: row.get("model"),
                    user_id: row.get("userId"),
                })
            }
            None => None,
        };

        Ok(task)
    }

    async fn delete(&self, id: &str) -> Result<bool, DatabaseError> {
        debug!("Deleting task with ID: {}", id);

        let result = sqlx::query(r#"DELETE FROM "Task" WHERE id = $1"#)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to delete task {}: {}", id, e);
                DatabaseError::QueryError(e)
            })?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!("Successfully deleted task with ID: {}", id);
        } else {
            warn!("No task found to delete with ID: {}", id);
        }

        Ok(deleted)
    }
    async fn list(
        &self,
        filter: &TaskFilter,
        pagination: &PaginationParams,
    ) -> Result<(Vec<Task>, u64), DatabaseError> {
        debug!("Listing tasks with filter: {:?}", filter);

        let page = pagination.page.unwrap_or(1);
        let limit = pagination.limit.unwrap_or(20);
        let offset = (page - 1) * limit;

        // Build the WHERE clause for filtering
        let (where_clause, _params) = Self::build_filter_clause(filter);

        // Count total matching records
        let count_query = format!(r#"SELECT COUNT(*) as count FROM "Task" {where_clause}"#);

        let total_count: i64 = sqlx::query(&count_query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to count tasks: {}", e);
                DatabaseError::QueryError(e)
            })?
            .get("count");

        // Fetch paginated results
        let data_query = format!(
            r#"
            SELECT 
                id,
                description,
                type as task_type,
                status,
                priority,
                control,
                "createdAt" as created_at,
                "createdBy" as created_by,
                "scheduledFor" as scheduled_for,
                "updatedAt" as updated_at,
                "executedAt" as executed_at,
                "completedAt" as completed_at,
                "queuedAt" as queued_at,
                error,
                result,
                model,
                "userId" as user_id
            FROM "Task"
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
                error!("Failed to fetch tasks: {}", e);
                DatabaseError::QueryError(e)
            })?;

        let tasks: Result<Vec<Task>, DatabaseError> = rows
            .into_iter()
            .map(|row| {
                Ok::<Task, DatabaseError>(Task {
                    id: row.get("id"),
                    description: row.get("description"),
                    task_type: row.get::<String, _>("task_type").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid task type".to_string())
                    })?,
                    status: row.get::<String, _>("status").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid status".to_string())
                    })?,
                    priority: row.get::<String, _>("priority").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid priority".to_string())
                    })?,
                    control: row.get::<String, _>("control").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid control".to_string())
                    })?,
                    created_at: row.get("created_at"),
                    created_by: row.get::<String, _>("created_by").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid created_by".to_string())
                    })?,
                    scheduled_for: row.get("scheduled_for"),
                    updated_at: row.get("updated_at"),
                    executed_at: row.get("executed_at"),
                    completed_at: row.get("completed_at"),
                    queued_at: row.get("queued_at"),
                    error: row.get("error"),
                    result: row.get("result"),
                    model: row.get("model"),
                    user_id: row.get("user_id"),
                })
            })
            .collect();

        let tasks = tasks?;

        debug!("Found {} tasks (total: {})", tasks.len(), total_count);
        Ok((tasks, total_count as u64))
    }

    async fn update_status(
        &self,
        id: &str,
        status: TaskStatus,
    ) -> Result<Option<Task>, DatabaseError> {
        debug!("Updating task {} status to {:?}", id, status);

        // Get current task to validate transition
        let current_task = self.get_by_id(id).await?;
        let current_task = match current_task {
            Some(task) => task,
            None => return Ok(None),
        };

        // Validate status transition
        Self::validate_status_transition(current_task.status, status)?;

        let now = Utc::now();
        let mut executed_at = current_task.executed_at;
        let mut completed_at = current_task.completed_at;

        // Set timestamps based on status
        match status {
            TaskStatus::Running if executed_at.is_none() => {
                executed_at = Some(now);
            }
            TaskStatus::Completed | TaskStatus::Failed if completed_at.is_none() => {
                completed_at = Some(now);
                if executed_at.is_none() {
                    executed_at = Some(now);
                }
            }
            _ => {}
        }

        let row = sqlx::query(
            r#"
            UPDATE "Task"
            SET 
                status = $2,
                "executedAt" = $3,
                "completedAt" = $4,
                "updatedAt" = $5
            WHERE id = $1
            RETURNING 
                id,
                description,
                type,
                status,
                priority,
                control,
                "createdAt",
                "createdBy",
                "scheduledFor",
                "updatedAt",
                "executedAt",
                "completedAt",
                "queuedAt",
                error,
                result,
                model,
                "userId"
            "#,
        )
        .bind(id)
        .bind(status.to_string())
        .bind(executed_at)
        .bind(completed_at)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to update task status for {}: {}", id, e);
            DatabaseError::QueryError(e)
        })?;

        let task = match row {
            Some(row) => {
                info!("Successfully updated task {} status to {:?}", id, status);
                Some(Task {
                    id: row.get("id"),
                    description: row.get("description"),
                    task_type: row.get::<String, _>("type").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid task type".to_string())
                    })?,
                    status: row.get::<String, _>("status").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid status".to_string())
                    })?,
                    priority: row.get::<String, _>("priority").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid priority".to_string())
                    })?,
                    control: row.get::<String, _>("control").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid control".to_string())
                    })?,
                    created_at: row.get("createdAt"),
                    created_by: row.get::<String, _>("createdBy").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid created_by".to_string())
                    })?,
                    scheduled_for: row.get("scheduledFor"),
                    updated_at: row.get("updatedAt"),
                    executed_at: row.get("executedAt"),
                    completed_at: row.get("completedAt"),
                    queued_at: row.get("queuedAt"),
                    error: row.get("error"),
                    result: row.get("result"),
                    model: row.get("model"),
                    user_id: row.get("userId"),
                })
            }
            None => None,
        };

        Ok(task)
    }

    async fn get_tasks_by_status(&self, status: TaskStatus) -> Result<Vec<Task>, DatabaseError> {
        debug!("Fetching tasks with status: {:?}", status);

        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                description,
                type,
                status,
                priority,
                control,
                "createdAt",
                "createdBy",
                "scheduledFor",
                "updatedAt",
                "executedAt",
                "completedAt",
                "queuedAt",
                error,
                result,
                model,
                "userId"
            FROM "Task"
            WHERE status = $1
            ORDER BY "createdAt" DESC
            "#,
        )
        .bind(status.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to fetch tasks by status {:?}: {}", status, e);
            DatabaseError::QueryError(e)
        })?;

        let tasks: Result<Vec<Task>, DatabaseError> = rows
            .into_iter()
            .map(|row| {
                Ok::<Task, DatabaseError>(Task {
                    id: row.get("id"),
                    description: row.get("description"),
                    task_type: row.get::<String, _>("type").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid task type".to_string())
                    })?,
                    status: row.get::<String, _>("status").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid status".to_string())
                    })?,
                    priority: row.get::<String, _>("priority").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid priority".to_string())
                    })?,
                    control: row.get::<String, _>("control").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid control".to_string())
                    })?,
                    created_at: row.get("createdAt"),
                    created_by: row.get::<String, _>("createdBy").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid created_by".to_string())
                    })?,
                    scheduled_for: row.get("scheduledFor"),
                    updated_at: row.get("updatedAt"),
                    executed_at: row.get("executedAt"),
                    completed_at: row.get("completedAt"),
                    queued_at: row.get("queuedAt"),
                    error: row.get("error"),
                    result: row.get("result"),
                    model: row.get("model"),
                    user_id: row.get("userId"),
                })
            })
            .collect();

        let tasks = tasks?;
        debug!("Found {} tasks with status {:?}", tasks.len(), status);
        Ok(tasks)
    }

    async fn get_scheduled_tasks(&self, before: DateTime<Utc>) -> Result<Vec<Task>, DatabaseError> {
        debug!("Fetching scheduled tasks before: {}", before);

        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                description,
                type,
                status,
                priority,
                control,
                "createdAt",
                "createdBy",
                "scheduledFor",
                "updatedAt",
                "executedAt",
                "completedAt",
                "queuedAt",
                error,
                result,
                model,
                "userId"
            FROM "Task"
            WHERE type = $1 
            AND "scheduledFor" <= $2 
            AND status = $3
            ORDER BY "scheduledFor" ASC
            "#,
        )
        .bind(TaskType::Scheduled.to_string())
        .bind(before)
        .bind(TaskStatus::Pending.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to fetch scheduled tasks: {}", e);
            DatabaseError::QueryError(e)
        })?;

        let tasks: Result<Vec<Task>, DatabaseError> = rows
            .into_iter()
            .map(|row| {
                Ok::<Task, DatabaseError>(Task {
                    id: row.get("id"),
                    description: row.get("description"),
                    task_type: row.get::<String, _>("type").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid task type".to_string())
                    })?,
                    status: row.get::<String, _>("status").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid status".to_string())
                    })?,
                    priority: row.get::<String, _>("priority").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid priority".to_string())
                    })?,
                    control: row.get::<String, _>("control").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid control".to_string())
                    })?,
                    created_at: row.get("createdAt"),
                    created_by: row.get::<String, _>("createdBy").parse().map_err(|_| {
                        DatabaseError::SerializationError("Invalid created_by".to_string())
                    })?,
                    scheduled_for: row.get("scheduledFor"),
                    updated_at: row.get("updatedAt"),
                    executed_at: row.get("executedAt"),
                    completed_at: row.get("completedAt"),
                    queued_at: row.get("queuedAt"),
                    error: row.get("error"),
                    result: row.get("result"),
                    model: row.get("model"),
                    user_id: row.get("userId"),
                })
            })
            .collect();

        let tasks = tasks?;
        debug!("Found {} scheduled tasks ready for execution", tasks.len());
        Ok(tasks)
    }

    async fn count_by_status(&self) -> Result<HashMap<TaskStatus, u64>, DatabaseError> {
        debug!("Counting tasks by status");

        let rows = sqlx::query(
            r#"
            SELECT status, COUNT(*) as count
            FROM "Task"
            GROUP BY status
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to count tasks by status: {}", e);
            DatabaseError::QueryError(e)
        })?;

        let mut counts = HashMap::new();
        for row in rows {
            let status: TaskStatus = row.get::<String, _>("status").parse().map_err(|_| {
                DatabaseError::SerializationError("Invalid status in count query".to_string())
            })?;
            counts.insert(status, row.get::<i64, _>("count") as u64);
        }

        debug!("Task counts by status: {:?}", counts);
        Ok(counts)
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use mockall::mock;

    use super::*;

    // Mock implementation for testing
    mock! {
        TaskRepo {}

        #[async_trait]
        impl TaskRepositoryTrait for TaskRepo {
            async fn create(&self, dto: &CreateTaskDto) -> Result<Task, DatabaseError>;
            async fn get_by_id(&self, id: &str) -> Result<Option<Task>, DatabaseError>;
            async fn update(&self, id: &str, dto: &UpdateTaskDto) -> Result<Option<Task>, DatabaseError>;
            async fn delete(&self, id: &str) -> Result<bool, DatabaseError>;
            async fn list(
                &self,
                filter: &TaskFilter,
                pagination: &PaginationParams,
            ) -> Result<(Vec<Task>, u64), DatabaseError>;
            async fn update_status(&self, id: &str, status: TaskStatus) -> Result<Option<Task>, DatabaseError>;
            async fn get_tasks_by_status(&self, status: TaskStatus) -> Result<Vec<Task>, DatabaseError>;
            async fn get_scheduled_tasks(&self, before: DateTime<Utc>) -> Result<Vec<Task>, DatabaseError>;
            async fn count_by_status(&self) -> Result<HashMap<TaskStatus, u64>, DatabaseError>;
        }
    }

    fn create_test_task() -> Task {
        let now = Utc::now();
        Task {
            id: Uuid::new_v4().to_string(),
            description: "Test task".to_string(),
            task_type: TaskType::Immediate,
            status: TaskStatus::Pending,
            priority: TaskPriority::Medium,
            control: Role::Assistant,
            created_at: now,
            created_by: Role::User,
            scheduled_for: None,
            updated_at: now,
            executed_at: None,
            completed_at: None,
            queued_at: None,
            error: None,
            result: None,
            model: serde_json::json!({
                "provider": "anthropic",
                "name": "claude-3-sonnet-20240229",
                "title": "Claude 3 Sonnet"
            }),
            user_id: None,
        }
    }

    fn create_test_create_dto() -> CreateTaskDto {
        CreateTaskDto {
            description: "Test task".to_string(),
            task_type: Some(TaskType::Immediate),
            scheduled_for: None,
            priority: Some(TaskPriority::Medium),
            created_by: Some(Role::User),
            user_id: None,
            model: Some(serde_json::json!({
                "provider": "anthropic",
                "name": "claude-3-sonnet-20240229",
                "title": "Claude 3 Sonnet"
            })),
            files: None,
        }
    }

    #[test]
    fn test_validate_status_transition_valid() {
        // Valid transitions
        assert!(TaskRepository::validate_status_transition(
            TaskStatus::Pending,
            TaskStatus::Running
        )
        .is_ok());

        assert!(TaskRepository::validate_status_transition(
            TaskStatus::Running,
            TaskStatus::Completed
        )
        .is_ok());

        assert!(TaskRepository::validate_status_transition(
            TaskStatus::NeedsHelp,
            TaskStatus::Running
        )
        .is_ok());
    }

    #[test]
    fn test_validate_status_transition_invalid() {
        // Invalid transitions
        assert!(TaskRepository::validate_status_transition(
            TaskStatus::Completed,
            TaskStatus::Running
        )
        .is_err());

        assert!(TaskRepository::validate_status_transition(
            TaskStatus::Pending,
            TaskStatus::Completed
        )
        .is_err());

        assert!(TaskRepository::validate_status_transition(
            TaskStatus::Failed,
            TaskStatus::Pending
        )
        .is_err());
    }

    #[test]
    fn test_build_filter_clause_empty() {
        let filter = TaskFilter::default();
        let (where_clause, params) = TaskRepository::build_filter_clause(&filter);

        assert!(where_clause.is_empty());
        assert!(params.is_empty());
    }

    #[test]
    fn test_build_filter_clause_with_status() {
        let filter = TaskFilter {
            status: Some(TaskStatus::Running),
            ..Default::default()
        };
        let (where_clause, params) = TaskRepository::build_filter_clause(&filter);

        assert!(where_clause.contains("status = $1"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_build_filter_clause_multiple_conditions() {
        let filter = TaskFilter {
            status: Some(TaskStatus::Running),
            priority: Some(TaskPriority::High),
            user_id: Some("user123".to_string()),
            ..Default::default()
        };
        let (where_clause, params) = TaskRepository::build_filter_clause(&filter);

        assert!(where_clause.contains("WHERE"));
        assert!(where_clause.contains("status = $1"));
        assert!(where_clause.contains("priority = $2"));
        assert!(where_clause.contains("\"userId\" = $3"));
        assert!(where_clause.contains("AND"));
        assert_eq!(params.len(), 3);
    }

    #[tokio::test]
    async fn test_mock_repository_create() {
        let mut mock_repo = MockTaskRepo::new();
        let test_task = create_test_task();
        let create_dto = create_test_create_dto();

        mock_repo
            .expect_create()
            .with(mockall::predicate::eq(create_dto.clone()))
            .times(1)
            .returning(move |_| Ok(test_task.clone()));

        let result = mock_repo.create(&create_dto).await;
        assert!(result.is_ok());

        let task = result.unwrap();
        assert_eq!(task.description, "Test task");
        assert_eq!(task.status, TaskStatus::Pending);
    }

    #[tokio::test]
    async fn test_mock_repository_get_by_id() {
        let mut mock_repo = MockTaskRepo::new();
        let test_task = create_test_task();
        let task_id = test_task.id.clone();

        mock_repo
            .expect_get_by_id()
            .with(mockall::predicate::eq(task_id.clone()))
            .times(1)
            .returning(move |_| Ok(Some(test_task.clone())));

        let result = mock_repo.get_by_id(&task_id).await;
        assert!(result.is_ok());

        let task = result.unwrap();
        assert!(task.is_some());
        assert_eq!(task.unwrap().id, task_id);
    }

    #[tokio::test]
    async fn test_mock_repository_update_status() {
        let mut mock_repo = MockTaskRepo::new();
        let mut test_task = create_test_task();
        test_task.status = TaskStatus::Running;
        let task_id = test_task.id.clone();

        mock_repo
            .expect_update_status()
            .with(
                mockall::predicate::eq(task_id.clone()),
                mockall::predicate::eq(TaskStatus::Running),
            )
            .times(1)
            .returning(move |_, _| Ok(Some(test_task.clone())));

        let result = mock_repo.update_status(&task_id, TaskStatus::Running).await;
        assert!(result.is_ok());

        let task = result.unwrap();
        assert!(task.is_some());
        assert_eq!(task.unwrap().status, TaskStatus::Running);
    }

    #[tokio::test]
    async fn test_mock_repository_list_with_pagination() {
        let mut mock_repo = MockTaskRepo::new();
        let test_tasks = vec![create_test_task(), create_test_task()];
        let filter = TaskFilter::default();
        let pagination = PaginationParams::default();

        mock_repo
            .expect_list()
            .with(
                mockall::predicate::eq(filter),
                mockall::predicate::eq(pagination),
            )
            .times(1)
            .returning(move |_, _| Ok((test_tasks.clone(), 2)));

        let result = mock_repo
            .list(&TaskFilter::default(), &PaginationParams::default())
            .await;
        assert!(result.is_ok());

        let (tasks, total) = result.unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(total, 2);
    }

    #[tokio::test]
    async fn test_mock_repository_delete() {
        let mut mock_repo = MockTaskRepo::new();
        let task_id = "test-task-id";

        mock_repo
            .expect_delete()
            .with(mockall::predicate::eq(task_id))
            .times(1)
            .returning(|_| Ok(true));

        let result = mock_repo.delete(task_id).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_mock_repository_get_scheduled_tasks() {
        let mut mock_repo = MockTaskRepo::new();
        let now = Utc::now();
        let mut scheduled_task = create_test_task();
        scheduled_task.task_type = TaskType::Scheduled;
        scheduled_task.scheduled_for = Some(now - Duration::hours(1));

        mock_repo
            .expect_get_scheduled_tasks()
            .with(mockall::predicate::eq(now))
            .times(1)
            .returning(move |_| Ok(vec![scheduled_task.clone()]));

        let result = mock_repo.get_scheduled_tasks(now).await;
        assert!(result.is_ok());

        let tasks = result.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task_type, TaskType::Scheduled);
    }

    #[tokio::test]
    async fn test_mock_repository_count_by_status() {
        let mut mock_repo = MockTaskRepo::new();
        let mut expected_counts = HashMap::new();
        expected_counts.insert(TaskStatus::Pending, 5);
        expected_counts.insert(TaskStatus::Running, 2);
        expected_counts.insert(TaskStatus::Completed, 10);

        mock_repo
            .expect_count_by_status()
            .times(1)
            .returning(move || Ok(expected_counts.clone()));

        let result = mock_repo.count_by_status().await;
        assert!(result.is_ok());

        let counts = result.unwrap();
        assert_eq!(counts.get(&TaskStatus::Pending), Some(&5));
        assert_eq!(counts.get(&TaskStatus::Running), Some(&2));
        assert_eq!(counts.get(&TaskStatus::Completed), Some(&10));
    }
}
