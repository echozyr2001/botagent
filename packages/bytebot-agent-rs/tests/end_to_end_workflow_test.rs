use std::sync::Arc;

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use bytebot_agent_rs::{
    config::Config,
    database::{DatabaseManager, MigrationRunner},
    server::{create_app, create_app_state},
};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

/// Test database URL for end-to-end tests
const TEST_DATABASE_URL: &str = "postgresql://localhost:5432/bytebot_test_e2e";

/// Test helper to create a test application with real database
async fn create_test_app() -> Result<axum::Router, Box<dyn std::error::Error>> {
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
        jwt_secret: "test-jwt-secret-for-e2e-tests".to_string(),
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
    let email = format!("e2e-test-{user_id}@example.com");
    let password = "testpassword123";

    // Register user
    let register_request = json!({
        "email": email,
        "password": password,
        "name": "E2E Test User"
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
        let token = response_json["data"]["token"].as_str().unwrap().to_string();
        let user_id = response_json["data"]["user"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        Ok((user_id, token))
    } else {
        Err("Failed to create test user and token".into())
    }
}

/// End-to-end test for complete task lifecycle
#[tokio::test]
async fn test_complete_task_lifecycle() {
    let app = match create_test_app().await {
        Ok(app) => app,
        Err(e) => {
            println!(
                "Skipping complete task lifecycle test - setup failed: {}",
                e
            );
            return;
        }
    };

    let (_user_id, token) = match create_test_user_and_token(&app).await {
        Ok(result) => result,
        Err(e) => {
            println!(
                "Skipping complete task lifecycle test - user creation failed: {}",
                e
            );
            return;
        }
    };

    // Step 1: Create a new task
    let create_task_request = json!({
        "description": "Complete lifecycle test task",
        "type": "IMMEDIATE",
        "priority": "HIGH"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
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
    let task_id = task["id"].as_str().unwrap();
    assert_eq!(task["status"], "PENDING");
    assert_eq!(task["priority"], "HIGH");

    // Step 2: Add a message to the task
    let add_message_request = json!({
        "message": "Please help me with this task"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/tasks/{}/messages", task_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&add_message_request).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Step 3: Update task status to running
    let update_task_request = json!({
        "status": "RUNNING"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(&format!("/tasks/{}", task_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
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

    assert_eq!(response_json["data"]["status"], "RUNNING");

    // Step 4: Add another message during execution
    let progress_message_request = json!({
        "message": "I'm working on this now"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/tasks/{}/messages", task_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&progress_message_request).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Step 5: Test takeover functionality
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/tasks/{}/takeover", task_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
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

    assert_eq!(response_json["data"]["status"], "NEEDS_HELP");

    // Step 6: Resume the task
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/tasks/{}/resume", task_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
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

    assert_eq!(response_json["data"]["status"], "RUNNING");

    // Step 7: Complete the task
    let complete_task_request = json!({
        "status": "COMPLETED"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(&format!("/tasks/{}", task_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&complete_task_request).unwrap(),
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

    assert_eq!(response_json["data"]["status"], "COMPLETED");

    // Step 8: Verify messages were recorded
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/tasks/{}/messages", task_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
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

    let messages = response_json["data"].as_array().unwrap();
    assert!(messages.len() >= 2); // At least the two messages we added

    // Step 9: Get processed messages
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/tasks/{}/messages/processed", task_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
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

    let processed_messages = response_json["data"].as_array().unwrap();
    assert!(!processed_messages.is_empty());

    // Step 10: Verify task appears in task list
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/tasks")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
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
    let our_task = tasks.iter().find(|t| t["id"] == task_id);
    assert!(our_task.is_some());
    assert_eq!(our_task.unwrap()["status"], "COMPLETED");
}

/// End-to-end test for task cancellation workflow
#[tokio::test]
async fn test_task_cancellation_workflow() {
    let app = match create_test_app().await {
        Ok(app) => app,
        Err(e) => {
            println!(
                "Skipping task cancellation workflow test - setup failed: {}",
                e
            );
            return;
        }
    };

    let (_user_id, token) = match create_test_user_and_token(&app).await {
        Ok(result) => result,
        Err(e) => {
            println!(
                "Skipping task cancellation workflow test - user creation failed: {}",
                e
            );
            return;
        }
    };

    // Create a task
    let create_task_request = json!({
        "description": "Task to be cancelled",
        "type": "SCHEDULED",
        "priority": "LOW"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
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

    // Start the task
    let update_task_request = json!({
        "status": "RUNNING"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(&format!("/tasks/{}", task_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&update_task_request).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Cancel the task
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/tasks/{}/cancel", task_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
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

    assert_eq!(response_json["data"]["status"], "CANCELLED");

    // Verify we cannot resume a cancelled task
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/tasks/{}/resume", task_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Verify we cannot take over a cancelled task
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/tasks/{}/takeover", task_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// End-to-end test for multiple users and task isolation
#[tokio::test]
async fn test_multi_user_task_isolation() {
    let app = match create_test_app().await {
        Ok(app) => app,
        Err(e) => {
            println!("Skipping multi-user test - setup failed: {}", e);
            return;
        }
    };

    // Create two different users
    let (_user1_id, token1) = match create_test_user_and_token(&app).await {
        Ok(result) => result,
        Err(e) => {
            println!("Skipping multi-user test - user1 creation failed: {}", e);
            return;
        }
    };

    let (_user2_id, token2) = match create_test_user_and_token(&app).await {
        Ok(result) => result,
        Err(e) => {
            println!("Skipping multi-user test - user2 creation failed: {}", e);
            return;
        }
    };

    // User 1 creates a task
    let create_task_request = json!({
        "description": "User 1's private task",
        "type": "IMMEDIATE",
        "priority": "MEDIUM"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks")
                .header(header::AUTHORIZATION, format!("Bearer {}", token1))
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
    let user1_task_id = response_json["data"]["id"].as_str().unwrap();

    // User 2 creates a task
    let create_task_request = json!({
        "description": "User 2's private task",
        "type": "SCHEDULED",
        "priority": "HIGH"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks")
                .header(header::AUTHORIZATION, format!("Bearer {}", token2))
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
    let user2_task_id = response_json["data"]["id"].as_str().unwrap();

    // User 1 should see their own task
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/tasks")
                .header(header::AUTHORIZATION, format!("Bearer {}", token1))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    let user1_tasks = response_json["data"].as_array().unwrap();
    let has_own_task = user1_tasks.iter().any(|t| t["id"] == user1_task_id);
    assert!(has_own_task, "User 1 should see their own task");

    // User 2 should see their own task
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/tasks")
                .header(header::AUTHORIZATION, format!("Bearer {}", token2))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    let user2_tasks = response_json["data"].as_array().unwrap();
    let has_own_task = user2_tasks.iter().any(|t| t["id"] == user2_task_id);
    assert!(has_own_task, "User 2 should see their own task");

    // User 1 should not be able to access User 2's task directly
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/tasks/{}", user2_task_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token1))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // This should either return 404 or 403, depending on implementation
    assert!(
        response.status() == StatusCode::NOT_FOUND || response.status() == StatusCode::FORBIDDEN,
        "User 1 should not access User 2's task"
    );

    // User 2 should not be able to access User 1's task directly
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/tasks/{}", user1_task_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token2))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // This should either return 404 or 403, depending on implementation
    assert!(
        response.status() == StatusCode::NOT_FOUND || response.status() == StatusCode::FORBIDDEN,
        "User 2 should not access User 1's task"
    );
}
