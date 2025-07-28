use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Connection manager for tracking client connections and room memberships
#[derive(Debug, Clone)]
pub struct ConnectionManager {
    /// Maps socket IDs to their joined task rooms
    connections: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// Maps task room IDs to socket IDs in that room
    rooms: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            rooms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Handle a new client connection
    pub async fn handle_connection(&self, socket_id: String) {
        info!("Client connected: {}", socket_id);
        let mut connections = self.connections.write().await;
        connections.insert(socket_id, Vec::new());
    }

    /// Handle client disconnection
    pub async fn handle_disconnection(&self, socket_id: String) {
        info!("Client disconnected: {}", socket_id);
        
        let mut connections = self.connections.write().await;
        if let Some(task_rooms) = connections.remove(&socket_id) {
            drop(connections); // Release the lock before acquiring rooms lock
            
            // Remove the socket from all rooms it was in
            let mut rooms = self.rooms.write().await;
            for task_id in task_rooms {
                if let Some(room_sockets) = rooms.get_mut(&task_id) {
                    room_sockets.retain(|id| id != &socket_id);
                    if room_sockets.is_empty() {
                        rooms.remove(&task_id);
                        debug!("Removed empty room: task_{}", task_id);
                    }
                }
            }
        }
    }

    /// Join a client to a task room
    pub async fn join_task(&self, socket_id: String, task_id: String) -> Result<(), String> {
        debug!("Client {} joining task {}", socket_id, task_id);
        
        let room_key = format!("task_{}", task_id);
        
        // Add task to client's room list
        {
            let mut connections = self.connections.write().await;
            if let Some(client_rooms) = connections.get_mut(&socket_id) {
                if !client_rooms.contains(&task_id) {
                    client_rooms.push(task_id.clone());
                }
            } else {
                warn!("Attempted to join task for unknown socket: {}", socket_id);
                return Err("Socket not found".to_string());
            }
        }
        
        // Add client to room
        {
            let mut rooms = self.rooms.write().await;
            rooms.entry(room_key.clone())
                .or_insert_with(Vec::new)
                .push(socket_id.clone());
        }
        
        info!("Client {} joined {}", socket_id, room_key);
        Ok(())
    }

    /// Remove a client from a task room
    pub async fn leave_task(&self, socket_id: String, task_id: String) -> Result<(), String> {
        debug!("Client {} leaving task {}", socket_id, task_id);
        
        let room_key = format!("task_{}", task_id);
        
        // Remove task from client's room list
        {
            let mut connections = self.connections.write().await;
            if let Some(client_rooms) = connections.get_mut(&socket_id) {
                client_rooms.retain(|id| id != &task_id);
            } else {
                warn!("Attempted to leave task for unknown socket: {}", socket_id);
                return Err("Socket not found".to_string());
            }
        }
        
        // Remove client from room
        {
            let mut rooms = self.rooms.write().await;
            if let Some(room_sockets) = rooms.get_mut(&room_key) {
                room_sockets.retain(|id| id != &socket_id);
                if room_sockets.is_empty() {
                    rooms.remove(&room_key);
                    debug!("Removed empty room: {}", room_key);
                }
            }
        }
        
        info!("Client {} left {}", socket_id, room_key);
        Ok(())
    }

    /// Get all socket IDs in a specific task room
    pub async fn get_room_sockets(&self, task_id: &str) -> Vec<String> {
        let room_key = format!("task_{}", task_id);
        let rooms = self.rooms.read().await;
        rooms.get(&room_key).cloned().unwrap_or_default()
    }

    /// Get all connected socket IDs (for global broadcasts)
    pub async fn get_all_sockets(&self) -> Vec<String> {
        let connections = self.connections.read().await;
        connections.keys().cloned().collect()
    }

    /// Get connection statistics for monitoring
    pub async fn get_stats(&self) -> ConnectionStats {
        let connections = self.connections.read().await;
        let rooms = self.rooms.read().await;
        
        ConnectionStats {
            total_connections: connections.len(),
            total_rooms: rooms.len(),
            rooms_with_clients: rooms.iter().map(|(k, v)| (k.clone(), v.len())).collect(),
        }
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about current connections
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub total_connections: usize,
    pub total_rooms: usize,
    pub rooms_with_clients: Vec<(String, usize)>,
}

#[cfg(test)]
mod tests {
    use super::*;


    fn create_test_socket_id() -> String {
        // Create a mock socket ID for testing
        // In real usage, this would come from socketioxide
        "test-socket-1".to_string()
    }

    #[tokio::test]
    async fn test_connection_lifecycle() {
        let manager = ConnectionManager::new();
        let socket_id = create_test_socket_id();

        // Test connection
        manager.handle_connection(socket_id.clone()).await;
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_connections, 1);

        // Test disconnection
        manager.handle_disconnection(socket_id).await;
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_connections, 0);
    }

    #[tokio::test]
    async fn test_room_management() {
        let manager = ConnectionManager::new();
        let socket_id = create_test_socket_id();
        let task_id = "test-task-123".to_string();

        // Connect client
        manager.handle_connection(socket_id.clone()).await;

        // Join room
        let result = manager.join_task(socket_id.clone(), task_id.clone()).await;
        assert!(result.is_ok());

        // Check room membership
        let room_sockets = manager.get_room_sockets(&task_id).await;
        assert_eq!(room_sockets.len(), 1);
        assert_eq!(room_sockets[0], socket_id);

        // Leave room
        let result = manager.leave_task(socket_id, task_id.clone()).await;
        assert!(result.is_ok());

        // Check room is empty
        let room_sockets = manager.get_room_sockets(&task_id).await;
        assert_eq!(room_sockets.len(), 0);
    }

    #[tokio::test]
    async fn test_multiple_clients_same_room() {
        let manager = ConnectionManager::new();
        let socket1 = "test-socket-1".to_string();
        let socket2 = "test-socket-2".to_string();
        let task_id = "test-task-123".to_string();

        // Connect both clients
        manager.handle_connection(socket1.clone()).await;
        manager.handle_connection(socket2.clone()).await;

        // Both join the same room
        manager.join_task(socket1.clone(), task_id.clone()).await.unwrap();
        manager.join_task(socket2.clone(), task_id.clone()).await.unwrap();

        // Check both are in the room
        let room_sockets = manager.get_room_sockets(&task_id).await;
        assert_eq!(room_sockets.len(), 2);
        assert!(room_sockets.contains(&socket1));
        assert!(room_sockets.contains(&socket2));

        // One leaves
        manager.leave_task(socket1.clone(), task_id.clone()).await.unwrap();
        let room_sockets = manager.get_room_sockets(&task_id).await;
        assert_eq!(room_sockets.len(), 1);
        assert_eq!(room_sockets[0], socket2);
    }
}