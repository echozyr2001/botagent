use serde::{Deserialize, Serialize};
use bytebot_shared_rs::types::{Task, Message};

/// WebSocket event types that match the TypeScript implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WebSocketEvent {
    /// Task was updated
    TaskUpdated { task_id: String, task: Task },
    /// New message was added to a task
    NewMessage { task_id: String, message: Message },
    /// Task was created
    TaskCreated { task: Task },
    /// Task was deleted
    TaskDeleted { task_id: String },
}

/// Client-to-server messages
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    /// Join a specific task room
    JoinTask { task_id: String },
    /// Leave a specific task room
    LeaveTask { task_id: String },
}

/// Server-to-client messages
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    /// Confirmation of joining a task room
    TaskJoined { task_id: String },
    /// Confirmation of leaving a task room
    TaskLeft { task_id: String },
    /// Task update event
    TaskUpdated { task: Task },
    /// New message event
    NewMessage { message: Message },
    /// Task created event
    TaskCreated { task: Task },
    /// Task deleted event
    TaskDeleted { task_id: String },
    /// Error message
    Error { message: String },
}

impl From<WebSocketEvent> for ServerMessage {
    fn from(event: WebSocketEvent) -> Self {
        match event {
            WebSocketEvent::TaskUpdated { task, .. } => ServerMessage::TaskUpdated { task },
            WebSocketEvent::NewMessage { message, .. } => ServerMessage::NewMessage { message },
            WebSocketEvent::TaskCreated { task } => ServerMessage::TaskCreated { task },
            WebSocketEvent::TaskDeleted { task_id } => ServerMessage::TaskDeleted { task_id },
        }
    }
}