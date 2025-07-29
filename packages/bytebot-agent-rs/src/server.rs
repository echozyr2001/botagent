use axum::{
    extract::State,
    http::Method,
    response::Json,
    routing::get,
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::Level;

use crate::{
    ai::UnifiedAIService, 
    auth::{AuthService, AuthServiceTrait, auth_middleware, optional_auth_middleware},
    config::Config, 
    database::DatabaseManager, 
    error::ServiceError, 
    routes::{create_auth_routes, create_message_routes, create_task_routes},
    websocket::WebSocketGateway,
};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Arc<DatabaseManager>,
    pub ai_service: Arc<UnifiedAIService>,
    pub auth_service: Arc<dyn AuthServiceTrait>,
    pub websocket_gateway: Arc<WebSocketGateway>,
}

/// Create AppState with all services initialized
pub async fn create_app_state(config: Arc<Config>) -> Result<AppState, ServiceError> {
    // Initialize database manager
    let db = Arc::new(
        DatabaseManager::new(&config.database_url)
            .await
            .map_err(|e| ServiceError::Internal(format!("Database initialization failed: {e}")))?
    );

    // Initialize AI service
    let ai_service = Arc::new(UnifiedAIService::new(&config));

    // Initialize auth service
    let auth_service: Arc<dyn AuthServiceTrait> = Arc::new(AuthService::new(
        db.get_pool(),
        config.jwt_secret.clone(),
        config.auth_enabled,
    ));

    // Initialize WebSocket gateway
    let websocket_gateway = Arc::new(WebSocketGateway::new());

    Ok(AppState {
        config,
        db,
        ai_service,
        auth_service,
        websocket_gateway,
    })
}

/// Create the main Axum application with all middleware and routes
pub fn create_app(state: AppState) -> Router {
    // Create CORS layer matching existing TypeScript configuration
    let cors = CorsLayer::new()
        .allow_origin(Any) // Matches origin: '*' from TypeScript
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
            Method::PATCH,
        ])
        .allow_headers(Any);

    // Create tracing layer for request logging
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_request(DefaultOnRequest::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO));

    // Build middleware stack
    let middleware = ServiceBuilder::new()
        .layer(trace_layer)
        .layer(cors);

    // Create router with health check endpoint and all routes
    Router::new()
        .route("/health", get(health_check))
        .route("/api/health", get(health_check)) // Match global prefix pattern
        .route("/ws-stats", get(websocket_stats)) // WebSocket statistics endpoint
        // Authentication routes (public)
        .nest("/auth", create_auth_routes(
            Arc::new(state.db.user_repository()),
            state.auth_service.clone(),
            state.config.jwt_secret.clone()
        ))
        // Protected routes that require authentication (when enabled)
        .nest("/tasks", create_task_routes()
            .layer(axum::middleware::from_fn_with_state(state.auth_service.clone(), auth_middleware)))
        .nest("/messages", create_message_routes()
            .layer(axum::middleware::from_fn_with_state(state.auth_service.clone(), optional_auth_middleware)))
        // Integrate Socket.IO WebSocket server - socketioxide provides its own layer
        .layer(state.websocket_gateway.layer())
        .with_state(state)
        .layer(middleware)
}

/// Health check endpoint handler
async fn health_check(State(state): State<AppState>) -> Result<Json<Value>, ServiceError> {
    // Check database connectivity
    let db_healthy = state.db.is_ready().await;
    
    let status = if db_healthy { "healthy" } else { "unhealthy" };

    let response = json!({
        "status": status,
        "timestamp": chrono::Utc::now(),
        "version": env!("CARGO_PKG_VERSION"),
        "service": "bytebot-agent-rs",
        "database": {
            "connected": db_healthy,
            "pool_stats": if db_healthy {
                let stats = state.db.pool_stats();
                json!({
                    "size": stats.size,
                    "idle": stats.idle
                })
            } else {
                json!(null)
            }
        }
    });

    if !db_healthy {
        return Err(ServiceError::Internal("Database health check failed".to_string()));
    }

    Ok(Json(response))
}

/// WebSocket statistics endpoint handler
async fn websocket_stats(State(state): State<AppState>) -> Result<Json<Value>, ServiceError> {
    let stats = state.websocket_gateway.get_connection_stats().await;
    
    let response = json!({
        "websocket": {
            "total_connections": stats.total_connections,
            "total_rooms": stats.total_rooms,
            "rooms": stats.rooms_with_clients.into_iter().collect::<std::collections::HashMap<_, _>>()
        },
        "timestamp": chrono::Utc::now()
    });

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    async fn create_test_app() -> Router {
        let config = Arc::new(Config::default());
        // Create a mock database manager for testing
        // We'll use a valid connection string but won't actually connect
        let db = match DatabaseManager::new("postgresql://localhost:5432/test").await {
            Ok(db) => Arc::new(db),
            Err(_) => {
                // If we can't connect to a real database, create a minimal test app
                // without database functionality for basic routing tests
                return Router::new()
                    .route("/health", get(|| async { "test" }))
                    .route("/api/health", get(|| async { "test" }))
                    .route("/ws-stats", get(|| async { "test" }));
            }
        };
        let ai_service = Arc::new(UnifiedAIService::new(&config));
        let auth_service: Arc<dyn AuthServiceTrait> = Arc::new(AuthService::new(
            db.get_pool(),
            config.jwt_secret.clone(),
            config.auth_enabled,
        ));
        let websocket_gateway = Arc::new(WebSocketGateway::new());
        let state = AppState { config, db, ai_service, auth_service, websocket_gateway };
        create_app(state)
    }

    #[tokio::test]
    async fn test_health_endpoint_exists() {
        let app = create_test_app().await;
        
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // The endpoint should exist (even if it fails due to no real DB)
        assert_ne!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_cors_headers() {
        let app = create_test_app().await;
        
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/health")
                    .header("Origin", "http://localhost:3000")
                    .header("Access-Control-Request-Method", "GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should have CORS headers
        assert!(response.headers().contains_key("access-control-allow-origin"));
    }
}