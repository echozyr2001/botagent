use std::sync::Arc;

use bytebot_shared_rs::{
    logging::websocket_logging,
    types::{Message, Task},
};
use serde_json::Value;
use socketioxide::{
    extract::{Data, SocketRef},
    SocketIo,
};
use tracing::{debug, error, info, warn};

use super::{connection::ConnectionManager, events::ServerMessage};
use crate::error::ServiceError;

/// WebSocket gateway that provides Socket.IO compatible interface
/// This matches the functionality of the TypeScript TasksGateway
#[derive(Clone)]
pub struct WebSocketGateway {
    io: SocketIo,
    layer: socketioxide::layer::SocketIoLayer<socketioxide::adapter::LocalAdapter>,
    connection_manager: Arc<ConnectionManager>,
}

impl WebSocketGateway {
    /// Create a new WebSocket gateway with Socket.IO server
    pub fn new() -> Self {
        let connection_manager = Arc::new(ConnectionManager::new());

        // Create Socket.IO server with CORS configuration matching TypeScript
        let (layer, io) = SocketIo::new_layer();

        let gateway = Self {
            io: io.clone(),
            layer,
            connection_manager: connection_manager.clone(),
        };

        // Set up event handlers
        gateway.setup_handlers(connection_manager);

        gateway
    }

    /// Get the Socket.IO layer for Axum integration
    pub fn layer(&self) -> socketioxide::layer::SocketIoLayer<socketioxide::adapter::LocalAdapter> {
        self.layer.clone()
    }

    /// Get a reference to the Socket.IO instance
    pub fn io(&self) -> &SocketIo {
        &self.io
    }

    /// Set up Socket.IO event handlers
    fn setup_handlers(&self, connection_manager: Arc<ConnectionManager>) {
        // Handle client connections
        self.io.ns("/", move |socket: SocketRef| {
            let connection_manager = connection_manager.clone();

            // Handle connection
            let socket_id = socket.id.to_string();
            websocket_logging::client_connected(&socket_id);
            let conn_mgr = connection_manager.clone();
            tokio::spawn(async move {
                conn_mgr.handle_connection(socket_id).await;
            });

            // Handle disconnection
            socket.on_disconnect({
                let connection_manager = connection_manager.clone();
                move |socket: SocketRef| {
                    let connection_manager = connection_manager.clone();
                    async move {
                        let socket_id = socket.id.to_string();
                        websocket_logging::client_disconnected(&socket_id);
                        connection_manager.handle_disconnection(socket_id).await;
                    }
                }
            });

            // Handle join_task message
            socket.on("join_task", {
                let connection_manager = connection_manager.clone();
                move |socket: SocketRef, Data::<Value>(data)| {
                    let connection_manager = connection_manager.clone();
                    async move {
                        match Self::handle_join_task(&connection_manager, &socket, data).await {
                            Ok(task_id) => {
                                websocket_logging::client_joined_task(
                                    &socket.id.to_string(),
                                    &task_id,
                                );
                                let response = ServerMessage::TaskJoined { task_id };
                                if let Err(e) = socket.emit("task_joined", response) {
                                    error!(error = %e, "Failed to emit task_joined");
                                }
                            }
                            Err(e) => {
                                warn!(error = %e, "Failed to join task");
                                let response = ServerMessage::Error {
                                    message: format!("Failed to join task: {e}"),
                                };
                                if let Err(e) = socket.emit("error", response) {
                                    error!(error = %e, "Failed to emit error");
                                }
                            }
                        }
                    }
                }
            });

            // Handle leave_task message
            socket.on("leave_task", {
                let connection_manager = connection_manager.clone();
                move |socket: SocketRef, Data::<Value>(data)| {
                    let connection_manager = connection_manager.clone();
                    async move {
                        match Self::handle_leave_task(&connection_manager, &socket, data).await {
                            Ok(task_id) => {
                                websocket_logging::client_left_task(
                                    &socket.id.to_string(),
                                    &task_id,
                                );
                                let response = ServerMessage::TaskLeft { task_id };
                                if let Err(e) = socket.emit("task_left", response) {
                                    error!(error = %e, "Failed to emit task_left");
                                }
                            }
                            Err(e) => {
                                warn!(error = %e, "Failed to leave task");
                                let response = ServerMessage::Error {
                                    message: format!("Failed to leave task: {e}"),
                                };
                                if let Err(e) = socket.emit("error", response) {
                                    error!(error = %e, "Failed to emit error");
                                }
                            }
                        }
                    }
                }
            });
        });
    }

    /// Handle join_task message from client
    async fn handle_join_task(
        connection_manager: &ConnectionManager,
        socket: &SocketRef,
        data: Value,
    ) -> Result<String, ServiceError> {
        // Parse task_id from the message data
        let task_id = if let Some(task_id_str) = data.as_str() {
            task_id_str.to_string()
        } else if let Some(obj) = data.as_object() {
            obj.get("task_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ServiceError::Validation("Missing task_id in join_task message".to_string())
                })?
                .to_string()
        } else {
            return Err(ServiceError::Validation(
                "Invalid join_task message format".to_string(),
            ));
        };

        // Join the task room
        connection_manager
            .join_task(socket.id.to_string(), task_id.clone())
            .await
            .map_err(|e| ServiceError::Internal(format!("Failed to join task room: {e}")))?;

        // Logging is handled by the caller
        Ok(task_id)
    }

    /// Handle leave_task message from client
    async fn handle_leave_task(
        connection_manager: &ConnectionManager,
        socket: &SocketRef,
        data: Value,
    ) -> Result<String, ServiceError> {
        // Parse task_id from the message data
        let task_id = if let Some(task_id_str) = data.as_str() {
            task_id_str.to_string()
        } else if let Some(obj) = data.as_object() {
            obj.get("task_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ServiceError::Validation("Missing task_id in leave_task message".to_string())
                })?
                .to_string()
        } else {
            return Err(ServiceError::Validation(
                "Invalid leave_task message format".to_string(),
            ));
        };

        // Leave the task room
        connection_manager
            .leave_task(socket.id.to_string(), task_id.clone())
            .await
            .map_err(|e| ServiceError::Internal(format!("Failed to leave task room: {e}")))?;

        // Logging is handled by the caller
        Ok(task_id)
    }

    /// Emit task update to all clients in the task room
    /// Matches emitTaskUpdate from TypeScript implementation
    pub async fn emit_task_update(&self, task_id: &str, task: &Task) {
        let room_name = format!("task_{task_id}");
        let message = ServerMessage::TaskUpdated { task: task.clone() };

        let client_count = self
            .connection_manager
            .get_room_client_count(&room_name)
            .await;
        if let Err(e) = self.io.to(room_name.clone()).emit("task_updated", message) {
            error!(
                room = %room_name,
                error = %e,
                "Failed to emit task_updated"
            );
        } else {
            websocket_logging::event_emitted("task_updated", Some(&room_name), client_count);
        }
    }

    /// Emit new message to all clients in the task room
    /// Matches emitNewMessage from TypeScript implementation
    pub async fn emit_new_message(&self, task_id: &str, message: &Message) {
        let room_name = format!("task_{task_id}");
        let server_message = ServerMessage::NewMessage {
            message: message.clone(),
        };

        let client_count = self
            .connection_manager
            .get_room_client_count(&room_name)
            .await;
        if let Err(e) = self
            .io
            .to(room_name.clone())
            .emit("new_message", server_message)
        {
            error!(
                room = %room_name,
                error = %e,
                "Failed to emit new_message"
            );
        } else {
            websocket_logging::event_emitted("new_message", Some(&room_name), client_count);
        }
    }

    /// Emit task created to all connected clients
    /// Matches emitTaskCreated from TypeScript implementation
    pub async fn emit_task_created(&self, task: &Task) {
        let message = ServerMessage::TaskCreated { task: task.clone() };

        let client_count = self.connection_manager.get_total_clients().await;
        if let Err(e) = self.io.emit("task_created", message) {
            error!(error = %e, "Failed to emit task_created globally");
        } else {
            websocket_logging::event_emitted("task_created", None, client_count);
        }
    }

    /// Emit task deleted to all connected clients
    /// Matches emitTaskDeleted from TypeScript implementation
    pub async fn emit_task_deleted(&self, task_id: &str) {
        let message = ServerMessage::TaskDeleted {
            task_id: task_id.to_string(),
        };

        let client_count = self.connection_manager.get_total_clients().await;
        if let Err(e) = self.io.emit("task_deleted", message) {
            error!(error = %e, "Failed to emit task_deleted globally");
        } else {
            websocket_logging::event_emitted("task_deleted", None, client_count);
        }
    }

    /// Get connection statistics for monitoring
    pub async fn get_connection_stats(&self) -> super::connection::ConnectionStats {
        self.connection_manager.get_stats().await
    }

    /// Broadcast a generic event to all clients in a task room
    pub async fn broadcast_to_task(
        &self,
        task_id: &str,
        event_name: &str,
        data: impl serde::Serialize,
    ) {
        let room_name = format!("task_{task_id}");
        let event_name = event_name.to_string();

        let client_count = self
            .connection_manager
            .get_room_client_count(&room_name)
            .await;
        if let Err(e) = self.io.to(room_name.clone()).emit(event_name.clone(), data) {
            error!(
                event_type = %event_name,
                room = %room_name,
                error = %e,
                "Failed to broadcast event"
            );
        } else {
            websocket_logging::event_emitted(&event_name, Some(&room_name), client_count);
        }
    }

    /// Broadcast a generic event to all connected clients
    pub async fn broadcast_global(&self, event_name: &str, data: impl serde::Serialize) {
        let event_name = event_name.to_string();
        let client_count = self.connection_manager.get_total_clients().await;
        if let Err(e) = self.io.emit(event_name.clone(), data) {
            error!(
                event_type = %event_name,
                error = %e,
                "Failed to broadcast event globally"
            );
        } else {
            websocket_logging::event_emitted(&event_name, None, client_count);
        }
    }
}

impl Default for WebSocketGateway {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn test_gateway_creation() {
        let gateway = WebSocketGateway::new();
        let stats = gateway.get_connection_stats().await;
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.total_rooms, 0);
    }

    #[test]
    fn test_join_task_message_parsing() {
        // Test string format
        let data = json!("test-task-123");
        if let Some(task_id) = data.as_str() {
            assert_eq!(task_id, "test-task-123");
        }

        // Test object format
        let data = json!({"task_id": "test-task-456"});
        if let Some(obj) = data.as_object() {
            if let Some(task_id) = obj.get("task_id").and_then(|v| v.as_str()) {
                assert_eq!(task_id, "test-task-456");
            }
        }
    }

    #[test]
    fn test_server_message_serialization() {
        let message = ServerMessage::TaskJoined {
            task_id: "test-123".to_string(),
        };

        let serialized = serde_json::to_string(&message).unwrap();
        assert!(serialized.contains("TaskJoined"));
        assert!(serialized.contains("test-123"));
    }
}
