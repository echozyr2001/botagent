use std::collections::HashMap;

use bytebot_shared_rs::types::{
    api::{CreateTaskDto, PaginationParams, UpdateTaskDto},
    task::{Role, Task, TaskPriority, TaskStatus, TaskType},
};
use chrono::{DateTime, Utc};
use mockall::mock;
use tokio_test;

use crate::database::{DatabaseError, TaskFilter, TaskRepositoryTrait};

// Mock implementation for testing
mock! {
    pub TaskRepository {}

    #[async_trait::async_trait]
    impl TaskRepositoryTrait for TaskRepository {
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
}

/// Helper functions for creating test data
fn create_test_task_dto() -> CreateTaskDto {
    CreateTaskDto {
        description: "Test task description".to_string(),
        task_type: Some(TaskType::Immediate),
        priority: Some(TaskPriority::Medium),
        created_by: Some(Role::User),
        scheduled_for: None,
        model: Some(serde_json::json!({
            "provider": "anthropic",
            "name": "claude-3-sonnet-20240229",
            "title": "Claude 3 Sonnet"
        })),
        user_id: Some("user-123".to_string()),
    }
}

fn create_test_task() -> Task {
    let now = Utc::now();
    Task {
        id: "task-123".to_string(),
        description: "Test task description".to_string(),
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
        user_id: Some("user-123".to_string()),
    }
}

fn create_test_update_dto() -> UpdateTaskDto {
    UpdateTaskDto {
        status: Some(TaskStatus::Running),
        priority: Some(TaskPriority::High),
        queued_at: None,
        executed_at: Some(Utc::now()),
        completed_at: None,
    }
}

#[cfg(test)]
mod task_repository_tests {
    use super::*;
    use mockall::predicate::*;

    #[tokio::test]
    async fn test_create_task_success() {
        // Arrange
        let mut mock_repo = MockTaskRepository::new();
        let dto = create_test_task_dto();
        let expected_task = create_test_task();

        mock_repo
            .expect_create()
            .with(eq(dto.clone()))
            .times(1)
            .returning(move |_| Ok(expected_task.clone()));

        // Act
        let result = mock_repo.create(&dto).await;

        // Assert
        assert!(result.is_ok());
        let task = result.unwrap();
        assert_eq!(task.description, "Test task description");
        assert_eq!(task.task_type, TaskType::Immediate);
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.priority, TaskPriority::Medium);
    }

    #[tokio::test]
    async fn test_create_task_validation_error() {
        // Arrange
        let mut mock_repo = MockTaskRepository::new();
        let dto = CreateTaskDto {
            description: "".to_string(), // Invalid empty description
            task_type: Some(TaskType::Immediate),
            priority: Some(TaskPriority::Medium),
            created_by: Some(Role::User),
            scheduled_for: None,
            model: None,
            user_id: None,
        };

        mock_repo
            .expect_create()
            .with(eq(dto.clone()))
            .times(1)
            .returning(|_| {
                Err(DatabaseError::ValidationError(
                    "Description cannot be empty".to_string(),
                ))
            });

        // Act
        let result = mock_repo.create(&dto).await;

        // Assert
        assert!(result.is_err());
        match result.err().unwrap() {
            DatabaseError::ValidationError(msg) => {
                assert!(msg.contains("Description cannot be empty"));
            }
            _ => panic!("Expected ValidationError"),
        }
    }

    #[tokio::test]
    async fn test_create_scheduled_task_without_scheduled_for() {
        // Arrange
        let mut mock_repo = MockTaskRepository::new();
        let dto = CreateTaskDto {
            description: "Scheduled task".to_string(),
            task_type: Some(TaskType::Scheduled),
            priority: Some(TaskPriority::Medium),
            created_by: Some(Role::User),
            scheduled_for: None, // Missing required field for scheduled task
            model: None,
            user_id: None,
        };

        mock_repo
            .expect_create()
            .with(eq(dto.clone()))
            .times(1)
            .returning(|_| {
                Err(DatabaseError::ValidationError(
                    "Scheduled tasks must have scheduled_for timestamp".to_string(),
                ))
            });

        // Act
        let result = mock_repo.create(&dto).await;

        // Assert
        assert!(result.is_err());
        match result.err().unwrap() {
            DatabaseError::ValidationError(msg) => {
                assert!(msg.contains("Scheduled tasks must have scheduled_for timestamp"));
            }
            _ => panic!("Expected ValidationError"),
        }
    }

    #[tokio::test]
    async fn test_get_by_id_found() {
        // Arrange
        let mut mock_repo = MockTaskRepository::new();
        let task_id = "task-123";
        let expected_task = create_test_task();

        mock_repo
            .expect_get_by_id()
            .with(eq(task_id))
            .times(1)
            .returning(move |_| Ok(Some(expected_task.clone())));

        // Act
        let result = mock_repo.get_by_id(task_id).await;

        // Assert
        assert!(result.is_ok());
        let task = result.unwrap();
        assert!(task.is_some());
        let task = task.unwrap();
        assert_eq!(task.id, "task-123");
        assert_eq!(task.description, "Test task description");
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        // Arrange
        let mut mock_repo = MockTaskRepository::new();
        let task_id = "nonexistent-task";

        mock_repo
            .expect_get_by_id()
            .with(eq(task_id))
            .times(1)
            .returning(|_| Ok(None));

        // Act
        let result = mock_repo.get_by_id(task_id).await;

        // Assert
        assert!(result.is_ok());
        let task = result.unwrap();
        assert!(task.is_none());
    }

    #[tokio::test]
    async fn test_update_task_success() {
        // Arrange
        let mut mock_repo = MockTaskRepository::new();
        let task_id = "task-123";
        let dto = create_test_update_dto();
        let mut expected_task = create_test_task();
        expected_task.status = TaskStatus::Running;
        expected_task.priority = TaskPriority::High;

        mock_repo
            .expect_update()
            .with(eq(task_id), eq(dto.clone()))
            .times(1)
            .returning(move |_, _| Ok(Some(expected_task.clone())));

        // Act
        let result = mock_repo.update(task_id, &dto).await;

        // Assert
        assert!(result.is_ok());
        let task = result.unwrap();
        assert!(task.is_some());
        let task = task.unwrap();
        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.priority, TaskPriority::High);
    }

    #[tokio::test]
    async fn test_update_task_not_found() {
        // Arrange
        let mut mock_repo = MockTaskRepository::new();
        let task_id = "nonexistent-task";
        let dto = create_test_update_dto();

        mock_repo
            .expect_update()
            .with(eq(task_id), eq(dto.clone()))
            .times(1)
            .returning(|_, _| Ok(None));

        // Act
        let result = mock_repo.update(task_id, &dto).await;

        // Assert
        assert!(result.is_ok());
        let task = result.unwrap();
        assert!(task.is_none());
    }

    #[tokio::test]
    async fn test_update_task_invalid_status_transition() {
        // Arrange
        let mut mock_repo = MockTaskRepository::new();
        let task_id = "task-123";
        let dto = UpdateTaskDto {
            status: Some(TaskStatus::Running), // Invalid transition from Completed
            priority: None,
            queued_at: None,
            executed_at: None,
            completed_at: None,
        };

        mock_repo
            .expect_update()
            .with(eq(task_id), eq(dto.clone()))
            .times(1)
            .returning(|_, _| {
                Err(DatabaseError::InvalidStatusTransition {
                    from: TaskStatus::Completed,
                    to: TaskStatus::Running,
                })
            });

        // Act
        let result = mock_repo.update(task_id, &dto).await;

        // Assert
        assert!(result.is_err());
        match result.err().unwrap() {
            DatabaseError::InvalidStatusTransition { from, to } => {
                assert_eq!(from, TaskStatus::Completed);
                assert_eq!(to, TaskStatus::Running);
            }
            _ => panic!("Expected InvalidStatusTransition error"),
        }
    }

    #[tokio::test]
    async fn test_delete_task_success() {
        // Arrange
        let mut mock_repo = MockTaskRepository::new();
        let task_id = "task-123";

        mock_repo
            .expect_delete()
            .with(eq(task_id))
            .times(1)
            .returning(|_| Ok(true));

        // Act
        let result = mock_repo.delete(task_id).await;

        // Assert
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_delete_task_not_found() {
        // Arrange
        let mut mock_repo = MockTaskRepository::new();
        let task_id = "nonexistent-task";

        mock_repo
            .expect_delete()
            .with(eq(task_id))
            .times(1)
            .returning(|_| Ok(false));

        // Act
        let result = mock_repo.delete(task_id).await;

        // Assert
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_list_tasks_with_filter() {
        // Arrange
        let mut mock_repo = MockTaskRepository::new();
        let filter = TaskFilter {
            status: Some(TaskStatus::Pending),
            priority: Some(TaskPriority::High),
            ..Default::default()
        };
        let pagination = PaginationParams {
            page: Some(1),
            limit: Some(10),
        };
        let expected_tasks = vec![create_test_task()];
        let expected_total = 1u64;

        mock_repo
            .expect_list()
            .with(eq(filter.clone()), eq(pagination.clone()))
            .times(1)
            .returning(move |_, _| Ok((expected_tasks.clone(), expected_total)));

        // Act
        let result = mock_repo.list(&filter, &pagination).await;

        // Assert
        assert!(result.is_ok());
        let (tasks, total) = result.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(total, 1);
        assert_eq!(tasks[0].id, "task-123");
    }

    #[tokio::test]
    async fn test_list_tasks_empty_result() {
        // Arrange
        let mut mock_repo = MockTaskRepository::new();
        let filter = TaskFilter {
            status: Some(TaskStatus::Completed),
            ..Default::default()
        };
        let pagination = PaginationParams {
            page: Some(1),
            limit: Some(10),
        };

        mock_repo
            .expect_list()
            .with(eq(filter.clone()), eq(pagination.clone()))
            .times(1)
            .returning(|_, _| Ok((vec![], 0)));

        // Act
        let result = mock_repo.list(&filter, &pagination).await;

        // Assert
        assert!(result.is_ok());
        let (tasks, total) = result.unwrap();
        assert_eq!(tasks.len(), 0);
        assert_eq!(total, 0);
    }

    #[tokio::test]
    async fn test_update_status_success() {
        // Arrange
        let mut mock_repo = MockTaskRepository::new();
        let task_id = "task-123";
        let new_status = TaskStatus::Running;
        let mut expected_task = create_test_task();
        expected_task.status = new_status;
        expected_task.executed_at = Some(Utc::now());

        mock_repo
            .expect_update_status()
            .with(eq(task_id), eq(new_status))
            .times(1)
            .returning(move |_, _| Ok(Some(expected_task.clone())));

        // Act
        let result = mock_repo.update_status(task_id, new_status).await;

        // Assert
        assert!(result.is_ok());
        let task = result.unwrap();
        assert!(task.is_some());
        let task = task.unwrap();
        assert_eq!(task.status, TaskStatus::Running);
        assert!(task.executed_at.is_some());
    }

    #[tokio::test]
    async fn test_update_status_invalid_transition() {
        // Arrange
        let mut mock_repo = MockTaskRepository::new();
        let task_id = "task-123";
        let new_status = TaskStatus::Pending; // Invalid transition from Completed

        mock_repo
            .expect_update_status()
            .with(eq(task_id), eq(new_status))
            .times(1)
            .returning(|_, _| {
                Err(DatabaseError::InvalidStatusTransition {
                    from: TaskStatus::Completed,
                    to: TaskStatus::Pending,
                })
            });

        // Act
        let result = mock_repo.update_status(task_id, new_status).await;

        // Assert
        assert!(result.is_err());
        match result.err().unwrap() {
            DatabaseError::InvalidStatusTransition { from, to } => {
                assert_eq!(from, TaskStatus::Completed);
                assert_eq!(to, TaskStatus::Pending);
            }
            _ => panic!("Expected InvalidStatusTransition error"),
        }
    }

    #[tokio::test]
    async fn test_get_tasks_by_status() {
        // Arrange
        let mut mock_repo = MockTaskRepository::new();
        let status = TaskStatus::Running;
        let expected_tasks = vec![create_test_task()];

        mock_repo
            .expect_get_tasks_by_status()
            .with(eq(status))
            .times(1)
            .returning(move |_| Ok(expected_tasks.clone()));

        // Act
        let result = mock_repo.get_tasks_by_status(status).await;

        // Assert
        assert!(result.is_ok());
        let tasks = result.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, TaskStatus::Pending); // From test data
    }

    #[tokio::test]
    async fn test_get_scheduled_tasks() {
        // Arrange
        let mut mock_repo = MockTaskRepository::new();
        let before = Utc::now();
        let mut expected_task = create_test_task();
        expected_task.task_type = TaskType::Scheduled;
        expected_task.scheduled_for = Some(before - chrono::Duration::hours(1));
        let expected_tasks = vec![expected_task];

        mock_repo
            .expect_get_scheduled_tasks()
            .with(eq(before))
            .times(1)
            .returning(move |_| Ok(expected_tasks.clone()));

        // Act
        let result = mock_repo.get_scheduled_tasks(before).await;

        // Assert
        assert!(result.is_ok());
        let tasks = result.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task_type, TaskType::Scheduled);
        assert!(tasks[0].scheduled_for.is_some());
    }

    #[tokio::test]
    async fn test_count_by_status() {
        // Arrange
        let mut mock_repo = MockTaskRepository::new();
        let mut expected_counts = HashMap::new();
        expected_counts.insert(TaskStatus::Pending, 5);
        expected_counts.insert(TaskStatus::Running, 3);
        expected_counts.insert(TaskStatus::Completed, 10);

        mock_repo
            .expect_count_by_status()
            .times(1)
            .returning(move || Ok(expected_counts.clone()));

        // Act
        let result = mock_repo.count_by_status().await;

        // Assert
        assert!(result.is_ok());
        let counts = result.unwrap();
        assert_eq!(counts.get(&TaskStatus::Pending), Some(&5));
        assert_eq!(counts.get(&TaskStatus::Running), Some(&3));
        assert_eq!(counts.get(&TaskStatus::Completed), Some(&10));
    }

    /// Property-based tests for task repository
    mod property_tests {
        use super::*;

        #[tokio::test]
        async fn test_task_status_transition_validation() {
            // Test all valid status transitions
            let valid_transitions = vec![
                (TaskStatus::Pending, TaskStatus::Running),
                (TaskStatus::Pending, TaskStatus::Cancelled),
                (TaskStatus::Running, TaskStatus::NeedsHelp),
                (TaskStatus::Running, TaskStatus::NeedsReview),
                (TaskStatus::Running, TaskStatus::Completed),
                (TaskStatus::Running, TaskStatus::Failed),
                (TaskStatus::Running, TaskStatus::Cancelled),
                (TaskStatus::NeedsHelp, TaskStatus::Running),
                (TaskStatus::NeedsHelp, TaskStatus::Cancelled),
                (TaskStatus::NeedsReview, TaskStatus::Running),
                (TaskStatus::NeedsReview, TaskStatus::Completed),
                (TaskStatus::NeedsReview, TaskStatus::Cancelled),
            ];

            for (from_status, to_status) in valid_transitions {
                let mut mock_repo = MockTaskRepository::new();
                let task_id = "task-123";

                mock_repo
                    .expect_update_status()
                    .with(eq(task_id), eq(to_status))
                    .times(1)
                    .returning(move |_, _| {
                        let mut task = create_test_task();
                        task.status = to_status;
                        Ok(Some(task))
                    });

                let result = mock_repo.update_status(task_id, to_status).await;
                assert!(
                    result.is_ok(),
                    "Valid transition from {:?} to {:?} should succeed",
                    from_status,
                    to_status
                );
            }
        }

        #[tokio::test]
        async fn test_task_status_invalid_transitions() {
            // Test invalid transitions from terminal states
            let invalid_transitions = vec![
                (TaskStatus::Completed, TaskStatus::Pending),
                (TaskStatus::Completed, TaskStatus::Running),
                (TaskStatus::Failed, TaskStatus::Pending),
                (TaskStatus::Failed, TaskStatus::Running),
                (TaskStatus::Cancelled, TaskStatus::Pending),
                (TaskStatus::Cancelled, TaskStatus::Running),
            ];

            for (from_status, to_status) in invalid_transitions {
                let mut mock_repo = MockTaskRepository::new();
                let task_id = "task-123";

                mock_repo
                    .expect_update_status()
                    .with(eq(task_id), eq(to_status))
                    .times(1)
                    .returning(move |_, _| {
                        Err(DatabaseError::InvalidStatusTransition {
                            from: from_status,
                            to: to_status,
                        })
                    });

                let result = mock_repo.update_status(task_id, to_status).await;
                assert!(
                    result.is_err(),
                    "Invalid transition from {:?} to {:?} should fail",
                    from_status,
                    to_status
                );
            }
        }

        #[tokio::test]
        async fn test_pagination_consistency() {
            let mut mock_repo = MockTaskRepository::new();
            let filter = TaskFilter::default();

            // Test different pagination parameters
            let pagination_tests = vec![
                (PaginationParams { page: Some(1), limit: Some(10) }, 10),
                (PaginationParams { page: Some(2), limit: Some(5) }, 5),
                (PaginationParams { page: Some(1), limit: Some(20) }, 20),
                (PaginationParams { page: None, limit: None }, 20), // Default values
            ];

            for (pagination, expected_limit) in pagination_tests {
                mock_repo
                    .expect_list()
                    .with(eq(filter.clone()), eq(pagination.clone()))
                    .times(1)
                    .returning(move |_, _| {
                        let tasks = (0..expected_limit.min(5))
                            .map(|i| {
                                let mut task = create_test_task();
                                task.id = format!("task-{}", i);
                                task
                            })
                            .collect();
                        Ok((tasks, 100)) // Total count
                    });

                let result = mock_repo.list(&filter, &pagination).await;
                assert!(result.is_ok());
                let (tasks, total) = result.unwrap();
                assert!(tasks.len() <= expected_limit as usize);
                assert_eq!(total, 100);
            }
        }
    }
}