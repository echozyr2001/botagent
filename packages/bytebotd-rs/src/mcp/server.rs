use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use tracing::{error, info};

use super::tools::ComputerUseTools;
use crate::{automation::AutomationService, error::AutomationError};

/// MCP Server implementation for ByteBot desktop automation
#[derive(Clone)]
pub struct McpServer {
    automation_service: Arc<AutomationService>,
    tools: ComputerUseTools,
}

impl McpServer {
    /// Create a new MCP server instance
    pub fn new(automation_service: Arc<AutomationService>) -> Self {
        let tools = ComputerUseTools::new(automation_service.clone());
        Self {
            automation_service,
            tools,
        }
    }

    /// Create the MCP routes for integration with the main Axum router
    pub fn create_routes(
        _automation_service: Arc<AutomationService>,
    ) -> Router<Arc<AutomationService>> {
        Router::new().route("/mcp", get(mcp_sse_handler))
    }

    /// Start the MCP server with stdio transport
    pub async fn start_stdio_server(&self) -> Result<(), AutomationError> {
        info!("Starting MCP stdio server");

        let tools = self.tools.get_tools();
        info!("Registered {} MCP tools", tools.len());

        // For now, just log the available tools
        // In a full implementation, this would start the actual MCP server
        for tool in &tools {
            info!("Available tool: {} - {}", tool.name(), tool.description());
        }

        info!("MCP stdio server started successfully (placeholder implementation)");
        Ok(())
    }

    /// Get available tool names and descriptions
    pub fn get_tool_info(&self) -> Vec<(String, String)> {
        self.tools
            .get_tools()
            .iter()
            .map(|tool| (tool.name().to_string(), tool.description().to_string()))
            .collect()
    }

    /// Get the tools instance
    pub fn tools(&self) -> &ComputerUseTools {
        &self.tools
    }
}

/// SSE handler for MCP connections
async fn mcp_sse_handler(
    State(_automation_service): State<Arc<AutomationService>>,
) -> Result<Response, StatusCode> {
    info!("New MCP SSE connection established");

    // For now, return a simple SSE response
    // In a full implementation, this would handle the SSE connection
    let response = axum::response::Response::builder()
        .status(200)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Headers", "Cache-Control")
        .body("data: MCP SSE endpoint ready\n\n".to_string())
        .map_err(|e| {
            error!("Failed to create SSE response: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(response.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::AutomationService;

    #[tokio::test]
    async fn test_mcp_server_creation() {
        let automation_service = Arc::new(AutomationService::new().unwrap());
        let mcp_server = McpServer::new(automation_service);

        // Test that the server can be created without panicking
        let tools = mcp_server.tools().get_tools();
        assert!(!tools.is_empty());
        assert_eq!(tools.len(), 12); // We have 12 different tools
    }

    #[tokio::test]
    async fn test_mcp_routes_creation() {
        let automation_service = Arc::new(AutomationService::new().unwrap());
        let _routes = McpServer::create_routes(automation_service);

        // Test that routes are created without panicking
        // The actual route testing would require more complex setup
    }

    #[tokio::test]
    async fn test_get_tool_info() {
        let automation_service = Arc::new(AutomationService::new().unwrap());
        let mcp_server = McpServer::new(automation_service);

        let tool_info = mcp_server.get_tool_info();
        assert_eq!(tool_info.len(), 12);

        // Check that we have expected tools
        let tool_names: Vec<String> = tool_info.iter().map(|(name, _)| name.clone()).collect();
        assert!(tool_names.contains(&"move_mouse".to_string()));
        assert!(tool_names.contains(&"click_mouse".to_string()));
        assert!(tool_names.contains(&"type_text".to_string()));
        assert!(tool_names.contains(&"screenshot".to_string()));
        assert!(tool_names.contains(&"application".to_string()));

        // Check that descriptions are not empty
        for (name, description) in tool_info {
            assert!(!name.is_empty());
            assert!(!description.is_empty());
        }
    }
}
