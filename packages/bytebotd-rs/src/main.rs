mod automation;
mod config;
mod error;
mod routes;

use anyhow::Result;
use axum::http::{
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    HeaderValue, Method,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::{error, info, Level};

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize tracing
    let log_level = std::env::var("LOG_LEVEL")
        .unwrap_or_else(|_| "info".to_string())
        .parse::<Level>()
        .unwrap_or(Level::INFO);
    
    tracing_subscriber::fmt().with_max_level(log_level).init();

    info!("Starting ByteBot Desktop Automation Daemon Rust service...");

    // Load configuration
    let config = config::Config::from_env().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    info!("Configuration loaded: {:?}", config);

    // Initialize automation service
    let automation_service = Arc::new(automation::AutomationService::new().map_err(|e| {
        error!("Failed to initialize automation service: {}", e);
        anyhow::anyhow!("Automation service initialization failed: {}", e)
    })?);

    info!("Automation service initialized successfully");

    // Create CORS layer
    let cors = CorsLayer::new()
        .allow_origin("http://localhost:9992".parse::<HeaderValue>().unwrap())
        .allow_origin("http://127.0.0.1:9992".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE])
        .allow_credentials(true);

    // Create routes
    let app = routes::create_routes(automation_service)
        .layer(cors)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    // Get socket address
    let addr = config.socket_addr();
    info!("Starting server on {}", addr);

    // Start the server
    let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|e| {
        error!("Failed to bind to address {}: {}", addr, e);
        anyhow::anyhow!("Failed to bind to address: {}", e)
    })?;

    info!("ByteBot Desktop Automation Daemon is running on {}", addr);
    info!("Health check available at: http://{}/health", addr);
    info!("Computer-use API available at: http://{}/computer-use", addr);

    axum::serve(listener, app).await.map_err(|e| {
        error!("Server error: {}", e);
        anyhow::anyhow!("Server error: {}", e)
    })?;

    Ok(())
}
