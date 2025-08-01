mod automation;
mod config;
mod error;
mod mcp;
mod routes;

use std::sync::Arc;

use anyhow::Result;
use axum::http::{
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    HeaderValue, Method,
};
use bytebot_shared_rs::{
    logging::{init_logging, LoggingConfig},
    MetricsCollector,
};
use tower_http::cors::CorsLayer;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize structured logging
    let logging_config = LoggingConfig::for_service("bytebotd-rs");
    init_logging(logging_config).map_err(|e| {
        eprintln!("Failed to initialize logging: {e}");
        anyhow::anyhow!("Logging initialization failed: {}", e)
    })?;

    info!(
        service = "bytebotd-rs",
        version = env!("CARGO_PKG_VERSION"),
        "Starting ByteBot Desktop Automation Daemon Rust service"
    );

    // Load configuration
    let config = config::Config::from_env().map_err(|e| {
        error!(error = %e, "Failed to load configuration");
        e
    })?;

    info!(config = ?config, "Configuration loaded successfully");

    // Initialize metrics collector
    let metrics = Arc::new(MetricsCollector::new("bytebotd-rs").map_err(|e| {
        error!(error = %e, "Failed to initialize metrics collector");
        anyhow::anyhow!("Metrics collector initialization failed: {}", e)
    })?);

    info!("Metrics collector initialized successfully");

    // Initialize automation service
    let automation_service = Arc::new(automation::AutomationService::new().map_err(|e| {
        error!(error = %e, "Failed to initialize automation service");
        anyhow::anyhow!("Automation service initialization failed: {}", e)
    })?);

    info!("Automation service initialized successfully");

    // Create CORS layer
    let cors = CorsLayer::new()
        .allow_origin("http://localhost:9992".parse::<HeaderValue>().unwrap())
        .allow_origin("http://127.0.0.1:9992".parse::<HeaderValue>().unwrap())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE])
        .allow_credentials(true);

    // Create routes with metrics
    let app = routes::create_routes(automation_service, metrics)
        .layer(cors)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    // Get socket address
    let addr = config.socket_addr();
    info!(bind_address = %addr, "Starting server");

    // Start the server
    let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|e| {
        error!(
            bind_address = %addr,
            error = %e,
            "Failed to bind to address"
        );
        anyhow::anyhow!("Failed to bind to address: {}", e)
    })?;

    info!(
        bind_address = %addr,
        service = "bytebotd-rs",
        version = env!("CARGO_PKG_VERSION"),
        "Desktop Automation Daemon is running"
    );
    info!(
        health_endpoint = format!("http://{}/health", addr),
        "Health check endpoint available"
    );
    info!(
        computer_use_endpoint = format!("http://{}/computer-use", addr),
        "Computer-use API endpoint available"
    );

    axum::serve(listener, app).await.map_err(|e| {
        error!(error = %e, "Server error occurred");
        anyhow::anyhow!("Server error: {}", e)
    })?;

    info!(service = "bytebotd-rs", "Service shutdown complete");
    Ok(())
}
