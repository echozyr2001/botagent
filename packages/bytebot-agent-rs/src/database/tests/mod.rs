use std::sync::Once;

use sqlx::{PgPool, Pool, Postgres};
use tracing_subscriber;

static INIT: Once = Once::new();

/// Initialize test logging
pub fn init_test_logging() {
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter("debug")
            .with_test_writer()
            .init();
    });
}

/// Create a test database pool
pub async fn create_test_pool() -> Pool<Postgres> {
    init_test_logging();

    let database_url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("TEST_DATABASE_URL or DATABASE_URL must be set for integration tests");

    PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database")
}

/// Clean up test data from all tables
pub async fn cleanup_test_data(pool: &PgPool) {
    // Delete in order to respect foreign key constraints
    let _ = sqlx::query(r#"DELETE FROM "Message""#).execute(pool).await;
    let _ = sqlx::query(r#"DELETE FROM "Session""#).execute(pool).await;
    let _ = sqlx::query(r#"DELETE FROM "Account""#).execute(pool).await;
    let _ = sqlx::query(r#"DELETE FROM "Verification""#)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "Task""#).execute(pool).await;
    let _ = sqlx::query(r#"DELETE FROM "User""#).execute(pool).await;
}

pub mod message_repository_tests;
pub mod user_repository_tests;
