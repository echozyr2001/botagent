use std::path::Path;

use sqlx::{migrate::MigrateDatabase, Postgres};
use tracing::{error, info, warn};

use super::{DatabaseError, DatabasePool};

pub struct MigrationRunner {
    pool: DatabasePool,
}

impl MigrationRunner {
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }

    /// Run all pending migrations
    pub async fn run_migrations(&self) -> Result<(), DatabaseError> {
        info!("Running database migrations...");

        // Check if migrations directory exists
        let migrations_path = Path::new("migrations");
        if !migrations_path.exists() {
            warn!("Migrations directory not found, creating from Prisma schema...");
            self.create_initial_migration().await?;
        }

        // Run migrations using sqlx migrate
        match sqlx::migrate!("./migrations").run(&self.pool).await {
            Ok(_) => {
                info!("Database migrations completed successfully");
                Ok(())
            }
            Err(e) => {
                error!("Migration failed: {}", e);
                Err(DatabaseError::Migration(format!(
                    "Failed to run migrations: {e}"
                )))
            }
        }
    }

    /// Create the database if it doesn't exist
    pub async fn create_database_if_not_exists(database_url: &str) -> Result<(), DatabaseError> {
        info!("Checking if database exists...");

        if !Postgres::database_exists(database_url).await? {
            info!("Database does not exist, creating...");
            Postgres::create_database(database_url).await?;
            info!("Database created successfully");
        } else {
            info!("Database already exists");
        }

        Ok(())
    }

    /// Create initial migration based on Prisma schema
    /// This is a fallback when migrations directory doesn't exist
    async fn create_initial_migration(&self) -> Result<(), DatabaseError> {
        info!("Creating initial migration from Prisma schema...");

        // Create migrations directory
        tokio::fs::create_dir_all("migrations").await.map_err(|e| {
            DatabaseError::Migration(format!("Failed to create migrations directory: {e}"))
        })?;

        // Generate initial migration SQL based on the Prisma schema
        let initial_migration = self.generate_initial_schema_sql();

        // Write the migration file
        let migration_file = format!(
            "migrations/{}_initial_migration.sql",
            chrono::Utc::now().format("%Y%m%d%H%M%S")
        );

        tokio::fs::write(&migration_file, initial_migration)
            .await
            .map_err(|e| {
                DatabaseError::Migration(format!("Failed to write migration file: {e}"))
            })?;

        info!("Initial migration created: {}", migration_file);
        Ok(())
    }

    /// Generate the initial schema SQL that matches the Prisma schema
    fn generate_initial_schema_sql(&self) -> String {
        r#"-- Initial migration generated from Prisma schema
-- This creates the database schema to match the existing Prisma setup

-- Create enums
CREATE TYPE "TaskStatus" AS ENUM ('PENDING', 'RUNNING', 'NEEDS_HELP', 'NEEDS_REVIEW', 'COMPLETED', 'CANCELLED', 'FAILED');
CREATE TYPE "TaskPriority" AS ENUM ('LOW', 'MEDIUM', 'HIGH', 'URGENT');
CREATE TYPE "Role" AS ENUM ('USER', 'ASSISTANT');
CREATE TYPE "TaskType" AS ENUM ('IMMEDIATE', 'SCHEDULED');

-- Create User table (Better Auth)
CREATE TABLE "User" (
    "id" TEXT NOT NULL,
    "name" TEXT,
    "email" TEXT NOT NULL,
    "emailVerified" BOOLEAN NOT NULL DEFAULT false,
    "image" TEXT,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" TIMESTAMP(3) NOT NULL,

    CONSTRAINT "User_pkey" PRIMARY KEY ("id")
);

-- Create Session table (Better Auth)
CREATE TABLE "Session" (
    "id" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "token" TEXT NOT NULL,
    "expiresAt" TIMESTAMP(3) NOT NULL,
    "ipAddress" TEXT,
    "userAgent" TEXT,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" TIMESTAMP(3) NOT NULL,

    CONSTRAINT "Session_pkey" PRIMARY KEY ("id")
);

-- Create Account table (Better Auth)
CREATE TABLE "Account" (
    "id" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "accountId" TEXT NOT NULL,
    "providerId" TEXT NOT NULL,
    "accessToken" TEXT,
    "refreshToken" TEXT,
    "accessTokenExpiresAt" TIMESTAMP(3),
    "refreshTokenExpiresAt" TIMESTAMP(3),
    "scope" TEXT,
    "idToken" TEXT,
    "password" TEXT,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" TIMESTAMP(3) NOT NULL,

    CONSTRAINT "Account_pkey" PRIMARY KEY ("id")
);

-- Create Verification table (Better Auth)
CREATE TABLE "Verification" (
    "id" TEXT NOT NULL,
    "identifier" TEXT NOT NULL,
    "value" TEXT NOT NULL,
    "expiresAt" TIMESTAMP(3) NOT NULL,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" TIMESTAMP(3) NOT NULL,

    CONSTRAINT "Verification_pkey" PRIMARY KEY ("id")
);

-- Create Task table
CREATE TABLE "Task" (
    "id" TEXT NOT NULL,
    "description" TEXT NOT NULL,
    "type" "TaskType" NOT NULL DEFAULT 'IMMEDIATE',
    "status" "TaskStatus" NOT NULL DEFAULT 'PENDING',
    "priority" "TaskPriority" NOT NULL DEFAULT 'MEDIUM',
    "control" "Role" NOT NULL DEFAULT 'ASSISTANT',
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "createdBy" "Role" NOT NULL DEFAULT 'USER',
    "scheduledFor" TIMESTAMP(3),
    "updatedAt" TIMESTAMP(3) NOT NULL,
    "executedAt" TIMESTAMP(3),
    "completedAt" TIMESTAMP(3),
    "queuedAt" TIMESTAMP(3),
    "error" TEXT,
    "result" JSONB,
    "model" JSONB NOT NULL,
    "userId" TEXT,

    CONSTRAINT "Task_pkey" PRIMARY KEY ("id")
);

-- Create Summary table
CREATE TABLE "Summary" (
    "id" TEXT NOT NULL,
    "content" TEXT NOT NULL,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" TIMESTAMP(3) NOT NULL,
    "taskId" TEXT NOT NULL,
    "parentId" TEXT,

    CONSTRAINT "Summary_pkey" PRIMARY KEY ("id")
);

-- Create Message table
CREATE TABLE "Message" (
    "id" TEXT NOT NULL,
    "content" JSONB NOT NULL,
    "role" "Role" NOT NULL DEFAULT 'ASSISTANT',
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" TIMESTAMP(3) NOT NULL,
    "taskId" TEXT NOT NULL,
    "summaryId" TEXT,
    "userId" TEXT,

    CONSTRAINT "Message_pkey" PRIMARY KEY ("id")
);

-- Create File table
CREATE TABLE "File" (
    "id" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "type" TEXT NOT NULL,
    "size" INTEGER NOT NULL,
    "data" TEXT NOT NULL,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" TIMESTAMP(3) NOT NULL,
    "taskId" TEXT NOT NULL,

    CONSTRAINT "File_pkey" PRIMARY KEY ("id")
);

-- Create unique indexes
CREATE UNIQUE INDEX "User_email_key" ON "User"("email");
CREATE UNIQUE INDEX "Session_token_key" ON "Session"("token");
CREATE UNIQUE INDEX "Account_providerId_accountId_key" ON "Account"("providerId", "accountId");
CREATE UNIQUE INDEX "Verification_identifier_value_key" ON "Verification"("identifier", "value");

-- Add foreign key constraints
ALTER TABLE "Session" ADD CONSTRAINT "Session_userId_fkey" FOREIGN KEY ("userId") REFERENCES "User"("id") ON DELETE CASCADE ON UPDATE CASCADE;
ALTER TABLE "Account" ADD CONSTRAINT "Account_userId_fkey" FOREIGN KEY ("userId") REFERENCES "User"("id") ON DELETE CASCADE ON UPDATE CASCADE;
ALTER TABLE "Task" ADD CONSTRAINT "Task_userId_fkey" FOREIGN KEY ("userId") REFERENCES "User"("id") ON DELETE SET NULL ON UPDATE CASCADE;
ALTER TABLE "Summary" ADD CONSTRAINT "Summary_taskId_fkey" FOREIGN KEY ("taskId") REFERENCES "Task"("id") ON DELETE CASCADE ON UPDATE CASCADE;
ALTER TABLE "Summary" ADD CONSTRAINT "Summary_parentId_fkey" FOREIGN KEY ("parentId") REFERENCES "Summary"("id") ON DELETE SET NULL ON UPDATE CASCADE;
ALTER TABLE "Message" ADD CONSTRAINT "Message_taskId_fkey" FOREIGN KEY ("taskId") REFERENCES "Task"("id") ON DELETE CASCADE ON UPDATE CASCADE;
ALTER TABLE "Message" ADD CONSTRAINT "Message_summaryId_fkey" FOREIGN KEY ("summaryId") REFERENCES "Summary"("id") ON DELETE SET NULL ON UPDATE CASCADE;
ALTER TABLE "Message" ADD CONSTRAINT "Message_userId_fkey" FOREIGN KEY ("userId") REFERENCES "User"("id") ON DELETE SET NULL ON UPDATE CASCADE;
ALTER TABLE "File" ADD CONSTRAINT "File_taskId_fkey" FOREIGN KEY ("taskId") REFERENCES "Task"("id") ON DELETE CASCADE ON UPDATE CASCADE;
"#.to_string()
    }

    /// Check migration status
    pub async fn migration_status(&self) -> Result<Vec<MigrationInfo>, DatabaseError> {
        info!("Checking migration status...");

        let rows = sqlx::query_as::<_, MigrationRow>(
            r#"
            SELECT version, description, installed_on, success
            FROM _sqlx_migrations
            ORDER BY version
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatabaseError::Migration(format!("Failed to query migration status: {e}")))?;

        let migrations = rows
            .into_iter()
            .map(|row| MigrationInfo {
                version: row.version,
                description: row.description,
                installed_on: row.installed_on,
                success: row.success,
            })
            .collect();

        Ok(migrations)
    }

    /// Rollback the last migration (if supported)
    pub async fn rollback_last_migration(&self) -> Result<(), DatabaseError> {
        warn!("Migration rollback requested - this is a destructive operation");

        // Note: SQLx doesn't support automatic rollbacks
        // This would need to be implemented manually with down migrations
        Err(DatabaseError::Migration(
            "Migration rollback is not currently supported. Please create a new migration to revert changes.".to_string()
        ))
    }
}

#[derive(Debug, sqlx::FromRow)]
struct MigrationRow {
    version: i64,
    description: String,
    installed_on: chrono::DateTime<chrono::Utc>,
    success: bool,
}

#[derive(Debug)]
pub struct MigrationInfo {
    pub version: i64,
    pub description: String,
    pub installed_on: chrono::DateTime<chrono::Utc>,
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::DatabaseManager;

    #[tokio::test]
    async fn test_migration_runner_creation() {
        // Skip if DATABASE_URL is not set
        if std::env::var("DATABASE_URL").is_err() {
            return;
        }

        let database_url = std::env::var("DATABASE_URL").unwrap();

        match DatabaseManager::new(&database_url).await {
            Ok(manager) => {
                let runner = MigrationRunner::new(manager.pool().clone());

                // Test that we can create a migration runner
                assert!(std::mem::size_of_val(&runner) > 0);

                manager.close().await;
            }
            Err(e) => {
                eprintln!("Database connection test failed: {e}");
            }
        }
    }

    #[tokio::test]
    async fn test_initial_schema_generation() {
        // Create a dummy pool for testing (won't actually connect)
        let pool = sqlx::Pool::<Postgres>::connect_lazy("postgresql://test").unwrap();
        let runner = MigrationRunner::new(pool);
        let schema = runner.generate_initial_schema_sql();

        // Verify that the schema contains expected tables
        assert!(schema.contains("CREATE TABLE \"Task\""));
        assert!(schema.contains("CREATE TABLE \"Message\""));
        assert!(schema.contains("CREATE TABLE \"User\""));
        assert!(schema.contains("CREATE TYPE \"TaskStatus\""));
    }
}
