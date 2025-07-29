use std::sync::Arc;

use axum::async_trait;
use chrono::Utc;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use sqlx::{PgPool, Row};
use tracing::{debug, warn};

use crate::auth::{AuthContext, AuthError, AuthSession, AuthUser, JwtClaims};

/// Authentication service for Better Auth integration
#[derive(Clone)]
pub struct AuthService {
    db_pool: Arc<PgPool>,
    jwt_secret: String,
    auth_enabled: bool,
}

impl AuthService {
    pub fn new(db_pool: Arc<PgPool>, jwt_secret: String, auth_enabled: bool) -> Self {
        Self {
            db_pool,
            jwt_secret,
            auth_enabled,
        }
    }

    /// Validate JWT token and return authentication context
    pub async fn validate_token(&self, token: &str) -> Result<AuthContext, AuthError> {
        if !self.auth_enabled {
            debug!("Authentication disabled, skipping token validation");
            return Err(AuthError::InvalidToken(
                "Authentication disabled".to_string(),
            ));
        }

        // Decode and validate JWT token
        let claims = self.decode_jwt(token)?;

        // Check if token is expired
        let now = Utc::now().timestamp();
        if claims.exp < now {
            warn!("Token expired: exp={}, now={}", claims.exp, now);
            return Err(AuthError::TokenExpired);
        }

        // Fetch session from database
        let session = self.get_session(&claims.session_id).await?;

        // Check if session is expired
        if session.expires_at < Utc::now() {
            warn!("Session expired: session_id={}", session.id);
            return Err(AuthError::TokenExpired);
        }

        // Fetch user from database
        let user = self.get_user(&session.user_id).await?;

        Ok(AuthContext { user, session })
    }

    /// Extract token from Authorization header
    pub fn extract_token_from_header(&self, auth_header: &str) -> Result<String, AuthError> {
        if !auth_header.starts_with("Bearer ") {
            return Err(AuthError::InvalidAuthHeaderFormat);
        }

        let token = auth_header.strip_prefix("Bearer ").unwrap();
        if token.is_empty() {
            return Err(AuthError::InvalidAuthHeaderFormat);
        }

        Ok(token.to_string())
    }

    /// Decode JWT token and extract claims
    fn decode_jwt(&self, token: &str) -> Result<JwtClaims, AuthError> {
        let key = DecodingKey::from_secret(self.jwt_secret.as_ref());
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = false; // We'll validate expiration manually

        let token_data = decode::<JwtClaims>(token, &key, &validation)?;
        Ok(token_data.claims)
    }

    /// Fetch session from database
    async fn get_session(&self, session_id: &str) -> Result<AuthSession, AuthError> {
        let row = sqlx::query(
            r#"
            SELECT 
                id,
                "userId" as user_id,
                token,
                "expiresAt" as expires_at,
                "ipAddress" as ip_address,
                "userAgent" as user_agent,
                "createdAt" as created_at,
                "updatedAt" as updated_at
            FROM "Session"
            WHERE id = $1
            "#,
        )
        .bind(session_id)
        .fetch_optional(&*self.db_pool)
        .await?;

        match row {
            Some(row) => {
                let session = AuthSession {
                    id: row.get("id"),
                    user_id: row.get("user_id"),
                    token: row.get("token"),
                    expires_at: row.get("expires_at"),
                    ip_address: row.get("ip_address"),
                    user_agent: row.get("user_agent"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                };
                Ok(session)
            }
            None => Err(AuthError::SessionNotFound),
        }
    }

    /// Fetch user from database
    async fn get_user(&self, user_id: &str) -> Result<AuthUser, AuthError> {
        let row = sqlx::query(
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
            WHERE id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&*self.db_pool)
        .await?;

        match row {
            Some(row) => {
                let user = AuthUser {
                    id: row.get("id"),
                    name: row.get("name"),
                    email: row.get("email"),
                    email_verified: row.get("email_verified"),
                    image: row.get("image"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                };
                Ok(user)
            }
            None => Err(AuthError::UserNotFound),
        }
    }

    /// Check if authentication is enabled
    pub fn is_auth_enabled(&self) -> bool {
        self.auth_enabled
    }
}

#[async_trait]
pub trait AuthServiceTrait: Send + Sync {
    async fn validate_token(&self, token: &str) -> Result<AuthContext, AuthError>;
    fn extract_token_from_header(&self, auth_header: &str) -> Result<String, AuthError>;
    fn is_auth_enabled(&self) -> bool;
}

#[async_trait]
impl AuthServiceTrait for AuthService {
    async fn validate_token(&self, token: &str) -> Result<AuthContext, AuthError> {
        self.validate_token(token).await
    }

    fn extract_token_from_header(&self, auth_header: &str) -> Result<String, AuthError> {
        self.extract_token_from_header(auth_header)
    }

    fn is_auth_enabled(&self) -> bool {
        self.is_auth_enabled()
    }
}

#[cfg(test)]
mod tests {
    use mockall::{mock, predicate::*};

    use super::*;

    mock! {
        AuthService {}

        #[async_trait]
        impl AuthServiceTrait for AuthService {
            async fn validate_token(&self, token: &str) -> Result<AuthContext, AuthError>;
            fn extract_token_from_header(&self, auth_header: &str) -> Result<String, AuthError>;
            fn is_auth_enabled(&self) -> bool;
        }
    }

    #[tokio::test]
    async fn test_extract_token_from_header_success() {
        // Create a mock pool for testing
        let pool =
            Arc::new(sqlx::PgPool::connect_lazy("postgresql://localhost:5432/test").unwrap());
        let service = AuthService::new(pool, "secret".to_string(), true);

        let result = service.extract_token_from_header("Bearer abc123");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "abc123");
    }

    #[tokio::test]
    async fn test_extract_token_from_header_invalid_format() {
        let pool =
            Arc::new(sqlx::PgPool::connect_lazy("postgresql://localhost:5432/test").unwrap());
        let service = AuthService::new(pool, "secret".to_string(), true);

        let result = service.extract_token_from_header("Invalid abc123");
        assert!(matches!(result, Err(AuthError::InvalidAuthHeaderFormat)));
    }

    #[tokio::test]
    async fn test_extract_token_from_header_empty_token() {
        let pool =
            Arc::new(sqlx::PgPool::connect_lazy("postgresql://localhost:5432/test").unwrap());
        let service = AuthService::new(pool, "secret".to_string(), true);

        let result = service.extract_token_from_header("Bearer ");
        assert!(matches!(result, Err(AuthError::InvalidAuthHeaderFormat)));
    }
}
