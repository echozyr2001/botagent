use async_trait::async_trait;
use bytebot_shared_rs::types::{
    api::PaginationParams,
    user::{Account, Session, User, Verification},
};
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres, Row};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::DatabaseError;

/// User filtering options for complex queries
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UserFilter {
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub has_name: Option<bool>,
}

/// Session filtering options
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionFilter {
    pub user_id: Option<String>,
    pub expired: Option<bool>,
    pub ip_address: Option<String>,
}

/// Data transfer object for creating a new user
#[derive(Debug, Clone)]
pub struct CreateUserDto {
    pub email: String,
    pub name: Option<String>,
    pub email_verified: Option<bool>,
    pub image: Option<String>,
}

/// Data transfer object for updating an existing user
#[derive(Debug, Clone)]
pub struct UpdateUserDto {
    pub name: Option<String>,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub image: Option<String>,
}

/// Data transfer object for creating a new session
#[derive(Debug, Clone)]
pub struct CreateSessionDto {
    pub user_id: String,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// Data transfer object for creating a new account
#[derive(Debug, Clone)]
pub struct CreateAccountDto {
    pub user_id: String,
    pub account_id: String,
    pub provider_id: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub access_token_expires_at: Option<DateTime<Utc>>,
    pub refresh_token_expires_at: Option<DateTime<Utc>>,
    pub scope: Option<String>,
    pub id_token: Option<String>,
    pub password: Option<String>,
}

/// User repository trait for dependency injection and testing
#[async_trait]
pub trait UserRepositoryTrait: Send + Sync {
    // User operations
    async fn create_user(&self, dto: &CreateUserDto) -> Result<User, DatabaseError>;
    async fn get_user_by_id(&self, id: &str) -> Result<Option<User>, DatabaseError>;
    async fn get_user_by_email(&self, email: &str) -> Result<Option<User>, DatabaseError>;
    async fn update_user(
        &self,
        id: &str,
        dto: &UpdateUserDto,
    ) -> Result<Option<User>, DatabaseError>;
    async fn delete_user(&self, id: &str) -> Result<bool, DatabaseError>;
    async fn list_users(
        &self,
        filter: &UserFilter,
        pagination: &PaginationParams,
    ) -> Result<(Vec<User>, u64), DatabaseError>;
    async fn verify_user_email(&self, id: &str) -> Result<Option<User>, DatabaseError>;

    // Session operations
    async fn create_session(&self, dto: &CreateSessionDto) -> Result<Session, DatabaseError>;
    async fn get_session_by_token(&self, token: &str) -> Result<Option<Session>, DatabaseError>;
    async fn get_sessions_by_user_id(&self, user_id: &str) -> Result<Vec<Session>, DatabaseError>;
    async fn delete_session(&self, id: &str) -> Result<bool, DatabaseError>;
    async fn delete_expired_sessions(&self) -> Result<u64, DatabaseError>;
    async fn delete_user_sessions(&self, user_id: &str) -> Result<u64, DatabaseError>;

    // Account operations
    async fn create_account(&self, dto: &CreateAccountDto) -> Result<Account, DatabaseError>;
    async fn get_account_by_provider(
        &self,
        user_id: &str,
        provider_id: &str,
    ) -> Result<Option<Account>, DatabaseError>;
    async fn get_accounts_by_user_id(&self, user_id: &str) -> Result<Vec<Account>, DatabaseError>;
    async fn update_account_tokens(
        &self,
        id: &str,
        access_token: Option<String>,
        refresh_token: Option<String>,
        access_token_expires_at: Option<DateTime<Utc>>,
        refresh_token_expires_at: Option<DateTime<Utc>>,
    ) -> Result<Option<Account>, DatabaseError>;
    async fn update_account_password(
        &self,
        id: &str,
        password_hash: &str,
    ) -> Result<Option<Account>, DatabaseError>;
    async fn delete_account(&self, id: &str) -> Result<bool, DatabaseError>;

    // Verification operations
    async fn create_verification(
        &self,
        identifier: &str,
        value: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<Verification, DatabaseError>;
    async fn get_verification(
        &self,
        identifier: &str,
        value: &str,
    ) -> Result<Option<Verification>, DatabaseError>;
    async fn delete_verification(&self, id: &str) -> Result<bool, DatabaseError>;
    async fn delete_expired_verifications(&self) -> Result<u64, DatabaseError>;
}

/// SQLx-based user repository implementation
pub struct UserRepository {
    pool: Pool<Postgres>,
}

impl UserRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    /// Build WHERE clause for user filtering
    fn build_user_filter_clause(
        filter: &UserFilter,
    ) -> (String, Vec<Box<dyn sqlx::Encode<'_, Postgres> + Send>>) {
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn sqlx::Encode<'_, Postgres> + Send>> = Vec::new();
        let mut param_count = 1;

        if let Some(ref email) = filter.email {
            conditions.push(format!("email ILIKE ${param_count}"));
            params.push(Box::new(format!("%{email}%")));
            param_count += 1;
        }

        if let Some(email_verified) = filter.email_verified {
            conditions.push(format!("\"emailVerified\" = ${param_count}"));
            params.push(Box::new(email_verified));
            param_count += 1;
        }

        if let Some(created_after) = filter.created_after {
            conditions.push(format!("\"createdAt\" >= ${param_count}"));
            params.push(Box::new(created_after));
            param_count += 1;
        }

        if let Some(created_before) = filter.created_before {
            conditions.push(format!("\"createdAt\" <= ${param_count}"));
            params.push(Box::new(created_before));
            param_count += 1;
        }

        if let Some(has_name) = filter.has_name {
            if has_name {
                conditions.push("name IS NOT NULL AND name != ''".to_string());
            } else {
                conditions.push("(name IS NULL OR name = '')".to_string());
            }
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        (where_clause, params)
    }

    /// Validate email format using basic regex
    fn validate_email(email: &str) -> Result<(), DatabaseError> {
        if email.is_empty() {
            return Err(DatabaseError::ValidationError(
                "Email cannot be empty".to_string(),
            ));
        }

        if !email.contains('@') || !email.contains('.') {
            return Err(DatabaseError::ValidationError(
                "Invalid email format".to_string(),
            ));
        }

        if email.len() > 255 {
            return Err(DatabaseError::ValidationError(
                "Email too long (max 255 characters)".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate user name
    fn validate_name(name: &str) -> Result<(), DatabaseError> {
        if name.is_empty() {
            return Err(DatabaseError::ValidationError(
                "Name cannot be empty".to_string(),
            ));
        }

        if name.len() > 255 {
            return Err(DatabaseError::ValidationError(
                "Name too long (max 255 characters)".to_string(),
            ));
        }

        Ok(())
    }
}
#[async_trait]
impl UserRepositoryTrait for UserRepository {
    async fn create_user(&self, dto: &CreateUserDto) -> Result<User, DatabaseError> {
        debug!("Creating new user with email: {}", dto.email);

        // Validate input
        Self::validate_email(&dto.email)?;
        if let Some(ref name) = dto.name {
            Self::validate_name(name)?;
        }

        let user_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let email_verified = dto.email_verified.unwrap_or(false);

        let row = sqlx::query(
            r#"
            INSERT INTO "User" (
                id, name, email, "emailVerified", image, "createdAt", "updatedAt"
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING 
                id,
                name,
                email,
                "emailVerified",
                image,
                "createdAt",
                "updatedAt"
            "#,
        )
        .bind(&user_id)
        .bind(&dto.name)
        .bind(&dto.email)
        .bind(email_verified)
        .bind(&dto.image)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to create user: {}", e);
            // Check for unique constraint violation (email already exists)
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.constraint() == Some("User_email_key") {
                    return DatabaseError::ValidationError(
                        "User with this email already exists".to_string(),
                    );
                }
            }
            DatabaseError::QueryError(e)
        })?;

        let user = User {
            id: row.get("id"),
            name: row.get("name"),
            email: row.get("email"),
            email_verified: row.get("emailVerified"),
            image: row.get("image"),
            created_at: row.get("createdAt"),
            updated_at: row.get("updatedAt"),
        };

        info!("Successfully created user with ID: {}", user.id);
        Ok(user)
    }

    async fn get_user_by_id(&self, id: &str) -> Result<Option<User>, DatabaseError> {
        debug!("Fetching user by ID: {}", id);

        let row = sqlx::query(
            r#"
            SELECT 
                id,
                name,
                email,
                "emailVerified",
                image,
                "createdAt",
                "updatedAt"
            FROM "User"
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to fetch user by ID {}: {}", id, e);
            DatabaseError::QueryError(e)
        })?;

        let user = match row {
            Some(row) => {
                debug!("Found user with ID: {}", id);
                Some(User {
                    id: row.get("id"),
                    name: row.get("name"),
                    email: row.get("email"),
                    email_verified: row.get("emailVerified"),
                    image: row.get("image"),
                    created_at: row.get("createdAt"),
                    updated_at: row.get("updatedAt"),
                })
            }
            None => {
                debug!("No user found with ID: {}", id);
                None
            }
        };

        Ok(user)
    }

    async fn get_user_by_email(&self, email: &str) -> Result<Option<User>, DatabaseError> {
        debug!("Fetching user by email: {}", email);

        Self::validate_email(email)?;

        let row = sqlx::query(
            r#"
            SELECT 
                id,
                name,
                email,
                "emailVerified",
                image,
                "createdAt",
                "updatedAt"
            FROM "User"
            WHERE email = $1
            "#,
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to fetch user by email {}: {}", email, e);
            DatabaseError::QueryError(e)
        })?;

        let user = match row {
            Some(row) => {
                debug!("Found user with email: {}", email);
                Some(User {
                    id: row.get("id"),
                    name: row.get("name"),
                    email: row.get("email"),
                    email_verified: row.get("emailVerified"),
                    image: row.get("image"),
                    created_at: row.get("createdAt"),
                    updated_at: row.get("updatedAt"),
                })
            }
            None => {
                debug!("No user found with email: {}", email);
                None
            }
        };

        Ok(user)
    }

    async fn update_user(
        &self,
        id: &str,
        dto: &UpdateUserDto,
    ) -> Result<Option<User>, DatabaseError> {
        debug!("Updating user with ID: {}", id);

        // Get current user to preserve existing values
        let current_user = self.get_user_by_id(id).await?;
        let current_user = match current_user {
            Some(user) => user,
            None => {
                warn!("Attempted to update non-existent user: {}", id);
                return Ok(None);
            }
        };

        // Validate new values
        let email = dto.email.as_ref().unwrap_or(&current_user.email);
        Self::validate_email(email)?;

        let name = dto.name.as_ref().or(current_user.name.as_ref());
        if let Some(name) = name {
            Self::validate_name(name)?;
        }

        let now = Utc::now();
        let email_verified = dto.email_verified.unwrap_or(current_user.email_verified);
        let image = dto.image.as_ref().or(current_user.image.as_ref());

        let row = sqlx::query(
            r#"
            UPDATE "User"
            SET 
                name = $2,
                email = $3,
                "emailVerified" = $4,
                image = $5,
                "updatedAt" = $6
            WHERE id = $1
            RETURNING 
                id,
                name,
                email,
                "emailVerified",
                image,
                "createdAt",
                "updatedAt"
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(email)
        .bind(email_verified)
        .bind(image)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to update user {}: {}", id, e);
            // Check for unique constraint violation (email already exists)
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.constraint() == Some("User_email_key") {
                    return DatabaseError::ValidationError(
                        "User with this email already exists".to_string(),
                    );
                }
            }
            DatabaseError::QueryError(e)
        })?;

        let user = match row {
            Some(row) => {
                info!("Successfully updated user with ID: {}", id);
                Some(User {
                    id: row.get("id"),
                    name: row.get("name"),
                    email: row.get("email"),
                    email_verified: row.get("emailVerified"),
                    image: row.get("image"),
                    created_at: row.get("createdAt"),
                    updated_at: row.get("updatedAt"),
                })
            }
            None => None,
        };

        Ok(user)
    }
    async fn delete_user(&self, id: &str) -> Result<bool, DatabaseError> {
        debug!("Deleting user with ID: {}", id);

        // First delete related sessions and accounts (cascade delete)
        self.delete_user_sessions(id).await?;

        let result = sqlx::query(r#"DELETE FROM "User" WHERE id = $1"#)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to delete user {}: {}", id, e);
                DatabaseError::QueryError(e)
            })?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!("Successfully deleted user with ID: {}", id);
        } else {
            warn!("No user found to delete with ID: {}", id);
        }

        Ok(deleted)
    }

    async fn list_users(
        &self,
        filter: &UserFilter,
        pagination: &PaginationParams,
    ) -> Result<(Vec<User>, u64), DatabaseError> {
        debug!("Listing users with filter: {:?}", filter);

        let page = pagination.page.unwrap_or(1);
        let limit = pagination.limit.unwrap_or(20);
        let offset = (page - 1) * limit;

        // Build the WHERE clause for filtering
        let (where_clause, _params) = Self::build_user_filter_clause(filter);

        // Count total matching records
        let count_query = format!(r#"SELECT COUNT(*) as count FROM "User" {where_clause}"#);

        let total_count: i64 = sqlx::query(&count_query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to count users: {}", e);
                DatabaseError::QueryError(e)
            })?
            .get("count");

        // Fetch paginated results
        let data_query = format!(
            r#"
            SELECT 
                id,
                name,
                email,
                "emailVerified" as email_verified,
                image,
                "createdAt" as created_at,
                "updatedAt" as updated_at
            FROM "User"
            {where_clause}
            ORDER BY "createdAt" DESC
            LIMIT $1 OFFSET $2
            "#
        );

        let rows = sqlx::query(&data_query)
            .bind(limit as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to fetch users: {}", e);
                DatabaseError::QueryError(e)
            })?;

        let users: Result<Vec<User>, DatabaseError> = rows
            .into_iter()
            .map(|row| {
                Ok::<User, DatabaseError>(User {
                    id: row.get("id"),
                    name: row.get("name"),
                    email: row.get("email"),
                    email_verified: row.get("email_verified"),
                    image: row.get("image"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect();

        let users = users?;

        debug!("Found {} users (total: {})", users.len(), total_count);
        Ok((users, total_count as u64))
    }

    async fn verify_user_email(&self, id: &str) -> Result<Option<User>, DatabaseError> {
        debug!("Verifying email for user: {}", id);

        let now = Utc::now();

        let row = sqlx::query(
            r#"
            UPDATE "User"
            SET 
                "emailVerified" = true,
                "updatedAt" = $2
            WHERE id = $1
            RETURNING 
                id,
                name,
                email,
                "emailVerified",
                image,
                "createdAt",
                "updatedAt"
            "#,
        )
        .bind(id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to verify email for user {}: {}", id, e);
            DatabaseError::QueryError(e)
        })?;

        let user = match row {
            Some(row) => {
                info!("Successfully verified email for user: {}", id);
                Some(User {
                    id: row.get("id"),
                    name: row.get("name"),
                    email: row.get("email"),
                    email_verified: row.get("emailVerified"),
                    image: row.get("image"),
                    created_at: row.get("createdAt"),
                    updated_at: row.get("updatedAt"),
                })
            }
            None => {
                warn!("No user found to verify email for ID: {}", id);
                None
            }
        };

        Ok(user)
    }

    // Session operations
    async fn create_session(&self, dto: &CreateSessionDto) -> Result<Session, DatabaseError> {
        debug!("Creating new session for user: {}", dto.user_id);

        let session_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let row = sqlx::query(
            r#"
            INSERT INTO "Session" (
                id, "userId", token, "expiresAt", "ipAddress", 
                "userAgent", "createdAt", "updatedAt"
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING 
                id,
                "userId",
                token,
                "expiresAt",
                "ipAddress",
                "userAgent",
                "createdAt",
                "updatedAt"
            "#,
        )
        .bind(&session_id)
        .bind(&dto.user_id)
        .bind(&dto.token)
        .bind(dto.expires_at)
        .bind(&dto.ip_address)
        .bind(&dto.user_agent)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to create session: {}", e);
            DatabaseError::QueryError(e)
        })?;

        let session = Session {
            id: row.get("id"),
            user_id: row.get("userId"),
            token: row.get("token"),
            expires_at: row.get("expiresAt"),
            ip_address: row.get("ipAddress"),
            user_agent: row.get("userAgent"),
            created_at: row.get("createdAt"),
            updated_at: row.get("updatedAt"),
        };

        info!("Successfully created session with ID: {}", session.id);
        Ok(session)
    }
    async fn get_session_by_token(&self, token: &str) -> Result<Option<Session>, DatabaseError> {
        debug!("Fetching session by token");

        let row = sqlx::query(
            r#"
            SELECT 
                id,
                "userId",
                token,
                "expiresAt",
                "ipAddress",
                "userAgent",
                "createdAt",
                "updatedAt"
            FROM "Session"
            WHERE token = $1
            "#,
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to fetch session by token: {}", e);
            DatabaseError::QueryError(e)
        })?;

        let session = match row {
            Some(row) => {
                debug!("Found session by token");
                Some(Session {
                    id: row.get("id"),
                    user_id: row.get("userId"),
                    token: row.get("token"),
                    expires_at: row.get("expiresAt"),
                    ip_address: row.get("ipAddress"),
                    user_agent: row.get("userAgent"),
                    created_at: row.get("createdAt"),
                    updated_at: row.get("updatedAt"),
                })
            }
            None => {
                debug!("No session found with provided token");
                None
            }
        };

        Ok(session)
    }

    async fn get_sessions_by_user_id(&self, user_id: &str) -> Result<Vec<Session>, DatabaseError> {
        debug!("Fetching sessions for user: {}", user_id);

        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                "userId",
                token,
                "expiresAt",
                "ipAddress",
                "userAgent",
                "createdAt",
                "updatedAt"
            FROM "Session"
            WHERE "userId" = $1
            ORDER BY "createdAt" DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to fetch sessions for user {}: {}", user_id, e);
            DatabaseError::QueryError(e)
        })?;

        let sessions: Result<Vec<Session>, DatabaseError> = rows
            .into_iter()
            .map(|row| {
                Ok::<Session, DatabaseError>(Session {
                    id: row.get("id"),
                    user_id: row.get("userId"),
                    token: row.get("token"),
                    expires_at: row.get("expiresAt"),
                    ip_address: row.get("ipAddress"),
                    user_agent: row.get("userAgent"),
                    created_at: row.get("createdAt"),
                    updated_at: row.get("updatedAt"),
                })
            })
            .collect();

        let sessions = sessions?;
        debug!("Found {} sessions for user {}", sessions.len(), user_id);
        Ok(sessions)
    }

    async fn delete_session(&self, id: &str) -> Result<bool, DatabaseError> {
        debug!("Deleting session with ID: {}", id);

        let result = sqlx::query(r#"DELETE FROM "Session" WHERE id = $1"#)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to delete session {}: {}", id, e);
                DatabaseError::QueryError(e)
            })?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!("Successfully deleted session with ID: {}", id);
        } else {
            warn!("No session found to delete with ID: {}", id);
        }

        Ok(deleted)
    }

    async fn delete_expired_sessions(&self) -> Result<u64, DatabaseError> {
        debug!("Deleting expired sessions");

        let now = Utc::now();
        let result = sqlx::query(r#"DELETE FROM "Session" WHERE "expiresAt" <= $1"#)
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to delete expired sessions: {}", e);
                DatabaseError::QueryError(e)
            })?;

        let deleted_count = result.rows_affected();
        info!("Successfully deleted {} expired sessions", deleted_count);
        Ok(deleted_count)
    }

    async fn delete_user_sessions(&self, user_id: &str) -> Result<u64, DatabaseError> {
        debug!("Deleting all sessions for user: {}", user_id);

        let result = sqlx::query(r#"DELETE FROM "Session" WHERE "userId" = $1"#)
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to delete sessions for user {}: {}", user_id, e);
                DatabaseError::QueryError(e)
            })?;

        let deleted_count = result.rows_affected();
        info!(
            "Successfully deleted {} sessions for user {}",
            deleted_count, user_id
        );
        Ok(deleted_count)
    }

    // Account operations
    async fn create_account(&self, dto: &CreateAccountDto) -> Result<Account, DatabaseError> {
        debug!(
            "Creating new account for user: {} with provider: {}",
            dto.user_id, dto.provider_id
        );

        let account_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let row = sqlx::query(
            r#"
            INSERT INTO "Account" (
                id, "userId", "accountId", "providerId", "accessToken", 
                "refreshToken", "accessTokenExpiresAt", "refreshTokenExpiresAt",
                scope, "idToken", password, "createdAt", "updatedAt"
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING 
                id,
                "userId",
                "accountId",
                "providerId",
                "accessToken",
                "refreshToken",
                "accessTokenExpiresAt",
                "refreshTokenExpiresAt",
                scope,
                "idToken",
                password,
                "createdAt",
                "updatedAt"
            "#,
        )
        .bind(&account_id)
        .bind(&dto.user_id)
        .bind(&dto.account_id)
        .bind(&dto.provider_id)
        .bind(&dto.access_token)
        .bind(&dto.refresh_token)
        .bind(dto.access_token_expires_at)
        .bind(dto.refresh_token_expires_at)
        .bind(&dto.scope)
        .bind(&dto.id_token)
        .bind(&dto.password)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to create account: {}", e);
            DatabaseError::QueryError(e)
        })?;

        let account = Account {
            id: row.get("id"),
            user_id: row.get("userId"),
            account_id: row.get("accountId"),
            provider_id: row.get("providerId"),
            access_token: row.get("accessToken"),
            refresh_token: row.get("refreshToken"),
            access_token_expires_at: row.get("accessTokenExpiresAt"),
            refresh_token_expires_at: row.get("refreshTokenExpiresAt"),
            scope: row.get("scope"),
            id_token: row.get("idToken"),
            password: row.get("password"),
            created_at: row.get("createdAt"),
            updated_at: row.get("updatedAt"),
        };

        info!("Successfully created account with ID: {}", account.id);
        Ok(account)
    }
    async fn get_account_by_provider(
        &self,
        user_id: &str,
        provider_id: &str,
    ) -> Result<Option<Account>, DatabaseError> {
        debug!(
            "Fetching account for user {} with provider {}",
            user_id, provider_id
        );

        let row = sqlx::query(
            r#"
            SELECT 
                id,
                "userId",
                "accountId",
                "providerId",
                "accessToken",
                "refreshToken",
                "accessTokenExpiresAt",
                "refreshTokenExpiresAt",
                scope,
                "idToken",
                password,
                "createdAt",
                "updatedAt"
            FROM "Account"
            WHERE "userId" = $1 AND "providerId" = $2
            "#,
        )
        .bind(user_id)
        .bind(provider_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!(
                "Failed to fetch account for user {} with provider {}: {}",
                user_id, provider_id, e
            );
            DatabaseError::QueryError(e)
        })?;

        let account = match row {
            Some(row) => {
                debug!(
                    "Found account for user {} with provider {}",
                    user_id, provider_id
                );
                Some(Account {
                    id: row.get("id"),
                    user_id: row.get("userId"),
                    account_id: row.get("accountId"),
                    provider_id: row.get("providerId"),
                    access_token: row.get("accessToken"),
                    refresh_token: row.get("refreshToken"),
                    access_token_expires_at: row.get("accessTokenExpiresAt"),
                    refresh_token_expires_at: row.get("refreshTokenExpiresAt"),
                    scope: row.get("scope"),
                    id_token: row.get("idToken"),
                    password: row.get("password"),
                    created_at: row.get("createdAt"),
                    updated_at: row.get("updatedAt"),
                })
            }
            None => {
                debug!(
                    "No account found for user {} with provider {}",
                    user_id, provider_id
                );
                None
            }
        };

        Ok(account)
    }

    async fn get_accounts_by_user_id(&self, user_id: &str) -> Result<Vec<Account>, DatabaseError> {
        debug!("Fetching accounts for user: {}", user_id);

        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                "userId",
                "accountId",
                "providerId",
                "accessToken",
                "refreshToken",
                "accessTokenExpiresAt",
                "refreshTokenExpiresAt",
                scope,
                "idToken",
                password,
                "createdAt",
                "updatedAt"
            FROM "Account"
            WHERE "userId" = $1
            ORDER BY "createdAt" DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to fetch accounts for user {}: {}", user_id, e);
            DatabaseError::QueryError(e)
        })?;

        let accounts: Result<Vec<Account>, DatabaseError> = rows
            .into_iter()
            .map(|row| {
                Ok::<Account, DatabaseError>(Account {
                    id: row.get("id"),
                    user_id: row.get("userId"),
                    account_id: row.get("accountId"),
                    provider_id: row.get("providerId"),
                    access_token: row.get("accessToken"),
                    refresh_token: row.get("refreshToken"),
                    access_token_expires_at: row.get("accessTokenExpiresAt"),
                    refresh_token_expires_at: row.get("refreshTokenExpiresAt"),
                    scope: row.get("scope"),
                    id_token: row.get("idToken"),
                    password: row.get("password"),
                    created_at: row.get("createdAt"),
                    updated_at: row.get("updatedAt"),
                })
            })
            .collect();

        let accounts = accounts?;
        debug!("Found {} accounts for user {}", accounts.len(), user_id);
        Ok(accounts)
    }

    async fn update_account_tokens(
        &self,
        id: &str,
        access_token: Option<String>,
        refresh_token: Option<String>,
        access_token_expires_at: Option<DateTime<Utc>>,
        refresh_token_expires_at: Option<DateTime<Utc>>,
    ) -> Result<Option<Account>, DatabaseError> {
        debug!("Updating tokens for account: {}", id);

        let now = Utc::now();

        let row = sqlx::query(
            r#"
            UPDATE "Account"
            SET 
                "accessToken" = $2,
                "refreshToken" = $3,
                "accessTokenExpiresAt" = $4,
                "refreshTokenExpiresAt" = $5,
                "updatedAt" = $6
            WHERE id = $1
            RETURNING 
                id,
                "userId",
                "accountId",
                "providerId",
                "accessToken",
                "refreshToken",
                "accessTokenExpiresAt",
                "refreshTokenExpiresAt",
                scope,
                "idToken",
                password,
                "createdAt",
                "updatedAt"
            "#,
        )
        .bind(id)
        .bind(&access_token)
        .bind(&refresh_token)
        .bind(access_token_expires_at)
        .bind(refresh_token_expires_at)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to update account tokens {}: {}", id, e);
            DatabaseError::QueryError(e)
        })?;

        let account = match row {
            Some(row) => {
                info!("Successfully updated tokens for account: {}", id);
                Some(Account {
                    id: row.get("id"),
                    user_id: row.get("userId"),
                    account_id: row.get("accountId"),
                    provider_id: row.get("providerId"),
                    access_token: row.get("accessToken"),
                    refresh_token: row.get("refreshToken"),
                    access_token_expires_at: row.get("accessTokenExpiresAt"),
                    refresh_token_expires_at: row.get("refreshTokenExpiresAt"),
                    scope: row.get("scope"),
                    id_token: row.get("idToken"),
                    password: row.get("password"),
                    created_at: row.get("createdAt"),
                    updated_at: row.get("updatedAt"),
                })
            }
            None => {
                warn!("No account found to update tokens for ID: {}", id);
                None
            }
        };

        Ok(account)
    }

    async fn update_account_password(
        &self,
        id: &str,
        password_hash: &str,
    ) -> Result<Option<Account>, DatabaseError> {
        debug!("Updating password for account: {}", id);

        let now = Utc::now();

        let row = sqlx::query(
            r#"
            UPDATE "Account"
            SET 
                password = $2,
                "updatedAt" = $3
            WHERE id = $1
            RETURNING 
                id,
                "userId",
                "accountId",
                "providerId",
                "accessToken",
                "refreshToken",
                "accessTokenExpiresAt",
                "refreshTokenExpiresAt",
                scope,
                "idToken",
                password,
                "createdAt",
                "updatedAt"
            "#,
        )
        .bind(id)
        .bind(password_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to update account password {}: {}", id, e);
            DatabaseError::QueryError(e)
        })?;

        let account = match row {
            Some(row) => {
                info!("Successfully updated password for account: {}", id);
                Some(Account {
                    id: row.get("id"),
                    user_id: row.get("userId"),
                    account_id: row.get("accountId"),
                    provider_id: row.get("providerId"),
                    access_token: row.get("accessToken"),
                    refresh_token: row.get("refreshToken"),
                    access_token_expires_at: row.get("accessTokenExpiresAt"),
                    refresh_token_expires_at: row.get("refreshTokenExpiresAt"),
                    scope: row.get("scope"),
                    id_token: row.get("idToken"),
                    password: row.get("password"),
                    created_at: row.get("createdAt"),
                    updated_at: row.get("updatedAt"),
                })
            }
            None => {
                warn!("No account found to update password for ID: {}", id);
                None
            }
        };

        Ok(account)
    }

    async fn delete_account(&self, id: &str) -> Result<bool, DatabaseError> {
        debug!("Deleting account with ID: {}", id);

        let result = sqlx::query(r#"DELETE FROM "Account" WHERE id = $1"#)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to delete account {}: {}", id, e);
                DatabaseError::QueryError(e)
            })?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!("Successfully deleted account with ID: {}", id);
        } else {
            warn!("No account found to delete with ID: {}", id);
        }

        Ok(deleted)
    } // Verification operations
    async fn create_verification(
        &self,
        identifier: &str,
        value: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<Verification, DatabaseError> {
        debug!("Creating verification for identifier: {}", identifier);

        let verification_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let row = sqlx::query(
            r#"
            INSERT INTO "Verification" (
                id, identifier, value, "expiresAt", "createdAt", "updatedAt"
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING 
                id,
                identifier,
                value,
                "expiresAt",
                "createdAt",
                "updatedAt"
            "#,
        )
        .bind(&verification_id)
        .bind(identifier)
        .bind(value)
        .bind(expires_at)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to create verification: {}", e);
            DatabaseError::QueryError(e)
        })?;

        let verification = Verification {
            id: row.get("id"),
            identifier: row.get("identifier"),
            value: row.get("value"),
            expires_at: row.get("expiresAt"),
            created_at: row.get("createdAt"),
            updated_at: row.get("updatedAt"),
        };

        info!(
            "Successfully created verification with ID: {}",
            verification.id
        );
        Ok(verification)
    }

    async fn get_verification(
        &self,
        identifier: &str,
        value: &str,
    ) -> Result<Option<Verification>, DatabaseError> {
        debug!("Fetching verification for identifier: {}", identifier);

        let row = sqlx::query(
            r#"
            SELECT 
                id,
                identifier,
                value,
                "expiresAt",
                "createdAt",
                "updatedAt"
            FROM "Verification"
            WHERE identifier = $1 AND value = $2
            "#,
        )
        .bind(identifier)
        .bind(value)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!(
                "Failed to fetch verification for identifier {}: {}",
                identifier, e
            );
            DatabaseError::QueryError(e)
        })?;

        let verification = match row {
            Some(row) => {
                debug!("Found verification for identifier: {}", identifier);
                Some(Verification {
                    id: row.get("id"),
                    identifier: row.get("identifier"),
                    value: row.get("value"),
                    expires_at: row.get("expiresAt"),
                    created_at: row.get("createdAt"),
                    updated_at: row.get("updatedAt"),
                })
            }
            None => {
                debug!("No verification found for identifier: {}", identifier);
                None
            }
        };

        Ok(verification)
    }

    async fn delete_verification(&self, id: &str) -> Result<bool, DatabaseError> {
        debug!("Deleting verification with ID: {}", id);

        let result = sqlx::query(r#"DELETE FROM "Verification" WHERE id = $1"#)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to delete verification {}: {}", id, e);
                DatabaseError::QueryError(e)
            })?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!("Successfully deleted verification with ID: {}", id);
        } else {
            warn!("No verification found to delete with ID: {}", id);
        }

        Ok(deleted)
    }

    async fn delete_expired_verifications(&self) -> Result<u64, DatabaseError> {
        debug!("Deleting expired verifications");

        let now = Utc::now();
        let result = sqlx::query(r#"DELETE FROM "Verification" WHERE "expiresAt" <= $1"#)
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to delete expired verifications: {}", e);
                DatabaseError::QueryError(e)
            })?;

        let deleted_count = result.rows_affected();
        info!(
            "Successfully deleted {} expired verifications",
            deleted_count
        );
        Ok(deleted_count)
    }
}
