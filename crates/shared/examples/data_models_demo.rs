use chrono::Utc;
use serde_json::json;
use shared::{types::*, utils::validation::*};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("ByteBot Shared Rust Library - Data Models Demo");
    println!("==============================================");

    // Demo 1: Create and validate a Task
    println!("\n1. Creating and validating a Task:");
    let model_config = json!({
        "provider": "anthropic",
        "name": "claude-3-opus-20240229",
        "title": "Claude 3 Opus"
    });

    let mut task = Task::new(
        "Analyze the uploaded document and provide a summary".to_string(),
        model_config,
    );
    task.priority = TaskPriority::High;
    task.task_type = TaskType::Immediate;

    println!("Task created: {}", task.id);
    println!("Description: {}", task.description);
    println!("Status: {:?}", task.status);
    println!("Priority: {:?}", task.priority);

    // Validate task
    match validate_with_custom(&task) {
        Ok(_) => println!("✓ Task validation passed"),
        Err(e) => println!("✗ Task validation failed: {e}"),
    }

    // Demo 2: Create and validate Messages with content blocks
    println!("\n2. Creating Messages with content blocks:");

    let text_content = MessageContentBlock::text("Hello, I need help with this document.");
    let image_content = MessageContentBlock::image(
        "image/png",
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChAI9jU8j8wAAAABJRU5ErkJggg=="
    );

    let user_message = Message::new(
        vec![text_content, image_content],
        Role::User,
        task.id.clone(),
    );

    println!("User message created: {}", user_message.id);
    println!(
        "Content blocks: {}",
        user_message.get_content_blocks()?.len()
    );
    println!("Extracted text: {}", user_message.extract_text());

    // Validate message
    match validate_with_custom(&user_message) {
        Ok(_) => println!("✓ Message validation passed"),
        Err(e) => println!("✗ Message validation failed: {e}"),
    }

    // Demo 3: Create API DTOs
    println!("\n3. Creating and validating API DTOs:");

    let create_task_dto = CreateTaskDto {
        description: "Process customer feedback data".to_string(),
        task_type: Some(TaskType::Immediate),
        scheduled_for: None,
        priority: Some(TaskPriority::Medium),
        created_by: Some(Role::User),
        user_id: None,
        model: Some(json!({
            "provider": "openai",
            "name": "gpt-4",
            "title": "GPT-4"
        })),
        files: None,
    };

    println!("CreateTaskDto created");
    println!("Description: {}", create_task_dto.description);

    match validate_with_custom(&create_task_dto) {
        Ok(_) => println!("✓ CreateTaskDto validation passed"),
        Err(e) => println!("✗ CreateTaskDto validation failed: {e}"),
    }

    // Demo 4: Create User and Session
    println!("\n4. Creating User and Session:");

    let user = User::new("john.doe@example.com".to_string());
    println!("User created: {}", user.id);
    println!("Email: {}", user.email);
    println!("Display name: {}", user.display_name());

    let session = Session::new(
        user.id.clone(),
        "session_token_123".to_string(),
        Utc::now() + chrono::Duration::hours(24),
    );

    println!("Session created: {}", session.id);
    println!("Valid: {}", session.is_valid());
    println!("Remaining seconds: {}", session.remaining_seconds());

    // Demo 5: File handling
    println!("\n5. Creating and validating File:");

    let file_data = "SGVsbG8gV29ybGQ="; // "Hello World" in base64
    let file = File::new(
        "document.txt".to_string(),
        "text/plain".to_string(),
        11, // "Hello World" is 11 bytes
        file_data.to_string(),
        task.id.clone(),
    );

    println!("File created: {}", file.id);
    println!("Name: {}", file.name);
    println!("Type: {}", file.file_type);
    println!("Size: {}", file.human_readable_size());
    println!("Is image: {}", file.is_image());
    println!("Is document: {}", file.is_document());

    match validate_with_custom(&file) {
        Ok(_) => println!("✓ File validation passed"),
        Err(e) => println!("✗ File validation failed: {e}"),
    }

    // Demo 6: Validation utilities
    println!("\n6. Testing validation utilities:");

    // Email validation
    match validate_email("test@example.com") {
        Ok(_) => println!("✓ Email validation passed"),
        Err(e) => println!("✗ Email validation failed: {e}"),
    }

    // UUID validation
    match validate_uuid(&task.id) {
        Ok(_) => println!("✓ UUID validation passed"),
        Err(e) => println!("✗ UUID validation failed: {e}"),
    }

    // File path validation
    match validate_file_path("/safe/path/file.txt") {
        Ok(_) => println!("✓ File path validation passed"),
        Err(e) => println!("✗ File path validation failed: {e}"),
    }

    // Test dangerous path
    match validate_file_path("../../../etc/passwd") {
        Ok(_) => println!("✗ Dangerous path validation should have failed"),
        Err(e) => println!("✓ Dangerous path correctly rejected: {e}"),
    }

    // Demo 7: Task state transitions
    println!("\n7. Task state management:");

    println!(
        "Initial state - Active: {}, Terminal: {}",
        task.is_active(),
        task.is_terminal()
    );

    task.status = TaskStatus::Running;
    task.executed_at = Some(Utc::now());
    println!(
        "Running state - Active: {}, Terminal: {}",
        task.is_active(),
        task.is_terminal()
    );

    task.status = TaskStatus::Completed;
    task.completed_at = Some(Utc::now());
    println!(
        "Completed state - Active: {}, Terminal: {}",
        task.is_active(),
        task.is_terminal()
    );

    println!("\n✓ All demos completed successfully!");
    Ok(())
}
