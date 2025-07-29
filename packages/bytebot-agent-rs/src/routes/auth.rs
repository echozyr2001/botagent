use std::sync::Arc;

use axum::{
    extract::{FromRequestParts, State},
    http::request::Parts,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use validator::Validate;

use crate::{
    auth::{AuthContext, AuthServiceTrait},
    database::user_repository::UserRepositoryTrait,
    error::ServiceError,
};

/// Request body for user registration
#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,

    #[validate(length(
        min = 8,
        max = 128,
        message = "Password must be between 8 and 128 characters"
    ))]
    pub password: String,

    #[validate(length(
        min = 1,
        max = 255,
        message = "Name must be between 1 and 255 characters"
    ))]
    pub name: Option<String>,
}

/// Request body for user login
#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,

    #[validate(length(min = 1, max = 128, message = "Password cannot be empty"))]
    pub password: String,
}

/// Request body for password change
#[derive(Debug, Deserialize, Validate)]
pub struct ChangePasswordRequest {
    #[validate(length(min = 1, max = 128, message = "Current password cannot be empty"))]
    pub current_password: String,

    #[validate(length(
        min = 8,
        max = 128,
        message = "New password must be between 8 and 128 characters"
    ))]
    pub new_password: String,
}

/// Response body for successful authentication
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub user: AuthUserResponse,
    pub session: AuthSessionResponse,
    pub token: String,
}

/// User information in auth response
#[derive(Debug, Serialize)]
pub struct AuthUserResponse {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub email_verified: bool,
    pub image: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

/// Session information in auth response
#[derive(Debug, Serialize)]
pub struct AuthSessionResponse {
    pub id: String,
    pub expires_at: chrono::DateTime<Utc>,
    pub created_at: chrono::DateTime<Utc>,
}

/// Response body for user profile
#[derive(Debug, Serialize)]
pub struct ProfileResponse {
    pub user: AuthUserResponse,
    pub sessions: Vec<SessionInfo>,
}

/// Session information for profile response
#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub expires_at: chrono::DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
    pub is_current: bool,
}

/// Error response body
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub details: Option<String>,
}

/// Custom extractor for AuthContext
pub struct AuthContextExtract(pub AuthContext);

#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthContextExtract
where
    S: Send + Sync,
{
    type Rejection = ServiceError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthContext>()
            .cloned()
            .map(AuthContextExtract)
            .ok_or(ServiceError::Unauthorized)
    }
}

/// Authentication service for handling user management
pub struct AuthenticationService {
    user_repository: Arc<dyn UserRepositoryTrait>,
    auth_service: Arc<dyn AuthServiceTrait>,
    jwt_secret: String,
}

impl AuthenticationService {
    pub fn new(
        user_repository: Arc<dyn UserRepositoryTrait>,
        auth_service: Arc<dyn AuthServiceTrait>,
        jwt_secret: String,
    ) -> Self {
        Self {
            user_repository,
            auth_service,
            jwt_secret,
        }
    }

    /// Hash password using Argon2
    pub fn hash_password(&self, password: &str) -> Result<String, ServiceError> {
        use argon2::{
            password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
            Argon2,
        };

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| {
                error!("Failed to hash password: {}", e);
                ServiceError::Internal("Password hashing failed".to_string())
            })?;

        Ok(password_hash.to_string())
    }

    /// Verify password against hash using Argon2
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<bool, ServiceError> {
        use argon2::{
            password_hash::{PasswordHash, PasswordVerifier},
            Argon2,
        };

        let parsed_hash = PasswordHash::new(hash).map_err(|e| {
            error!("Failed to parse password hash: {}", e);
            ServiceError::Internal("Invalid password hash".to_string())
        })?;

        let argon2 = Argon2::default();
        match argon2.verify_password(password.as_bytes(), &parsed_hash) {
            Ok(()) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(e) => {
                error!("Password verification error: {}", e);
                Err(ServiceError::Internal(
                    "Password verification failed".to_string(),
                ))
            }
        }
    }

    /// Generate JWT token for user session
    pub fn generate_jwt_token(
        &self,
        user_id: &str,
        session_id: &str,
    ) -> Result<String, ServiceError> {
        use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};

        let claims = crate::auth::JwtClaims {
            sub: user_id.to_string(),
            session_id: session_id.to_string(),
            exp: (Utc::now() + Duration::hours(24)).timestamp(),
            iat: Utc::now().timestamp(),
            iss: "bytebot-agent-rs".to_string(),
            aud: "bytebot-ui".to_string(),
        };

        let token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_ref()),
        )
        .map_err(|e| {
            error!("Failed to generate JWT token: {}", e);
            ServiceError::Internal("Token generation failed".to_string())
        })?;

        Ok(token)
    }

    /// Create a new user session
    pub async fn create_session(
        &self,
        user_id: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(String, String), ServiceError> {
        use crate::database::user_repository::CreateSessionDto;

        let session_token = Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::days(30); // 30 day session

        let session_dto = CreateSessionDto {
            user_id: user_id.to_string(),
            token: session_token.clone(),
            expires_at,
            ip_address,
            user_agent,
        };

        let session = self.user_repository.create_session(&session_dto).await?;
        let jwt_token = self.generate_jwt_token(user_id, &session.id)?;

        Ok((jwt_token, session.id))
    }
}

/// Register a new user
pub async fn register(
    State(auth_service): State<Arc<AuthenticationService>>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, ServiceError> {
    debug!("User registration attempt for email: {}", request.email);

    // Validate request
    request.validate().map_err(|e| {
        warn!("Registration validation failed: {}", e);
        ServiceError::Validation(format!("Validation error: {e}"))
    })?;

    // Check if user already exists
    if let Some(_existing_user) = auth_service
        .user_repository
        .get_user_by_email(&request.email)
        .await?
    {
        warn!("Registration attempt for existing email: {}", request.email);
        return Err(ServiceError::Validation(
            "User with this email already exists".to_string(),
        ));
    }

    // Hash password
    let password_hash = auth_service.hash_password(&request.password)?;

    // Create user
    use crate::database::user_repository::CreateUserDto;
    let user_dto = CreateUserDto {
        email: request.email.clone(),
        name: request.name,
        email_verified: Some(false),
        image: None,
    };

    let user = auth_service.user_repository.create_user(&user_dto).await?;

    // Create account with password
    use crate::database::user_repository::CreateAccountDto;
    let account_dto = CreateAccountDto {
        user_id: user.id.clone(),
        account_id: user.email.clone(),
        provider_id: "credential".to_string(),
        access_token: None,
        refresh_token: None,
        access_token_expires_at: None,
        refresh_token_expires_at: None,
        scope: None,
        id_token: None,
        password: Some(password_hash),
    };

    auth_service
        .user_repository
        .create_account(&account_dto)
        .await?;

    // Create session
    let (jwt_token, session_id) = auth_service.create_session(&user.id, None, None).await?;

    // Get session details
    let session = auth_service
        .user_repository
        .get_session_by_token(&session_id)
        .await?
        .ok_or_else(|| ServiceError::Internal("Failed to retrieve created session".to_string()))?;

    info!("Successfully registered user: {}", user.id);

    let response = AuthResponse {
        user: AuthUserResponse {
            id: user.id,
            email: user.email,
            name: user.name,
            email_verified: user.email_verified,
            image: user.image,
            created_at: user.created_at,
            updated_at: user.updated_at,
        },
        session: AuthSessionResponse {
            id: session.id,
            expires_at: session.expires_at,
            created_at: session.created_at,
        },
        token: jwt_token,
    };

    Ok(Json(response))
}

/// Login user with email and password
pub async fn login(
    State(auth_service): State<Arc<AuthenticationService>>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, ServiceError> {
    debug!("User login attempt for email: {}", request.email);

    // Validate request
    request.validate().map_err(|e| {
        warn!("Login validation failed: {}", e);
        ServiceError::Validation(format!("Validation error: {e}"))
    })?;

    // Get user by email
    let user = auth_service
        .user_repository
        .get_user_by_email(&request.email)
        .await?
        .ok_or_else(|| {
            warn!("Login attempt for non-existent email: {}", request.email);
            ServiceError::Unauthorized
        })?;

    // Get user's credential account
    let account = auth_service
        .user_repository
        .get_account_by_provider(&user.id, "credential")
        .await?
        .ok_or_else(|| {
            warn!("No credential account found for user: {}", user.id);
            ServiceError::Unauthorized
        })?;

    // Verify password
    let password_hash = account.password.ok_or_else(|| {
        warn!("No password hash found for user: {}", user.id);
        ServiceError::Unauthorized
    })?;

    let password_valid = auth_service.verify_password(&request.password, &password_hash)?;
    if !password_valid {
        warn!("Invalid password for user: {}", user.id);
        return Err(ServiceError::Unauthorized);
    }

    // Create session
    let (jwt_token, session_id) = auth_service.create_session(&user.id, None, None).await?;

    // Get session details
    let session = auth_service
        .user_repository
        .get_session_by_token(&session_id)
        .await?
        .ok_or_else(|| ServiceError::Internal("Failed to retrieve created session".to_string()))?;

    info!("Successfully logged in user: {}", user.id);

    let response = AuthResponse {
        user: AuthUserResponse {
            id: user.id,
            email: user.email,
            name: user.name,
            email_verified: user.email_verified,
            image: user.image,
            created_at: user.created_at,
            updated_at: user.updated_at,
        },
        session: AuthSessionResponse {
            id: session.id,
            expires_at: session.expires_at,
            created_at: session.created_at,
        },
        token: jwt_token,
    };

    Ok(Json(response))
}

/// Logout user (invalidate session)
pub async fn logout(
    State(auth_service): State<Arc<AuthenticationService>>,
    AuthContextExtract(auth_context): AuthContextExtract,
) -> Result<Json<serde_json::Value>, ServiceError> {
    debug!("User logout for user: {}", auth_context.user.id);

    // Delete the current session
    let deleted = auth_service
        .user_repository
        .delete_session(&auth_context.session.id)
        .await?;

    if deleted {
        info!("Successfully logged out user: {}", auth_context.user.id);
        Ok(Json(
            serde_json::json!({ "message": "Logged out successfully" }),
        ))
    } else {
        warn!(
            "Failed to delete session during logout for user: {}",
            auth_context.user.id
        );
        Err(ServiceError::Internal("Failed to logout".to_string()))
    }
}

/// Get current user profile
pub async fn profile(
    State(auth_service): State<Arc<AuthenticationService>>,
    AuthContextExtract(auth_context): AuthContextExtract,
) -> Result<Json<ProfileResponse>, ServiceError> {
    debug!("Profile request for user: {}", auth_context.user.id);

    // Get all user sessions
    let sessions = auth_service
        .user_repository
        .get_sessions_by_user_id(&auth_context.user.id)
        .await?;

    let session_infos: Vec<SessionInfo> = sessions
        .into_iter()
        .map(|session| SessionInfo {
            is_current: session.id == auth_context.session.id,
            id: session.id,
            expires_at: session.expires_at,
            ip_address: session.ip_address,
            user_agent: session.user_agent,
            created_at: session.created_at,
        })
        .collect();

    let response = ProfileResponse {
        user: AuthUserResponse {
            id: auth_context.user.id.clone(),
            email: auth_context.user.email.clone(),
            name: auth_context.user.name.clone(),
            email_verified: auth_context.user.email_verified,
            image: auth_context.user.image.clone(),
            created_at: auth_context.user.created_at,
            updated_at: auth_context.user.updated_at,
        },
        sessions: session_infos,
    };

    Ok(Json(response))
}

/// Change user password
pub async fn change_password(
    State(auth_service): State<Arc<AuthenticationService>>,
    AuthContextExtract(auth_context): AuthContextExtract,
    Json(change_request): Json<ChangePasswordRequest>,
) -> Result<Json<serde_json::Value>, ServiceError> {
    debug!("Password change request for user: {}", auth_context.user.id);

    // Validate request
    change_request.validate().map_err(|e| {
        warn!("Password change validation failed: {}", e);
        ServiceError::Validation(format!("Validation error: {e}"))
    })?;

    // Get user's credential account
    let account = auth_service
        .user_repository
        .get_account_by_provider(&auth_context.user.id, "credential")
        .await?
        .ok_or_else(|| {
            warn!(
                "No credential account found for user: {}",
                auth_context.user.id
            );
            ServiceError::Internal("No credential account found".to_string())
        })?;

    // Verify current password
    let current_password_hash = account.password.ok_or_else(|| {
        warn!("No password hash found for user: {}", auth_context.user.id);
        ServiceError::Internal("No password found".to_string())
    })?;

    let current_password_valid =
        auth_service.verify_password(&change_request.current_password, &current_password_hash)?;
    if !current_password_valid {
        warn!(
            "Invalid current password for user: {}",
            auth_context.user.id
        );
        return Err(ServiceError::Validation(
            "Current password is incorrect".to_string(),
        ));
    }

    // Hash new password
    let new_password_hash = auth_service.hash_password(&change_request.new_password)?;

    // Update account with new password hash
    auth_service
        .user_repository
        .update_account_password(&account.id, &new_password_hash)
        .await?;

    info!(
        "Successfully changed password for user: {}",
        auth_context.user.id
    );

    Ok(Json(
        serde_json::json!({ "message": "Password changed successfully" }),
    ))
}

/// Delete user session
pub async fn delete_session(
    State(auth_service): State<Arc<AuthenticationService>>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    AuthContextExtract(auth_context): AuthContextExtract,
) -> Result<Json<serde_json::Value>, ServiceError> {
    debug!(
        "Delete session request for session: {} by user: {}",
        session_id, auth_context.user.id
    );

    // Verify the session belongs to the authenticated user
    let sessions = auth_service
        .user_repository
        .get_sessions_by_user_id(&auth_context.user.id)
        .await?;
    let session_exists = sessions.iter().any(|s| s.id == session_id);

    if !session_exists {
        warn!(
            "Attempt to delete non-existent or unauthorized session: {}",
            session_id
        );
        return Err(ServiceError::NotFound("Session not found".to_string()));
    }

    // Delete the session
    let deleted = auth_service
        .user_repository
        .delete_session(&session_id)
        .await?;

    if deleted {
        info!(
            "Successfully deleted session: {} for user: {}",
            session_id, auth_context.user.id
        );
        Ok(Json(
            serde_json::json!({ "message": "Session deleted successfully" }),
        ))
    } else {
        warn!("Failed to delete session: {}", session_id);
        Err(ServiceError::Internal(
            "Failed to delete session".to_string(),
        ))
    }
}

/// Create auth routes
pub fn create_auth_routes<S>(
    user_repository: Arc<dyn UserRepositoryTrait>,
    auth_service: Arc<dyn AuthServiceTrait>,
    jwt_secret: String,
) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let authentication_service = Arc::new(AuthenticationService::new(
        user_repository,
        auth_service.clone(),
        jwt_secret,
    ));

    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/profile", get(profile))
        .route("/change-password", post(change_password))
        .route(
            "/sessions/:session_id",
            axum::routing::delete(delete_session),
        )
        .with_state(authentication_service)
}
