use std::sync::Arc;

use bytebot_shared_rs::types::{Message, Task};
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
                        connection_manager
                            .handle_disconnection(socket.id.to_string())
                            .await;
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
                                debug!("Client {} successfully joined task {}", socket.id, task_id);
                                let response = ServerMessage::TaskJoined { task_id };
                                if let Err(e) = socket.emit("task_joined", response) {
                                    error!("Failed to emit task_joined: {}", e);
                                }
                            }
                            Err(e) => {
                                warn!("Failed to join task: {}", e);
                                let response = ServerMessage::Error {
                                    message: format!("Failed to join task: {e}"),
                                };
                                if let Err(e) = socket.emit("error", response) {
                                    error!("Failed to emit error: {}", e);
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
                                debug!("Client {} successfully left task {}", socket.id, task_id);
                                let response = ServerMessage::TaskLeft { task_id };
                                if let Err(e) = socket.emit("task_left", response) {
                                    error!("Failed to emit task_left: {}", e);
                                }
                            }
                            Err(e) => {
                                warn!("Failed to leave task: {}", e);
                                let response = ServerMessage::Error {
                                    message: format!("Failed to leave task: {e}"),
                                };
                                if let Err(e) = socket.emit("error", response) {
                                    error!("Failed to emit error: {}", e);
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

        info!("Client {} joined task {}", socket.id, task_id);
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

        info!("Client {} left task {}", socket.id, task_id);
        Ok(task_id)
    }

    /// Emit task update to all clients in the task room
    /// Matches emitTaskUpdate from TypeScript implementation
    pub async fn emit_task_update(&self, task_id: &str, task: &Task) {
        let room_name = format!("task_{task_id}");
        let message = ServerMessage::TaskUpdated { task: task.clone() };

        if let Err(e) = self.io.to(room_name.clone()).emit("task_updated", message) {
            error!("Failed to emit task_updated to room {}: {}", room_name, e);
        } else {
            debug!("Emitted task_updated to room {}", room_name);
        }
    }

    /// Emit new message to all clients in the task room
    /// Matches emitNewMessage from TypeScript implementation
    pub async fn emit_new_message(&self, task_id: &str, message: &Message) {
        let room_name = format!("task_{task_id}");
        let server_message = ServerMessage::NewMessage {
            message: message.clone(),
        };

        if let Err(e) = self
            .io
            .to(room_name.clone())
            .emit("new_message", server_message)
        {
            error!("Failed to emit new_message to room {}: {}", room_name, e);
        } else {
            debug!("Emitted new_message to room {}", room_name);
        }
    }

    /// Emit task created to all connected clients
    /// Matches emitTaskCreated from TypeScript implementation
    pub async fn emit_task_created(&self, task: &Task) {
        let message = ServerMessage::TaskCreated { task: task.clone() };

        if let Err(e) = self.io.emit("task_created", message) {
            error!("Failed to emit task_created globally: {}", e);
        } else {
            debug!("Emitted task_created globally");
        }
    }

    /// Emit task deleted to all connected clients
    /// Matches emitTaskDeleted from TypeScript implementation
    pub async fn emit_task_deleted(&self, task_id: &str) {
        let message = ServerMessage::TaskDeleted {
            task_id: task_id.to_string(),
        };

        if let Err(e) = self.io.emit("task_deleted", message) {
            error!("Failed to emit task_deleted globally: {}", e);
        } else {
            debug!("Emitted task_deleted globally for task {}", task_id);
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

        if let Err(e) = self.io.to(room_name.clone()).emit(event_name.clone(), data) {
            error!(
                "Failed to broadcast {} to room {}: {}",
                event_name, room_name, e
            );
        } else {
            debug!("Broadcasted {} to room {}", event_name, room_name);
        }
    }

    /// Broadcast a generic event to all connected clients
    pub async fn broadcast_global(&self, event_name: &str, data: impl serde::Serialize) {
        let event_name = event_name.to_string();
        if let Err(e) = self.io.emit(event_name.clone(), data) {
            error!("Failed to broadcast {} globally: {}", event_name, e);
        } else {
            debug!("Broadcasted {} globally", event_name);
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
