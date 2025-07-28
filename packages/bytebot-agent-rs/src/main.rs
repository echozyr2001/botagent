mod ai;
mod config;
mod database;
mod error;
mod routes;
mod server;
mod websocket;

use anyhow::Result;
use ai::UnifiedAIService;
use config::Config;
use database::{DatabaseManager, MigrationRunner};
use server::{create_app, AppState};
use websocket::WebSocketGateway;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info, Level};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing with environment-based log level
    let log_level = std::env::var("LOG_LEVEL")
        .unwrap_or_else(|_| "info".to_string())
        .parse::<Level>()
        .unwrap_or(Level::INFO);
    
    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .init();

    info!("Starting ByteBot Agent Rust service...");

    // Load configuration
    let config = Config::from_env().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    info!("Configuration loaded successfully");
    info!("Server will bind to {}:{}", config.server.host, config.server.port);

    // Create database if it doesn't exist
    MigrationRunner::create_database_if_not_exists(&config.database_url).await?;

    // Initialize database connection pool
    let db_manager = DatabaseManager::new(&config.database_url).await.map_err(|e| {
        error!("Failed to initialize database: {}", e);
        e
    })?;

    info!("Database connection pool initialized");

    // Run migrations
    let migration_runner = MigrationRunner::new(db_manager.pool().clone());
    migration_runner.run_migrations().await.map_err(|e| {
        error!("Failed to run migrations: {}", e);
        e
    })?;

    info!("Database migrations completed");

    // Perform health check
    if !db_manager.is_ready().await {
        error!("Database health check failed");
        return Err(anyhow::anyhow!("Database is not ready"));
    }

    info!("Database health check passed");

    // Log pool statistics
    let stats = db_manager.pool_stats();
    info!("Database pool stats - Size: {}, Idle: {}", stats.size, stats.idle);

    // Initialize AI service
    let ai_service = UnifiedAIService::new(&config);
    info!("AI service initialized with {} available providers", 
          ai_service.get_available_providers().len());

    // Initialize WebSocket gateway
    let websocket_gateway = WebSocketGateway::new();
    info!("WebSocket gateway initialized");

    // Create application state
    let app_state = AppState {
        config: Arc::new(config.clone()),
        db: Arc::new(db_manager),
        ai_service: Arc::new(ai_service),
        websocket_gateway: Arc::new(websocket_gateway),
    };

    // Create Axum application with middleware
    let app = create_app(app_state);

    // Create TCP listener
    let bind_address = format!("{}:{}", config.server.host, config.server.port);
    let listener = TcpListener::bind(&bind_address).await.map_err(|e| {
        error!("Failed to bind to {}: {}", bind_address, e);
        anyhow::anyhow!("Failed to bind to address: {}", e)
    })?;

    info!("ByteBot Agent Rust service started successfully on {}", bind_address);
    info!("Health check available at: http://{}/health", bind_address);
    info!("WebSocket endpoint available at: ws://{}/socket.io/", bind_address);
    info!("WebSocket stats available at: http://{}/ws-stats", bind_address);

    // Start the server
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| {
            error!("Server error: {}", e);
            anyhow::anyhow!("Server error: {}", e)
        })?;

    info!("Service shutdown complete");
    Ok(())
}

/// Graceful shutdown signal handler
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, shutting down gracefully...");
        },
        _ = terminate => {
            info!("Received SIGTERM, shutting down gracefully...");
        },
    }
}
