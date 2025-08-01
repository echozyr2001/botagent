pub mod computer_use;
pub mod health;

use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use bytebot_shared_rs::{MetricsCollector, middleware::metrics_middleware};
use chrono::Utc;

use crate::{automation::AutomationService, mcp::McpServer};

pub fn create_routes(
    automation_service: Arc<AutomationService>,
    metrics: Arc<MetricsCollector>,
) -> Router {
    let mcp_routes = McpServer::create_routes(automation_service.clone());

    // Create health state for enhanced health endpoints
    let health_state = health::HealthState {
        metrics: metrics.clone(),
        start_time: Utc::now(),
    };

    Router::new()
        // Basic health endpoint
        .route("/health", get(health::health_check))
        // Computer use endpoint
        .route("/computer-use", post(computer_use::handle_computer_action))
        .nest("/", mcp_routes)
        .with_state(automation_service)
        // Enhanced health endpoints with metrics
        .merge(
            Router::new()
                .route("/health/detailed", get(health::health_detailed))
                .route("/health/ready", get(health::readiness))
                .route("/health/live", get(health::liveness))
                .route("/metrics", get(health::metrics))
                .with_state(health_state)
        )
        // Add metrics middleware
        .layer(axum::middleware::from_fn(metrics_middleware))
}
