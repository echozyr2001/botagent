use std::{collections::HashMap, sync::Arc};

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use bytebot_agent_rs::{
    auth::{AuthService, AuthServiceTrait},
    config::Config,
    database::{DatabaseManager, MigrationRunner},
    server::{create_app, create_app_state},
};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

/// Test database URL - uses a separate test database
const TEST_DATABASE_URL: &str = "postgresql://localhost:5432/bytebot_test_integration";

/// Test helper to create a test application with real database
async fn create_test_app() -> Result<axum::Router, Box<dyn std::error::Error>> {
    // Skip test if no database URL is provided
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| TEST_DATABASE_URL.to_string());

    // Create test database if it doesn't exist
    MigrationRunner::create_database_if_not_exists(&database_url).await?;

    // Create database connection and run migrations
    let temp_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;

    let migration_runner = MigrationRunner::new(temp_pool.clone());
    migration_runner.run_migrations().await?;
    temp_pool.close().await;

    // Create test configuration
    let config = Arc::new(Config {
        database_url,
        anthropic_api_key: Some("test-anthropic-key".to_string()),
        openai_api_key: Some("test-openai-key".to_string()),
        google_api_key: Some("test-google-key".to_string()),
        auth_enabled: true,
        jwt_secret: "test-jwt-secret-for-integration-tests".to_string(),
        ..Config::default()
    });

    // Create application state
    let app_state = create_app_state(config).await?;

    // Create the application
    Ok(create_app(app_state))
}

/// Test helper to create a test user and get auth token
async fn create_test_user_and_token(
    app: &axum::Router,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let user_id = Uuid::new_v4().to_string();
    let email = format!("test-{user_id}@example.com");
    let password = "testpassword123";

    // Register user
    let register_request = json!({
        "email": email,
        "password": password,
        "name": "Test User"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&register_request)?))?,
        )
        .await?;

    // Login to get token
    let login_request = json!({
        "email": email,
        "password": password
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&login_request)?))?,
        )
        .await?;

    if response.status() == StatusCode::OK {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
        let response_json: Value = serde_json::from_slice(&body)?;
        let token = response_json["token"].as_str().unwrap().to_string();
        let user_id = response_json["user"]["id"].as_str().unwrap().to_string();
        Ok((user_id, token))
    } else {
        Err("Failed to create test user and token".into())
    }
}

/// Integration test for health endpoints
#[tokio::test]
async fn test_health_endpoints() {
    let app = match create_test_app().await {
        Ok(app) => app,
        Err(e) => {
            println!("Skipping health endpoints test - setup failed: {e}");
            return;
        }
    };

    // Test /health endpoint
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(response_json["status"], "healthy");
    assert!(response_json["timestamp"].is_string());
    assert_eq!(response_json["service"], "bytebot-agent-rs");
    assert!(response_json["database"]["connected"].as_bool().unwrap());

    // Test /api/health endpoint
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

/// Integration test for WebSocket stats endpoint
#[tokio::test]
async fn test_websocket_stats_endpoint() {
    let app = match create_test_app().await {
        Ok(app) => app,
        Err(e) => {
            println!("Skipping WebSocket stats test - setup failed: {e}");
            return;
        }
    };

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ws-stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    assert!(response_json["websocket"]["total_connections"].is_number());
    assert!(response_json["websocket"]["total_rooms"].is_number());
    assert!(response_json["websocket"]["rooms"].is_object());
    assert!(response_json["timestamp"].is_string());
}

/// Integration test for authentication endpoints
#[tokio::test]
async fn test_auth_endpoints() {
    let app = match create_test_app().await {
        Ok(app) => app,
        Err(e) => {
            println!("Skipping auth endpoints test - setup failed: {e}");
            return;
        }
    };

    let user_id = Uuid::new_v4().to_string();
    let email = format!("auth-test-{user_id}@example.com");
    let password = "testpassword123";

    // Test user registration
    let register_request = json!({
        "email": email,
        "password": password,
        "name": "Auth Test User"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&register_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Registration should succeed
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    assert!(response_json["success"].as_bool().unwrap());
    assert!(response_json["data"]["user"]["id"].is_string());
    assert_eq!(response_json["data"]["user"]["email"], email);
    assert!(response_json["data"]["token"].is_string());

    // Test user login
    let login_request = json!({
        "email": email,
        "password": password
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&login_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    assert!(response_json["success"].as_bool().unwrap());
    assert_eq!(response_json["data"]["user"]["email"], email);
    assert!(response_json["data"]["token"].is_string());
}

/// Integration test for task endpoints
#[tokio::test]
async fn test_task_endpoints() {
    let app = match create_test_app().await {
        Ok(app) => app,
        Err(e) => {
            println!("Skipping task endpoints test - setup failed: {e}");
            return;
        }
    };

    let (_user_id, token) = match create_test_user_and_token(&app).await {
        Ok(result) => result,
        Err(e) => {
            println!("Skipping task endpoints test - user creation failed: {e}");
            return;
        }
    };

    // Test GET /tasks/models endpoint
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/tasks/models")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    assert!(response_json["success"].as_bool().unwrap());
    let models = response_json["data"].as_array().unwrap();
    assert_eq!(models.len(), 9); // 2 + 4 + 3 models from all providers

    // Test POST /tasks endpoint (create task)
    let create_task_request = json!({
        "description": "Integration test task",
        "type": "IMMEDIATE",
        "priority": "MEDIUM"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&create_task_request).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    assert!(response_json["success"].as_bool().unwrap());
    let task = &response_json["data"];
    assert!(task["id"].is_string());
    assert_eq!(task["description"], "Integration test task");
    assert_eq!(task["type"], "IMMEDIATE");
    assert_eq!(task["priority"], "MEDIUM");
    assert_eq!(task["status"], "PENDING");

    let task_id = task["id"].as_str().unwrap();

    // Test GET /tasks endpoint (list tasks)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/tasks")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    assert!(response_json["success"].as_bool().unwrap());
    let tasks = response_json["data"].as_array().unwrap();
    assert!(!tasks.is_empty());
    assert!(response_json["pagination"]["total"].as_u64().unwrap() > 0);

    // Test GET /tasks/:id endpoint (get specific task)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/tasks/{task_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    assert!(response_json["success"].as_bool().unwrap());
    let retrieved_task = &response_json["data"];
    assert_eq!(retrieved_task["id"], task_id);
    assert_eq!(retrieved_task["description"], "Integration test task");

    // Test PATCH /tasks/:id endpoint (update task)
    let update_task_request = json!({
        "status": "RUNNING"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/tasks/{task_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&update_task_request).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    assert!(response_json["success"].as_bool().unwrap());
    let updated_task = &response_json["data"];
    assert_eq!(updated_task["id"], task_id);
    assert_eq!(updated_task["status"], "RUNNING");

    // Test POST /tasks/:id/takeover endpoint
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/tasks/{task_id}/takeover"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    assert!(response_json["success"].as_bool().unwrap());
    let takeover_task = &response_json["data"];
    assert_eq!(takeover_task["id"], task_id);
    assert_eq!(takeover_task["status"], "NEEDS_HELP");

    // Test POST /tasks/:id/resume endpoint
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/tasks/{task_id}/resume"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    assert!(response_json["success"].as_bool().unwrap());
    let resumed_task = &response_json["data"];
    assert_eq!(resumed_task["id"], task_id);
    assert_eq!(resumed_task["status"], "RUNNING");

    // Test POST /tasks/:id/cancel endpoint
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/tasks/{task_id}/cancel"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    assert!(response_json["success"].as_bool().unwrap());
    let cancelled_task = &response_json["data"];
    assert_eq!(cancelled_task["id"], task_id);
    assert_eq!(cancelled_task["status"], "CANCELLED");

    // Test DELETE /tasks/:id endpoint
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/tasks/{task_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

/// Integration test for message endpoints
#[tokio::test]
async fn test_message_endpoints() {
    let app = match create_test_app().await {
        Ok(app) => app,
        Err(e) => {
            println!("Skipping message endpoints test - setup failed: {e}");
            return;
        }
    };

    let (_user_id, token) = match create_test_user_and_token(&app).await {
        Ok(result) => result,
        Err(e) => {
            println!("Skipping message endpoints test - user creation failed: {e}");
            return;
        }
    };

    // First create a task to add messages to
    let create_task_request = json!({
        "description": "Message test task",
        "type": "IMMEDIATE",
        "priority": "MEDIUM"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&create_task_request).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();
    let task_id = response_json["data"]["id"].as_str().unwrap();

    // Test POST /tasks/:id/messages endpoint (add message)
    let add_message_request = json!({
        "message": "Hello, this is a test message"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/tasks/{task_id}/messages"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&add_message_request).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    assert!(response_json["success"].as_bool().unwrap());

    // Test GET /tasks/:id/messages endpoint (get messages)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/tasks/{task_id}/messages"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    assert!(response_json["success"].as_bool().unwrap());
    let messages = response_json["data"].as_array().unwrap();
    assert!(!messages.is_empty());
    assert!(response_json["pagination"]["total"].as_u64().unwrap() > 0);

    // Test GET /tasks/:id/messages/raw endpoint
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/tasks/{task_id}/messages/raw"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    assert!(response_json["success"].as_bool().unwrap());
    let raw_messages = response_json["data"].as_array().unwrap();
    assert!(!raw_messages.is_empty());

    // Test GET /tasks/:id/messages/processed endpoint
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/tasks/{task_id}/messages/processed"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    assert!(response_json["success"].as_bool().unwrap());
    let processed_messages = response_json["data"].as_array().unwrap();
    assert!(!processed_messages.is_empty());

    // Verify processed message structure
    let first_group = &processed_messages[0];
    assert!(first_group["role"].is_string());
    assert!(first_group["messages"].is_array());
}

/// Integration test for error handling scenarios
#[tokio::test]
async fn test_error_handling() {
    let app = match create_test_app().await {
        Ok(app) => app,
        Err(e) => {
            println!("Skipping error handling test - setup failed: {e}");
            return;
        }
    };

    let (_user_id, token) = match create_test_user_and_token(&app).await {
        Ok(result) => result,
        Err(e) => {
            println!("Skipping error handling test - user creation failed: {e}");
            return;
        }
    };

    // Test 404 for non-existent task
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/tasks/non-existent-task-id")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Test 400 for invalid task creation data
    let invalid_create_request = json!({
        "description": "", // Empty description should fail validation
        "type": "INVALID_TYPE",
        "priority": "INVALID_PRIORITY"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&invalid_create_request).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Test 401 for unauthorized access (no token)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/tasks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Test 401 for invalid token
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/tasks")
                .header(header::AUTHORIZATION, "Bearer invalid-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// Integration test for pagination and filtering
#[tokio::test]
async fn test_pagination_and_filtering() {
    let app = match create_test_app().await {
        Ok(app) => app,
        Err(e) => {
            println!("Skipping pagination test - setup failed: {e}");
            return;
        }
    };

    let (_user_id, token) = match create_test_user_and_token(&app).await {
        Ok(result) => result,
        Err(e) => {
            println!("Skipping pagination test - user creation failed: {e}");
            return;
        }
    };

    // Create multiple tasks with different properties
    let task_configs = vec![
        ("Task 1", "IMMEDIATE", "HIGH"),
        ("Task 2", "SCHEDULED", "MEDIUM"),
        ("Task 3", "IMMEDIATE", "LOW"),
    ];

    let mut task_ids = Vec::new();

    for (description, task_type, priority) in task_configs {
        let create_request = json!({
            "description": description,
            "type": task_type,
            "priority": priority
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/tasks")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&create_request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let response_json: Value = serde_json::from_slice(&body).unwrap();
        task_ids.push(response_json["data"]["id"].as_str().unwrap().to_string());
    }

    // Test pagination with limit
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/tasks?page=1&limit=2")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    let tasks = response_json["data"].as_array().unwrap();
    assert!(tasks.len() <= 2);
    assert_eq!(response_json["pagination"]["page"], 1);
    assert_eq!(response_json["pagination"]["limit"], 2);

    // Test filtering by priority
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/tasks?priority=HIGH")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    let tasks = response_json["data"].as_array().unwrap();
    for task in tasks {
        assert_eq!(task["priority"], "HIGH");
    }

    // Test filtering by type
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/tasks?type=IMMEDIATE")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    let tasks = response_json["data"].as_array().unwrap();
    for task in tasks {
        assert_eq!(task["type"], "IMMEDIATE");
    }

    // Test filtering by status
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/tasks?status=PENDING")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    let tasks = response_json["data"].as_array().unwrap();
    for task in tasks {
        assert_eq!(task["status"], "PENDING");
    }
}
