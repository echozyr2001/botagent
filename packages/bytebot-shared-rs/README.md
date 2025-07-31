# ByteBot Shared Rust Library

This library contains shared types, utilities, and logging functionality used across ByteBot Rust services.

## Structured Logging

The library provides a comprehensive structured logging system that is compatible with the existing TypeScript logging patterns while leveraging Rust's performance and safety advantages.

### Features

- **Structured JSON logging** for production environments
- **Pretty console logging** for development
- **Environment-based configuration**
- **Service-specific logging contexts**
- **Runtime log level control**
- **Compatibility with existing log aggregation systems**

### Configuration

Configure logging through environment variables:

```bash
# Log level (trace, debug, info, warn, error)
LOG_LEVEL=info

# Log format (json or pretty)
LOG_FORMAT=json

# Service name (automatically set by services)
SERVICE_NAME=bytebot-agent-rs

# Include file and line information
LOG_INCLUDE_LOCATION=false

# Include thread information
LOG_INCLUDE_THREAD=false

# Custom fields as JSON
LOG_CUSTOM_FIELDS='{"environment":"production","version":"1.0.0"}'
```

### Usage

#### Basic Setup

```rust
use bytebot_shared_rs::logging::{init_logging, LoggingConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging for your service
    let config = LoggingConfig::for_service("my-service");
    init_logging(config)?;
    
    // Now you can use structured logging
    tracing::info!(
        service = "my-service",
        version = "1.0.0",
        "Service started successfully"
    );
    
    Ok(())
}
```

#### Structured Logging Modules

The library provides specialized logging modules for different operations:

##### Task Operations

```rust
use bytebot_shared_rs::logging::task_logging;

// Log task lifecycle events
task_logging::task_created("task-123", "Process user request");
task_logging::task_started("task-123");
task_logging::task_completed("task-123", 1500); // duration in ms
task_logging::task_failed("task-123", "Network timeout");
task_logging::task_cancelled("task-123");
task_logging::task_takeover("task-123", "assistant", "user");
```

##### Computer Automation

```rust
use bytebot_shared_rs::logging::automation_logging;

// Log automation actions
automation_logging::screenshot_taken();
automation_logging::mouse_moved(100, 200);
automation_logging::mouse_clicked(100, 200, "left", 1);
automation_logging::text_typed("Hello, world!");
automation_logging::keys_pressed(&["ctrl".to_string(), "c".to_string()]);
automation_logging::file_read("/path/to/file.txt", true);
automation_logging::file_written("/path/to/output.txt", true);
automation_logging::application_switched("firefox");
automation_logging::automation_error("screenshot", "Display not available");
```

##### AI Service Operations

```rust
use bytebot_shared_rs::logging::ai_logging;

// Log AI service interactions
ai_logging::ai_request_started("anthropic", "claude-3-sonnet", 5);
ai_logging::ai_request_completed("anthropic", "claude-3-sonnet", 2500, Some(150));
ai_logging::ai_request_failed("openai", "gpt-4", "Rate limit exceeded");
ai_logging::ai_rate_limited("google", Some(60));
ai_logging::model_list_updated("anthropic", 3);
```

##### WebSocket Operations

```rust
use bytebot_shared_rs::logging::websocket_logging;

// Log WebSocket events
websocket_logging::client_connected("client-123");
websocket_logging::client_disconnected("client-123");
websocket_logging::client_joined_task("client-123", "task-456");
websocket_logging::client_left_task("client-123", "task-456");
websocket_logging::event_emitted("task_updated", Some("task_456"), 3);
```

##### Database Operations

```rust
use bytebot_shared_rs::logging::database_logging;

// Log database operations
database_logging::connection_established("postgresql://...", 10);
database_logging::migration_started("20240101_initial");
database_logging::migration_completed("20240101_initial", 500);
database_logging::query_executed("SELECT", 25, Some(5));
database_logging::query_failed("INSERT", "Unique constraint violation");
database_logging::connection_pool_stats(10, 3, 7);
```

### Log Output Examples

#### Development (Pretty Format)

```
2024-01-31T10:30:45.123456Z  INFO bytebot_agent_rs: Service started successfully
    at packages/bytebot-agent-rs/src/main.rs:25
    with service: "bytebot-agent-rs", version: "0.1.0"

2024-01-31T10:30:45.234567Z DEBUG bytebot_agent_rs: Task created
    at packages/bytebot-agent-rs/src/tasks/service.rs:45
    with task_id: "task-123", description: "Process user request"
```

#### Production (JSON Format)

```json
{
  "timestamp": "2024-01-31T10:30:45.123456Z",
  "level": "INFO",
  "target": "bytebot_agent_rs",
  "message": "Service started successfully",
  "service": "bytebot-agent-rs",
  "version": "0.1.0",
  "span": {
    "name": "main"
  }
}

{
  "timestamp": "2024-01-31T10:30:45.234567Z",
  "level": "DEBUG",
  "target": "bytebot_agent_rs",
  "message": "Task created",
  "task_id": "task-123",
  "description": "Process user request",
  "span": {
    "name": "task_service"
  }
}
```

### Migration from TypeScript

The Rust logging system maintains compatibility with existing TypeScript logging patterns:

#### TypeScript (Before)
```typescript
console.log(`Client ${client.id} joined task ${taskId}`);
this.logger.debug(`Processing task ID: ${task.id}`);
this.logger.error(`Error executing ${block.name} tool: ${error.message}`, error.stack);
```

#### Rust (After)
```rust
websocket_logging::client_joined_task(&client_id, &task_id);
tracing::debug!(task_id = %task.id, "Processing task");
tracing::error!(
    tool_name = %block.name,
    error = %error,
    "Error executing tool"
);
```

### Performance Benefits

- **Zero-cost abstractions**: Logging macros compile to efficient code
- **Structured data**: JSON serialization is optimized for performance
- **Async-friendly**: Non-blocking logging operations
- **Memory efficient**: Reduced string allocations compared to format strings

### Best Practices

1. **Use structured fields**: Prefer `tracing::info!(user_id = %id, "User logged in")` over `tracing::info!("User {} logged in", id)`

2. **Include context**: Add relevant fields like `task_id`, `user_id`, `request_id` to help with debugging

3. **Use appropriate log levels**:
   - `ERROR`: System errors that require immediate attention
   - `WARN`: Recoverable errors or unusual conditions
   - `INFO`: Important system events and state changes
   - `DEBUG`: Detailed information for debugging
   - `TRACE`: Very detailed execution flow

4. **Leverage specialized modules**: Use the provided logging modules (`task_logging`, `automation_logging`, etc.) for consistent formatting

5. **Configure for environment**: Use JSON format in production, pretty format in development

### Runtime Configuration

Log levels can be controlled at runtime through environment variables:

```bash
# Set global log level
RUST_LOG=info

# Set per-module log levels
RUST_LOG=bytebot_agent_rs=debug,bytebot_shared_rs=info

# Complex filtering
RUST_LOG=debug,hyper=warn,sqlx=error
```

This logging system provides a solid foundation for observability in the ByteBot Rust services while maintaining compatibility with existing infrastructure and tooling.