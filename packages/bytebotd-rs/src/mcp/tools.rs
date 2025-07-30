use std::sync::Arc;

use async_trait::async_trait;
use bytebot_shared_rs::types::computer_action::{
    Application, Button, ComputerAction, Coordinates, Press,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::automation::AutomationService;

/// MCP Tool error types
#[derive(Debug, thiserror::Error)]
pub enum McpToolError {
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("Execution error: {0}")]
    ExecutionError(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// MCP Tool result type
pub type McpToolResult = Result<String, McpToolError>;

/// MCP Tool trait for computer automation actions
#[async_trait]
pub trait McpTool: Send + Sync {
    /// Get the tool name
    fn name(&self) -> &str;

    /// Get the tool description
    fn description(&self) -> &str;

    /// Get the JSON schema for input validation
    fn input_schema(&self) -> serde_json::Value;

    /// Execute the tool with given arguments
    async fn call(&self, arguments: serde_json::Value) -> McpToolResult;
}

/// MCP Tools implementation for computer automation
#[derive(Clone)]
pub struct ComputerUseTools {
    automation_service: Arc<AutomationService>,
}

// Request types for MCP tools
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MoveMouseRequest {
    #[schemars(description = "The coordinates to move the mouse to")]
    pub coordinates: CoordinatesSchema,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CoordinatesSchema {
    #[schemars(description = "The x-coordinate")]
    pub x: i32,
    #[schemars(description = "The y-coordinate")]
    pub y: i32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ClickMouseRequest {
    #[schemars(
        description = "Optional coordinates for the click. If not provided, clicks at current position"
    )]
    pub coordinates: Option<CoordinatesSchema>,
    #[schemars(description = "The mouse button to click")]
    pub button: ButtonSchema,
    #[schemars(description = "Number of clicks to perform (e.g., 2 for double-click)")]
    #[serde(default = "default_click_count")]
    pub click_count: Option<u32>,
}

fn default_click_count() -> Option<u32> {
    Some(1)
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ButtonSchema {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TypeTextRequest {
    #[schemars(description = "The text string to type")]
    pub text: String,
    #[schemars(description = "Optional delay in milliseconds between key presses")]
    pub delay: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PressKeysRequest {
    #[schemars(description = "Array of key names to press or release")]
    pub keys: Vec<String>,
    #[schemars(description = "Whether to press the keys down or release them up")]
    pub press: PressSchema,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PressSchema {
    Down,
    Up,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ScrollRequest {
    #[schemars(description = "Optional coordinates for where the scroll should occur")]
    pub coordinates: Option<CoordinatesSchema>,
    #[schemars(description = "The direction to scroll")]
    pub direction: ScrollDirection,
    #[schemars(description = "The number of times to scroll")]
    pub scroll_count: u32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ApplicationRequest {
    #[schemars(description = "The application to open or switch to")]
    pub application: ApplicationSchema,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ApplicationSchema {
    Firefox,
    #[serde(rename = "1password")]
    OnePassword,
    Thunderbird,
    Vscode,
    Terminal,
    Desktop,
    Directory,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReadFileRequest {
    #[schemars(description = "The file path to read from")]
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WriteFileRequest {
    #[schemars(description = "The file path where the file should be written")]
    pub path: String,
    #[schemars(description = "Base64 encoded file data to write")]
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WaitRequest {
    #[schemars(description = "The duration to wait in milliseconds")]
    #[serde(default = "default_wait_duration")]
    pub duration: Option<u64>,
}

fn default_wait_duration() -> Option<u64> {
    Some(500)
}

impl ComputerUseTools {
    pub fn new(automation_service: Arc<AutomationService>) -> Self {
        Self { automation_service }
    }

    /// Get all available MCP tools
    pub fn get_tools(&self) -> Vec<Box<dyn McpTool>> {
        vec![
            Box::new(MoveMouseTool::new(self.automation_service.clone())),
            Box::new(ClickMouseTool::new(self.automation_service.clone())),
            Box::new(TypeTextTool::new(self.automation_service.clone())),
            Box::new(PasteTextTool::new(self.automation_service.clone())),
            Box::new(PressKeysTool::new(self.automation_service.clone())),
            Box::new(ScrollTool::new(self.automation_service.clone())),
            Box::new(ScreenshotTool::new(self.automation_service.clone())),
            Box::new(CursorPositionTool::new(self.automation_service.clone())),
            Box::new(ApplicationTool::new(self.automation_service.clone())),
            Box::new(ReadFileTool::new(self.automation_service.clone())),
            Box::new(WriteFileTool::new(self.automation_service.clone())),
            Box::new(WaitTool::new(self.automation_service.clone())),
        ]
    }

    /// Execute a computer automation action (legacy method for compatibility)
    pub async fn execute_computer_action(
        &self,
        action: ComputerAction,
    ) -> Result<serde_json::Value, crate::error::AutomationError> {
        self.automation_service.execute_action(action).await
    }
}

// Individual MCP Tool implementations

/// MCP Tool for moving the mouse cursor
#[derive(Clone)]
pub struct MoveMouseTool {
    automation_service: Arc<AutomationService>,
}

impl MoveMouseTool {
    pub fn new(automation_service: Arc<AutomationService>) -> Self {
        Self { automation_service }
    }
}

#[async_trait]
impl McpTool for MoveMouseTool {
    fn name(&self) -> &str {
        "move_mouse"
    }

    fn description(&self) -> &str {
        "Move the mouse cursor to specific coordinates on the screen"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(MoveMouseRequest)).unwrap()
    }

    async fn call(&self, arguments: serde_json::Value) -> McpToolResult {
        let request: MoveMouseRequest = serde_json::from_value(arguments)
            .map_err(|e| McpToolError::InvalidArguments(e.to_string()))?;

        let coordinates = Coordinates {
            x: request.coordinates.x,
            y: request.coordinates.y,
        };

        let action = ComputerAction::MoveMouse { coordinates };

        match self.automation_service.execute_action(action).await {
            Ok(_) => Ok(format!(
                "Mouse moved to ({}, {})",
                coordinates.x, coordinates.y
            )),
            Err(e) => Err(McpToolError::ExecutionError(e.to_string())),
        }
    }
}

/// MCP Tool for clicking the mouse
#[derive(Clone)]
pub struct ClickMouseTool {
    automation_service: Arc<AutomationService>,
}

impl ClickMouseTool {
    pub fn new(automation_service: Arc<AutomationService>) -> Self {
        Self { automation_service }
    }
}

#[async_trait]
impl McpTool for ClickMouseTool {
    fn name(&self) -> &str {
        "click_mouse"
    }

    fn description(&self) -> &str {
        "Click the mouse at specific coordinates or current position"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(ClickMouseRequest)).unwrap()
    }

    async fn call(&self, arguments: serde_json::Value) -> McpToolResult {
        let request: ClickMouseRequest = serde_json::from_value(arguments)
            .map_err(|e| McpToolError::InvalidArguments(e.to_string()))?;

        let coordinates = request.coordinates.map(|c| Coordinates { x: c.x, y: c.y });
        let button = match request.button {
            ButtonSchema::Left => Button::Left,
            ButtonSchema::Right => Button::Right,
            ButtonSchema::Middle => Button::Middle,
        };
        let click_count = request.click_count.unwrap_or(1);

        let action = ComputerAction::ClickMouse {
            coordinates,
            button,
            click_count,
            hold_keys: None,
        };

        match self.automation_service.execute_action(action).await {
            Ok(_) => {
                let location = if let Some(coords) = coordinates {
                    format!(" at ({}, {})", coords.x, coords.y)
                } else {
                    " at current position".to_string()
                };
                Ok(format!(
                    "Mouse clicked with {:?} button{}",
                    button, location
                ))
            }
            Err(e) => Err(McpToolError::ExecutionError(e.to_string())),
        }
    }
}

/// MCP Tool for typing text
#[derive(Clone)]
pub struct TypeTextTool {
    automation_service: Arc<AutomationService>,
}

impl TypeTextTool {
    pub fn new(automation_service: Arc<AutomationService>) -> Self {
        Self { automation_service }
    }
}

#[async_trait]
impl McpTool for TypeTextTool {
    fn name(&self) -> &str {
        "type_text"
    }

    fn description(&self) -> &str {
        "Type text at the current cursor position"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(TypeTextRequest)).unwrap()
    }

    async fn call(&self, arguments: serde_json::Value) -> McpToolResult {
        let request: TypeTextRequest = serde_json::from_value(arguments)
            .map_err(|e| McpToolError::InvalidArguments(e.to_string()))?;

        let action = ComputerAction::TypeText {
            text: request.text.clone(),
            delay: request.delay,
            sensitive: None,
        };

        match self.automation_service.execute_action(action).await {
            Ok(_) => Ok(format!("Typed text: {}", request.text)),
            Err(e) => Err(McpToolError::ExecutionError(e.to_string())),
        }
    }
}

/// MCP Tool for pasting text
#[derive(Clone)]
pub struct PasteTextTool {
    automation_service: Arc<AutomationService>,
}

impl PasteTextTool {
    pub fn new(automation_service: Arc<AutomationService>) -> Self {
        Self { automation_service }
    }
}

#[async_trait]
impl McpTool for PasteTextTool {
    fn name(&self) -> &str {
        "paste_text"
    }

    fn description(&self) -> &str {
        "Paste text from clipboard or provided text"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(TypeTextRequest)).unwrap()
    }

    async fn call(&self, arguments: serde_json::Value) -> McpToolResult {
        let request: TypeTextRequest = serde_json::from_value(arguments)
            .map_err(|e| McpToolError::InvalidArguments(e.to_string()))?;

        let action = ComputerAction::PasteText {
            text: request.text.clone(),
        };

        match self.automation_service.execute_action(action).await {
            Ok(_) => Ok(format!("Pasted text: {}", request.text)),
            Err(e) => Err(McpToolError::ExecutionError(e.to_string())),
        }
    }
}

/// MCP Tool for pressing keys
#[derive(Clone)]
pub struct PressKeysTool {
    automation_service: Arc<AutomationService>,
}

impl PressKeysTool {
    pub fn new(automation_service: Arc<AutomationService>) -> Self {
        Self { automation_service }
    }
}

#[async_trait]
impl McpTool for PressKeysTool {
    fn name(&self) -> &str {
        "press_keys"
    }

    fn description(&self) -> &str {
        "Press or release specific keys"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(PressKeysRequest)).unwrap()
    }

    async fn call(&self, arguments: serde_json::Value) -> McpToolResult {
        let request: PressKeysRequest = serde_json::from_value(arguments)
            .map_err(|e| McpToolError::InvalidArguments(e.to_string()))?;

        let press = match request.press {
            PressSchema::Down => Press::Down,
            PressSchema::Up => Press::Up,
        };

        let action = ComputerAction::PressKeys {
            keys: request.keys.clone(),
            press,
        };

        match self.automation_service.execute_action(action).await {
            Ok(_) => Ok(format!("Pressed keys: {:?} ({:?})", request.keys, press)),
            Err(e) => Err(McpToolError::ExecutionError(e.to_string())),
        }
    }
}

/// MCP Tool for scrolling
#[derive(Clone)]
pub struct ScrollTool {
    automation_service: Arc<AutomationService>,
}

impl ScrollTool {
    pub fn new(automation_service: Arc<AutomationService>) -> Self {
        Self { automation_service }
    }
}

#[async_trait]
impl McpTool for ScrollTool {
    fn name(&self) -> &str {
        "scroll"
    }

    fn description(&self) -> &str {
        "Scroll in a specific direction at given coordinates"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(ScrollRequest)).unwrap()
    }

    async fn call(&self, arguments: serde_json::Value) -> McpToolResult {
        let request: ScrollRequest = serde_json::from_value(arguments)
            .map_err(|e| McpToolError::InvalidArguments(e.to_string()))?;

        let coordinates = request.coordinates.map(|c| Coordinates { x: c.x, y: c.y });
        let direction = match request.direction {
            ScrollDirection::Up => bytebot_shared_rs::types::computer_action::ScrollDirection::Up,
            ScrollDirection::Down => {
                bytebot_shared_rs::types::computer_action::ScrollDirection::Down
            }
            ScrollDirection::Left => {
                bytebot_shared_rs::types::computer_action::ScrollDirection::Left
            }
            ScrollDirection::Right => {
                bytebot_shared_rs::types::computer_action::ScrollDirection::Right
            }
        };

        let action = ComputerAction::Scroll {
            coordinates,
            direction,
            scroll_count: request.scroll_count,
            hold_keys: None,
        };

        match self.automation_service.execute_action(action).await {
            Ok(_) => Ok(format!(
                "Scrolled {:?} {} times",
                direction, request.scroll_count
            )),
            Err(e) => Err(McpToolError::ExecutionError(e.to_string())),
        }
    }
}

/// MCP Tool for taking screenshots
#[derive(Clone)]
pub struct ScreenshotTool {
    automation_service: Arc<AutomationService>,
}

impl ScreenshotTool {
    pub fn new(automation_service: Arc<AutomationService>) -> Self {
        Self { automation_service }
    }
}

#[async_trait]
impl McpTool for ScreenshotTool {
    fn name(&self) -> &str {
        "screenshot"
    }

    fn description(&self) -> &str {
        "Take a screenshot of the current screen"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    async fn call(&self, _arguments: serde_json::Value) -> McpToolResult {
        let action = ComputerAction::Screenshot;

        match self.automation_service.execute_action(action).await {
            Ok(result) => {
                if let Some(image_data) = result.get("image").and_then(|v| v.as_str()) {
                    Ok(format!(
                        "Screenshot taken successfully. Image data: {} bytes",
                        image_data.len()
                    ))
                } else {
                    Ok("Screenshot taken but no image data returned".to_string())
                }
            }
            Err(e) => Err(McpToolError::ExecutionError(e.to_string())),
        }
    }
}

/// MCP Tool for getting cursor position
#[derive(Clone)]
pub struct CursorPositionTool {
    automation_service: Arc<AutomationService>,
}

impl CursorPositionTool {
    pub fn new(automation_service: Arc<AutomationService>) -> Self {
        Self { automation_service }
    }
}

#[async_trait]
impl McpTool for CursorPositionTool {
    fn name(&self) -> &str {
        "cursor_position"
    }

    fn description(&self) -> &str {
        "Get the current cursor position"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    async fn call(&self, _arguments: serde_json::Value) -> McpToolResult {
        let action = ComputerAction::CursorPosition;

        match self.automation_service.execute_action(action).await {
            Ok(result) => {
                if let (Some(x), Some(y)) = (
                    result.get("x").and_then(|v| v.as_i64()),
                    result.get("y").and_then(|v| v.as_i64()),
                ) {
                    Ok(format!("Cursor position: ({x}, {y})"))
                } else {
                    Ok(format!("Cursor position result: {result}"))
                }
            }
            Err(e) => Err(McpToolError::ExecutionError(e.to_string())),
        }
    }
}

/// MCP Tool for switching applications
#[derive(Clone)]
pub struct ApplicationTool {
    automation_service: Arc<AutomationService>,
}

impl ApplicationTool {
    pub fn new(automation_service: Arc<AutomationService>) -> Self {
        Self { automation_service }
    }
}

#[async_trait]
impl McpTool for ApplicationTool {
    fn name(&self) -> &str {
        "application"
    }

    fn description(&self) -> &str {
        "Switch to or open a specific application"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(ApplicationRequest)).unwrap()
    }

    async fn call(&self, arguments: serde_json::Value) -> McpToolResult {
        let request: ApplicationRequest = serde_json::from_value(arguments)
            .map_err(|e| McpToolError::InvalidArguments(e.to_string()))?;

        let application = match request.application {
            ApplicationSchema::Firefox => Application::Firefox,
            ApplicationSchema::OnePassword => Application::OnePassword,
            ApplicationSchema::Thunderbird => Application::Thunderbird,
            ApplicationSchema::Vscode => Application::Vscode,
            ApplicationSchema::Terminal => Application::Terminal,
            ApplicationSchema::Desktop => Application::Desktop,
            ApplicationSchema::Directory => Application::Directory,
        };

        let action = ComputerAction::Application { application };

        match self.automation_service.execute_action(action).await {
            Ok(_) => Ok(format!("Switched to application: {application:?}")),
            Err(e) => Err(McpToolError::ExecutionError(e.to_string())),
        }
    }
}

/// MCP Tool for reading files
#[derive(Clone)]
pub struct ReadFileTool {
    automation_service: Arc<AutomationService>,
}

impl ReadFileTool {
    pub fn new(automation_service: Arc<AutomationService>) -> Self {
        Self { automation_service }
    }
}

#[async_trait]
impl McpTool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(ReadFileRequest)).unwrap()
    }

    async fn call(&self, arguments: serde_json::Value) -> McpToolResult {
        let request: ReadFileRequest = serde_json::from_value(arguments)
            .map_err(|e| McpToolError::InvalidArguments(e.to_string()))?;

        let action = ComputerAction::ReadFile {
            path: request.path.clone(),
        };

        match self.automation_service.execute_action(action).await {
            Ok(result) => {
                if let Some(content) = result.get("content").and_then(|v| v.as_str()) {
                    Ok(format!(
                        "File '{}' read successfully. Content length: {} bytes",
                        request.path,
                        content.len()
                    ))
                } else {
                    Ok(format!("File '{}' read result: {}", request.path, result))
                }
            }
            Err(e) => Err(McpToolError::ExecutionError(e.to_string())),
        }
    }
}

/// MCP Tool for writing files
#[derive(Clone)]
pub struct WriteFileTool {
    automation_service: Arc<AutomationService>,
}

impl WriteFileTool {
    pub fn new(automation_service: Arc<AutomationService>) -> Self {
        Self { automation_service }
    }
}

#[async_trait]
impl McpTool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write data to a file"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(WriteFileRequest)).unwrap()
    }

    async fn call(&self, arguments: serde_json::Value) -> McpToolResult {
        let request: WriteFileRequest = serde_json::from_value(arguments)
            .map_err(|e| McpToolError::InvalidArguments(e.to_string()))?;

        let action = ComputerAction::WriteFile {
            path: request.path.clone(),
            data: request.data.clone(),
        };

        match self.automation_service.execute_action(action).await {
            Ok(_) => Ok(format!(
                "File '{}' written successfully. Data length: {} bytes",
                request.path,
                request.data.len()
            )),
            Err(e) => Err(McpToolError::ExecutionError(e.to_string())),
        }
    }
}

/// MCP Tool for waiting
#[derive(Clone)]
pub struct WaitTool {
    automation_service: Arc<AutomationService>,
}

impl WaitTool {
    pub fn new(automation_service: Arc<AutomationService>) -> Self {
        Self { automation_service }
    }
}

#[async_trait]
impl McpTool for WaitTool {
    fn name(&self) -> &str {
        "wait"
    }

    fn description(&self) -> &str {
        "Wait for a specified duration in milliseconds"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(WaitRequest)).unwrap()
    }

    async fn call(&self, arguments: serde_json::Value) -> McpToolResult {
        let request: WaitRequest = serde_json::from_value(arguments)
            .map_err(|e| McpToolError::InvalidArguments(e.to_string()))?;

        let duration = request.duration.unwrap_or(500);
        let action = ComputerAction::Wait { duration };

        match self.automation_service.execute_action(action).await {
            Ok(_) => Ok(format!("Waited for {duration} milliseconds")),
            Err(e) => Err(McpToolError::ExecutionError(e.to_string())),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::AutomationService;

    #[tokio::test]
    async fn test_computer_use_tools_creation() {
        let automation_service = Arc::new(AutomationService::new().unwrap());
        let tools = ComputerUseTools::new(automation_service);

        // Test that tools can be retrieved
        let mcp_tools = tools.get_tools();
        assert_eq!(mcp_tools.len(), 12); // We have 12 different tools

        // Verify tool names
        let tool_names: Vec<&str> = mcp_tools.iter().map(|t| t.name()).collect();
        assert!(tool_names.contains(&"move_mouse"));
        assert!(tool_names.contains(&"click_mouse"));
        assert!(tool_names.contains(&"type_text"));
        assert!(tool_names.contains(&"paste_text"));
        assert!(tool_names.contains(&"press_keys"));
        assert!(tool_names.contains(&"scroll"));
        assert!(tool_names.contains(&"screenshot"));
        assert!(tool_names.contains(&"cursor_position"));
        assert!(tool_names.contains(&"application"));
        assert!(tool_names.contains(&"read_file"));
        assert!(tool_names.contains(&"write_file"));
        assert!(tool_names.contains(&"wait"));
    }

    #[test]
    fn test_schema_serialization() {
        let request = MoveMouseRequest {
            coordinates: CoordinatesSchema { x: 100, y: 200 },
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("100"));
        assert!(json.contains("200"));
    }

    #[test]
    fn test_move_mouse_tool_schema() {
        let automation_service = Arc::new(AutomationService::new().unwrap());
        let tool = MoveMouseTool::new(automation_service);

        assert_eq!(tool.name(), "move_mouse");
        assert_eq!(
            tool.description(),
            "Move the mouse cursor to specific coordinates on the screen"
        );

        let schema = tool.input_schema();
        assert!(schema.is_object());

        // Verify schema contains required properties
        let properties = schema.get("properties").unwrap();
        assert!(properties.get("coordinates").is_some());
    }

    #[test]
    fn test_click_mouse_tool_schema() {
        let automation_service = Arc::new(AutomationService::new().unwrap());
        let tool = ClickMouseTool::new(automation_service);

        assert_eq!(tool.name(), "click_mouse");
        assert_eq!(
            tool.description(),
            "Click the mouse at specific coordinates or current position"
        );

        let schema = tool.input_schema();
        assert!(schema.is_object());

        let properties = schema.get("properties").unwrap();
        assert!(properties.get("coordinates").is_some());
        assert!(properties.get("button").is_some());
        assert!(properties.get("click_count").is_some());
    }

    #[test]
    fn test_type_text_tool_schema() {
        let automation_service = Arc::new(AutomationService::new().unwrap());
        let tool = TypeTextTool::new(automation_service);

        assert_eq!(tool.name(), "type_text");
        assert_eq!(
            tool.description(),
            "Type text at the current cursor position"
        );

        let schema = tool.input_schema();
        assert!(schema.is_object());

        let properties = schema.get("properties").unwrap();
        assert!(properties.get("text").is_some());
        assert!(properties.get("delay").is_some());
    }

    #[test]
    fn test_application_tool_schema() {
        let automation_service = Arc::new(AutomationService::new().unwrap());
        let tool = ApplicationTool::new(automation_service);

        assert_eq!(tool.name(), "application");
        assert_eq!(
            tool.description(),
            "Switch to or open a specific application"
        );

        let schema = tool.input_schema();
        assert!(schema.is_object());

        let properties = schema.get("properties").unwrap();
        assert!(properties.get("application").is_some());
    }

    #[test]
    fn test_file_tools_schema() {
        let automation_service = Arc::new(AutomationService::new().unwrap());

        let read_tool = ReadFileTool::new(automation_service.clone());
        assert_eq!(read_tool.name(), "read_file");
        assert_eq!(read_tool.description(), "Read the contents of a file");

        let write_tool = WriteFileTool::new(automation_service);
        assert_eq!(write_tool.name(), "write_file");
        assert_eq!(write_tool.description(), "Write data to a file");

        let read_schema = read_tool.input_schema();
        let write_schema = write_tool.input_schema();

        assert!(read_schema.is_object());
        assert!(write_schema.is_object());

        let read_props = read_schema.get("properties").unwrap();
        let write_props = write_schema.get("properties").unwrap();

        assert!(read_props.get("path").is_some());
        assert!(write_props.get("path").is_some());
        assert!(write_props.get("data").is_some());
    }

    #[test]
    fn test_request_validation() {
        // Test valid move mouse request
        let valid_request = MoveMouseRequest {
            coordinates: CoordinatesSchema { x: 100, y: 200 },
        };
        let json = serde_json::to_value(&valid_request).unwrap();
        let _parsed: MoveMouseRequest = serde_json::from_value(json).unwrap();

        // Test valid click mouse request
        let valid_click = ClickMouseRequest {
            coordinates: Some(CoordinatesSchema { x: 50, y: 75 }),
            button: ButtonSchema::Left,
            click_count: Some(2),
        };
        let json = serde_json::to_value(&valid_click).unwrap();
        let _parsed: ClickMouseRequest = serde_json::from_value(json).unwrap();

        // Test valid type text request
        let valid_type = TypeTextRequest {
            text: "Hello World".to_string(),
            delay: Some(100),
        };
        let json = serde_json::to_value(&valid_type).unwrap();
        let _parsed: TypeTextRequest = serde_json::from_value(json).unwrap();
    }

    #[test]
    fn test_button_schema_serialization() {
        let left_button = ButtonSchema::Left;
        let right_button = ButtonSchema::Right;
        let middle_button = ButtonSchema::Middle;

        let left_json = serde_json::to_string(&left_button).unwrap();
        let right_json = serde_json::to_string(&right_button).unwrap();
        let middle_json = serde_json::to_string(&middle_button).unwrap();

        assert_eq!(left_json, "\"left\"");
        assert_eq!(right_json, "\"right\"");
        assert_eq!(middle_json, "\"middle\"");

        // Test deserialization
        let left_parsed: ButtonSchema = serde_json::from_str(&left_json).unwrap();
        let right_parsed: ButtonSchema = serde_json::from_str(&right_json).unwrap();
        let middle_parsed: ButtonSchema = serde_json::from_str(&middle_json).unwrap();

        assert!(matches!(left_parsed, ButtonSchema::Left));
        assert!(matches!(right_parsed, ButtonSchema::Right));
        assert!(matches!(middle_parsed, ButtonSchema::Middle));
    }

    #[test]
    fn test_application_schema_serialization() {
        let firefox = ApplicationSchema::Firefox;
        let vscode = ApplicationSchema::Vscode;
        let onepassword = ApplicationSchema::OnePassword;

        let firefox_json = serde_json::to_string(&firefox).unwrap();
        let vscode_json = serde_json::to_string(&vscode).unwrap();
        let onepassword_json = serde_json::to_string(&onepassword).unwrap();

        assert_eq!(firefox_json, "\"firefox\"");
        assert_eq!(vscode_json, "\"vscode\"");
        assert_eq!(onepassword_json, "\"1password\"");

        // Test deserialization
        let firefox_parsed: ApplicationSchema = serde_json::from_str(&firefox_json).unwrap();
        let vscode_parsed: ApplicationSchema = serde_json::from_str(&vscode_json).unwrap();
        let onepassword_parsed: ApplicationSchema =
            serde_json::from_str(&onepassword_json).unwrap();

        assert!(matches!(firefox_parsed, ApplicationSchema::Firefox));
        assert!(matches!(vscode_parsed, ApplicationSchema::Vscode));
        assert!(matches!(onepassword_parsed, ApplicationSchema::OnePassword));
    }

    #[tokio::test]
    async fn test_tool_argument_validation() {
        let automation_service = Arc::new(AutomationService::new().unwrap());
        let _tool = MoveMouseTool::new(automation_service);

        // Test valid arguments
        let valid_args = serde_json::json!({
            "coordinates": {
                "x": 100,
                "y": 200
            }
        });

        // This should not panic during argument parsing
        let result = serde_json::from_value::<MoveMouseRequest>(valid_args);
        assert!(result.is_ok());

        // Test invalid arguments (missing coordinates)
        let invalid_args = serde_json::json!({
            "invalid_field": "value"
        });

        let result = serde_json::from_value::<MoveMouseRequest>(invalid_args);
        assert!(result.is_err());
    }

    #[test]
    fn test_scroll_direction_serialization() {
        let directions = [
            ScrollDirection::Up,
            ScrollDirection::Down,
            ScrollDirection::Left,
            ScrollDirection::Right,
        ];

        let expected_strings = ["up", "down", "left", "right"];

        for (direction, expected) in directions.iter().zip(expected_strings.iter()) {
            let json = serde_json::to_string(direction).unwrap();
            assert_eq!(json, format!("\"{expected}\""));

            let parsed: ScrollDirection = serde_json::from_str(&json).unwrap();
            assert_eq!(&parsed, direction);
        }
    }

    #[test]
    fn test_press_schema_serialization() {
        let down = PressSchema::Down;
        let up = PressSchema::Up;

        let down_json = serde_json::to_string(&down).unwrap();
        let up_json = serde_json::to_string(&up).unwrap();

        assert_eq!(down_json, "\"down\"");
        assert_eq!(up_json, "\"up\"");

        let down_parsed: PressSchema = serde_json::from_str(&down_json).unwrap();
        let up_parsed: PressSchema = serde_json::from_str(&up_json).unwrap();

        assert!(matches!(down_parsed, PressSchema::Down));
        assert!(matches!(up_parsed, PressSchema::Up));
    }

    #[test]
    fn test_wait_request_defaults() {
        // Test default duration
        let wait_request_json = serde_json::json!({});
        let parsed: WaitRequest = serde_json::from_value(wait_request_json).unwrap();
        assert_eq!(parsed.duration, Some(500));

        // Test explicit duration
        let wait_request_json = serde_json::json!({"duration": 1000});
        let parsed: WaitRequest = serde_json::from_value(wait_request_json).unwrap();
        assert_eq!(parsed.duration, Some(1000));
    }

    #[test]
    fn test_click_count_defaults() {
        // Test default click count
        let click_request_json = serde_json::json!({
            "button": "left"
        });
        let parsed: ClickMouseRequest = serde_json::from_value(click_request_json).unwrap();
        assert_eq!(parsed.click_count, Some(1));

        // Test explicit click count
        let click_request_json = serde_json::json!({
            "button": "left",
            "click_count": 3
        });
        let parsed: ClickMouseRequest = serde_json::from_value(click_request_json).unwrap();
        assert_eq!(parsed.click_count, Some(3));
    }

    #[test]
    fn test_schema_generation() {
        // Test that schemas can be generated without panicking
        let move_schema = schemars::schema_for!(MoveMouseRequest);
        assert!(move_schema.schema.object.is_some());

        let click_schema = schemars::schema_for!(ClickMouseRequest);
        assert!(click_schema.schema.object.is_some());

        let type_schema = schemars::schema_for!(TypeTextRequest);
        assert!(type_schema.schema.object.is_some());

        let app_schema = schemars::schema_for!(ApplicationRequest);
        assert!(app_schema.schema.object.is_some());

        let file_read_schema = schemars::schema_for!(ReadFileRequest);
        assert!(file_read_schema.schema.object.is_some());

        let file_write_schema = schemars::schema_for!(WriteFileRequest);
        assert!(file_write_schema.schema.object.is_some());
    }

    #[tokio::test]
    async fn test_mock_tool_execution() {
        let automation_service = Arc::new(AutomationService::new().unwrap());

        // Test that tools can be created and their methods called without panicking
        let move_tool = MoveMouseTool::new(automation_service.clone());
        let click_tool = ClickMouseTool::new(automation_service.clone());
        let type_tool = TypeTextTool::new(automation_service.clone());
        let screenshot_tool = ScreenshotTool::new(automation_service.clone());

        // Verify tool properties
        assert_eq!(move_tool.name(), "move_mouse");
        assert_eq!(click_tool.name(), "click_mouse");
        assert_eq!(type_tool.name(), "type_text");
        assert_eq!(screenshot_tool.name(), "screenshot");

        // Verify descriptions are not empty
        assert!(!move_tool.description().is_empty());
        assert!(!click_tool.description().is_empty());
        assert!(!type_tool.description().is_empty());
        assert!(!screenshot_tool.description().is_empty());

        // Verify schemas are valid JSON objects
        let move_schema = move_tool.input_schema();
        let click_schema = click_tool.input_schema();
        let type_schema = type_tool.input_schema();
        let screenshot_schema = screenshot_tool.input_schema();

        assert!(move_schema.is_object());
        assert!(click_schema.is_object());
        assert!(type_schema.is_object());
        assert!(screenshot_schema.is_object());
    }

    #[test]
    fn test_comprehensive_schema_validation() {
        // Test all request types can be serialized and deserialized
        let test_cases = vec![
            (
                "MoveMouseRequest",
                serde_json::json!({
                    "coordinates": {"x": 100, "y": 200}
                }),
            ),
            (
                "ClickMouseRequest",
                serde_json::json!({
                    "coordinates": {"x": 50, "y": 75},
                    "button": "left",
                    "click_count": 2
                }),
            ),
            (
                "TypeTextRequest",
                serde_json::json!({
                    "text": "Hello World",
                    "delay": 100
                }),
            ),
            (
                "PressKeysRequest",
                serde_json::json!({
                    "keys": ["ctrl", "c"],
                    "press": "down"
                }),
            ),
            (
                "ScrollRequest",
                serde_json::json!({
                    "coordinates": {"x": 300, "y": 400},
                    "direction": "up",
                    "scroll_count": 3
                }),
            ),
            (
                "ApplicationRequest",
                serde_json::json!({
                    "application": "firefox"
                }),
            ),
            (
                "ReadFileRequest",
                serde_json::json!({
                    "path": "/tmp/test.txt"
                }),
            ),
            (
                "WriteFileRequest",
                serde_json::json!({
                    "path": "/tmp/output.txt",
                    "data": "SGVsbG8gV29ybGQ="
                }),
            ),
            (
                "WaitRequest",
                serde_json::json!({
                    "duration": 1000
                }),
            ),
        ];

        for (name, json_value) in test_cases {
            match name {
                "MoveMouseRequest" => {
                    let parsed: Result<MoveMouseRequest, _> = serde_json::from_value(json_value);
                    assert!(parsed.is_ok(), "Failed to parse {name}");
                }
                "ClickMouseRequest" => {
                    let parsed: Result<ClickMouseRequest, _> = serde_json::from_value(json_value);
                    assert!(parsed.is_ok(), "Failed to parse {name}");
                }
                "TypeTextRequest" => {
                    let parsed: Result<TypeTextRequest, _> = serde_json::from_value(json_value);
                    assert!(parsed.is_ok(), "Failed to parse {name}");
                }
                "PressKeysRequest" => {
                    let parsed: Result<PressKeysRequest, _> = serde_json::from_value(json_value);
                    assert!(parsed.is_ok(), "Failed to parse {name}");
                }
                "ScrollRequest" => {
                    let parsed: Result<ScrollRequest, _> = serde_json::from_value(json_value);
                    assert!(parsed.is_ok(), "Failed to parse {name}");
                }
                "ApplicationRequest" => {
                    let parsed: Result<ApplicationRequest, _> = serde_json::from_value(json_value);
                    assert!(parsed.is_ok(), "Failed to parse {name}");
                }
                "ReadFileRequest" => {
                    let parsed: Result<ReadFileRequest, _> = serde_json::from_value(json_value);
                    assert!(parsed.is_ok(), "Failed to parse {name}");
                }
                "WriteFileRequest" => {
                    let parsed: Result<WriteFileRequest, _> = serde_json::from_value(json_value);
                    assert!(parsed.is_ok(), "Failed to parse {name}");
                }
                "WaitRequest" => {
                    let parsed: Result<WaitRequest, _> = serde_json::from_value(json_value);
                    assert!(parsed.is_ok(), "Failed to parse {name}");
                }
                _ => panic!("Unknown request type: {name}"),
            }
        }
    }
}
