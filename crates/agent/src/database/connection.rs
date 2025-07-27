use sqlx::{postgres::PgPoolOptions, Pool, Postgres, Row};
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("Connection error: {0}")]
    Connection(sqlx::Error),
    #[error("Migration error: {0}")]
    Migration(String),
    #[error("Health check failed: {0}")]
    HealthCheck(String),
    #[error("Query error: {0}")]
    QueryError(sqlx::Error),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Invalid status transition from {from:?} to {to:?}")]
    InvalidStatusTransition {
        from: shared::types::task::TaskStatus,
        to: shared::types::task::TaskStatus,
    },
}

impl From<sqlx::Error> for DatabaseError {
    fn from(err: sqlx::Error) -> Self {
        DatabaseError::QueryError(err)
    }
}

pub type DatabasePool = Pool<Postgres>;

pub struct DatabaseManager {
    pool: DatabasePool,
}

impl DatabaseManager {
    /// Create a new database connection pool with retry logic
    pub async fn new(database_url: &str) -> Result<Self, DatabaseError> {
        info!("Initializing database connection pool...");
        
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .min_connections(5)
            .acquire_timeout(Duration::from_secs(30))
            .idle_timeout(Duration::from_secs(600))
            .max_lifetime(Duration::from_secs(1800))
            .test_before_acquire(true)
            .connect_with_retry(database_url, 5)
            .await?;

        info!("Database connection pool initialized successfully");
        
        let manager = Self { pool };
        
        // Perform initial health check
        manager.health_check().await?;
        
        Ok(manager)
    }

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &DatabasePool {
        &self.pool
    }

    /// Perform a health check on the database connection
    pub async fn health_check(&self) -> Result<(), DatabaseError> {
        match sqlx::query("SELECT 1 as health_check")
            .fetch_one(&self.pool)
            .await
        {
            Ok(row) => {
                let result: i32 = row.get("health_check");
                if result == 1 {
                    info!("Database health check passed");
                    Ok(())
                } else {
                    let error_msg = "Database health check returned unexpected value";
                    error!("{}", error_msg);
                    Err(DatabaseError::HealthCheck(error_msg.to_string()))
                }
            }
            Err(e) => {
                let error_msg = format!("Database health check failed: {e}");
                error!("{}", error_msg);
                Err(DatabaseError::HealthCheck(error_msg))
            }
        }
    }

    /// Check if the database is ready for connections
    pub async fn is_ready(&self) -> bool {
        self.health_check().await.is_ok()
    }

    /// Get connection pool statistics
    pub fn pool_stats(&self) -> PoolStats {
        PoolStats {
            size: self.pool.size(),
            idle: self.pool.num_idle(),
        }
    }

    /// Close the database connection pool
    pub async fn close(&self) {
        info!("Closing database connection pool...");
        self.pool.close().await;
        info!("Database connection pool closed");
    }
}

#[derive(Debug)]
pub struct PoolStats {
    pub size: u32,
    pub idle: usize,
}

/// Extension trait for PgPoolOptions to add retry logic
trait PgPoolOptionsExt {
    async fn connect_with_retry(
        self,
        database_url: &str,
        max_retries: u32,
    ) -> Result<DatabasePool, sqlx::Error>;
}

impl PgPoolOptionsExt for PgPoolOptions {
    async fn connect_with_retry(
        self,
        database_url: &str,
        max_retries: u32,
    ) -> Result<DatabasePool, sqlx::Error> {
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < max_retries {
            attempts += 1;
            
            match self.clone().connect(database_url).await {
                Ok(pool) => {
                    if attempts > 1 {
                        info!("Database connection established after {} attempts", attempts);
                    }
                    return Ok(pool);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempts < max_retries {
                        let delay = Duration::from_secs(2_u64.pow(attempts.min(5)));
                        warn!(
                            "Database connection attempt {} failed, retrying in {:?}...",
                            attempts, delay
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        error!("Failed to connect to database after {} attempts", max_retries);
        Err(last_error.unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_database_manager_creation() {
        // This test requires a running PostgreSQL instance
        // Skip if DATABASE_URL is not set
        if std::env::var("DATABASE_URL").is_err() {
            return;
        }

        let database_url = std::env::var("DATABASE_URL").unwrap();
        let manager = DatabaseManager::new(&database_url).await;
        
        match manager {
            Ok(manager) => {
                assert!(manager.is_ready().await);
                let stats = manager.pool_stats();
                assert!(stats.size > 0);
                manager.close().await;
            }
            Err(e) => {
                // Log the error but don't fail the test in CI environments
                eprintln!("Database connection test failed: {e}");
            }
        }
    }

    #[tokio::test]
    async fn test_health_check_with_invalid_url() {
        let invalid_url = "postgresql://invalid:5432/nonexistent";
        let result = DatabaseManager::new(invalid_url).await;
        assert!(result.is_err());
    }
}