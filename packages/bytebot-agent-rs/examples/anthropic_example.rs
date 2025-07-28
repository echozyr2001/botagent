use bytebot_agent_rs::{ai::anthropic::AnthropicService, ai::AIService, config::Config};
use bytebot_shared_rs::types::{message::MessageContentBlock, task::Role};

/// Example demonstrating how to use the Anthropic service
/// This example shows the service setup but doesn't make actual API calls
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("ByteBot Anthropic Service Example");
    println!("=================================");

    // Load configuration from environment
    let config = Config::from_env().unwrap_or_else(|_| {
        println!("Warning: Using default config. Set ANTHROPIC_API_KEY for actual API calls.");
        Config::default()
    });

    // Create the Anthropic service
    let anthropic_service = AnthropicService::new(&config);

    // Check if the service is available
    if anthropic_service.is_available() {
        println!("✓ Anthropic service is available (API key configured)");
    } else {
        println!("⚠ Anthropic service is not available (no API key configured)");
        println!("  Set ANTHROPIC_API_KEY environment variable to enable API calls");
    }

    // List available models
    println!("\nAvailable Anthropic models:");
    let models = anthropic_service.list_models();
    for model in &models {
        println!("  - {} ({})", model.title, model.name);
    }

    // Create example messages
    let messages = vec![bytebot_shared_rs::types::message::Message::new(
        vec![MessageContentBlock::text(
            "Hello! Can you help me with a task?",
        )],
        Role::User,
        "example-task-id".to_string(),
    )];

    println!("\nExample message content:");
    for message in &messages {
        println!("  Role: {:?}", message.role);
        if let Ok(blocks) = message.get_content_blocks() {
            for block in blocks {
                if let Some(text) = block.as_text() {
                    println!("  Text: {text}");
                }
            }
        }
    }

    // Note: The format_messages_for_anthropic method is private, so we can't demonstrate it here
    // This is by design as it's an internal implementation detail

    // Note about actual API usage
    println!("\nNote:");
    println!("This example demonstrates the service setup and message formatting.");
    println!("To make actual API calls, ensure ANTHROPIC_API_KEY is set and call:");
    println!(
        "  service.generate_response(system_prompt, messages, model, use_tools, signal).await"
    );

    Ok(())
}
