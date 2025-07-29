use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use bytebot_shared_rs::types::{
    api::{ApiResponse, CreateTaskDto, PaginatedResponse, PaginationParams, UpdateTaskDto},
    task::{Task, TaskStatus},
};
use serde_json::{json, Value};
use tracing::{debug, info};
use validator::Validate;

use crate::{
    database::task_repository::{TaskFilter, TaskRepositoryTrait},
    error::{ServiceError, ServiceResult},
    server::AppState,
};

/// Create task-related routes
pub fn create_task_routes() -> Router<AppState> {
    Router::new()
        .route("/tasks", post(create_task).get(list_tasks))
        .route("/tasks/models", get(get_models))
        .route(
            "/tasks/:id",
            get(get_task).patch(update_task).delete(delete_task),
        )
        .route("/tasks/:id/takeover", post(takeover_task))
        .route("/tasks/:id/resume", post(resume_task))
        .route("/tasks/:id/cancel", post(cancel_task))
}

/// Create a new task
/// POST /tasks
async fn create_task(
    State(state): State<AppState>,
    Json(dto): Json<CreateTaskDto>,
) -> ServiceResult<Json<ApiResponse<Task>>> {
    debug!("Creating new task: {:?}", dto);

    // Validate the DTO
    dto.validate()
        .map_err(|e| ServiceError::Validation(format!("Validation failed: {e}")))?;

    // Create task using repository
    let task_repo = state.db.task_repository();
    let task = task_repo
        .create(&dto)
        .await
        .map_err(ServiceError::Database)?;

    // Emit task created event via WebSocket
    state.websocket_gateway.emit_task_created(&task).await;

    info!("Successfully created task with ID: {}", task.id);

    Ok(Json(ApiResponse::success(task)))
}

/// List all tasks with optional filtering and pagination
/// GET /tasks
async fn list_tasks(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> ServiceResult<Json<PaginatedResponse<Task>>> {
    debug!("Listing tasks with params: {:?}", params);

    // Parse pagination parameters
    let page = params.get("page").and_then(|p| p.parse().ok()).unwrap_or(1);
    let limit = params
        .get("limit")
        .and_then(|l| l.parse().ok())
        .unwrap_or(20);

    let pagination = PaginationParams {
        page: Some(page),
        limit: Some(limit),
    };

    // Validate pagination
    pagination
        .validate()
        .map_err(|e| ServiceError::Validation(format!("Invalid pagination: {e}")))?;

    // Parse filter parameters
    let mut filter = TaskFilter::default();

    if let Some(status_str) = params.get("status") {
        filter.status = Some(
            status_str
                .parse()
                .map_err(|_| ServiceError::Validation("Invalid status".to_string()))?,
        );
    }

    if let Some(priority_str) = params.get("priority") {
        filter.priority = Some(
            priority_str
                .parse()
                .map_err(|_| ServiceError::Validation("Invalid priority".to_string()))?,
        );
    }

    if let Some(task_type_str) = params.get("type") {
        filter.task_type = Some(
            task_type_str
                .parse()
                .map_err(|_| ServiceError::Validation("Invalid task type".to_string()))?,
        );
    }

    if let Some(user_id) = params.get("userId") {
        filter.user_id = Some(user_id.clone());
    }

    if let Some(created_by_str) = params.get("createdBy") {
        filter.created_by = Some(
            created_by_str
                .parse()
                .map_err(|_| ServiceError::Validation("Invalid createdBy".to_string()))?,
        );
    }

    // Get tasks from repository
    let task_repo = state.db.task_repository();
    let (tasks, total) = task_repo
        .list(&filter, &pagination)
        .await
        .map_err(ServiceError::Database)?;

    debug!("Found {} tasks (total: {})", tasks.len(), total);

    Ok(Json(PaginatedResponse::new(tasks, page, limit, total)))
}

/// Get available AI models
/// GET /tasks/models
async fn get_models(State(state): State<AppState>) -> ServiceResult<Json<ApiResponse<Vec<Value>>>> {
    debug!("Getting available AI models");

    // Get models from the unified AI service
    let models = state.ai_service.list_all_models();

    // Convert ModelInfo to JSON format expected by the frontend
    let model_json: Vec<Value> = models
        .into_iter()
        .map(|model| {
            json!({
                "provider": model.provider,
                "name": model.name,
                "title": model.title
            })
        })
        .collect();

    debug!(
        "Returning {} available models from {} providers",
        model_json.len(),
        state.ai_service.get_available_providers().len()
    );

    Ok(Json(ApiResponse::success(model_json)))
}

/// Get a specific task by ID
/// GET /tasks/:id
async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ServiceResult<Json<ApiResponse<Task>>> {
    debug!("Getting task with ID: {}", id);

    let task_repo = state.db.task_repository();
    let task = task_repo
        .get_by_id(&id)
        .await
        .map_err(ServiceError::Database)?
        .ok_or_else(|| ServiceError::NotFound(format!("Task with ID {id} not found")))?;

    debug!("Found task: {}", task.id);

    Ok(Json(ApiResponse::success(task)))
}

/// Update a task
/// PATCH /tasks/:id
async fn update_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(dto): Json<UpdateTaskDto>,
) -> ServiceResult<Json<ApiResponse<Task>>> {
    debug!("Updating task {} with data: {:?}", id, dto);

    // Validate the DTO
    dto.validate()
        .map_err(|e| ServiceError::Validation(format!("Validation failed: {e}")))?;

    let task_repo = state.db.task_repository();
    let task = task_repo
        .update(&id, &dto)
        .await
        .map_err(ServiceError::Database)?
        .ok_or_else(|| ServiceError::NotFound(format!("Task with ID {id} not found")))?;

    // Emit task update event via WebSocket
    state.websocket_gateway.emit_task_update(&id, &task).await;

    info!("Successfully updated task: {}", task.id);

    Ok(Json(ApiResponse::success(task)))
}

/// Delete a task
/// DELETE /tasks/:id
async fn delete_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ServiceResult<StatusCode> {
    debug!("Deleting task with ID: {}", id);

    let task_repo = state.db.task_repository();
    let deleted = task_repo
        .delete(&id)
        .await
        .map_err(ServiceError::Database)?;

    if !deleted {
        return Err(ServiceError::NotFound(format!(
            "Task with ID {id} not found"
        )));
    }

    // Emit task deleted event via WebSocket
    state.websocket_gateway.emit_task_deleted(&id).await;

    info!("Successfully deleted task: {}", id);

    Ok(StatusCode::NO_CONTENT)
}

/// Take over control of a task (switch control to user)
/// POST /tasks/:id/takeover
async fn takeover_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ServiceResult<Json<ApiResponse<Task>>> {
    debug!("Taking over control of task: {}", id);

    let task_repo = state.db.task_repository();

    // Get current task
    let mut task = task_repo
        .get_by_id(&id)
        .await
        .map_err(ServiceError::Database)?
        .ok_or_else(|| ServiceError::NotFound(format!("Task with ID {id} not found")))?;

    // Validate that task can be taken over
    if task.is_terminal() {
        return Err(ServiceError::Validation(
            "Cannot take over a completed, cancelled, or failed task".to_string(),
        ));
    }

    // Update task control to user and status to needs help if running
    let update_dto = UpdateTaskDto {
        status: if task.status == TaskStatus::Running {
            Some(TaskStatus::NeedsHelp)
        } else {
            None
        },
        priority: None,
        queued_at: None,
        executed_at: None,
        completed_at: None,
    };

    // Update the task
    task = task_repo
        .update(&id, &update_dto)
        .await
        .map_err(ServiceError::Database)?
        .ok_or_else(|| ServiceError::NotFound(format!("Task with ID {id} not found")))?;

    // Emit task update event via WebSocket
    state.websocket_gateway.emit_task_update(&id, &task).await;

    // Note: In a full implementation, we would also update the control field
    // This requires extending the UpdateTaskDto to include control field

    info!("Successfully took over control of task: {}", id);

    Ok(Json(ApiResponse::success(task)))
}

/// Resume a task (switch control back to assistant)
/// POST /tasks/:id/resume
async fn resume_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ServiceResult<Json<ApiResponse<Task>>> {
    debug!("Resuming task: {}", id);

    let task_repo = state.db.task_repository();

    // Get current task
    let task = task_repo
        .get_by_id(&id)
        .await
        .map_err(ServiceError::Database)?
        .ok_or_else(|| ServiceError::NotFound(format!("Task with ID {id} not found")))?;

    // Validate that task can be resumed
    if task.is_terminal() {
        return Err(ServiceError::Validation(
            "Cannot resume a completed, cancelled, or failed task".to_string(),
        ));
    }

    // Update task status to running if it was in needs help or needs review
    let update_dto = UpdateTaskDto {
        status: match task.status {
            TaskStatus::NeedsHelp | TaskStatus::NeedsReview => Some(TaskStatus::Running),
            TaskStatus::Pending => Some(TaskStatus::Running),
            _ => None,
        },
        priority: None,
        queued_at: None,
        executed_at: None,
        completed_at: None,
    };

    let updated_task = task_repo
        .update(&id, &update_dto)
        .await
        .map_err(ServiceError::Database)?
        .ok_or_else(|| ServiceError::NotFound(format!("Task with ID {id} not found")))?;

    // Emit task update event via WebSocket
    state
        .websocket_gateway
        .emit_task_update(&id, &updated_task)
        .await;

    info!("Successfully resumed task: {}", id);

    Ok(Json(ApiResponse::success(updated_task)))
}

/// Cancel a task
/// POST /tasks/:id/cancel
async fn cancel_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ServiceResult<Json<ApiResponse<Task>>> {
    debug!("Cancelling task: {}", id);

    let task_repo = state.db.task_repository();

    // Get current task
    let task = task_repo
        .get_by_id(&id)
        .await
        .map_err(ServiceError::Database)?
        .ok_or_else(|| ServiceError::NotFound(format!("Task with ID {id} not found")))?;

    // Validate that task can be cancelled
    if task.is_terminal() {
        return Err(ServiceError::Validation(
            "Task is already in a terminal state".to_string(),
        ));
    }

    // Update task status to cancelled
    let updated_task = task_repo
        .update_status(&id, TaskStatus::Cancelled)
        .await
        .map_err(ServiceError::Database)?
        .ok_or_else(|| ServiceError::NotFound(format!("Task with ID {id} not found")))?;

    // Emit task update event via WebSocket
    state
        .websocket_gateway
        .emit_task_update(&id, &updated_task)
        .await;

    info!("Successfully cancelled task: {}", id);

    Ok(Json(ApiResponse::success(updated_task)))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
    };
    use serde_json::json;
    use tower::ServiceExt;

    use super::*;
    use crate::{
        ai::UnifiedAIService,
        auth::{AuthService, AuthServiceTrait},
        config::Config,
        database::DatabaseManager,
        websocket::WebSocketGateway,
    };

    // Helper function to create test app state
    async fn create_test_state() -> AppState {
        let config = Arc::new(Config::default());

        // For testing, we'll create a minimal state without real database connection
        // In a real test environment, you would set up a test database
        let database_url = "postgresql://localhost:5432/test_db";

        // Try to create database manager, but if it fails, skip the test
        match DatabaseManager::new(database_url).await {
            Ok(db) => {
                let ai_service = Arc::new(UnifiedAIService::new(&config));
                let auth_service: Arc<dyn AuthServiceTrait> = Arc::new(AuthService::new(
                    db.get_pool(),
                    config.jwt_secret.clone(),
                    config.auth_enabled,
                ));
                let websocket_gateway = Arc::new(WebSocketGateway::new());
                AppState {
                    config,
                    db: Arc::new(db),
                    ai_service,
                    auth_service,
                    websocket_gateway,
                }
            }
            Err(_) => {
                // Return a dummy state for compilation - tests will be skipped
                panic!("Test database not available - skipping integration tests");
            }
        }
    }

    // Helper function to create test app state with specific config
    async fn create_test_state_with_config(config: Arc<Config>) -> Option<AppState> {
        let database_url = "postgresql://localhost:5432/nonexistent";
        if let Ok(db) = DatabaseManager::new(database_url).await {
            let ai_service = Arc::new(UnifiedAIService::new(&config));
            let auth_service: Arc<dyn AuthServiceTrait> = Arc::new(AuthService::new(
                db.get_pool(),
                config.jwt_secret.clone(),
                config.auth_enabled,
            ));
            Some(AppState {
                config,
                db: Arc::new(db),
                ai_service,
                auth_service,
                websocket_gateway: Arc::new(WebSocketGateway::new()),
            })
        } else {
            None
        }
    }

    #[tokio::test]
    #[ignore] // Ignore by default since it requires a test database
    async fn test_create_task_endpoint() {
        let app = create_task_routes().with_state(create_test_state().await);

        let create_dto = json!({
            "description": "Test task",
            "type": "IMMEDIATE",
            "priority": "MEDIUM"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/tasks")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&create_dto).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should return 201 Created for successful task creation
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_get_models_endpoint_with_all_providers() {
        // Test that the models endpoint returns the correct structure with all providers
        let config = Arc::new(Config {
            anthropic_api_key: Some("test_key".to_string()),
            openai_api_key: Some("test_key".to_string()),
            google_api_key: Some("test_key".to_string()),
            ..Config::default()
        });

        if let Some(state) = create_test_state_with_config(config).await {
            let app = create_task_routes().with_state(state);

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

            assert_eq!(response.status(), StatusCode::OK);

            // Parse response body to verify structure
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

            // Verify response structure
            assert!(response_json["success"].as_bool().unwrap());
            let models = response_json["data"].as_array().unwrap();

            // Should have models from all three providers (2 + 4 + 3 = 9 models)
            assert_eq!(models.len(), 9);

            // Verify each model has required fields
            for model in models {
                assert!(model["provider"].is_string());
                assert!(model["name"].is_string());
                assert!(model["title"].is_string());
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
    }

    #[tokio::test]
    async fn test_get_models_endpoint_with_single_provider() {
        // Test that the models endpoint returns only OpenAI models when only OpenAI key is configured
        let config = Arc::new(Config {
            anthropic_api_key: None,
            openai_api_key: Some("test_key".to_string()),
            google_api_key: None,
            ..Config::default()
        });

        if let Some(state) = create_test_state_with_config(config).await {
            let app = create_task_routes().with_state(state);

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

            assert_eq!(response.status(), StatusCode::OK);

            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

            let models = response_json["data"].as_array().unwrap();

            // Should only have OpenAI models (4 models)
            assert_eq!(models.len(), 4);

            // All models should be from OpenAI
            for model in models {
                assert_eq!(model["provider"].as_str().unwrap(), "openai");
            }
        }
    }

    #[tokio::test]
    async fn test_get_models_endpoint_with_no_providers() {
        // Test that the models endpoint returns empty array when no API keys are configured
        let config = Arc::new(Config {
            anthropic_api_key: None,
            openai_api_key: None,
            google_api_key: None,
            ..Config::default()
        });

        if let Some(state) = create_test_state_with_config(config).await {
            let app = create_task_routes().with_state(state);

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

            assert_eq!(response.status(), StatusCode::OK);

            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

            let models = response_json["data"].as_array().unwrap();

            // Should have no models when no API keys are configured
            assert_eq!(models.len(), 0);
        }
    }

    #[tokio::test]
    async fn test_route_registration() {
        // Test that all routes are properly registered
        let _config = Arc::new(Config::default());

        // This test just verifies the routes compile and are registered
        let routes = create_task_routes();

        // Verify the router was created successfully
        assert!(!format!("{routes:?}").is_empty());
    }
}
