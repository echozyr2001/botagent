use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{error, warn};

use crate::{config::Config, error::AIError};
use bytebot_shared_rs::types::{
    message::{Message, MessageContentBlock},
    task::Role,
};

use super::{AIService, ModelInfo};

/// Anthropic API constants
const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com/v1";
const DEFAULT_MODEL: &str = "claude-opus-4-20250514";
const MAX_TOKENS: u32 = 8192;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// Get available Anthropic models
fn get_anthropic_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            provider: "anthropic".to_string(),
            name: "claude-opus-4-20250514".to_string(),
            title: "Claude Opus 4".to_string(),
        },
        ModelInfo {
            provider: "anthropic".to_string(),
            name: "claude-sonnet-4-20250514".to_string(),
            title: "Claude Sonnet 4".to_string(),
        },
    ]
}

/// Anthropic API request structures
#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: Vec<SystemMessage>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<serde_json::Value>,
    thinking: ThinkingConfig,
}

#[derive(Debug, Serialize)]
struct SystemMessage {
    #[serde(rename = "type")]
    message_type: String,
    text: String,
    cache_control: CacheControl,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct ThinkingConfig {
    #[serde(rename = "type")]
    thinking_type: String,
}

#[derive(Debug, Serialize)]
struct CacheControl {
    #[serde(rename = "type")]
    cache_type: String,
}

/// Anthropic API response structures
#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    Thinking {
        thinking: String,
        signature: String,
    },
    RedactedThinking {
        data: String,
    },
}

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

/// Anthropic API error response
#[derive(Debug, Deserialize)]
struct AnthropicError {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicErrorResponse {
    error: AnthropicError,
}

/// Anthropic service implementation
pub struct AnthropicService {
    client: Client,
    api_key: Option<String>,
}

impl AnthropicService {
    /// Create a new Anthropic service instance
    pub fn new(config: &Config) -> Self {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key: config.anthropic_api_key.clone(),
        }
    }

    /// Convert internal messages to Anthropic format
    fn format_messages_for_anthropic(
        &self,
        messages: Vec<Message>,
    ) -> Result<Vec<AnthropicMessage>, AIError> {
        let mut anthropic_messages = Vec::new();

        for (index, message) in messages.iter().enumerate() {
            let content_blocks = message
                .get_content_blocks()
                .map_err(AIError::Serialization)?;

            // Skip user messages that contain tool use (as per TypeScript implementation)
            if message.role == Role::User
                && content_blocks
                    .iter()
                    .any(|block| matches!(block, MessageContentBlock::ToolUse { .. }))
            {
                continue;
            }

            let mut content: Vec<serde_json::Value> = Vec::new();

            for block in content_blocks {
                let anthropic_block = match block {
                    MessageContentBlock::Text { text } => {
                        serde_json::json!({
                            "type": "text",
                            "text": text
                        })
                    }
                    MessageContentBlock::Image { source } => {
                        serde_json::json!({
                            "type": "image",
                            "source": {
                                "type": source.source_type,
                                "media_type": source.media_type,
                                "data": source.data
                            }
                        })
                    }
                    MessageContentBlock::ToolUse { name, id, input } => {
                        serde_json::json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": input
                        })
                    }
                    MessageContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        let tool_content: Vec<serde_json::Value> = content.iter().map(|block| {
                            match block {
                                MessageContentBlock::Text { text } => {
                                    serde_json::json!({
                                        "type": "text",
                                        "text": text
                                    })
                                }
                                _ => serde_json::json!({
                                    "type": "text",
                                    "text": format!("Unsupported content block in tool result: {:?}", block)
                                })
                            }
                        }).collect();

                        let mut result = serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tool_use_id,
                            "content": tool_content
                        });

                        if let Some(is_error) = is_error {
                            result["is_error"] = serde_json::Value::Bool(is_error);
                        }

                        result
                    }
                    _ => {
                        // Handle other content types as text for now
                        serde_json::json!({
                            "type": "text",
                            "text": format!("Unsupported content block: {:?}", block)
                        })
                    }
                };

                content.push(anthropic_block);
            }

            // Add cache control to the last content block of the last message
            if index == messages.len() - 1 && !content.is_empty() {
                if let Some(last_content) = content.last_mut() {
                    last_content["cache_control"] = serde_json::json!({
                        "type": "ephemeral"
                    });
                }
            }

            anthropic_messages.push(AnthropicMessage {
                role: match message.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                },
                content,
            });
        }

        Ok(anthropic_messages)
    }

    /// Convert Anthropic response to internal format
    fn format_anthropic_response(
        &self,
        content: Vec<AnthropicContentBlock>,
    ) -> Vec<MessageContentBlock> {
        content
            .into_iter()
            .map(|block| match block {
                AnthropicContentBlock::Text { text } => MessageContentBlock::Text { text },
                AnthropicContentBlock::ToolUse { id, name, input } => {
                    MessageContentBlock::ToolUse { id, name, input }
                }
                AnthropicContentBlock::Thinking {
                    thinking,
                    signature,
                } => MessageContentBlock::Thinking {
                    thinking,
                    signature,
                },
                AnthropicContentBlock::RedactedThinking { data } => {
                    MessageContentBlock::RedactedThinking { data }
                }
            })
            .collect()
    }

    /// Handle API errors and convert to AIError
    fn handle_api_error(&self, status: StatusCode, body: &str) -> AIError {
        match status {
            StatusCode::TOO_MANY_REQUESTS => {
                warn!("Anthropic API rate limit exceeded");
                AIError::RateLimit
            }
            StatusCode::UNAUTHORIZED => {
                error!("Anthropic API authentication failed");
                AIError::Api {
                    status: status.as_u16(),
                    message: "Authentication failed".to_string(),
                }
            }
            StatusCode::BAD_REQUEST => {
                // Try to parse the error response
                if let Ok(error_response) = serde_json::from_str::<AnthropicErrorResponse>(body) {
                    error!(
                        "Anthropic API bad request: {}",
                        error_response.error.message
                    );
                    AIError::Api {
                        status: status.as_u16(),
                        message: error_response.error.message,
                    }
                } else {
                    error!("Anthropic API bad request: {}", body);
                    AIError::Api {
                        status: status.as_u16(),
                        message: "Bad request".to_string(),
                    }
                }
            }
            _ => {
                error!("Anthropic API error {}: {}", status, body);
                AIError::Api {
                    status: status.as_u16(),
                    message: format!("API error: {status}"),
                }
            }
        }
    }
}

#[async_trait]
impl AIService for AnthropicService {
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
            message: "Anthropic API key not configured".to_string(),
        })?;

        let model = model.unwrap_or_else(|| DEFAULT_MODEL.to_string());

        // Validate model
        let models = get_anthropic_models();
        if !models.iter().any(|m| m.name == model) {
            return Err(AIError::InvalidModel(format!(
                "Invalid Anthropic model: {model}"
            )));
        }

        let anthropic_messages = self.format_messages_for_anthropic(messages)?;

        let request = AnthropicRequest {
            model,
            max_tokens: MAX_TOKENS * 2, // Match TypeScript implementation
            system: vec![SystemMessage {
                message_type: "text".to_string(),
                text: system_prompt.to_string(),
                cache_control: CacheControl {
                    cache_type: "ephemeral".to_string(),
                },
            }],
            messages: anthropic_messages,
            tools: if use_tools {
                // TODO: Implement tools from agent.tools - for now return empty array
                // This will be implemented in a future task
                vec![]
            } else {
                vec![]
            },
            thinking: ThinkingConfig {
                thinking_type: "disabled".to_string(),
            },
        };

        let response = self
            .client
            .post(format!("{ANTHROPIC_API_BASE}/messages"))
            .header("Content-Type", "application/json")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await
            .map_err(AIError::Http)?;

        let status = response.status();
        let body = response.text().await.map_err(AIError::Http)?;

        if !status.is_success() {
            return Err(self.handle_api_error(status, &body));
        }

        let anthropic_response: AnthropicResponse =
            serde_json::from_str(&body).map_err(AIError::Serialization)?;

        Ok(self.format_anthropic_response(anthropic_response.content))
    }

    fn list_models(&self) -> Vec<ModelInfo> {
        get_anthropic_models()
    }

    fn is_available(&self) -> bool {
        self.api_key.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytebot_shared_rs::types::message::Message;

    fn create_test_config() -> Config {
        Config {
            anthropic_api_key: Some("test-key".to_string()),
            ..Default::default()
        }
    }

    fn create_test_message(content: Vec<MessageContentBlock>, role: Role) -> Message {
        Message::new(content, role, "test-task-id".to_string())
    }

    #[test]
    fn test_anthropic_service_creation() {
        let config = create_test_config();
        let service = AnthropicService::new(&config);
        assert!(service.is_available());
    }

    #[test]
    fn test_anthropic_service_without_api_key() {
        let config = Config {
            anthropic_api_key: None,
            ..Default::default()
        };
        let service = AnthropicService::new(&config);
        assert!(!service.is_available());
    }

    #[test]
    fn test_list_models() {
        let config = create_test_config();
        let service = AnthropicService::new(&config);
        let models = service.list_models();

        assert_eq!(models.len(), 2);
        assert_eq!(models[0].name, "claude-opus-4-20250514");
        assert_eq!(models[0].provider, "anthropic");
        assert_eq!(models[1].name, "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_format_messages_for_anthropic() {
        let config = create_test_config();
        let service = AnthropicService::new(&config);

        let messages = vec![
            create_test_message(vec![MessageContentBlock::text("Hello")], Role::User),
            create_test_message(
                vec![MessageContentBlock::text("Hi there!")],
                Role::Assistant,
            ),
        ];

        let result = service.format_messages_for_anthropic(messages).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].role, "user");
        assert_eq!(result[1].role, "assistant");
    }

    #[test]
    fn test_format_messages_skips_user_tool_use() {
        let config = create_test_config();
        let service = AnthropicService::new(&config);

        let messages = vec![
            create_test_message(
                vec![MessageContentBlock::tool_use(
                    "test_tool",
                    "123",
                    serde_json::json!({}),
                )],
                Role::User,
            ),
            create_test_message(vec![MessageContentBlock::text("Response")], Role::Assistant),
        ];

        let result = service.format_messages_for_anthropic(messages).unwrap();
        // Should skip the user message with tool use
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "assistant");
    }

    #[test]
    fn test_format_anthropic_response() {
        let config = create_test_config();
        let service = AnthropicService::new(&config);

        let anthropic_content = vec![
            AnthropicContentBlock::Text {
                text: "Hello world".to_string(),
            },
            AnthropicContentBlock::ToolUse {
                id: "123".to_string(),
                name: "test_tool".to_string(),
                input: serde_json::json!({"param": "value"}),
            },
        ];

        let result = service.format_anthropic_response(anthropic_content);
        assert_eq!(result.len(), 2);

        match &result[0] {
            MessageContentBlock::Text { text } => assert_eq!(text, "Hello world"),
            _ => panic!("Expected text block"),
        }

        match &result[1] {
            MessageContentBlock::ToolUse { id, name, .. } => {
                assert_eq!(id, "123");
                assert_eq!(name, "test_tool");
            }
            _ => panic!("Expected tool use block"),
        }
    }

    /// Integration tests that demonstrate the Anthropic service functionality
    /// These tests don't make actual API calls but verify the service setup
    mod integration_tests {
        use super::*;

        #[tokio::test]
        async fn test_anthropic_service_integration() {
            // Create a config with a mock API key
            let config = Config {
                anthropic_api_key: Some("test-api-key".to_string()),
                ..Default::default()
            };

            // Create the service
            let service = AnthropicService::new(&config);

            // Verify the service is available
            assert!(service.is_available());

            // Verify models are listed correctly
            let models = service.list_models();
            assert_eq!(models.len(), 2);
            assert_eq!(models[0].provider, "anthropic");
            assert_eq!(models[0].name, "claude-opus-4-20250514");

            // Create test messages
            let messages = vec![Message::new(
                vec![MessageContentBlock::text("Hello, how are you?")],
                Role::User,
                "test-task-id".to_string(),
            )];

            // Test message formatting (this doesn't make API calls)
            let formatted = service.format_messages_for_anthropic(messages).unwrap();
            assert_eq!(formatted.len(), 1);
            assert_eq!(formatted[0].role, "user");

            // Test response formatting
            let anthropic_response = vec![AnthropicContentBlock::Text {
                text: "I'm doing well, thank you!".to_string(),
            }];
            let formatted_response = service.format_anthropic_response(anthropic_response);
            assert_eq!(formatted_response.len(), 1);
            match &formatted_response[0] {
                MessageContentBlock::Text { text } => {
                    assert_eq!(text, "I'm doing well, thank you!");
                }
                _ => panic!("Expected text content block"),
            }
        }

        #[tokio::test]
        async fn test_anthropic_service_without_api_key_fails() {
            let config = Config {
                anthropic_api_key: None,
                ..Default::default()
            };

            let service = AnthropicService::new(&config);
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
        async fn test_anthropic_service_invalid_model() {
            let config = Config {
                anthropic_api_key: Some("test-api-key".to_string()),
                ..Default::default()
            };

            let service = AnthropicService::new(&config);
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
                    assert!(msg.contains("Invalid Anthropic model: invalid-model"));
                }
                _ => panic!("Expected InvalidModel error"),
            }
        }
    }
}
