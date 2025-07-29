# Better Auth Integration for ByteBot Rust Agent

This module provides a complete Better Auth integration layer for the ByteBot Rust agent service, maintaining compatibility with the existing TypeScript implementation.

## Overview

The authentication system provides JWT token validation, user session management, and middleware for protecting routes. It integrates with the existing Better Auth database schema and supports both required and optional authentication patterns.

## Components

### Types (`types.rs`)
- `AuthUser`: User information extracted from session
- `AuthSession`: Session information from Better Auth
- `AuthContext`: Combined authentication context
- `JwtClaims`: JWT token structure for Better Auth tokens
- `AuthError`: Comprehensive error types for authentication failures

### Service (`service.rs`)
- `AuthService`: Core service for Better Auth integration
- JWT token validation with proper expiration checking
- Database integration for session and user lookup
- Token extraction from Authorization headers
- Configurable authentication (can be enabled/disabled)

### Middleware (`middleware.rs`)
- `auth_middleware`: Strict authentication middleware that requires valid tokens
- `optional_auth_middleware`: Optional authentication that works with or without tokens
- `AuthContextExtractor`: Trait for extracting auth context from requests

## Configuration

Add these environment variables to your `.env` file:

```env
AUTH_ENABLED=true
JWT_SECRET=your-jwt-secret-here
```

## Usage

### Basic Setup

```rust
use bytebot_agent_rs::{
    auth::{AuthService, AuthServiceTrait},
    config::Config,
    server::{create_app_state, AppState},
};

// Create application state with authentication
let config = Arc::new(Config::from_env()?);
let app_state = create_app_state(config).await?;
```

### Protected Routes

```rust
use axum::{
    extract::Request,
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use bytebot_agent_rs::auth::AuthContextExtractor;

async fn protected_route(request: Request) -> Result<Json<Value>, StatusCode> {
    // Extract authentication context (fails if not authenticated)
    let auth_context = request.require_auth_context()?;
    
    // Use authenticated user information
    Ok(Json(json!({
        "user_id": auth_context.user.id,
        "email": auth_context.user.email
    })))
}

// Apply authentication middleware to routes
Router::new()
    .route("/protected", get(protected_route))
    .layer(axum::middleware::from_fn_with_state(
        auth_service.clone(), 
        auth_middleware
    ))
```

### Optional Authentication

```rust
async fn optional_auth_route(request: Request) -> Json<Value> {
    match request.auth_context() {
        Some(auth_context) => {
            Json(json!({
                "message": "Hello authenticated user",
                "user_id": auth_context.user.id
            }))
        }
        None => {
            Json(json!({
                "message": "Hello anonymous user"
            }))
        }
    }
}

// Apply optional authentication middleware
Router::new()
    .route("/optional", get(optional_auth_route))
    .layer(axum::middleware::from_fn_with_state(
        auth_service.clone(), 
        optional_auth_middleware
    ))
```

### Client Usage

Send requests with Bearer tokens:

```bash
# Protected endpoint (requires token)
curl -H "Authorization: Bearer your-jwt-token" \
     http://localhost:9991/protected

# Optional endpoint (works with or without token)
curl http://localhost:9991/optional
curl -H "Authorization: Bearer your-jwt-token" \
     http://localhost:9991/optional
```

## Error Handling

The authentication system provides comprehensive error handling:

- `InvalidToken`: Token is malformed or invalid
- `TokenExpired`: Token has expired
- `SessionNotFound`: Session not found in database
- `UserNotFound`: User not found in database
- `MissingAuthHeader`: No Authorization header provided
- `InvalidAuthHeaderFormat`: Authorization header format is invalid

## Database Schema

The authentication system works with the existing Better Auth database schema:

- `User` table: User information
- `Session` table: Active user sessions
- `Account` table: User accounts (OAuth, etc.)
- `Verification` table: Email verification tokens

## Testing

Run authentication tests:

```bash
cargo test auth --lib
```

## Example

See `examples/auth_example.rs` for a complete working example of how to use the authentication system.

## Security Considerations

- JWT secrets should be strong and kept secure
- Tokens are validated for expiration on every request
- Sessions are checked against the database for validity
- All authentication errors are logged for security monitoring
- HTTPS should be used in production to protect tokens in transit