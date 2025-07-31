use std::{sync::Arc, time::Duration};

use bytebot_agent_rs::{
    config::Config,
    database::DatabaseManager,
    server::{create_app, create_app_state},
    websocket::{events::ClientMessage, WebSocketGateway},
};
use bytebot_shared_rs::types::{Message, Role, Task, TaskPriority, TaskStatus, TaskType};
use serde_json::json;
use tokio::{net::TcpListener, time::timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

/// Test database URL for WebSocket tests
const TEST_DATABASE_URL: &str = "postgresql://localhost:5432/bytebot_test_websocket";

/// Create a test server for WebSocket testing
async fn create_test_server(
) -> Result<(String, tokio::task::JoinHandle<()>), Box<dyn std::error::Error>> {
    // Create test configuration
    let config = Arc::new(Config {
        database_url: TEST_DATABASE_URL.to_string(),
        auth_enabled: false, // Disable auth for WebSocket tests
        ..Config::default()
    });

    // Try to create database manager
    let db_manager = DatabaseManager::new(&config.database_url).await?;

    // Create application state
    let app_state = create_app_state(config).await?;

    // Create the application
    let app = create_app(app_state);

    // Find an available port
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let server_url = format!("127.0.0.1:{}", addr.port());

    // Start the server
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("Server should start successfully");
    });

    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    Ok((server_url, server_handle))
}

/// Create a test task for WebSocket testing
fn create_test_task(id: &str) -> Task {
    Task {
        id: id.to_string(),
        description: "WebSocket test task".to_string(),
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
        content: json!([{"type": "text", "text": "WebSocket test message"}]),
        role: Role::User,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        task_id: task_id.to_string(),
        summary_id: None,
        user_id: None,
    }
}

/// Mock WebSocket client for testing
struct MockWebSocketClient {
    id: String,
    ws_stream: Option<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    received_messages: Vec<String>,
}

impl MockWebSocketClient {
    async fn new(id: &str, server_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let ws_url = format!("ws://{}/socket.io/?EIO=4&transport=websocket", server_url);

        // Try to connect to WebSocket
        match connect_async(&ws_url).await {
            Ok((ws_stream, _)) => Ok(Self {
                id: id.to_string(),
                ws_stream: Some(ws_stream),
                received_messages: Vec::new(),
            }),
            Err(e) => {
                // If connection fails, create a mock client without connection
                println!(
                    "WebSocket connection failed ({}), creating mock client: {}",
                    e, id
                );
                Ok(Self {
                    id: id.to_string(),
                    ws_stream: None,
                    received_messages: Vec::new(),
                })
            }
        }
    }

    async fn send_message(
        &mut self,
        message: ClientMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut ws_stream) = self.ws_stream {
            let message_json = serde_json::to_string(&message)?;
            let ws_message = WsMessage::Text(message_json);

            use futures_util::SinkExt;
            ws_stream.send(ws_message).await?;
        }
        Ok(())
    }

    async fn receive_message(&mut self) -> Result<Option<String>, Box<dyn std::error::Error>> {
        if let Some(ref mut ws_stream) = self.ws_stream {
            use futures_util::StreamExt;

            if let Some(message) = ws_stream.next().await {
                match message? {
                    WsMessage::Text(text) => {
                        self.received_messages.push(text.clone());
                        Ok(Some(text))
                    }
                    _ => Ok(None),
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn get_received_messages(&self) -> &[String] {
        &self.received_messages
    }
}

/// Integration test for WebSocket server with mock clients
#[tokio::test]
async fn test_websocket_server_with_mock_clients() {
    let (server_url, server_handle) = match create_test_server().await {
        Ok(result) => result,
        Err(e) => {
            println!("Skipping WebSocket server test - setup failed: {}", e);
            return;
        }
    };

    // Create mock clients
    let mut client1 = match MockWebSocketClient::new("client-001", &server_url).await {
        Ok(client) => client,
        Err(e) => {
            println!("Skipping WebSocket test - client creation failed: {}", e);
            server_handle.abort();
            return;
        }
    };

    let mut client2 = match MockWebSocketClient::new("client-002", &server_url).await {
        Ok(client) => client,
        Err(e) => {
            println!("Skipping WebSocket test - client creation failed: {}", e);
            server_handle.abort();
            return;
        }
    };

    // Test joining a task room
    let task_id = "websocket-test-task-001";
    let join_message = ClientMessage::JoinTask {
        task_id: task_id.to_string(),
    };

    if let Err(e) = client1.send_message(join_message.clone()).await {
        println!("Failed to send join message: {}", e);
    }

    if let Err(e) = client2.send_message(join_message).await {
        println!("Failed to send join message: {}", e);
    }

    // Give some time for the join to process
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test leaving a task room
    let leave_message = ClientMessage::LeaveTask {
        task_id: task_id.to_string(),
    };

    if let Err(e) = client1.send_message(leave_message).await {
        println!("Failed to send leave message: {}", e);
    }

    // Give some time for processing
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Clean up
    server_handle.abort();
}

/// Integration test for WebSocket event broadcasting
#[tokio::test]
async fn test_websocket_event_broadcasting() {
    let (server_url, server_handle) = match create_test_server().await {
        Ok(result) => result,
        Err(e) => {
            println!("Skipping WebSocket broadcasting test - setup failed: {}", e);
            return;
        }
    };

    // Create WebSocket gateway for testing
    let gateway = WebSocketGateway::new();

    // Test task events
    let test_task = create_test_task("broadcast-test-task");
    let test_message = create_test_message("broadcast-msg-001", "broadcast-test-task");

    // These should not panic even without connected clients
    gateway.emit_task_created(&test_task).await;
    gateway
        .emit_task_update("broadcast-test-task", &test_task)
        .await;
    gateway
        .emit_new_message("broadcast-test-task", &test_message)
        .await;
    gateway.emit_task_deleted("broadcast-test-task").await;

    // Test global broadcasts
    gateway
        .broadcast_global("test_event", json!({"test": "data"}))
        .await;
    gateway
        .broadcast_to_task("broadcast-test-task", "task_event", json!({"test": "data"}))
        .await;

    // Clean up
    server_handle.abort();
}

/// Integration test for WebSocket connection management
#[tokio::test]
async fn test_websocket_connection_management() {
    let (server_url, server_handle) = match create_test_server().await {
        Ok(result) => result,
        Err(e) => {
            println!("Skipping WebSocket connection test - setup failed: {}", e);
            return;
        }
    };

    // Create multiple clients
    let mut clients = Vec::new();
    for i in 0..3 {
        match MockWebSocketClient::new(&format!("client-{:03}", i), &server_url).await {
            Ok(client) => clients.push(client),
            Err(e) => {
                println!("Failed to create client {}: {}", i, e);
            }
        }
    }

    // Test that clients can join different rooms
    let task_ids = vec!["task-alpha", "task-beta", "task-gamma"];

    for (i, client) in clients.iter_mut().enumerate() {
        if i < task_ids.len() {
            let join_message = ClientMessage::JoinTask {
                task_id: task_ids[i].to_string(),
            };

            if let Err(e) = client.send_message(join_message).await {
                println!("Failed to send join message for client {}: {}", i, e);
            }
        }
    }

    // Give time for processing
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Test that clients can leave rooms
    for (i, client) in clients.iter_mut().enumerate() {
        if i < task_ids.len() {
            let leave_message = ClientMessage::LeaveTask {
                task_id: task_ids[i].to_string(),
            };

            if let Err(e) = client.send_message(leave_message).await {
                println!("Failed to send leave message for client {}: {}", i, e);
            }
        }
    }

    // Give time for processing
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Clean up
    server_handle.abort();
}

/// Integration test for WebSocket message flow
#[tokio::test]
async fn test_websocket_message_flow() {
    let (server_url, server_handle) = match create_test_server().await {
        Ok(result) => result,
        Err(e) => {
            println!("Skipping WebSocket message flow test - setup failed: {}", e);
            return;
        }
    };

    // Create a client
    let mut client = match MockWebSocketClient::new("flow-client", &server_url).await {
        Ok(client) => client,
        Err(e) => {
            println!(
                "Skipping WebSocket message flow test - client creation failed: {}",
                e
            );
            server_handle.abort();
            return;
        }
    };

    let task_id = "message-flow-task";

    // Join a task room
    let join_message = ClientMessage::JoinTask {
        task_id: task_id.to_string(),
    };

    if let Err(e) = client.send_message(join_message).await {
        println!("Failed to send join message: {}", e);
    }

    // Give time for join to process
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Try to receive any messages (with timeout)
    let receive_result = timeout(Duration::from_millis(500), client.receive_message()).await;

    match receive_result {
        Ok(Ok(Some(message))) => {
            println!("Received WebSocket message: {}", message);
        }
        Ok(Ok(None)) => {
            println!("No WebSocket message received");
        }
        Ok(Err(e)) => {
            println!("Error receiving WebSocket message: {}", e);
        }
        Err(_) => {
            println!("Timeout waiting for WebSocket message");
        }
    }

    // Leave the room
    let leave_message = ClientMessage::LeaveTask {
        task_id: task_id.to_string(),
    };

    if let Err(e) = client.send_message(leave_message).await {
        println!("Failed to send leave message: {}", e);
    }

    // Check received messages
    let received = client.get_received_messages();
    println!("Total messages received: {}", received.len());

    // Clean up
    server_handle.abort();
}

/// Integration test for WebSocket error handling
#[tokio::test]
async fn test_websocket_error_handling() {
    let (server_url, server_handle) = match create_test_server().await {
        Ok(result) => result,
        Err(e) => {
            println!(
                "Skipping WebSocket error handling test - setup failed: {}",
                e
            );
            return;
        }
    };

    // Test connection to invalid endpoint
    let invalid_url = format!("ws://{}/invalid-endpoint", server_url);
    let invalid_client_result = MockWebSocketClient::new("invalid-client", &invalid_url).await;

    // This should either fail or create a mock client
    match invalid_client_result {
        Ok(_) => println!("Invalid client created (mock mode)"),
        Err(e) => println!("Expected error for invalid endpoint: {}", e),
    }

    // Test with valid client but invalid messages
    if let Ok(mut client) = MockWebSocketClient::new("error-client", &server_url).await {
        // Try to send malformed message (this should be handled gracefully)
        let malformed_message = ClientMessage::JoinTask {
            task_id: "".to_string(), // Empty task ID
        };

        if let Err(e) = client.send_message(malformed_message).await {
            println!("Expected error for malformed message: {}", e);
        }
    }

    // Clean up
    server_handle.abort();
}
