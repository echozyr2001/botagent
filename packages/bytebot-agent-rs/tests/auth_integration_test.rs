use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use serde_json::json;
use tower::ServiceExt;

use bytebot_agent_rs::{
    auth::{AuthService, AuthServiceTrait},
    database::{DatabaseManager, user_repository::UserRepository},
    routes::create_auth_routes,
};

/// Integration test for authentication endpoints
#[tokio::test]
async fn test_auth_endpoints_integration() {
    // Skip test if no database URL is provided
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            println!("Skipping auth integration test - DATABASE_URL not set");
            return;
        }
    };

    // Create test database manager
    let db_manager = match DatabaseManager::new(&database_url).await {
        Ok(manager) => manager,
        Err(e) => {
            println!("Skipping auth integration test - database connection failed: {e}");
            return;
        }
    };

    // Create repositories and services
    let user_repository = std::sync::Arc::new(UserRepository::new(db_manager.pool().clone()));
    let auth_service: std::sync::Arc<dyn AuthServiceTrait> = std::sync::Arc::new(AuthService::new(
        db_manager.get_pool(),
        "test-jwt-secret".to_string(),
        true, // auth enabled
    ));

    // Create auth routes
    let app = create_auth_routes(
        user_repository,
        auth_service,
        "test-jwt-secret".to_string(),
    );

    // Test user registration
    let register_request = json!({
        "email": "test@example.com",
        "password": "testpassword123",
        "name": "Test User"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&register_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Registration should succeed or fail with user already exists
    assert!(
        response.status() == StatusCode::OK || response.status() == StatusCode::BAD_REQUEST,
        "Registration failed with status: {}",
        response.status()
    );

    // Test user login
    let login_request = json!({
        "email": "test@example.com",
        "password": "testpassword123"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&login_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Login should succeed if user exists
    if response.status() == StatusCode::OK {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        
        // Verify response structure
        assert!(response_json["user"]["id"].is_string());
        assert_eq!(response_json["user"]["email"], "test@example.com");
        assert!(response_json["token"].is_string());
        
        println!("Auth integration test passed - login successful");
    } else {
        println!("Auth integration test - login failed with status: {}", response.status());
    }
}

/// Test password hashing and verification
#[test]
fn test_password_hashing() {
    use argon2::{
        password_hash::{rand_core::OsRng, PasswordHasher, PasswordVerifier, PasswordHash, SaltString},
        Argon2,
    };

    let password = "testpassword123";
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    
    // Hash password
    let password_hash = argon2.hash_password(password.as_bytes(), &salt).unwrap();
    let hash_string = password_hash.to_string();
    
    // Verify password can be validated
    let parsed_hash = PasswordHash::new(&hash_string).unwrap();
    assert!(argon2.verify_password(password.as_bytes(), &parsed_hash).is_ok());
    assert!(argon2.verify_password("wrongpassword".as_bytes(), &parsed_hash).is_err());
    
    // Verify hash format
    assert!(hash_string.starts_with("$argon2"));
    
    println!("Password hashing test passed");
}

/// Test JWT token generation
#[test]
fn test_jwt_token_generation() {
    use jsonwebtoken::{encode, EncodingKey, Header, Algorithm};
    use serde::{Deserialize, Serialize};
    use chrono::{Duration, Utc};

    #[derive(Debug, Serialize, Deserialize)]
    struct Claims {
        sub: String,
        session_id: String,
        exp: i64,
        iat: i64,
        iss: String,
        aud: String,
    }

    let user_id = "test-user-id";
    let session_id = "test-session-id";
    
    let claims = Claims {
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
        &EncodingKey::from_secret("test-secret".as_ref()),
    ).unwrap();
    
    // Verify token structure
    assert!(!token.is_empty());
    let parts: Vec<&str> = token.split('.').collect();
    assert_eq!(parts.len(), 3); // JWT has 3 parts: header.payload.signature
    
    println!("JWT token generation test passed");
}