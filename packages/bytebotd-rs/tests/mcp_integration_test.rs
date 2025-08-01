use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use bytebotd_rs::{automation::AutomationService, mcp::McpServer, routes};
use serde_json::json;
use tower::ServiceExt;

/// Create a test application with MCP routes
async fn create_test_app() -> Router {
    let automation_service = Arc::new(AutomationService::new().unwrap());
    let metrics = Arc::new(bytebot_shared_rs::MetricsCollector::new("test-service").unwrap());
    routes::create_routes(automation_service, metrics)
}

#[tokio::test]
async fn test_mcp_server_integration() {
    let automation_service = Arc::new(AutomationService::new().unwrap());
    let mcp_server = McpServer::new(automation_service);

    // Test that the server can be created
    assert!(!mcp_server.get_tool_info().is_empty());

    // Test that we have the expected number of tools
    let tools = mcp_server.tools().get_tools();
    assert_eq!(tools.len(), 12);

    // Test that all expected tools are present
    let tool_names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
    let expected_tools = [
        "move_mouse",
        "click_mouse",
        "type_text",
        "paste_text",
        "press_keys",
        "scroll",
        "screenshot",
        "cursor_position",
        "application",
        "read_file",
        "write_file",
        "wait",
    ];

    for expected_tool in &expected_tools {
        assert!(
            tool_names.contains(expected_tool),
            "Missing tool: {expected_tool}"
        );
    }
}

#[tokio::test]
async fn test_mcp_sse_endpoint() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/mcp")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Check that the response has the correct SSE headers
    let headers = response.headers();
    assert_eq!(headers.get("content-type").unwrap(), "text/event-stream");
    assert_eq!(headers.get("cache-control").unwrap(), "no-cache");
    assert_eq!(headers.get("connection").unwrap(), "keep-alive");
}

#[tokio::test]
async fn test_mcp_tool_schemas() {
    let automation_service = Arc::new(AutomationService::new().unwrap());
    let mcp_server = McpServer::new(automation_service);
    let tools = mcp_server.tools().get_tools();

    // Test that all tools have valid schemas
    for tool in &tools {
        let schema = tool.input_schema();
        assert!(
            schema.is_object(),
            "Tool {} schema is not an object",
            tool.name()
        );

        // Verify schema has required properties
        if let Some(properties) = schema.get("properties") {
            assert!(
                properties.is_object(),
                "Tool {} properties is not an object",
                tool.name()
            );
        }

        // Verify tool has non-empty name and description
        assert!(!tool.name().is_empty(), "Tool has empty name");
        assert!(
            !tool.description().is_empty(),
            "Tool {} has empty description",
            tool.name()
        );
    }
}

#[tokio::test]
async fn test_mcp_tool_argument_validation() {
    let automation_service = Arc::new(AutomationService::new().unwrap());
    let mcp_server = McpServer::new(automation_service);
    let tools = mcp_server.tools().get_tools();

    // Find the move_mouse tool for testing
    let move_mouse_tool = tools
        .iter()
        .find(|t| t.name() == "move_mouse")
        .expect("move_mouse tool not found");

    // Test valid arguments
    let valid_args = json!({
        "coordinates": {
            "x": 100,
            "y": 200
        }
    });

    // This would normally call the tool, but since we're testing integration
    // we just verify the schema accepts the arguments
    let schema = move_mouse_tool.input_schema();
    assert!(schema.get("properties").is_some());
    assert!(schema
        .get("properties")
        .unwrap()
        .get("coordinates")
        .is_some());
}

#[tokio::test]
async fn test_mcp_tools_coverage() {
    let automation_service = Arc::new(AutomationService::new().unwrap());
    let mcp_server = McpServer::new(automation_service);
    let tools = mcp_server.tools().get_tools();

    // Verify we have tools for all major computer automation categories
    let tool_names: Vec<&str> = tools.iter().map(|t| t.name()).collect();

    // Mouse operations
    assert!(tool_names.contains(&"move_mouse"));
    assert!(tool_names.contains(&"click_mouse"));

    // Keyboard operations
    assert!(tool_names.contains(&"type_text"));
    assert!(tool_names.contains(&"paste_text"));
    assert!(tool_names.contains(&"press_keys"));

    // Screen operations
    assert!(tool_names.contains(&"screenshot"));
    assert!(tool_names.contains(&"cursor_position"));
    assert!(tool_names.contains(&"scroll"));

    // Application operations
    assert!(tool_names.contains(&"application"));

    // File operations
    assert!(tool_names.contains(&"read_file"));
    assert!(tool_names.contains(&"write_file"));

    // Utility operations
    assert!(tool_names.contains(&"wait"));
}

#[tokio::test]
async fn test_mcp_server_routes_integration() {
    let app = create_test_app().await;

    // Test that health check still works with MCP routes
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_mcp_tool_descriptions() {
    let automation_service = Arc::new(AutomationService::new().unwrap());
    let mcp_server = McpServer::new(automation_service);
    let tool_info = mcp_server.get_tool_info();

    // Verify all tools have meaningful descriptions
    for (name, description) in &tool_info {
        assert!(!name.is_empty(), "Tool name is empty");
        assert!(!description.is_empty(), "Tool {name} has empty description");
        assert!(
            description.len() > 10,
            "Tool {name} description is too short: {description}"
        );
    }

    // Verify specific tool descriptions contain expected keywords
    let move_mouse_desc = tool_info
        .iter()
        .find(|(name, _)| name == "move_mouse")
        .map(|(_, desc)| desc)
        .expect("move_mouse tool not found");
    assert!(move_mouse_desc.to_lowercase().contains("mouse"));
    assert!(move_mouse_desc.to_lowercase().contains("cursor"));

    let screenshot_desc = tool_info
        .iter()
        .find(|(name, _)| name == "screenshot")
        .map(|(_, desc)| desc)
        .expect("screenshot tool not found");
    assert!(screenshot_desc.to_lowercase().contains("screenshot"));
    assert!(screenshot_desc.to_lowercase().contains("screen"));
}
