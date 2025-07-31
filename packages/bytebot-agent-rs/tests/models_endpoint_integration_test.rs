use std::sync::Arc;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use bytebot_agent_rs::{
    ai::UnifiedAIService,
    auth::{AuthService, AuthServiceTrait},
    config::Config,
    database::DatabaseManager,
    routes::create_task_routes,
    server::AppState,
    websocket::WebSocketGateway,
};
use serde_json::Value;
use tower::ServiceExt;

/// Integration test for the models endpoint
#[tokio::test]
async fn test_models_endpoint_integration() {
    // Create test configuration with API keys
    let config = Config {
        anthropic_api_key: Some("test-anthropic-key".to_string()),
        openai_api_key: Some("test-openai-key".to_string()),
        google_api_key: Some("test-google-key".to_string()),
        database_url: "postgresql://localhost:5432/test_db".to_string(),
        ..Config::default()
    };

    // Try to create database manager - skip test if database is not available
    let db_manager = match DatabaseManager::new(&config.database_url).await {
        Ok(db) => db,
        Err(_) => {
            println!("Skipping integration test - database not available");
            return;
        }
    };

    // Create AI service
    let ai_service = UnifiedAIService::new(&config);

    // Create app state with all required fields
    let auth_service: Arc<dyn AuthServiceTrait> = Arc::new(AuthService::new(
        db_manager.get_pool(),
        config.jwt_secret.clone(),
        config.auth_enabled,
    ));
    let websocket_gateway = Arc::new(WebSocketGateway::new());

    let state = AppState {
        config: Arc::new(config),
        db: Arc::new(db_manager),
        ai_service: Arc::new(ai_service),
        auth_service,
        websocket_gateway,
    };

    // Create the router
    let app = create_task_routes().with_state(state);

    // Make request to models endpoint
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/tasks/models")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Verify response status
    assert_eq!(response.status(), StatusCode::OK);

    // Parse response body
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    // Verify response structure
    assert!(response_json["success"].as_bool().unwrap());
    let models = response_json["data"].as_array().unwrap();

    // Should have models from all three providers (2 + 4 + 3 = 9 models)
    assert_eq!(models.len(), 9);

    // Verify model structure
    for model in models {
        assert!(model["provider"].is_string());
        assert!(model["name"].is_string());
        assert!(model["title"].is_string());

        let provider = model["provider"].as_str().unwrap();
        let name = model["name"].as_str().unwrap();
        let title = model["title"].as_str().unwrap();

        // Verify provider-specific model names
        match provider {
            "anthropic" => {
                assert!(name.starts_with("claude-"));
                assert!(title.contains("Claude"));
            }
            "openai" => {
                assert!(name.starts_with("gpt-"));
                assert!(title.contains("GPT"));
            }
            "google" => {
                assert!(name.starts_with("gemini-"));
                assert!(title.contains("Gemini"));
            }
            _ => panic!("Unexpected provider: {provider}"),
        }
    }

    // Verify we have models from each provider
    let providers: std::collections::HashSet<String> = models
        .iter()
        .map(|m| m["provider"].as_str().unwrap().to_string())
        .collect();

    assert!(providers.contains("anthropic"));
    assert!(providers.contains("openai"));
    assert!(providers.contains("google"));
}

/// Test models endpoint with no API keys configured
#[tokio::test]
async fn test_models_endpoint_no_keys() {
    // Create test configuration without API keys
    let config = Config {
        anthropic_api_key: None,
        openai_api_key: None,
        google_api_key: None,
        database_url: "postgresql://localhost:5432/test_db".to_string(),
        ..Config::default()
    };

    // Try to create database manager - skip test if database is not available
    let db_manager = match DatabaseManager::new(&config.database_url).await {
        Ok(db) => db,
        Err(_) => {
            println!("Skipping integration test - database not available");
            return;
        }
    };

    // Create AI service
    let ai_service = UnifiedAIService::new(&config);

    // Create app state with all required fields
    let auth_service: Arc<dyn AuthServiceTrait> = Arc::new(AuthService::new(
        db_manager.get_pool(),
        config.jwt_secret.clone(),
        config.auth_enabled,
    ));
    let websocket_gateway = Arc::new(WebSocketGateway::new());

    let state = AppState {
        config: Arc::new(config),
        db: Arc::new(db_manager),
        ai_service: Arc::new(ai_service),
        auth_service,
        websocket_gateway,
    };

    // Create the router
    let app = create_task_routes().with_state(state);

    // Make request to models endpoint
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/tasks/models")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Verify response status
    assert_eq!(response.status(), StatusCode::OK);

    // Parse response body
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();

    // Verify response structure
    assert!(response_json["success"].as_bool().unwrap());
    let models = response_json["data"].as_array().unwrap();

    // Should have no models when no API keys are configured
    assert_eq!(models.len(), 0);
}
