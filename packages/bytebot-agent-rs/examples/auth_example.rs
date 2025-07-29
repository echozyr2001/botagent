use axum::{
    extract::Request,
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;

use bytebot_agent_rs::{
    auth::AuthContextExtractor,
    config::Config,
    server::{create_app_state, AppState},
};

/// Example protected route that requires authentication
async fn protected_route(request: Request) -> Result<Json<Value>, StatusCode> {
    // Extract authentication context from the request
    let auth_context = request.require_auth_context()?;
    
    // Use the authenticated user information
    let response = json!({
        "message": "Access granted to protected resource",
        "user": {
            "id": auth_context.user.id,
            "email": auth_context.user.email,
            "name": auth_context.user.name
        }
    });
    
    Ok(Json(response))
}

/// Example optional auth route that works with or without authentication
async fn optional_auth_route(request: Request) -> Json<Value> {
    match request.auth_context() {
        Some(auth_context) => {
            Json(json!({
                "message": "Hello authenticated user",
                "user": {
                    "id": auth_context.user.id,
                    "email": auth_context.user.email
                }
            }))
        }
        None => {
            Json(json!({
                "message": "Hello anonymous user"
            }))
        }
    }
}

/// Example of how to create an app with authentication middleware
pub fn create_example_app(state: AppState) -> Router {
    Router::new()
        .route("/protected", get(protected_route))
        .route("/optional", get(optional_auth_route))
        .with_state(state)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize configuration
    let config = Arc::new(Config::from_env()?);
    
    // Create application state with authentication service
    let app_state = create_app_state(config).await?;
    
    // Create the application with authentication middleware
    let app = create_example_app(app_state);
    
    println!("Example authentication server running on http://localhost:3000");
    println!("Try these endpoints:");
    println!("  GET /protected - Requires Bearer token");
    println!("  GET /optional - Works with or without token");
    
    // Start the server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}