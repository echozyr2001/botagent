use std::time::Duration;

use async_trait::async_trait;
use bytebot_shared_rs::types::{
    message::{Message, MessageContentBlock},
    task::Role,
};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

use super::{AIService, ModelInfo};
use crate::{config::Config, error::AIError};

/// OpenAI API constants
const OPENAI_API_BASE: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o";
const MAX_TOKENS: u32 = 4096;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// Get available OpenAI models
fn get_openai_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            provider: "openai".to_string(),
            name: "gpt-4o".to_string(),
            title: "GPT-4o".to_string(),
        },
        ModelInfo {
            provider: "openai".to_string(),
            name: "gpt-4o-mini".to_string(),
            title: "GPT-4o Mini".to_string(),
        },
        ModelInfo {
            provider: "openai".to_string(),
            name: "gpt-4-turbo".to_string(),
            title: "GPT-4 Turbo".to_string(),
        },
        ModelInfo {
            provider: "openai".to_string(),
            name: "gpt-3.5-turbo".to_string(),
            title: "GPT-3.5 Turbo".to_string(),
        },
    ]
}

/// OpenAI API request structures
#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    max_tokens: u32,
    temperature: f32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct OpenAIMessage {
    role: String,
    content: Vec<OpenAIContent>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OpenAIContent {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Serialize)]
struct ImageUrl {
    url: String,
}

/// OpenAI API response structures
#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: OpenAIResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAIResponseMessage {
    role: String,
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, Deserialize)]
struct ToolCall {
    id: String,
    #[serde(rename = "type")]
    tool_type: String,
    function: FunctionCall,
}

#[derive(Debug, Clone, Deserialize)]
struct FunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

/// OpenAI API error response
#[derive(Debug, Deserialize)]
struct OpenAIError {
    message: String,
    #[serde(rename = "type")]
    error_type: String,
    code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIErrorResponse {
    error: OpenAIError,
}

/// OpenAI service implementation
pub struct OpenAIService {
    client: Client,
    api_key: Option<String>,
}

impl OpenAIService {
    /// Create a new OpenAI service instance
    pub fn new(config: &Config) -> Self {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key: config.openai_api_key.clone(),
        }
    }

    /// Convert internal messages to OpenAI format
    fn format_messages_for_openai(
        &self,
        system_prompt: &str,
        messages: Vec<Message>,
    ) -> Result<Vec<OpenAIMessage>, AIError> {
        let mut openai_messages = Vec::new();

        // Add system message first
        if !system_prompt.is_empty() {
            openai_messages.push(OpenAIMessage {
                role: "system".to_string(),
                content: vec![OpenAIContent::Text {
                    text: system_prompt.to_string(),
                }],
            });
        }

        for message in messages {
            let content_blocks = message
                .get_content_blocks()
                .map_err(AIError::Serialization)?;

            let mut content: Vec<OpenAIContent> = Vec::new();

            for block in content_blocks {
                match block {
                    MessageContentBlock::Text { text } => {
                        content.push(OpenAIContent::Text { text });
                    }
                    MessageContentBlock::Image { source } => {
                        // Convert base64 image to data URL format
                        let data_url = format!("data:{};base64,{}", source.media_type, source.data);
                        content.push(OpenAIContent::ImageUrl {
                            image_url: ImageUrl { url: data_url },
                        });
                    }
                    MessageContentBlock::ToolUse { name, id, input } => {
                        // OpenAI handles tool calls differently - they appear in assistant messages
                        // For now, convert to text representation
                        let tool_text = format!("Tool call: {name} (id: {id}) with input: {input}");
                        content.push(OpenAIContent::Text { text: tool_text });
                    }
                    MessageContentBlock::ToolResult {
                        tool_use_id,
                        content: tool_content,
                        is_error,
                    } => {
                        // Convert tool result to text
                        let result_text = tool_content
                            .iter()
                            .filter_map(|block| match block {
                                MessageContentBlock::Text { text } => Some(text.clone()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join(" ");

                        let error_prefix = if is_error.unwrap_or(false) {
                            "Tool error"
                        } else {
                            "Tool result"
                        };

                        let tool_result_text =
                            format!("{error_prefix} for {tool_use_id}: {result_text}");
                        content.push(OpenAIContent::Text {
                            text: tool_result_text,
                        });
                    }
                    MessageContentBlock::Document { source, name, .. } => {
                        // Convert document to text representation
                        let doc_text = format!(
                            "Document: {} (type: {})",
                            name.unwrap_or_else(|| "unnamed".to_string()),
                            source.media_type
                        );
                        content.push(OpenAIContent::Text { text: doc_text });
                    }
                    MessageContentBlock::Thinking { thinking, .. } => {
                        // Include thinking content as text
                        content.push(OpenAIContent::Text { text: thinking });
                    }
                    MessageContentBlock::RedactedThinking { .. } => {
                        // Skip redacted thinking
                        continue;
                    }
                }
            }

            // Only add message if it has content
            if !content.is_empty() {
                openai_messages.push(OpenAIMessage {
                    role: match message.role {
                        Role::User => "user".to_string(),
                        Role::Assistant => "assistant".to_string(),
                    },
                    content,
                });
            }
        }

        Ok(openai_messages)
    }

    /// Convert OpenAI response to internal format
    fn format_openai_response(
        &self,
        response_message: OpenAIResponseMessage,
    ) -> Vec<MessageContentBlock> {
        let mut content_blocks = Vec::new();

        // Add text content if present
        if let Some(text) = response_message.content {
            if !text.is_empty() {
                content_blocks.push(MessageContentBlock::Text { text });
            }
        }

        // Add tool calls if present
        for tool_call in response_message.tool_calls {
            // Parse the arguments JSON
            let input = serde_json::from_str(&tool_call.function.arguments)
                .unwrap_or_else(|_| serde_json::json!({}));

            content_blocks.push(MessageContentBlock::ToolUse {
                id: tool_call.id,
                name: tool_call.function.name,
                input,
            });
        }

        content_blocks
    }

    /// Handle API errors and convert to AIError with retry logic
    fn handle_api_error(&self, status: StatusCode, body: &str) -> AIError {
        match status {
            StatusCode::TOO_MANY_REQUESTS => {
                warn!("OpenAI API rate limit exceeded");
                AIError::RateLimit
            }
            StatusCode::UNAUTHORIZED => {
                error!("OpenAI API authentication failed");
                AIError::Api {
                    status: status.as_u16(),
                    message: "Authentication failed - check API key".to_string(),
                }
            }
            StatusCode::BAD_REQUEST => {
                // Try to parse the error response
                if let Ok(error_response) = serde_json::from_str::<OpenAIErrorResponse>(body) {
                    error!("OpenAI API bad request: {}", error_response.error.message);
                    AIError::Api {
                        status: status.as_u16(),
                        message: error_response.error.message,
                    }
                } else {
                    error!("OpenAI API bad request: {}", body);
                    AIError::Api {
                        status: status.as_u16(),
                        message: "Bad request".to_string(),
                    }
                }
            }
            StatusCode::INTERNAL_SERVER_ERROR => {
                error!("OpenAI API internal server error: {}", body);
                AIError::Api {
                    status: status.as_u16(),
                    message: "OpenAI server error - please retry".to_string(),
                }
            }
            _ => {
                error!("OpenAI API error {}: {}", status, body);
                AIError::Api {
                    status: status.as_u16(),
                    message: format!("API error: {status}"),
                }
            }
        }
    }

    /// Make API request with retry logic for transient failures
    async fn make_request_with_retry(
        &self,
        request: &OpenAIRequest,
        api_key: &str,
        max_retries: u32,
    ) -> Result<OpenAIResponse, AIError> {
        let mut last_error = None;

        for attempt in 0..=max_retries {
            if attempt > 0 {
                // Exponential backoff: 1s, 2s, 4s, 8s
                let delay = Duration::from_secs(2_u64.pow(attempt - 1));
                tokio::time::sleep(delay).await;
                warn!(
                    "Retrying OpenAI API request (attempt {}/{})",
                    attempt + 1,
                    max_retries + 1
                );
            }

            let response = self
                .client
                .post(format!("{OPENAI_API_BASE}/chat/completions"))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {api_key}"))
                .json(request)
                .send()
                .await
                .map_err(AIError::Http)?;

            let status = response.status();
            let body = response.text().await.map_err(AIError::Http)?;

            match status {
                status if status.is_success() => {
                    let openai_response: OpenAIResponse =
                        serde_json::from_str(&body).map_err(AIError::Serialization)?;
                    return Ok(openai_response);
                }
                StatusCode::TOO_MANY_REQUESTS | StatusCode::INTERNAL_SERVER_ERROR => {
                    // These are retryable errors
                    last_error = Some(self.handle_api_error(status, &body));
                    continue;
                }
                _ => {
                    // Non-retryable errors
                    return Err(self.handle_api_error(status, &body));
                }
            }
        }

        // If we've exhausted all retries, return the last error
        Err(last_error.unwrap_or_else(|| AIError::Api {
            status: 500,
            message: "Max retries exceeded".to_string(),
        }))
    }
}

#[async_trait]
impl AIService for OpenAIService {
    async fn generate_response(
        &self,
        system_prompt: &str,
        messages: Vec<Message>,
        model: Option<String>,
        use_tools: bool,
        _signal: Option<tokio_util::sync::CancellationToken>,
    ) -> Result<Vec<MessageContentBlock>, AIError> {
        let api_key = self.api_key.as_ref().ok_or_else(|| AIError::Api {
            status: 401,
            message: "OpenAI API key not configured".to_string(),
        })?;

        let model = model.unwrap_or_else(|| DEFAULT_MODEL.to_string());

        // Validate model
        let models = get_openai_models();
        if !models.iter().any(|m| m.name == model) {
            return Err(AIError::InvalidModel(format!(
                "Invalid OpenAI model: {model}"
            )));
        }

        let openai_messages = self.format_messages_for_openai(system_prompt, messages)?;

        let request = OpenAIRequest {
            model,
            messages: openai_messages,
            max_tokens: MAX_TOKENS,
            temperature: 0.7,
            tools: if use_tools {
                // TODO: Implement tools from agent.tools - for now return empty array
                // This will be implemented in a future task
                vec![]
            } else {
                vec![]
            },
        };

        // Use retry logic for API calls
        let response = self.make_request_with_retry(&request, api_key, 3).await?;

        if response.choices.is_empty() {
            return Err(AIError::Api {
                status: 500,
                message: "No response choices returned from OpenAI".to_string(),
            });
        }

        let choice = &response.choices[0];
        Ok(self.format_openai_response(choice.message.clone()))
    }

    fn list_models(&self) -> Vec<ModelInfo> {
        get_openai_models()
    }

    fn is_available(&self) -> bool {
        self.api_key.is_some()
    }
}

#[cfg(test)]
mod tests {
    use bytebot_shared_rs::types::message::Message;

    use super::*;

    fn create_test_config() -> Config {
        Config {
            openai_api_key: Some("test-key".to_string()),
            ..Default::default()
        }
    }

    fn create_test_message(content: Vec<MessageContentBlock>, role: Role) -> Message {
        Message::new(content, role, "test-task-id".to_string())
    }

    #[test]
    fn test_openai_service_creation() {
        let config = create_test_config();
        let service = OpenAIService::new(&config);
        assert!(service.is_available());
    }

    #[test]
    fn test_openai_service_without_api_key() {
        let config = Config {
            openai_api_key: None,
            ..Default::default()
        };
        let service = OpenAIService::new(&config);
        assert!(!service.is_available());
    }

    #[test]
    fn test_list_models() {
        let config = create_test_config();
        let service = OpenAIService::new(&config);
        let models = service.list_models();

        assert_eq!(models.len(), 4);
        assert_eq!(models[0].name, "gpt-4o");
        assert_eq!(models[0].provider, "openai");
        assert_eq!(models[1].name, "gpt-4o-mini");
        assert_eq!(models[2].name, "gpt-4-turbo");
        assert_eq!(models[3].name, "gpt-3.5-turbo");
    }

    #[test]
    fn test_format_messages_for_openai() {
        let config = create_test_config();
        let service = OpenAIService::new(&config);

        let messages = vec![
            create_test_message(vec![MessageContentBlock::text("Hello")], Role::User),
            create_test_message(
                vec![MessageContentBlock::text("Hi there!")],
                Role::Assistant,
            ),
        ];

        let result = service
            .format_messages_for_openai("You are a helpful assistant", messages)
            .unwrap();

        // Should have system message + 2 user messages
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].role, "system");
        assert_eq!(result[1].role, "user");
        assert_eq!(result[2].role, "assistant");

        // Check system message content
        match &result[0].content[0] {
            OpenAIContent::Text { text } => {
                assert_eq!(text, "You are a helpful assistant");
            }
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_format_messages_with_image() {
        let config = create_test_config();
        let service = OpenAIService::new(&config);

        let messages = vec![create_test_message(
            vec![MessageContentBlock::image("image/png", "base64data")],
            Role::User,
        )];

        let result = service.format_messages_for_openai("", messages).unwrap();

        assert_eq!(result.len(), 1);
        match &result[0].content[0] {
            OpenAIContent::ImageUrl { image_url } => {
                assert_eq!(image_url.url, "data:image/png;base64,base64data");
            }
            _ => panic!("Expected image URL content"),
        }
    }

    #[test]
    fn test_format_messages_with_tool_use() {
        let config = create_test_config();
        let service = OpenAIService::new(&config);

        let messages = vec![create_test_message(
            vec![MessageContentBlock::tool_use(
                "test_tool",
                "123",
                serde_json::json!({"param": "value"}),
            )],
            Role::Assistant,
        )];

        let result = service.format_messages_for_openai("", messages).unwrap();

        assert_eq!(result.len(), 1);
        match &result[0].content[0] {
            OpenAIContent::Text { text } => {
                assert!(text.contains("Tool call: test_tool"));
                assert!(text.contains("id: 123"));
            }
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_format_openai_response() {
        let config = create_test_config();
        let service = OpenAIService::new(&config);

        let response_message = OpenAIResponseMessage {
            role: "assistant".to_string(),
            content: Some("Hello world".to_string()),
            tool_calls: vec![ToolCall {
                id: "123".to_string(),
                tool_type: "function".to_string(),
                function: FunctionCall {
                    name: "test_tool".to_string(),
                    arguments: r#"{"param": "value"}"#.to_string(),
                },
            }],
        };

        let result = service.format_openai_response(response_message);
        assert_eq!(result.len(), 2);

        match &result[0] {
            MessageContentBlock::Text { text } => assert_eq!(text, "Hello world"),
            _ => panic!("Expected text block"),
        }

        match &result[1] {
            MessageContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "123");
                assert_eq!(name, "test_tool");
                assert_eq!(input["param"], "value");
            }
            _ => panic!("Expected tool use block"),
        }
    }

    #[test]
    fn test_format_openai_response_empty_content() {
        let config = create_test_config();
        let service = OpenAIService::new(&config);

        let response_message = OpenAIResponseMessage {
            role: "assistant".to_string(),
            content: None,
            tool_calls: vec![],
        };

        let result = service.format_openai_response(response_message);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_handle_api_error_rate_limit() {
        let config = create_test_config();
        let service = OpenAIService::new(&config);

        let error = service.handle_api_error(StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded");
        match error {
            AIError::RateLimit => {} // Expected
            _ => panic!("Expected RateLimit error"),
        }
    }

    #[test]
    fn test_handle_api_error_unauthorized() {
        let config = create_test_config();
        let service = OpenAIService::new(&config);

        let error = service.handle_api_error(StatusCode::UNAUTHORIZED, "Unauthorized");
        match error {
            AIError::Api { status, message } => {
                assert_eq!(status, 401);
                assert!(message.contains("Authentication failed"));
            }
            _ => panic!("Expected API error"),
        }
    }

    #[test]
    fn test_handle_api_error_bad_request_with_json() {
        let config = create_test_config();
        let service = OpenAIService::new(&config);

        let error_body =
            r#"{"error": {"message": "Invalid request", "type": "invalid_request_error"}}"#;
        let error = service.handle_api_error(StatusCode::BAD_REQUEST, error_body);

        match error {
            AIError::Api { status, message } => {
                assert_eq!(status, 400);
                assert_eq!(message, "Invalid request");
            }
            _ => panic!("Expected API error"),
        }
    }

    /// Integration tests that demonstrate the OpenAI service functionality
    /// These tests don't make actual API calls but verify the service setup
    mod integration_tests {
        use super::*;

        #[tokio::test]
        async fn test_openai_service_integration() {
            // Create a config with a mock API key
            let config = Config {
                openai_api_key: Some("test-api-key".to_string()),
                ..Default::default()
            };

            // Create the service
            let service = OpenAIService::new(&config);

            // Verify the service is available
            assert!(service.is_available());

            // Verify models are listed correctly
            let models = service.list_models();
            assert_eq!(models.len(), 4);
            assert_eq!(models[0].provider, "openai");
            assert_eq!(models[0].name, "gpt-4o");

            // Create test messages
            let messages = vec![Message::new(
                vec![MessageContentBlock::text("Hello, how are you?")],
                Role::User,
                "test-task-id".to_string(),
            )];

            // Test message formatting (this doesn't make API calls)
            let formatted = service
                .format_messages_for_openai("You are helpful", messages)
                .unwrap();
            assert_eq!(formatted.len(), 2); // system + user message
            assert_eq!(formatted[0].role, "system");
            assert_eq!(formatted[1].role, "user");

            // Test response formatting
            let openai_response = OpenAIResponseMessage {
                role: "assistant".to_string(),
                content: Some("I'm doing well, thank you!".to_string()),
                tool_calls: vec![],
            };
            let formatted_response = service.format_openai_response(openai_response);
            assert_eq!(formatted_response.len(), 1);
            match &formatted_response[0] {
                MessageContentBlock::Text { text } => {
                    assert_eq!(text, "I'm doing well, thank you!");
                }
                _ => panic!("Expected text content block"),
            }
        }

        #[tokio::test]
        async fn test_openai_service_without_api_key_fails() {
            let config = Config {
                openai_api_key: None,
                ..Default::default()
            };

            let service = OpenAIService::new(&config);
            assert!(!service.is_available());

            // Attempting to generate a response should fail
            let messages = vec![Message::new(
                vec![MessageContentBlock::text("Test")],
                Role::User,
                "test-task-id".to_string(),
            )];

            let result = service
                .generate_response("Test prompt", messages, None, false, None)
                .await;

            assert!(result.is_err());
            match result.unwrap_err() {
                AIError::Api { status, message } => {
                    assert_eq!(status, 401);
                    assert!(message.contains("API key not configured"));
                }
                _ => panic!("Expected API error"),
            }
        }

        #[tokio::test]
        async fn test_openai_service_invalid_model() {
            let config = Config {
                openai_api_key: Some("test-api-key".to_string()),
                ..Default::default()
            };

            let service = OpenAIService::new(&config);
            let messages = vec![Message::new(
                vec![MessageContentBlock::text("Test")],
                Role::User,
                "test-task-id".to_string(),
            )];

            let result = service
                .generate_response(
                    "Test prompt",
                    messages,
                    Some("invalid-model".to_string()),
                    false,
                    None,
                )
                .await;

            assert!(result.is_err());
            match result.unwrap_err() {
                AIError::InvalidModel(msg) => {
                    assert!(msg.contains("Invalid OpenAI model: invalid-model"));
                }
                _ => panic!("Expected InvalidModel error"),
            }
        }

        #[tokio::test]
        async fn test_message_format_consistency() {
            let config = Config {
                openai_api_key: Some("test-api-key".to_string()),
                ..Default::default()
            };

            let service = OpenAIService::new(&config);

            // Test various message content types
            let messages = vec![
                Message::new(
                    vec![MessageContentBlock::text("Simple text message")],
                    Role::User,
                    "test-task-id".to_string(),
                ),
                Message::new(
                    vec![
                        MessageContentBlock::text("Mixed content:"),
                        MessageContentBlock::image("image/jpeg", "jpeg_data"),
                    ],
                    Role::User,
                    "test-task-id".to_string(),
                ),
                Message::new(
                    vec![MessageContentBlock::tool_use(
                        "calculator",
                        "calc_1",
                        serde_json::json!({"operation": "add", "a": 1, "b": 2}),
                    )],
                    Role::Assistant,
                    "test-task-id".to_string(),
                ),
            ];

            let formatted = service
                .format_messages_for_openai("System prompt", messages)
                .unwrap();

            // Should have system + 3 messages
            assert_eq!(formatted.len(), 4);

            // Verify system message
            assert_eq!(formatted[0].role, "system");

            // Verify user messages
            assert_eq!(formatted[1].role, "user");
            assert_eq!(formatted[2].role, "user");
            assert_eq!(formatted[3].role, "assistant");

            // Verify mixed content message has both text and image
            assert_eq!(formatted[2].content.len(), 2);
            match &formatted[2].content[0] {
                OpenAIContent::Text { text } => assert_eq!(text, "Mixed content:"),
                _ => panic!("Expected text content"),
            }
            match &formatted[2].content[1] {
                OpenAIContent::ImageUrl { image_url } => {
                    assert_eq!(image_url.url, "data:image/jpeg;base64,jpeg_data");
                }
                _ => panic!("Expected image content"),
            }
        }
    }
}
