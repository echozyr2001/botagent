mod ai;
mod auth;
mod config;
mod database;
mod error;
mod routes;
mod server;
mod websocket;

use std::sync::Arc;

use anyhow::Result;
use bytebot_shared_rs::logging::{init_logging, LoggingConfig};
use config::Config;
use database::MigrationRunner;
use server::{create_app, create_app_state};
use tokio::net::TcpListener;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured logging
    let logging_config = LoggingConfig::for_service("bytebot-agent-rs");
    init_logging(logging_config).map_err(|e| {
        eprintln!("Failed to initialize logging: {}", e);
        anyhow::anyhow!("Logging initialization failed: {}", e)
    })?;

    info!(
        service = "bytebot-agent-rs",
        version = env!("CARGO_PKG_VERSION"),
        "Starting ByteBot Agent Rust service"
    );

    // Load configuration
    let config = Config::from_env().map_err(|e| {
        error!(error = %e, "Failed to load configuration");
        e
    })?;

    info!("Configuration loaded successfully");
    info!(
        host = %config.server.host,
        port = config.server.port,
        "Server will bind to address"
    );

    // Create database if it doesn't exist
    MigrationRunner::create_database_if_not_exists(&config.database_url).await?;

    // Run migrations first
    let temp_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&config.database_url)
        .await
        .map_err(|e| {
            error!(
                error = %e,
                "Failed to create temporary database connection for migrations"
            );
            e
        })?;

    let migration_runner = MigrationRunner::new(temp_pool.clone());
    migration_runner.run_migrations().await.map_err(|e| {
        error!(error = %e, "Failed to run migrations");
        e
    })?;

    info!("Database migrations completed successfully");
    temp_pool.close().await;

    // Create application state with all services
    let config_arc = Arc::new(config.clone());
    let app_state = create_app_state(config_arc).await.map_err(|e| {
        error!(error = %e, "Failed to create application state");
        anyhow::anyhow!("Failed to create application state: {}", e)
    })?;

    info!("Application state initialized successfully");

    // Log authentication status
    info!(
        auth_enabled = app_state.config.auth_enabled,
        "Authentication configuration"
    );

    // Log pool statistics
    let stats = app_state.db.pool_stats();
    info!(
        pool_size = stats.size,
        idle_connections = stats.idle,
        "Database connection pool initialized"
    );

    // Create Axum application with middleware
    let app = create_app(app_state);

    // Create TCP listener
    let bind_address = format!("{}:{}", config.server.host, config.server.port);
    let listener = TcpListener::bind(&bind_address).await.map_err(|e| {
        error!(
            bind_address = %bind_address,
            error = %e,
            "Failed to bind to address"
        );
        anyhow::anyhow!("Failed to bind to address: {}", e)
    })?;

    info!(
        bind_address = %bind_address,
        service = "bytebot-agent-rs",
        version = env!("CARGO_PKG_VERSION"),
        "Service started successfully"
    );
    info!(
        health_endpoint = format!("http://{}/health", bind_address),
        "Health check endpoint available"
    );
    info!(
        websocket_endpoint = format!("ws://{}/socket.io/", bind_address),
        "WebSocket endpoint available"
    );
    info!(
        stats_endpoint = format!("http://{}/ws-stats", bind_address),
        "WebSocket stats endpoint available"
    );

    // Start the server
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| {
            error!(error = %e, "Server error occurred");
            anyhow::anyhow!("Server error: {}", e)
        })?;

    info!(service = "bytebot-agent-rs", "Service shutdown complete");
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
            info!(signal = "SIGINT", "Received shutdown signal, shutting down gracefully");
        },
        _ = terminate => {
            info!(signal = "SIGTERM", "Received shutdown signal, shutting down gracefully");
        },
    }
}
