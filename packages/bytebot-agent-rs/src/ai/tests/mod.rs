pub mod unified_ai_service_tests;
pub mod anthropic_service_tests;
pub mod openai_service_tests;
pub mod google_service_tests;
pub mod ai_service_property_tests;

use bytebot_shared_rs::types::{message::MessageContentBlock, task::Role};
use crate::config::Config;

/// Create a test config with all AI providers enabled
pub fn create_test_config_all_providers() -> Config {
    Config {
        anthropic_api_key: Some("test-anthropic-key".to_string()),
        openai_api_key: Some("test-openai-key".to_string()),
        google_api_key: Some("test-google-key".to_string()),
        ..Default::default()
    }
}

/// Create a test config with no AI providers
pub fn create_test_config_no_providers() -> Config {
    Config {
        anthropic_api_key: None,
        openai_api_key: None,
        google_api_key: None,
        ..Default::default()
    }
}

/// Create a test config with only Anthropic enabled
pub fn create_test_config_anthropic_only() -> Config {
    Config {
        anthropic_api_key: Some("test-anthropic-key".to_string()),
        openai_api_key: None,
        google_api_key: None,
        ..Default::default()
    }
}

/// Create a test config with only OpenAI enabled
pub fn create_test_config_openai_only() -> Config {
    Config {
        anthropic_api_key: None,
        openai_api_key: Some("test-openai-key".to_string()),
        google_api_key: None,
        ..Default::default()
    }
}

/// Create a test config with only Google enabled
pub fn create_test_config_google_only() -> Config {
    Config {
        anthropic_api_key: None,
        openai_api_key: None,
        google_api_key: Some("test-google-key".to_string()),
        ..Default::default()
    }
}

/// Helper function to create test message content blocks
pub fn create_test_text_content(text: &str) -> Vec<MessageContentBlock> {
    vec![MessageContentBlock::text(text)]
}

/// Helper function to create test image content blocks
pub fn create_test_image_content(media_type: &str, data: &str) -> Vec<MessageContentBlock> {
    vec![MessageContentBlock::image(media_type, data)]
}

/// Helper function to create test tool use content blocks
pub fn create_test_tool_use_content(
    name: &str,
    id: &str,
    input: serde_json::Value,
) -> Vec<MessageContentBlock> {
    vec![MessageContentBlock::tool_use(name, id, input)]
}

/// Helper function to create test tool result content blocks
pub fn create_test_tool_result_content(
    tool_use_id: &str,
    result_text: &str,
    is_error: bool,
) -> Vec<MessageContentBlock> {
    vec![MessageContentBlock::ToolResult {
        tool_use_id: tool_use_id.to_string(),
        content: vec![MessageContentBlock::text(result_text)],
        is_error: Some(is_error),
    }]
}

/// Helper function to create mixed content blocks for testing
pub fn create_test_mixed_content() -> Vec<MessageContentBlock> {
    vec![
        MessageContentBlock::text("Here's an image:"),
        MessageContentBlock::image("image/png", "base64data"),
        MessageContentBlock::text("And a tool call:"),
        MessageContentBlock::tool_use("calculator", "calc_1", serde_json::json!({"op": "add", "a": 1, "b": 2})),
    ]
}