use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{error, warn};

use crate::{config::Config, error::AIError};
use bytebot_shared_rs::types::{
    message::{DocumentSource, Message, MessageContentBlock},
    task::Role,
};

use super::{AIService, ModelInfo};

/// Google Gemini API constants
const GOOGLE_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";
const DEFAULT_MODEL: &str = "gemini-1.5-pro";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// Get available Google Gemini models
fn get_google_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            provider: "google".to_string(),
            name: "gemini-1.5-pro".to_string(),
            title: "Gemini 1.5 Pro".to_string(),
        },
        ModelInfo {
            provider: "google".to_string(),
            name: "gemini-1.5-flash".to_string(),
            title: "Gemini 1.5 Flash".to_string(),
        },
        ModelInfo {
            provider: "google".to_string(),
            name: "gemini-2.0-flash-exp".to_string(),
            title: "Gemini 2.0 Flash (Experimental)".to_string(),
        },
    ]
}

/// Google Gemini API request structures
#[derive(Debug, Serialize)]
struct GoogleRequest {
    contents: Vec<GoogleContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GoogleSystemInstruction>,
    generation_config: GenerationConfig,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct GoogleSystemInstruction {
    parts: Vec<GooglePart>,
}

#[derive(Debug, Serialize)]
struct GoogleContent {
    role: String,
    parts: Vec<GooglePart>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum GooglePart {
    Text { text: String },
    InlineData { inline_data: InlineData },
    FunctionCall { function_call: FunctionCall },
    FunctionResponse { function_response: FunctionResponse },
}

#[derive(Debug, Serialize)]
struct InlineData {
    mime_type: String,
    data: String,
}

#[derive(Debug, Serialize)]
struct FunctionCall {
    name: String,
    args: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct FunctionResponse {
    name: String,
    response: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct GenerationConfig {
    temperature: f32,
    max_output_tokens: u32,
}

/// Google Gemini API response structures
#[derive(Debug, Deserialize)]
struct GoogleResponse {
    candidates: Vec<Candidate>,
    #[serde(default)]
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: GoogleResponseContent,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleResponseContent {
    parts: Vec<GoogleResponsePart>,
    role: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum GoogleResponsePart {
    Text { text: String },
    FunctionCall { function_call: ResponseFunctionCall },
}

#[derive(Debug, Deserialize)]
struct ResponseFunctionCall {
    name: String,
    args: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct UsageMetadata {
    prompt_token_count: u32,
    candidates_token_count: u32,
    total_token_count: u32,
}

/// Google API error response
#[derive(Debug, Deserialize)]
struct GoogleError {
    code: u32,
    message: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct GoogleErrorResponse {
    error: GoogleError,
}

/// Google Gemini service implementation
pub struct GoogleService {
    client: Client,
    api_key: Option<String>,
}

impl GoogleService {
    /// Create a new Google service instance
    pub fn new(config: &Config) -> Self {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key: config.google_api_key.clone(),
        }
    }

    /// Convert internal messages to Google Gemini format
    fn format_messages_for_google(
        &self,
        system_prompt: &str,
        messages: Vec<Message>,
    ) -> Result<(Option<GoogleSystemInstruction>, Vec<GoogleContent>), AIError> {
        let mut google_contents = Vec::new();

        // Create system instruction if provided
        let system_instruction = if !system_prompt.is_empty() {
            Some(GoogleSystemInstruction {
                parts: vec![GooglePart::Text {
                    text: system_prompt.to_string(),
                }],
            })
        } else {
            None
        };

        for message in messages {
            let content_blocks = message
                .get_content_blocks()
                .map_err(AIError::Serialization)?;

            let mut parts: Vec<GooglePart> = Vec::new();

            for block in content_blocks {
                match block {
                    MessageContentBlock::Text { text } => {
                        parts.push(GooglePart::Text { text });
                    }
                    MessageContentBlock::Image { source } => {
                        parts.push(GooglePart::InlineData {
                            inline_data: InlineData {
                                mime_type: source.media_type,
                                data: source.data,
                            },
                        });
                    }
                    MessageContentBlock::ToolUse { name, input, .. } => {
                        parts.push(GooglePart::FunctionCall {
                            function_call: FunctionCall { name, args: input },
                        });
                    }
                    MessageContentBlock::ToolResult {
                        tool_use_id: _,
                        content,
                        is_error,
                    } => {
                        // Convert tool result content to response format
                        let result_text = content
                            .iter()
                            .filter_map(|block| match block {
                                MessageContentBlock::Text { text } => Some(text.clone()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join(" ");

                        let response_value = if is_error.unwrap_or(false) {
                            serde_json::json!({
                                "error": result_text
                            })
                        } else {
                            serde_json::json!({
                                "result": result_text
                            })
                        };

                        // For tool results, we need to infer the function name
                        // This is a limitation of the current message format
                        parts.push(GooglePart::FunctionResponse {
                            function_response: FunctionResponse {
                                name: "unknown_function".to_string(),
                                response: response_value,
                            },
                        });
                    }
                    MessageContentBlock::Document { source, name, .. } => {
                        // Convert document to text representation for now
                        let doc_text = format!(
                            "Document: {} (type: {})",
                            name.unwrap_or_else(|| "unnamed".to_string()),
                            source.media_type
                        );
                        parts.push(GooglePart::Text { text: doc_text });
                    }
                    MessageContentBlock::Thinking { thinking, .. } => {
                        // Include thinking content as text
                        parts.push(GooglePart::Text { text: thinking });
                    }
                    MessageContentBlock::RedactedThinking { .. } => {
                        // Skip redacted thinking
                        continue;
                    }
                }
            }

            // Only add content if it has parts
            if !parts.is_empty() {
                google_contents.push(GoogleContent {
                    role: match message.role {
                        Role::User => "user".to_string(),
                        Role::Assistant => "model".to_string(), // Google uses "model" for assistant
                    },
                    parts,
                });
            }
        }

        Ok((system_instruction, google_contents))
    }
    /// Convert Google response to internal format
    fn format_google_response(&self, candidates: Vec<Candidate>) -> Vec<MessageContentBlock> {
        if candidates.is_empty() {
            return vec![];
        }

        let candidate = &candidates[0]; // Use first candidate
        let mut content_blocks = Vec::new();

        for part in &candidate.content.parts {
            match part {
                GoogleResponsePart::Text { text } => {
                    if !text.is_empty() {
                        content_blocks.push(MessageContentBlock::Text { text: text.clone() });
                    }
                }
                GoogleResponsePart::FunctionCall { function_call } => {
                    // Generate a unique ID for the function call
                    let id = format!("call_{}", &uuid::Uuid::new_v4().to_string()[..8]);
                    content_blocks.push(MessageContentBlock::ToolUse {
                        id,
                        name: function_call.name.clone(),
                        input: function_call.args.clone(),
                    });
                }
            }
        }

        content_blocks
    }

    /// Handle API errors and convert to AIError
    fn handle_api_error(&self, status: StatusCode, body: &str) -> AIError {
        match status {
            StatusCode::TOO_MANY_REQUESTS => {
                warn!("Google Gemini API rate limit exceeded");
                AIError::RateLimit
            }
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                error!("Google Gemini API authentication failed");
                AIError::Api {
                    status: status.as_u16(),
                    message: "Authentication failed - check API key".to_string(),
                }
            }
            StatusCode::BAD_REQUEST => {
                // Try to parse the error response
                if let Ok(error_response) = serde_json::from_str::<GoogleErrorResponse>(body) {
                    error!(
                        "Google Gemini API bad request: {}",
                        error_response.error.message
                    );
                    AIError::Api {
                        status: status.as_u16(),
                        message: error_response.error.message,
                    }
                } else {
                    error!("Google Gemini API bad request: {}", body);
                    AIError::Api {
                        status: status.as_u16(),
                        message: "Bad request".to_string(),
                    }
                }
            }
            _ => {
                error!("Google Gemini API error {}: {}", status, body);
                AIError::Api {
                    status: status.as_u16(),
                    message: format!("API error: {status}"),
                }
            }
        }
    }
}

#[async_trait]
impl AIService for GoogleService {
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
            message: "Google API key not configured".to_string(),
        })?;

        let model = model.unwrap_or_else(|| DEFAULT_MODEL.to_string());

        // Validate model
        let models = get_google_models();
        if !models.iter().any(|m| m.name == model) {
            return Err(AIError::InvalidModel(format!(
                "Invalid Google model: {model}"
            )));
        }

        let (system_instruction, contents) =
            self.format_messages_for_google(system_prompt, messages)?;

        let request = GoogleRequest {
            contents,
            system_instruction,
            generation_config: GenerationConfig {
                temperature: 0.7,
                max_output_tokens: 8192,
            },
            tools: if use_tools {
                // TODO: Implement tools from agent.tools - for now return empty array
                // This will be implemented in a future task
                vec![]
            } else {
                vec![]
            },
        };

        let url = format!("{GOOGLE_API_BASE}/models/{model}:generateContent");
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .query(&[("key", api_key)])
            .json(&request)
            .send()
            .await
            .map_err(AIError::Http)?;

        let status = response.status();
        let body = response.text().await.map_err(AIError::Http)?;

        if !status.is_success() {
            return Err(self.handle_api_error(status, &body));
        }

        let google_response: GoogleResponse =
            serde_json::from_str(&body).map_err(AIError::Serialization)?;

        Ok(self.format_google_response(google_response.candidates))
    }

    fn list_models(&self) -> Vec<ModelInfo> {
        get_google_models()
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
            google_api_key: Some("test-key".to_string()),
            ..Default::default()
        }
    }

    fn create_test_message(content: Vec<MessageContentBlock>, role: Role) -> Message {
        Message::new(content, role, "test-task-id".to_string())
    }

    #[test]
    fn test_google_service_creation() {
        let config = create_test_config();
        let service = GoogleService::new(&config);
        assert!(service.is_available());
    }

    #[test]
    fn test_google_service_without_api_key() {
        let config = Config {
            google_api_key: None,
            ..Default::default()
        };
        let service = GoogleService::new(&config);
        assert!(!service.is_available());
    }

    #[test]
    fn test_list_models() {
        let config = create_test_config();
        let service = GoogleService::new(&config);
        let models = service.list_models();

        assert_eq!(models.len(), 3);
        assert_eq!(models[0].name, "gemini-1.5-pro");
        assert_eq!(models[0].provider, "google");
        assert_eq!(models[1].name, "gemini-1.5-flash");
        assert_eq!(models[2].name, "gemini-2.0-flash-exp");
    }

    #[test]
    fn test_format_messages_for_google() {
        let config = create_test_config();
        let service = GoogleService::new(&config);

        let messages = vec![
            create_test_message(vec![MessageContentBlock::text("Hello")], Role::User),
            create_test_message(
                vec![MessageContentBlock::text("Hi there!")],
                Role::Assistant,
            ),
        ];

        let result = service
            .format_messages_for_google("You are a helpful assistant", messages)
            .unwrap();

        let (system_instruction, contents) = result;

        // Check system instruction
        assert!(system_instruction.is_some());
        let system = system_instruction.unwrap();
        assert_eq!(system.parts.len(), 1);
        match &system.parts[0] {
            GooglePart::Text { text } => {
                assert_eq!(text, "You are a helpful assistant");
            }
            _ => panic!("Expected text part"),
        }

        // Check contents
        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0].role, "user");
        assert_eq!(contents[1].role, "model");
    }

    #[test]
    fn test_format_messages_with_image() {
        let config = create_test_config();
        let service = GoogleService::new(&config);

        let messages = vec![create_test_message(
            vec![MessageContentBlock::image("image/png", "base64data")],
            Role::User,
        )];

        let result = service.format_messages_for_google("", messages).unwrap();
        let (_, contents) = result;

        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].parts.len(), 1);
        match &contents[0].parts[0] {
            GooglePart::InlineData { inline_data } => {
                assert_eq!(inline_data.mime_type, "image/png");
                assert_eq!(inline_data.data, "base64data");
            }
            _ => panic!("Expected inline data part"),
        }
    }

    #[test]
    fn test_format_messages_with_tool_use() {
        let config = create_test_config();
        let service = GoogleService::new(&config);

        let messages = vec![create_test_message(
            vec![MessageContentBlock::tool_use(
                "test_tool",
                "123",
                serde_json::json!({"param": "value"}),
            )],
            Role::Assistant,
        )];

        let result = service.format_messages_for_google("", messages).unwrap();
        let (_, contents) = result;

        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].parts.len(), 1);
        match &contents[0].parts[0] {
            GooglePart::FunctionCall { function_call } => {
                assert_eq!(function_call.name, "test_tool");
                assert_eq!(function_call.args["param"], "value");
            }
            _ => panic!("Expected function call part"),
        }
    }

    #[test]
    fn test_format_messages_with_tool_result() {
        let config = create_test_config();
        let service = GoogleService::new(&config);

        let messages = vec![create_test_message(
            vec![MessageContentBlock::ToolResult {
                tool_use_id: "123".to_string(),
                content: vec![MessageContentBlock::text("Tool result")],
                is_error: Some(false),
            }],
            Role::User,
        )];

        let result = service.format_messages_for_google("", messages).unwrap();
        let (_, contents) = result;

        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].parts.len(), 1);
        match &contents[0].parts[0] {
            GooglePart::FunctionResponse { function_response } => {
                assert_eq!(function_response.name, "unknown_function");
                assert_eq!(function_response.response["result"], "Tool result");
            }
            _ => panic!("Expected function response part"),
        }
    }

    #[test]
    fn test_format_messages_with_tool_error() {
        let config = create_test_config();
        let service = GoogleService::new(&config);

        let messages = vec![create_test_message(
            vec![MessageContentBlock::ToolResult {
                tool_use_id: "123".to_string(),
                content: vec![MessageContentBlock::text("Error occurred")],
                is_error: Some(true),
            }],
            Role::User,
        )];

        let result = service.format_messages_for_google("", messages).unwrap();
        let (_, contents) = result;

        assert_eq!(contents.len(), 1);
        match &contents[0].parts[0] {
            GooglePart::FunctionResponse { function_response } => {
                assert_eq!(function_response.response["error"], "Error occurred");
            }
            _ => panic!("Expected function response part"),
        }
    }

    #[test]
    fn test_format_messages_with_document() {
        let config = create_test_config();
        let service = GoogleService::new(&config);

        let messages = vec![create_test_message(
            vec![MessageContentBlock::Document {
                source: DocumentSource {
                    media_type: "text/plain".to_string(),
                    source_type: "base64".to_string(),
                    data: "document_data".to_string(),
                },
                name: Some("test.txt".to_string()),
                size: None,
            }],
            Role::User,
        )];

        let result = service.format_messages_for_google("", messages).unwrap();
        let (_, contents) = result;

        assert_eq!(contents.len(), 1);
        match &contents[0].parts[0] {
            GooglePart::Text { text } => {
                assert!(text.contains("Document: test.txt"));
                assert!(text.contains("type: text/plain"));
            }
            _ => panic!("Expected text part"),
        }
    }

    #[test]
    fn test_format_messages_with_thinking() {
        let config = create_test_config();
        let service = GoogleService::new(&config);

        let messages = vec![create_test_message(
            vec![MessageContentBlock::Thinking {
                thinking: "Let me think about this".to_string(),
                signature: "signature".to_string(),
            }],
            Role::Assistant,
        )];

        let result = service.format_messages_for_google("", messages).unwrap();
        let (_, contents) = result;

        assert_eq!(contents.len(), 1);
        match &contents[0].parts[0] {
            GooglePart::Text { text } => {
                assert_eq!(text, "Let me think about this");
            }
            _ => panic!("Expected text part"),
        }
    }

    #[test]
    fn test_format_messages_skips_redacted_thinking() {
        let config = create_test_config();
        let service = GoogleService::new(&config);

        let messages = vec![create_test_message(
            vec![
                MessageContentBlock::text("Hello"),
                MessageContentBlock::RedactedThinking {
                    data: "redacted_data".to_string(),
                },
                MessageContentBlock::text("World"),
            ],
            Role::User,
        )];

        let result = service.format_messages_for_google("", messages).unwrap();
        let (_, contents) = result;

        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].parts.len(), 2); // Should skip redacted thinking
        match &contents[0].parts[0] {
            GooglePart::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected text part"),
        }
        match &contents[0].parts[1] {
            GooglePart::Text { text } => assert_eq!(text, "World"),
            _ => panic!("Expected text part"),
        }
    }

    #[test]
    fn test_format_google_response() {
        let config = create_test_config();
        let service = GoogleService::new(&config);

        let candidates = vec![Candidate {
            content: GoogleResponseContent {
                parts: vec![
                    GoogleResponsePart::Text {
                        text: "Hello world".to_string(),
                    },
                    GoogleResponsePart::FunctionCall {
                        function_call: ResponseFunctionCall {
                            name: "test_tool".to_string(),
                            args: serde_json::json!({"param": "value"}),
                        },
                    },
                ],
                role: "model".to_string(),
            },
            finish_reason: Some("stop".to_string()),
        }];

        let result = service.format_google_response(candidates);
        assert_eq!(result.len(), 2);

        match &result[0] {
            MessageContentBlock::Text { text } => assert_eq!(text, "Hello world"),
            _ => panic!("Expected text block"),
        }

        match &result[1] {
            MessageContentBlock::ToolUse { name, input, .. } => {
                assert_eq!(name, "test_tool");
                assert_eq!(input["param"], "value");
            }
            _ => panic!("Expected tool use block"),
        }
    }

    #[test]
    fn test_format_google_response_empty_candidates() {
        let config = create_test_config();
        let service = GoogleService::new(&config);

        let result = service.format_google_response(vec![]);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_format_google_response_empty_text() {
        let config = create_test_config();
        let service = GoogleService::new(&config);

        let candidates = vec![Candidate {
            content: GoogleResponseContent {
                parts: vec![GoogleResponsePart::Text {
                    text: "".to_string(),
                }],
                role: "model".to_string(),
            },
            finish_reason: Some("stop".to_string()),
        }];

        let result = service.format_google_response(candidates);
        assert_eq!(result.len(), 0); // Empty text should be filtered out
    }

    #[test]
    fn test_handle_api_error_rate_limit() {
        let config = create_test_config();
        let service = GoogleService::new(&config);

        let error = service.handle_api_error(StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded");
        match error {
            AIError::RateLimit => {} // Expected
            _ => panic!("Expected RateLimit error"),
        }
    }

    #[test]
    fn test_handle_api_error_unauthorized() {
        let config = create_test_config();
        let service = GoogleService::new(&config);

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
    fn test_handle_api_error_forbidden() {
        let config = create_test_config();
        let service = GoogleService::new(&config);

        let error = service.handle_api_error(StatusCode::FORBIDDEN, "Forbidden");
        match error {
            AIError::Api { status, message } => {
                assert_eq!(status, 403);
                assert!(message.contains("Authentication failed"));
            }
            _ => panic!("Expected API error"),
        }
    }

    #[test]
    fn test_handle_api_error_bad_request_with_json() {
        let config = create_test_config();
        let service = GoogleService::new(&config);

        let error_body = r#"{"error": {"code": 400, "message": "Invalid request", "status": "INVALID_ARGUMENT"}}"#;
        let error = service.handle_api_error(StatusCode::BAD_REQUEST, error_body);

        match error {
            AIError::Api { status, message } => {
                assert_eq!(status, 400);
                assert_eq!(message, "Invalid request");
            }
            _ => panic!("Expected API error"),
        }
    }

    #[test]
    fn test_handle_api_error_bad_request_without_json() {
        let config = create_test_config();
        let service = GoogleService::new(&config);

        let error = service.handle_api_error(StatusCode::BAD_REQUEST, "Invalid request");
        match error {
            AIError::Api { status, message } => {
                assert_eq!(status, 400);
                assert_eq!(message, "Bad request");
            }
            _ => panic!("Expected API error"),
        }
    }

    /// Integration tests that demonstrate the Google service functionality
    /// These tests don't make actual API calls but verify the service setup
    mod integration_tests {
        use super::*;

        #[tokio::test]
        async fn test_google_service_integration() {
            // Create a config with a mock API key
            let config = Config {
                google_api_key: Some("test-api-key".to_string()),
                ..Default::default()
            };

            // Create the service
            let service = GoogleService::new(&config);

            // Verify the service is available
            assert!(service.is_available());

            // Verify models are listed correctly
            let models = service.list_models();
            assert_eq!(models.len(), 3);
            assert_eq!(models[0].provider, "google");
            assert_eq!(models[0].name, "gemini-1.5-pro");

            // Create test messages
            let messages = vec![Message::new(
                vec![MessageContentBlock::text("Hello, how are you?")],
                Role::User,
                "test-task-id".to_string(),
            )];

            // Test message formatting (this doesn't make API calls)
            let (system_instruction, contents) = service
                .format_messages_for_google("You are helpful", messages)
                .unwrap();

            assert!(system_instruction.is_some());
            assert_eq!(contents.len(), 1);
            assert_eq!(contents[0].role, "user");

            // Test response formatting
            let google_response = vec![Candidate {
                content: GoogleResponseContent {
                    parts: vec![GoogleResponsePart::Text {
                        text: "I'm doing well, thank you!".to_string(),
                    }],
                    role: "model".to_string(),
                },
                finish_reason: Some("stop".to_string()),
            }];
            let formatted_response = service.format_google_response(google_response);
            assert_eq!(formatted_response.len(), 1);
            match &formatted_response[0] {
                MessageContentBlock::Text { text } => {
                    assert_eq!(text, "I'm doing well, thank you!");
                }
                _ => panic!("Expected text content block"),
            }
        }

        #[tokio::test]
        async fn test_google_service_without_api_key_fails() {
            let config = Config {
                google_api_key: None,
                ..Default::default()
            };

            let service = GoogleService::new(&config);
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
        async fn test_google_service_invalid_model() {
            let config = Config {
                google_api_key: Some("test-api-key".to_string()),
                ..Default::default()
            };

            let service = GoogleService::new(&config);
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
                    assert!(msg.contains("Invalid Google model: invalid-model"));
                }
                _ => panic!("Expected InvalidModel error"),
            }
        }

        #[tokio::test]
        async fn test_message_format_consistency() {
            let config = Config {
                google_api_key: Some("test-api-key".to_string()),
                ..Default::default()
            };

            let service = GoogleService::new(&config);

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

            let (system_instruction, contents) = service
                .format_messages_for_google("System prompt", messages)
                .unwrap();

            // Should have system instruction + 3 messages
            assert!(system_instruction.is_some());
            assert_eq!(contents.len(), 3);

            // Verify user messages
            assert_eq!(contents[0].role, "user");
            assert_eq!(contents[1].role, "user");
            assert_eq!(contents[2].role, "model"); // Google uses "model" for assistant

            // Verify mixed content message has both text and image
            assert_eq!(contents[1].parts.len(), 2);
            match &contents[1].parts[0] {
                GooglePart::Text { text } => assert_eq!(text, "Mixed content:"),
                _ => panic!("Expected text content"),
            }
            match &contents[1].parts[1] {
                GooglePart::InlineData { inline_data } => {
                    assert_eq!(inline_data.mime_type, "image/jpeg");
                    assert_eq!(inline_data.data, "jpeg_data");
                }
                _ => panic!("Expected inline data content"),
            }

            // Verify tool use message
            assert_eq!(contents[2].parts.len(), 1);
            match &contents[2].parts[0] {
                GooglePart::FunctionCall { function_call } => {
                    assert_eq!(function_call.name, "calculator");
                    assert_eq!(function_call.args["operation"], "add");
                    assert_eq!(function_call.args["a"], 1);
                    assert_eq!(function_call.args["b"], 2);
                }
                _ => panic!("Expected function call content"),
            }
        }

        #[tokio::test]
        async fn test_system_instruction_handling() {
            let config = Config {
                google_api_key: Some("test-api-key".to_string()),
                ..Default::default()
            };

            let service = GoogleService::new(&config);

            // Test with system prompt
            let messages = vec![Message::new(
                vec![MessageContentBlock::text("Hello")],
                Role::User,
                "test-task-id".to_string(),
            )];

            let (system_instruction, _) = service
                .format_messages_for_google("You are a helpful assistant", messages.clone())
                .unwrap();

            assert!(system_instruction.is_some());
            let system = system_instruction.unwrap();
            assert_eq!(system.parts.len(), 1);
            match &system.parts[0] {
                GooglePart::Text { text } => {
                    assert_eq!(text, "You are a helpful assistant");
                }
                _ => panic!("Expected text part"),
            }

            // Test without system prompt
            let (system_instruction, _) = service.format_messages_for_google("", messages).unwrap();

            assert!(system_instruction.is_none());
        }
    }
}
