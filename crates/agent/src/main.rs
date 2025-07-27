mod config;
mod database;
mod error;

use anyhow::Result;
use config::Config;
use database::{DatabaseManager, MigrationRunner};
use tracing::{error, info, Level};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting ByteBot Agent Rust service...");

    // Load configuration
    let config = Config::from_env().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    info!("Configuration loaded successfully");

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

    // TODO: Initialize web server and other services
    info!("ByteBot Agent Rust service started successfully");

    // Keep the service running
    tokio::signal::ctrl_c().await?;
    
    info!("Shutting down ByteBot Agent Rust service...");
    db_manager.close().await;
    info!("Service shutdown complete");

    Ok(())
}
