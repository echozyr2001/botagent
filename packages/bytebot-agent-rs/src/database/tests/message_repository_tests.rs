#[cfg(test)]
mod tests {
    use crate::database::{
        message_repository::{CreateMessageDto, MessageRepository, MessageRepositoryTrait, UpdateMessageDto},
        task_repository::{TaskRepository, TaskRepositoryTrait},
        tests::{cleanup_test_data, create_test_pool},
        DatabaseError,
    };
    use bytebot_shared_rs::types::{
        api::{CreateTaskDto, PaginationParams},
        message::MessageContentBlock,
        task::{Role, TaskType, TaskPriority},
    };

    async fn create_test_task(pool: &sqlx::PgPool) -> String {
        let task_repo = TaskRepository::new(pool.clone());
        let dto = CreateTaskDto {
            description: "Test task for message tests".to_string(),
            task_type: Some(TaskType::Immediate),
            scheduled_for: None,
            priority: Some(TaskPriority::Medium),
            created_by: Some(Role::User),
            user_id: Some("test-user-id".to_string()),
            model: None,
            files: None,
        };
        
        let task = task_repo.create(&dto).await.expect("Failed to create test task");
        task.id
    }

    #[tokio::test]
    async fn test_create_message() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let task_id = create_test_task(&pool).await;
        let repo = MessageRepository::new(pool.clone());

        let content = vec![
            MessageContentBlock::text("Hello, world!"),
            MessageContentBlock::tool_use("test_tool", "tool-123", serde_json::json!({"param": "value"})),
        ];

        let dto = CreateMessageDto {
            content: content.clone(),
            role: Role::User,
            task_id: task_id.clone(),
            user_id: Some("test-user".to_string()),
            summary_id: None,
        };

        let result = repo.create(&dto).await;
        assert!(result.is_ok());

        let message = result.unwrap();
        assert_eq!(message.role, Role::User);
        assert_eq!(message.task_id, task_id);
        assert_eq!(message.user_id, Some("test-user".to_string()));
        
        // Verify content was serialized correctly
        let content_blocks = message.get_content_blocks().expect("Failed to deserialize content");
        assert_eq!(content_blocks.len(), 2);
        
        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_create_message_validation_empty_content() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let task_id = create_test_task(&pool).await;
        let repo = MessageRepository::new(pool.clone());

        let dto = CreateMessageDto {
            content: vec![], // Empty content should fail
            role: Role::User,
            task_id,
            user_id: None,
            summary_id: None,
        };

        let result = repo.create(&dto).await;
        assert!(result.is_err());
        
        if let Err(DatabaseError::ValidationError(msg)) = result {
            assert!(msg.contains("cannot be empty"));
        } else {
            panic!("Expected ValidationError for empty content");
        }

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_create_message_validation_empty_text() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let task_id = create_test_task(&pool).await;
        let repo = MessageRepository::new(pool.clone());

        let content = vec![MessageContentBlock::text("")]; // Empty text should fail

        let dto = CreateMessageDto {
            content,
            role: Role::User,
            task_id,
            user_id: None,
            summary_id: None,
        };

        let result = repo.create(&dto).await;
        assert!(result.is_err());
        
        if let Err(DatabaseError::ValidationError(msg)) = result {
            assert!(msg.contains("cannot be empty"));
        } else {
            panic!("Expected ValidationError for empty text content");
        }

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_get_message_by_id() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let task_id = create_test_task(&pool).await;
        let repo = MessageRepository::new(pool.clone());

        // Create a message first
        let content = vec![MessageContentBlock::text("Test message")];
        let dto = CreateMessageDto {
            content,
            role: Role::Assistant,
            task_id,
            user_id: None,
            summary_id: None,
        };

        let created_message = repo.create(&dto).await.expect("Failed to create message");

        // Test getting by ID
        let result = repo.get_by_id(&created_message.id).await;
        assert!(result.is_ok());

        let message = result.unwrap();
        assert!(message.is_some());
        
        let message = message.unwrap();
        assert_eq!(message.id, created_message.id);
        assert_eq!(message.role, Role::Assistant);

        // Test getting non-existent message
        let result = repo.get_by_id("non-existent-id").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_update_message() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let task_id = create_test_task(&pool).await;
        let repo = MessageRepository::new(pool.clone());

        // Create a message first
        let content = vec![MessageContentBlock::text("Original message")];
        let dto = CreateMessageDto {
            content,
            role: Role::User,
            task_id,
            user_id: None,
            summary_id: None,
        };

        let created_message = repo.create(&dto).await.expect("Failed to create message");

        // Update the message
        let new_content = vec![MessageContentBlock::text("Updated message")];
        let update_dto = UpdateMessageDto {
            content: Some(new_content),
            summary_id: Some("test-summary-id".to_string()),
        };

        let result = repo.update(&created_message.id, &update_dto).await;
        assert!(result.is_ok());

        let updated_message = result.unwrap();
        assert!(updated_message.is_some());
        
        let updated_message = updated_message.unwrap();
        assert_eq!(updated_message.id, created_message.id);
        assert_eq!(updated_message.summary_id, Some("test-summary-id".to_string()));
        assert!(updated_message.updated_at > created_message.updated_at);

        // Verify content was updated
        let content_blocks = updated_message.get_content_blocks().expect("Failed to deserialize content");
        assert_eq!(content_blocks.len(), 1);
        if let Some(text) = content_blocks[0].as_text() {
            assert_eq!(text, "Updated message");
        } else {
            panic!("Expected text content block");
        }

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_delete_message() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let task_id = create_test_task(&pool).await;
        let repo = MessageRepository::new(pool.clone());

        // Create a message first
        let content = vec![MessageContentBlock::text("Message to delete")];
        let dto = CreateMessageDto {
            content,
            role: Role::User,
            task_id,
            user_id: None,
            summary_id: None,
        };

        let created_message = repo.create(&dto).await.expect("Failed to create message");

        // Delete the message
        let result = repo.delete(&created_message.id).await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Verify message is deleted
        let result = repo.get_by_id(&created_message.id).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        // Test deleting non-existent message
        let result = repo.delete("non-existent-id").await;
        assert!(result.is_ok());
        assert!(!result.unwrap());

        cleanup_test_data(&pool).await;
    }    #[tokio::test]
    async fn test_get_messages_by_task_id() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let task_id = create_test_task(&pool).await;
        let repo = MessageRepository::new(pool.clone());

        // Create multiple messages for the task
        for i in 0..3 {
            let content = vec![MessageContentBlock::text(&format!("Message {}", i))];
            let dto = CreateMessageDto {
                content,
                role: if i % 2 == 0 { Role::User } else { Role::Assistant },
                task_id: task_id.clone(),
                user_id: None,
                summary_id: None,
            };
            repo.create(&dto).await.expect("Failed to create message");
        }

        // Test getting all messages for task
        let result = repo.get_by_task_id(&task_id).await;
        assert!(result.is_ok());

        let messages = result.unwrap();
        assert_eq!(messages.len(), 3);
        
        // Messages should be ordered by creation time (ASC)
        for (i, message) in messages.iter().enumerate() {
            assert_eq!(message.task_id, task_id);
            let content_blocks = message.get_content_blocks().expect("Failed to deserialize content");
            if let Some(text) = content_blocks[0].as_text() {
                assert_eq!(text, format!("Message {}", i));
            }
        }

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_get_messages_by_task_id_paginated() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let task_id = create_test_task(&pool).await;
        let repo = MessageRepository::new(pool.clone());

        // Create 5 messages for the task
        for i in 0..5 {
            let content = vec![MessageContentBlock::text(&format!("Message {}", i))];
            let dto = CreateMessageDto {
                content,
                role: Role::User,
                task_id: task_id.clone(),
                user_id: None,
                summary_id: None,
            };
            repo.create(&dto).await.expect("Failed to create message");
        }

        // Test pagination
        let pagination = PaginationParams {
            page: Some(1),
            limit: Some(2),
        };

        let result = repo.get_by_task_id_paginated(&task_id, &pagination).await;
        assert!(result.is_ok());

        let (messages, total_count) = result.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(total_count, 5);

        // Test second page
        let pagination = PaginationParams {
            page: Some(2),
            limit: Some(2),
        };

        let result = repo.get_by_task_id_paginated(&task_id, &pagination).await;
        assert!(result.is_ok());

        let (messages, total_count) = result.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(total_count, 5);

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_delete_messages_by_task_id() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let task_id = create_test_task(&pool).await;
        let repo = MessageRepository::new(pool.clone());

        // Create multiple messages for the task
        for i in 0..3 {
            let content = vec![MessageContentBlock::text(&format!("Message {}", i))];
            let dto = CreateMessageDto {
                content,
                role: Role::User,
                task_id: task_id.clone(),
                user_id: None,
                summary_id: None,
            };
            repo.create(&dto).await.expect("Failed to create message");
        }

        // Verify messages exist
        let messages = repo.get_by_task_id(&task_id).await.expect("Failed to get messages");
        assert_eq!(messages.len(), 3);

        // Delete all messages for the task
        let result = repo.delete_by_task_id(&task_id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3);

        // Verify messages are deleted
        let messages = repo.get_by_task_id(&task_id).await.expect("Failed to get messages");
        assert_eq!(messages.len(), 0);

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_count_messages_by_task_id() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let task_id = create_test_task(&pool).await;
        let repo = MessageRepository::new(pool.clone());

        // Initially no messages
        let count = repo.count_by_task_id(&task_id).await.expect("Failed to count messages");
        assert_eq!(count, 0);

        // Create some messages
        for i in 0..4 {
            let content = vec![MessageContentBlock::text(&format!("Message {}", i))];
            let dto = CreateMessageDto {
                content,
                role: Role::User,
                task_id: task_id.clone(),
                user_id: None,
                summary_id: None,
            };
            repo.create(&dto).await.expect("Failed to create message");
        }

        // Count should be 4
        let count = repo.count_by_task_id(&task_id).await.expect("Failed to count messages");
        assert_eq!(count, 4);

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_get_latest_messages_by_task_id() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let task_id = create_test_task(&pool).await;
        let repo = MessageRepository::new(pool.clone());

        // Create 5 messages for the task
        for i in 0..5 {
            let content = vec![MessageContentBlock::text(&format!("Message {}", i))];
            let dto = CreateMessageDto {
                content,
                role: Role::User,
                task_id: task_id.clone(),
                user_id: None,
                summary_id: None,
            };
            repo.create(&dto).await.expect("Failed to create message");
            
            // Small delay to ensure different timestamps
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        // Get latest 3 messages
        let result = repo.get_latest_by_task_id(&task_id, 3).await;
        assert!(result.is_ok());

        let messages = result.unwrap();
        assert_eq!(messages.len(), 3);
        
        // Messages should be ordered by creation time DESC (latest first)
        for (i, message) in messages.iter().enumerate() {
            let content_blocks = message.get_content_blocks().expect("Failed to deserialize content");
            if let Some(text) = content_blocks[0].as_text() {
                // Latest messages should be 4, 3, 2 (in that order)
                assert_eq!(text, format!("Message {}", 4 - i));
            }
        }

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_message_content_validation() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let task_id = create_test_task(&pool).await;
        let repo = MessageRepository::new(pool.clone());

        // Test tool use validation - empty name should fail
        let content = vec![MessageContentBlock::tool_use("", "tool-123", serde_json::json!({}))];
        let dto = CreateMessageDto {
            content,
            role: Role::User,
            task_id: task_id.clone(),
            user_id: None,
            summary_id: None,
        };

        let result = repo.create(&dto).await;
        assert!(result.is_err());

        // Test tool result validation - empty tool_use_id should fail
        let content = vec![MessageContentBlock::tool_result("", vec![])];
        let dto = CreateMessageDto {
            content,
            role: Role::User,
            task_id,
            user_id: None,
            summary_id: None,
        };

        let result = repo.create(&dto).await;
        assert!(result.is_err());

        cleanup_test_data(&pool).await;
    }
}