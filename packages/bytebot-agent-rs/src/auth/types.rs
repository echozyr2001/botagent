use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};


/// User information extracted from session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: String,
    pub name: Option<String>,
    pub email: String,
    pub email_verified: bool,
    pub image: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Session information from Better Auth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub id: String,
    pub user_id: String,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Combined authentication context
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user: AuthUser,
    pub session: AuthSession,
}

/// JWT claims structure for Better Auth tokens
#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,        // User ID
    pub session_id: String, // Session ID
    pub exp: i64,          // Expiration timestamp
    pub iat: i64,          // Issued at timestamp
    pub iss: String,       // Issuer
    pub aud: String,       // Audience
}

/// Authentication error types
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Invalid token: {0}")]
    InvalidToken(String),
    
    #[error("Token expired")]
    TokenExpired,
    
    #[error("Session not found")]
    SessionNotFound,
    
    #[error("User not found")]
    UserNotFound,
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    
    #[error("Missing authorization header")]
    MissingAuthHeader,
    
    #[error("Invalid authorization header format")]
    InvalidAuthHeaderFormat,
}