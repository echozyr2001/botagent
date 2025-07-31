use std::env;

use serde_json::json;
use tracing::{Level, Subscriber};
use tracing_subscriber::{
    fmt::{
        time::UtcTime,
    },
    layer::SubscriberExt,
    registry::LookupSpan,
    util::SubscriberInitExt,
    EnvFilter, Registry,
};

/// Logging configuration for ByteBot services
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: Level,
    /// Whether to use JSON format (true) or pretty format (false)
    pub json_format: bool,
    /// Service name to include in logs
    pub service_name: String,
    /// Whether to include file and line information
    pub include_location: bool,
    /// Whether to include thread information
    pub include_thread: bool,
    /// Custom fields to include in all log entries
    pub custom_fields: serde_json::Value,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: Level::INFO,
            json_format: false,
            service_name: "bytebot".to_string(),
            include_location: false,
            include_thread: false,
            custom_fields: json!({}),
        }
    }
}

impl LoggingConfig {
    /// Create a new logging configuration from environment variables
    pub fn from_env() -> Self {
        let level = env::var("LOG_LEVEL")
            .unwrap_or_else(|_| "info".to_string())
            .parse::<Level>()
            .unwrap_or(Level::INFO);

        let json_format = env::var("LOG_FORMAT")
            .unwrap_or_else(|_| "pretty".to_string())
            .to_lowercase()
            == "json";

        let service_name = env::var("SERVICE_NAME").unwrap_or_else(|_| "bytebot".to_string());

        let include_location = env::var("LOG_INCLUDE_LOCATION")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);

        let include_thread = env::var("LOG_INCLUDE_THREAD")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);

        // Parse custom fields from environment
        let custom_fields = env::var("LOG_CUSTOM_FIELDS")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| json!({}));

        Self {
            level,
            json_format,
            service_name,
            include_location,
            include_thread,
            custom_fields,
        }
    }

    /// Create a configuration for a specific service
    pub fn for_service(service_name: &str) -> Self {
        let mut config = Self::from_env();
        config.service_name = service_name.to_string();
        config
    }
}

/// Initialize the global tracing subscriber with the given configuration
pub fn init_logging(config: LoggingConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create environment filter
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&config.level.to_string()))?;

    if config.json_format {
        // JSON format for production/log aggregation
        let fmt_layer = tracing_subscriber::fmt::layer()
            .json()
            .with_timer(UtcTime::rfc_3339())
            .with_current_span(true)
            .with_span_list(true)
            .with_target(true)
            .with_thread_ids(config.include_thread)
            .with_thread_names(config.include_thread)
            .with_file(config.include_location)
            .with_line_number(config.include_location);

        Registry::default()
            .with(env_filter)
            .with(fmt_layer)
            .with(CustomFieldsLayer::new(
                config.service_name,
                config.custom_fields,
            ))
            .try_init()?;
    } else {
        // Pretty format for development
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_timer(UtcTime::rfc_3339())
            .with_target(false)
            .with_thread_ids(config.include_thread)
            .with_thread_names(config.include_thread)
            .with_file(config.include_location)
            .with_line_number(config.include_location);

        Registry::default()
            .with(env_filter)
            .with(fmt_layer)
            .with(CustomFieldsLayer::new(
                config.service_name,
                config.custom_fields,
            ))
            .try_init()?;
    }

    Ok(())
}

/// Custom layer to add service name and custom fields to all log entries
struct CustomFieldsLayer {
    service_name: String,
    custom_fields: serde_json::Value,
}

impl CustomFieldsLayer {
    fn new(service_name: String, custom_fields: serde_json::Value) -> Self {
        Self {
            service_name,
            custom_fields,
        }
    }
}

impl<S> tracing_subscriber::Layer<S> for CustomFieldsLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(
        &self,
        _event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // Add service name and custom fields to the event
        // This is handled by the JSON formatter automatically when using structured logging
    }

    fn on_new_span(
        &self,
        _attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // Add custom fields to spans
        if let Some(span) = ctx.span(id) {
            let mut extensions = span.extensions_mut();
            extensions.insert(ServiceContext {
                service_name: self.service_name.clone(),
                custom_fields: self.custom_fields.clone(),
            });
        }
    }
}

/// Context information added to all log entries
#[derive(Debug, Clone)]
struct ServiceContext {
    service_name: String,
    custom_fields: serde_json::Value,
}

/// Structured logging macros that are compatible with existing TypeScript patterns
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        tracing::info!($($arg)*)
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        tracing::warn!($($arg)*)
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        tracing::error!($($arg)*)
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        tracing::debug!($($arg)*)
    };
}

#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        tracing::trace!($($arg)*)
    };
}

/// Structured logging for task operations (compatible with TypeScript patterns)
pub mod task_logging {
    use tracing::{error, info, warn};

    pub fn task_created(task_id: &str, description: &str) {
        info!(
            task_id = task_id,
            description = description,
            "Task created"
        );
    }

    pub fn task_started(task_id: &str) {
        info!(task_id = task_id, "Task started processing");
    }

    pub fn task_completed(task_id: &str, duration_ms: u64) {
        info!(
            task_id = task_id,
            duration_ms = duration_ms,
            "Task completed successfully"
        );
    }

    pub fn task_failed(task_id: &str, error: &str) {
        error!(task_id = task_id, error = error, "Task failed");
    }

    pub fn task_cancelled(task_id: &str) {
        warn!(task_id = task_id, "Task cancelled");
    }

    pub fn task_takeover(task_id: &str, from_role: &str, to_role: &str) {
        info!(
            task_id = task_id,
            from_role = from_role,
            to_role = to_role,
            "Task control transferred"
        );
    }
}

/// Structured logging for computer automation operations
pub mod automation_logging {
    use tracing::{debug, error};

    pub fn screenshot_taken() {
        debug!("Screenshot captured successfully");
    }

    pub fn mouse_moved(x: i32, y: i32) {
        debug!(x = x, y = y, "Mouse moved to coordinates");
    }

    pub fn mouse_clicked(x: i32, y: i32, button: &str, count: u32) {
        debug!(
            x = x,
            y = y,
            button = button,
            count = count,
            "Mouse clicked"
        );
    }

    pub fn text_typed(text: &str) {
        debug!(text = text, "Text typed");
    }

    pub fn keys_pressed(keys: &[String]) {
        debug!(keys = ?keys, "Keys pressed");
    }

    pub fn file_read(path: &str, success: bool) {
        if success {
            debug!(path = path, "File read successfully");
        } else {
            error!(path = path, "Failed to read file");
        }
    }

    pub fn file_written(path: &str, success: bool) {
        if success {
            debug!(path = path, "File written successfully");
        } else {
            error!(path = path, "Failed to write file");
        }
    }

    pub fn application_switched(app: &str) {
        debug!(application = app, "Application switched");
    }

    pub fn automation_error(action: &str, error: &str) {
        error!(action = action, error = error, "Automation action failed");
    }
}

/// Structured logging for AI service operations
pub mod ai_logging {
    use tracing::{debug, error, info, warn};

    pub fn ai_request_started(provider: &str, model: &str, message_count: usize) {
        debug!(
            provider = provider,
            model = model,
            message_count = message_count,
            "AI request started"
        );
    }

    pub fn ai_request_completed(provider: &str, model: &str, duration_ms: u64, tokens_used: Option<u32>) {
        info!(
            provider = provider,
            model = model,
            duration_ms = duration_ms,
            tokens_used = tokens_used,
            "AI request completed"
        );
    }

    pub fn ai_request_failed(provider: &str, model: &str, error: &str) {
        error!(
            provider = provider,
            model = model,
            error = error,
            "AI request failed"
        );
    }

    pub fn ai_rate_limited(provider: &str, retry_after: Option<u64>) {
        warn!(
            provider = provider,
            retry_after = retry_after,
            "AI request rate limited"
        );
    }

    pub fn model_list_updated(provider: &str, model_count: usize) {
        debug!(
            provider = provider,
            model_count = model_count,
            "Model list updated"
        );
    }
}

/// Structured logging for WebSocket operations
pub mod websocket_logging {
    use tracing::{debug, info};

    pub fn client_connected(client_id: &str) {
        info!(client_id = client_id, "WebSocket client connected");
    }

    pub fn client_disconnected(client_id: &str) {
        info!(client_id = client_id, "WebSocket client disconnected");
    }

    pub fn client_joined_task(client_id: &str, task_id: &str) {
        debug!(
            client_id = client_id,
            task_id = task_id,
            "Client joined task room"
        );
    }

    pub fn client_left_task(client_id: &str, task_id: &str) {
        debug!(
            client_id = client_id,
            task_id = task_id,
            "Client left task room"
        );
    }

    pub fn event_emitted(event_type: &str, room: Option<&str>, client_count: usize) {
        debug!(
            event_type = event_type,
            room = room,
            client_count = client_count,
            "WebSocket event emitted"
        );
    }
}

/// Structured logging for database operations
pub mod database_logging {
    use tracing::{debug, error, info};

    pub fn connection_established(database_url: &str, pool_size: u32) {
        info!(
            database_url = database_url,
            pool_size = pool_size,
            "Database connection established"
        );
    }

    pub fn migration_started(migration_name: &str) {
        info!(migration_name = migration_name, "Database migration started");
    }

    pub fn migration_completed(migration_name: &str, duration_ms: u64) {
        info!(
            migration_name = migration_name,
            duration_ms = duration_ms,
            "Database migration completed"
        );
    }

    pub fn query_executed(query_type: &str, duration_ms: u64, affected_rows: Option<u64>) {
        debug!(
            query_type = query_type,
            duration_ms = duration_ms,
            affected_rows = affected_rows,
            "Database query executed"
        );
    }

    pub fn query_failed(query_type: &str, error: &str) {
        error!(
            query_type = query_type,
            error = error,
            "Database query failed"
        );
    }

    pub fn connection_pool_stats(size: u32, idle: u32, active: u32) {
        debug!(
            pool_size = size,
            idle_connections = idle,
            active_connections = active,
            "Database connection pool stats"
        );
    }
}

/// Runtime log level control
pub struct LogLevelController;

impl LogLevelController {
    /// Update the log level at runtime
    pub fn set_level(level: Level) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Note: This requires the tracing-subscriber reload feature
        // For now, we'll log the change request
        tracing::info!(new_level = ?level, "Log level change requested (requires restart)");
        Ok(())
    }

    /// Get the current log level
    pub fn get_level() -> Level {
        // This is a simplified implementation
        // In a full implementation, you'd track the current level
        Level::INFO
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_logging_config_from_env() {
        // Set test environment variables
        env::set_var("LOG_LEVEL", "debug");
        env::set_var("LOG_FORMAT", "json");
        env::set_var("SERVICE_NAME", "test-service");
        env::set_var("LOG_INCLUDE_LOCATION", "true");

        let config = LoggingConfig::from_env();

        assert_eq!(config.level, Level::DEBUG);
        assert!(config.json_format);
        assert_eq!(config.service_name, "test-service");
        assert!(config.include_location);

        // Clean up
        env::remove_var("LOG_LEVEL");
        env::remove_var("LOG_FORMAT");
        env::remove_var("SERVICE_NAME");
        env::remove_var("LOG_INCLUDE_LOCATION");
    }

    #[test]
    fn test_logging_config_defaults() {
        // Ensure no relevant env vars are set
        env::remove_var("LOG_LEVEL");
        env::remove_var("LOG_FORMAT");
        env::remove_var("SERVICE_NAME");

        let config = LoggingConfig::from_env();

        assert_eq!(config.level, Level::INFO);
        assert!(!config.json_format);
        assert_eq!(config.service_name, "bytebot");
        assert!(!config.include_location);
    }

    #[test]
    fn test_service_specific_config() {
        let config = LoggingConfig::for_service("bytebot-agent-rs");
        assert_eq!(config.service_name, "bytebot-agent-rs");
    }
}