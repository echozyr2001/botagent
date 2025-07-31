use std::{sync::Arc, time::Duration};

use bytebot_agent_rs::{
    ai::UnifiedAIService,
    auth::{AuthService, AuthServiceTrait},
    config::Config,
    database::DatabaseManager,
    server::{create_app, AppState},
    websocket::WebSocketGateway,
};
use bytebot_shared_rs::types::{Message, Role, Task, TaskPriority, TaskStatus, TaskType};
use serde_json::json;
use tokio::time::timeout;

/// Create a test task for WebSocket testing
fn create_test_task(id: &str) -> Task {
    Task {
        id: id.to_string(),
        description: "Test task".to_string(),
        task_type: TaskType::Immediate,
        status: TaskStatus::Pending,
        priority: TaskPriority::Medium,
        control: Role::Assistant,
        created_at: chrono::Utc::now(),
        created_by: Role::User,
        scheduled_for: None,
        updated_at: chrono::Utc::now(),
        executed_at: None,
        completed_at: None,
        queued_at: None,
        error: None,
        result: None,
        model: json!({"provider": "test", "model": "test-model"}),
        user_id: None,
    }
}

/// Create a test message for WebSocket testing
fn create_test_message(id: &str, task_id: &str) -> Message {
    Message {
        id: id.to_string(),
        content: json!([{"type": "text", "text": "Test message"}]),
        role: Role::User,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        task_id: task_id.to_string(),
        summary_id: None,
        user_id: None,
    }
}

/// Integration test to verify WebSocket server functionality
#[tokio::test]
async fn test_websocket_server_integration() {
    // Create test configuration
    let config = Arc::new(Config::default());

    // Create AI service
    let ai_service = Arc::new(UnifiedAIService::new(&config));

    // Create WebSocket gateway
    let websocket_gateway = Arc::new(WebSocketGateway::new());

    // Verify gateway creation
    let stats = websocket_gateway.get_connection_stats().await;
    assert_eq!(stats.total_connections, 0);
    assert_eq!(stats.total_rooms, 0);

    // Create a minimal database manager for testing (will fail but that's ok for this test)
    let database_url = "postgresql://localhost:5432/test_nonexistent";
    if let Ok(db) = DatabaseManager::new(database_url).await {
        let auth_service: Arc<dyn AuthServiceTrait> = Arc::new(AuthService::new(
            db.get_pool(),
            config.jwt_secret.clone(),
            config.auth_enabled,
        ));

        let app_state = AppState {
            config,
            db: Arc::new(db),
            ai_service,
            auth_service,
            websocket_gateway: websocket_gateway.clone(),
        };

        // Create the app with WebSocket integration
        let _app = create_app(app_state);

        // Verify the WebSocket gateway is properly integrated
        let stats = websocket_gateway.get_connection_stats().await;
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.total_rooms, 0);
    }

    // Test WebSocket gateway methods
    let test_task_id = "test-task-123";
    let test_task = create_test_task(test_task_id);
    let test_message = create_test_message("msg-123", test_task_id);

    // Test emit methods (these should not panic)
    websocket_gateway.emit_task_created(&test_task).await;
    websocket_gateway
        .emit_task_update(test_task_id, &test_task)
        .await;
    websocket_gateway
        .emit_new_message(test_task_id, &test_message)
        .await;
    websocket_gateway.emit_task_deleted(test_task_id).await;

    // Test broadcast methods
    websocket_gateway
        .broadcast_global("test_event", json!({"test": "data"}))
        .await;
    websocket_gateway
        .broadcast_to_task(test_task_id, "test_event", json!({"test": "data"}))
        .await;
}

/// Test WebSocket gateway creation and basic functionality
#[tokio::test]
async fn test_websocket_gateway_functionality() {
    let gateway = WebSocketGateway::new();

    // Test connection stats
    let stats = gateway.get_connection_stats().await;
    assert_eq!(stats.total_connections, 0);
    assert_eq!(stats.total_rooms, 0);

    // Test layer method
    let _layer = gateway.layer();

    // Test that we can create multiple gateways
    let gateway2 = WebSocketGateway::new();
    let stats2 = gateway2.get_connection_stats().await;
    assert_eq!(stats2.total_connections, 0);
    assert_eq!(stats2.total_rooms, 0);
}

/// Test WebSocket event handlers with mock client interactions
#[tokio::test]
async fn test_websocket_event_handlers() {
    let gateway = WebSocketGateway::new();

    // Test initial state
    let stats = gateway.get_connection_stats().await;
    assert_eq!(stats.total_connections, 0);
    assert_eq!(stats.total_rooms, 0);

    // Test that event emitters work without connected clients (should not panic)
    let test_task = create_test_task("test-task-456");
    let test_message = create_test_message("msg-456", "test-task-456");

    // These should complete without error even with no connected clients
    gateway.emit_task_created(&test_task).await;
    gateway.emit_task_update("test-task-456", &test_task).await;
    gateway
        .emit_new_message("test-task-456", &test_message)
        .await;
    gateway.emit_task_deleted("test-task-456").await;

    // Test broadcast methods
    gateway
        .broadcast_global("global_event", json!({"data": "test"}))
        .await;
    gateway
        .broadcast_to_task("test-task-456", "task_event", json!({"data": "test"}))
        .await;
}

/// Test WebSocket connection management functionality
#[tokio::test]
async fn test_websocket_connection_management() {
    let gateway = WebSocketGateway::new();

    // Test connection statistics
    let stats = gateway.get_connection_stats().await;
    assert_eq!(stats.total_connections, 0);
    assert_eq!(stats.total_rooms, 0);
    assert!(stats.rooms_with_clients.is_empty());

    // Test that we can get the Socket.IO layer
    let _layer = gateway.layer();

    // Test that we can get the IO instance
    let _io = gateway.io();
}

/// Test WebSocket event emitters with different task scenarios
#[tokio::test]
async fn test_websocket_event_emitters() {
    let gateway = WebSocketGateway::new();

    // Test emit_task_created
    let task1 = create_test_task("task-001");
    gateway.emit_task_created(&task1).await;

    // Test emit_task_update with different task states
    let mut task2 = create_test_task("task-002");
    task2.status = TaskStatus::Running;
    gateway.emit_task_update("task-002", &task2).await;

    task2.status = TaskStatus::Completed;
    gateway.emit_task_update("task-002", &task2).await;

    task2.status = TaskStatus::Failed;
    task2.error = Some("Test error".to_string());
    gateway.emit_task_update("task-002", &task2).await;

    // Test emit_new_message with different message types
    let user_message = Message {
        id: "msg-001".to_string(),
        content: json!([{"type": "text", "text": "User message"}]),
        role: Role::User,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        task_id: "task-002".to_string(),
        summary_id: None,
        user_id: Some("user-123".to_string()),
    };
    gateway.emit_new_message("task-002", &user_message).await;

    let assistant_message = Message {
        id: "msg-002".to_string(),
        content: json!([
            {"type": "text", "text": "Assistant response"},
            {"type": "tool_use", "id": "tool-123", "name": "screenshot", "input": {}}
        ]),
        role: Role::Assistant,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        task_id: "task-002".to_string(),
        summary_id: None,
        user_id: None,
    };
    gateway
        .emit_new_message("task-002", &assistant_message)
        .await;

    // Test emit_task_deleted
    gateway.emit_task_deleted("task-002").await;
}

/// Test WebSocket broadcast functionality
#[tokio::test]
async fn test_websocket_broadcast_functionality() {
    let gateway = WebSocketGateway::new();

    // Test global broadcasts
    gateway
        .broadcast_global(
            "system_status",
            json!({
                "status": "healthy",
                "timestamp": chrono::Utc::now()
            }),
        )
        .await;

    gateway
        .broadcast_global(
            "maintenance_mode",
            json!({
                "enabled": false,
                "message": "System is operational"
            }),
        )
        .await;

    // Test task-specific broadcasts
    let task_id = "broadcast-test-task";

    gateway
        .broadcast_to_task(
            task_id,
            "task_progress",
            json!({
                "progress": 50,
                "step": "Processing data"
            }),
        )
        .await;

    gateway
        .broadcast_to_task(
            task_id,
            "task_warning",
            json!({
                "warning": "Low disk space",
                "severity": "medium"
            }),
        )
        .await;

    gateway
        .broadcast_to_task(
            task_id,
            "task_completion",
            json!({
                "completed": true,
                "result": "Success"
            }),
        )
        .await;
}

/// Test WebSocket error handling scenarios
#[tokio::test]
async fn test_websocket_error_handling() {
    let gateway = WebSocketGateway::new();

    // Test emitting to non-existent task rooms (should not panic)
    let non_existent_task = create_test_task("non-existent-task");
    gateway
        .emit_task_update("non-existent-task", &non_existent_task)
        .await;

    let non_existent_message = create_test_message("msg-999", "non-existent-task");
    gateway
        .emit_new_message("non-existent-task", &non_existent_message)
        .await;

    // Test broadcasting to empty rooms (should not panic)
    gateway
        .broadcast_to_task("empty-room", "test_event", json!({"test": true}))
        .await;

    // Test with invalid JSON data (should handle gracefully)
    gateway.broadcast_global("test_event", json!(null)).await;
    gateway
        .broadcast_to_task("test-task", "test_event", json!({}))
        .await;
}

/// Test WebSocket message format compatibility with TypeScript implementation
#[tokio::test]
async fn test_websocket_message_format_compatibility() {
    use bytebot_agent_rs::websocket::events::ServerMessage;

    let test_task = create_test_task("format-test-123");
    let test_message = create_test_message("msg-format-123", "format-test-123");

    // Test ServerMessage serialization matches expected format
    let task_created_msg = ServerMessage::TaskCreated {
        task: test_task.clone(),
    };
    let serialized = serde_json::to_string(&task_created_msg).expect("Should serialize");
    assert!(serialized.contains("TaskCreated"));
    assert!(serialized.contains("format-test-123"));

    let task_updated_msg = ServerMessage::TaskUpdated {
        task: test_task.clone(),
    };
    let serialized = serde_json::to_string(&task_updated_msg).expect("Should serialize");
    assert!(serialized.contains("TaskUpdated"));

    let new_message_msg = ServerMessage::NewMessage {
        message: test_message,
    };
    let serialized = serde_json::to_string(&new_message_msg).expect("Should serialize");
    assert!(serialized.contains("NewMessage"));
    assert!(serialized.contains("msg-format-123"));

    let task_deleted_msg = ServerMessage::TaskDeleted {
        task_id: "format-test-123".to_string(),
    };
    let serialized = serde_json::to_string(&task_deleted_msg).expect("Should serialize");
    assert!(serialized.contains("TaskDeleted"));
    assert!(serialized.contains("format-test-123"));

    let task_joined_msg = ServerMessage::TaskJoined {
        task_id: "format-test-123".to_string(),
    };
    let serialized = serde_json::to_string(&task_joined_msg).expect("Should serialize");
    assert!(serialized.contains("TaskJoined"));

    let task_left_msg = ServerMessage::TaskLeft {
        task_id: "format-test-123".to_string(),
    };
    let serialized = serde_json::to_string(&task_left_msg).expect("Should serialize");
    assert!(serialized.contains("TaskLeft"));

    let error_msg = ServerMessage::Error {
        message: "Test error message".to_string(),
    };
    let serialized = serde_json::to_string(&error_msg).expect("Should serialize");
    assert!(serialized.contains("Error"));
    assert!(serialized.contains("Test error message"));
}

/// Test WebSocket event serialization and deserialization
#[tokio::test]
async fn test_websocket_event_serialization() {
    use bytebot_agent_rs::websocket::events::{ClientMessage, ServerMessage};

    let test_task = create_test_task("serialization-test-123");

    // Test ServerMessage serialization
    let messages = vec![
        ServerMessage::TaskCreated {
            task: test_task.clone(),
        },
        ServerMessage::TaskUpdated {
            task: test_task.clone(),
        },
        ServerMessage::TaskDeleted {
            task_id: "serialization-test-123".to_string(),
        },
        ServerMessage::TaskJoined {
            task_id: "serialization-test-123".to_string(),
        },
        ServerMessage::TaskLeft {
            task_id: "serialization-test-123".to_string(),
        },
        ServerMessage::Error {
            message: "Test error".to_string(),
        },
    ];

    for message in messages {
        let serialized = serde_json::to_string(&message).expect("Should serialize");
        assert!(!serialized.is_empty());

        // Verify the serialized message contains expected structure
        let parsed: serde_json::Value = serde_json::from_str(&serialized).expect("Should parse");
        assert!(parsed.get("type").is_some());
        assert!(parsed.get("data").is_some());
    }

    // Test ClientMessage deserialization
    let join_task_json = json!({
        "type": "JoinTask",
        "data": {
            "task_id": "test-task-123"
        }
    });

    let client_message: Result<ClientMessage, _> = serde_json::from_value(join_task_json);
    assert!(client_message.is_ok());

    let leave_task_json = json!({
        "type": "LeaveTask",
        "data": {
            "task_id": "test-task-123"
        }
    });

    let client_message: Result<ClientMessage, _> = serde_json::from_value(leave_task_json);
    assert!(client_message.is_ok());
}

/// Mock WebSocket client for testing
struct MockWebSocketClient {
    id: String,
    connected_rooms: Vec<String>,
}

impl MockWebSocketClient {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            connected_rooms: Vec::new(),
        }
    }

    fn join_room(&mut self, room: &str) {
        if !self.connected_rooms.contains(&room.to_string()) {
            self.connected_rooms.push(room.to_string());
        }
    }

    fn leave_room(&mut self, room: &str) {
        self.connected_rooms.retain(|r| r != room);
    }

    fn is_in_room(&self, room: &str) -> bool {
        self.connected_rooms.contains(&room.to_string())
    }
}

/// Test WebSocket functionality with mock clients
#[tokio::test]
async fn test_websocket_with_mock_clients() {
    let gateway = WebSocketGateway::new();

    // Create mock clients
    let mut client1 = MockWebSocketClient::new("client-001");
    let mut client2 = MockWebSocketClient::new("client-002");
    let mut client3 = MockWebSocketClient::new("client-003");

    // Simulate clients joining different task rooms
    let task_id_1 = "task-alpha";
    let task_id_2 = "task-beta";

    // Client 1 and 2 join task-alpha
    client1.join_room(&format!("task_{task_id_1}"));
    client2.join_room(&format!("task_{task_id_1}"));

    // Client 3 joins task-beta
    client3.join_room(&format!("task_{task_id_2}"));

    // Verify room memberships
    assert!(client1.is_in_room(&format!("task_{task_id_1}")));
    assert!(client2.is_in_room(&format!("task_{task_id_1}")));
    assert!(client3.is_in_room(&format!("task_{task_id_2}")));
    assert!(!client3.is_in_room(&format!("task_{task_id_1}")));

    // Test task-specific events
    let task_alpha = create_test_task(task_id_1);
    let task_beta = create_test_task(task_id_2);

    // Emit task updates (would reach clients in respective rooms)
    gateway.emit_task_update(task_id_1, &task_alpha).await;
    gateway.emit_task_update(task_id_2, &task_beta).await;

    // Test message events
    let message_alpha = create_test_message("msg-alpha-1", task_id_1);
    let message_beta = create_test_message("msg-beta-1", task_id_2);

    gateway.emit_new_message(task_id_1, &message_alpha).await;
    gateway.emit_new_message(task_id_2, &message_beta).await;

    // Test global events (would reach all connected clients)
    gateway.emit_task_created(&task_alpha).await;
    gateway.emit_task_created(&task_beta).await;

    // Simulate client leaving a room
    client1.leave_room(&format!("task_{task_id_1}"));
    assert!(!client1.is_in_room(&format!("task_{task_id_1}")));
    assert!(client2.is_in_room(&format!("task_{task_id_1}"))); // Still in room

    // Test task deletion events
    gateway.emit_task_deleted(task_id_1).await;
    gateway.emit_task_deleted(task_id_2).await;
}

/// Test WebSocket event flow simulation
#[tokio::test]
async fn test_websocket_event_flow_simulation() {
    let gateway = WebSocketGateway::new();

    // Simulate a complete task lifecycle with WebSocket events
    let task_id = "lifecycle-task-001";
    let mut task = create_test_task(task_id);

    // 1. Task created
    gateway.emit_task_created(&task).await;

    // 2. Task status updates
    task.status = TaskStatus::Running;
    task.updated_at = chrono::Utc::now();
    gateway.emit_task_update(task_id, &task).await;

    // 3. Messages during execution
    let user_message = create_test_message("msg-001", task_id);
    gateway.emit_new_message(task_id, &user_message).await;

    let assistant_message = Message {
        id: "msg-002".to_string(),
        content: json!([{"type": "text", "text": "Processing your request..."}]),
        role: Role::Assistant,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        task_id: task_id.to_string(),
        summary_id: None,
        user_id: None,
    };
    gateway.emit_new_message(task_id, &assistant_message).await;

    // 4. Task progress updates
    gateway
        .broadcast_to_task(
            task_id,
            "task_progress",
            json!({
                "progress": 25,
                "step": "Initializing"
            }),
        )
        .await;

    gateway
        .broadcast_to_task(
            task_id,
            "task_progress",
            json!({
                "progress": 50,
                "step": "Processing"
            }),
        )
        .await;

    gateway
        .broadcast_to_task(
            task_id,
            "task_progress",
            json!({
                "progress": 75,
                "step": "Finalizing"
            }),
        )
        .await;

    // 5. Task completion
    task.status = TaskStatus::Completed;
    task.completed_at = Some(chrono::Utc::now());
    task.result = Some(json!({"success": true, "output": "Task completed successfully"}));
    gateway.emit_task_update(task_id, &task).await;

    // 6. Final completion message
    let completion_message = Message {
        id: "msg-003".to_string(),
        content: json!([{"type": "text", "text": "Task completed successfully!"}]),
        role: Role::Assistant,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        task_id: task_id.to_string(),
        summary_id: None,
        user_id: None,
    };
    gateway.emit_new_message(task_id, &completion_message).await;
}

/// Test WebSocket error scenarios and recovery
#[tokio::test]
async fn test_websocket_error_scenarios() {
    let gateway = WebSocketGateway::new();

    // Test task failure scenario
    let task_id = "error-task-001";
    let mut task = create_test_task(task_id);

    // Task starts normally
    gateway.emit_task_created(&task).await;

    task.status = TaskStatus::Running;
    gateway.emit_task_update(task_id, &task).await;

    // Error occurs during execution
    let error_message = Message {
        id: "msg-error-001".to_string(),
        content: json!([{"type": "text", "text": "An error occurred during processing"}]),
        role: Role::Assistant,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        task_id: task_id.to_string(),
        summary_id: None,
        user_id: None,
    };
    gateway.emit_new_message(task_id, &error_message).await;

    // Task fails
    task.status = TaskStatus::Failed;
    task.error = Some("Processing failed due to invalid input".to_string());
    task.completed_at = Some(chrono::Utc::now());
    gateway.emit_task_update(task_id, &task).await;

    // Test cancellation scenario
    let cancel_task_id = "cancel-task-001";
    let mut cancel_task = create_test_task(cancel_task_id);

    gateway.emit_task_created(&cancel_task).await;

    cancel_task.status = TaskStatus::Running;
    gateway.emit_task_update(cancel_task_id, &cancel_task).await;

    // User cancels the task
    cancel_task.status = TaskStatus::Cancelled;
    cancel_task.completed_at = Some(chrono::Utc::now());
    gateway.emit_task_update(cancel_task_id, &cancel_task).await;

    let cancel_message = Message {
        id: "msg-cancel-001".to_string(),
        content: json!([{"type": "text", "text": "Task was cancelled by user"}]),
        role: Role::Assistant,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        task_id: cancel_task_id.to_string(),
        summary_id: None,
        user_id: None,
    };
    gateway
        .emit_new_message(cancel_task_id, &cancel_message)
        .await;
}

/// Test WebSocket performance with multiple concurrent events
#[tokio::test]
async fn test_websocket_concurrent_events() {
    let gateway = Arc::new(WebSocketGateway::new());

    // Create multiple tasks concurrently
    let mut handles = Vec::new();

    for i in 0..10 {
        let gateway_clone = gateway.clone();
        let handle = tokio::spawn(async move {
            let task_id = format!("concurrent-task-{i:03}");
            let task = create_test_task(&task_id);

            // Emit task created
            gateway_clone.emit_task_created(&task).await;

            // Emit multiple messages
            for j in 0..5 {
                let message = Message {
                    id: format!("msg-{i}-{j}"),
                    content: json!([{"type": "text", "text": format!("Message {} for task {}", j, i)}]),
                    role: if j % 2 == 0 {
                        Role::User
                    } else {
                        Role::Assistant
                    },
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                    task_id: task_id.clone(),
                    summary_id: None,
                    user_id: None,
                };
                gateway_clone.emit_new_message(&task_id, &message).await;
            }

            // Update task status
            let mut updated_task = task;
            updated_task.status = TaskStatus::Completed;
            gateway_clone
                .emit_task_update(&task_id, &updated_task)
                .await;

            // Delete task
            gateway_clone.emit_task_deleted(&task_id).await;
        });

        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.expect("Task should complete successfully");
    }

    // Test global broadcasts concurrently
    let mut broadcast_handles = Vec::new();

    for i in 0..5 {
        let gateway_clone = gateway.clone();
        let handle = tokio::spawn(async move {
            gateway_clone
                .broadcast_global(
                    "concurrent_test",
                    json!({"iteration": i, "timestamp": chrono::Utc::now()}),
                )
                .await;
        });
        broadcast_handles.push(handle);
    }

    for handle in broadcast_handles {
        handle
            .await
            .expect("Broadcast should complete successfully");
    }
}

/// Test WebSocket timeout and reliability
#[tokio::test]
async fn test_websocket_timeout_reliability() {
    let gateway = WebSocketGateway::new();

    // Test that operations complete within reasonable time
    let task = create_test_task("timeout-test-001");

    // All operations should complete quickly
    let result = timeout(Duration::from_secs(1), async {
        gateway.emit_task_created(&task).await;
        gateway.emit_task_update("timeout-test-001", &task).await;

        let message = create_test_message("timeout-msg-001", "timeout-test-001");
        gateway.emit_new_message("timeout-test-001", &message).await;

        gateway
            .broadcast_global("timeout_test", json!({"test": true}))
            .await;
        gateway
            .broadcast_to_task("timeout-test-001", "timeout_test", json!({"test": true}))
            .await;

        gateway.emit_task_deleted("timeout-test-001").await;
    })
    .await;

    assert!(
        result.is_ok(),
        "WebSocket operations should complete within timeout"
    );
}
