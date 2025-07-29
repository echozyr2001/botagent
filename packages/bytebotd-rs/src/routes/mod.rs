pub mod computer_use;
pub mod health;

use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};

use crate::automation::AutomationService;

pub fn create_routes(automation_service: Arc<AutomationService>) -> Router {
    Router::new()
        .route("/health", get(health::health_check))
        .route("/computer-use", post(computer_use::handle_computer_action))
        .with_state(automation_service)
}
