use axum::{response::Json, http::StatusCode};
use serde_json::{json, Value};
use chrono::Utc;

/// Health check endpoint
pub async fn health_check() -> Result<Json<Value>, StatusCode> {
    let response = json!({
        "status": "healthy",
        "service": "bytebotd-rs",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": Utc::now().to_rfc3339(),
        "capabilities": [
            "screenshot",
            "mouse_control",
            "keyboard_control",
            "file_operations"
        ]
    });

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let result = health_check().await;
        assert!(result.is_ok(), "Health check should succeed");
        
        let response = result.unwrap();
        let json_value = response.0;
        
        assert_eq!(json_value["status"], "healthy");
        assert_eq!(json_value["service"], "bytebotd-rs");
        assert!(json_value["capabilities"].is_array());
    }
}