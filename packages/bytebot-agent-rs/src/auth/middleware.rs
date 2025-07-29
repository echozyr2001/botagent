use std::sync::Arc;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use tracing::{debug, error, warn};

use crate::auth::{AuthContext, AuthError, AuthServiceTrait};

/// Authentication middleware that validates JWT tokens and extracts user context
pub async fn auth_middleware(
    State(auth_service): State<Arc<dyn AuthServiceTrait>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip authentication if disabled
    if !auth_service.is_auth_enabled() {
        debug!("Authentication disabled, skipping middleware");
        return Ok(next.run(request).await);
    }

    // Extract authorization header
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok());

    let auth_header = match auth_header {
        Some(header) => header,
        None => {
            warn!("Missing authorization header");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Extract token from header
    let token = match auth_service.extract_token_from_header(auth_header) {
        Ok(token) => token,
        Err(AuthError::InvalidAuthHeaderFormat) => {
            warn!("Invalid authorization header format");
            return Err(StatusCode::UNAUTHORIZED);
        }
        Err(e) => {
            error!("Error extracting token: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Validate token and get auth context
    let auth_context = match auth_service.validate_token(&token).await {
        Ok(context) => context,
        Err(AuthError::TokenExpired) => {
            warn!("Token expired");
            return Err(StatusCode::UNAUTHORIZED);
        }
        Err(AuthError::InvalidToken(msg)) => {
            warn!("Invalid token: {}", msg);
            return Err(StatusCode::UNAUTHORIZED);
        }
        Err(AuthError::SessionNotFound) => {
            warn!("Session not found");
            return Err(StatusCode::UNAUTHORIZED);
        }
        Err(AuthError::UserNotFound) => {
            warn!("User not found");
            return Err(StatusCode::UNAUTHORIZED);
        }
        Err(e) => {
            error!("Authentication error: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    debug!("Authentication successful for user: {}", auth_context.user.id);

    // Add auth context to request extensions
    request.extensions_mut().insert(auth_context);

    Ok(next.run(request).await)
}

/// Optional authentication middleware that doesn't fail if no auth is provided
/// but still validates tokens if present
pub async fn optional_auth_middleware(
    State(auth_service): State<Arc<dyn AuthServiceTrait>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip authentication if disabled
    if !auth_service.is_auth_enabled() {
        debug!("Authentication disabled, skipping optional middleware");
        return Ok(next.run(request).await);
    }

    // Extract authorization header
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok());

    if let Some(auth_header) = auth_header {
        // Extract token from header
        let token = match auth_service.extract_token_from_header(auth_header) {
            Ok(token) => token,
            Err(_) => {
                // Invalid header format, but this is optional auth so continue without auth
                debug!("Invalid authorization header format in optional auth, continuing without auth");
                return Ok(next.run(request).await);
            }
        };

        // Validate token and get auth context
        match auth_service.validate_token(&token).await {
            Ok(auth_context) => {
                debug!("Optional authentication successful for user: {}", auth_context.user.id);
                request.extensions_mut().insert(auth_context);
            }
            Err(e) => {
                debug!("Optional authentication failed: {}, continuing without auth", e);
                // Continue without auth context
            }
        }
    } else {
        debug!("No authorization header in optional auth, continuing without auth");
    }

    Ok(next.run(request).await)
}

/// Extension trait to extract auth context from request
pub trait AuthContextExtractor {
    fn auth_context(&self) -> Option<&AuthContext>;
    fn require_auth_context(&self) -> Result<&AuthContext, StatusCode>;
}

impl AuthContextExtractor for Request {
    fn auth_context(&self) -> Option<&AuthContext> {
        self.extensions().get::<AuthContext>()
    }

    fn require_auth_context(&self) -> Result<&AuthContext, StatusCode> {
        self.auth_context().ok_or(StatusCode::UNAUTHORIZED)
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_context_extractor_trait_exists() {
        // Simple test to verify the trait compiles
        let request = axum::http::Request::builder()
            .body(axum::body::Body::empty())
            .unwrap();
        
        // Test that the trait methods exist
        assert!(request.auth_context().is_none());
    }
}