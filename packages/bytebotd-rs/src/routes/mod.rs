pub mod computer_use;
pub mod health;

use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};

use crate::{automation::AutomationService, mcp::McpServer};

pub fn create_routes(automation_service: Arc<AutomationService>) -> Router {
    let mcp_routes = McpServer::create_routes(automation_service.clone());
    
    Router::new()
        .route("/health", get(health::health_check))
        .route("/computer-use", post(computer_use::handle_computer_action))
        .nest("/", mcp_routes)
        .with_state(automation_service)
}
