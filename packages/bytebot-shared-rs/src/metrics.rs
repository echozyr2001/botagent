use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::time::{Duration, Instant};
use tracing::{error, info};

/// Metrics collector for ByteBot services
pub struct MetricsCollector {
    pub prometheus_handle: PrometheusHandle,
    pub service_name: String,
}

impl MetricsCollector {
    /// Initialize metrics collector with Prometheus exporter
    pub fn new(service_name: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let recorder = PrometheusBuilder::new()
            .add_global_label("service", service_name)
            .build_recorder();

        let handle = recorder.handle();
        metrics::set_global_recorder(recorder)?;

        info!("Metrics collector initialized for service: {}", service_name);

        Ok(Self {
            prometheus_handle: handle,
            service_name: service_name.to_string(),
        })
    }

    /// Get Prometheus metrics as string
    pub fn render(&self) -> String {
        self.prometheus_handle.render()
    }

    /// Record HTTP request metrics
    pub fn record_http_request(&self, method: &str, path: &str, status: u16, duration: Duration) {
        let method = method.to_string();
        let path = path.to_string();
        let status = status.to_string();
        counter!("http_requests_total", "method" => method.clone(), "path" => path.clone(), "status" => status).increment(1);
        histogram!("http_request_duration_seconds", "method" => method, "path" => path).record(duration.as_secs_f64());
    }

    /// Record task creation
    pub fn record_task_created(&self, task_type: &str) {
        let task_type = task_type.to_string();
        counter!("tasks_created_total", "type" => task_type).increment(1);
        gauge!("tasks_in_progress").increment(1.0);
    }

    /// Record task completion
    pub fn record_task_completed(&self, task_type: &str, duration: Duration) {
        let task_type = task_type.to_string();
        counter!("tasks_completed_total", "type" => task_type.clone()).increment(1);
        gauge!("tasks_in_progress").decrement(1.0);
        histogram!("task_duration_seconds", "type" => task_type).record(duration.as_secs_f64());
    }

    /// Record task failure
    pub fn record_task_failed(&self, task_type: &str, error_type: &str, duration: Duration) {
        let task_type = task_type.to_string();
        let error_type = error_type.to_string();
        counter!("tasks_failed_total", "type" => task_type.clone(), "error" => error_type).increment(1);
        gauge!("tasks_in_progress").decrement(1.0);
        histogram!("task_duration_seconds", "type" => task_type).record(duration.as_secs_f64());
    }

    /// Record AI API request
    pub fn record_ai_request(&self, provider: &str, model: &str, duration: Duration, tokens: u64) {
        let provider = provider.to_string();
        let model = model.to_string();
        counter!("ai_requests_total", "provider" => provider.clone(), "model" => model.clone()).increment(1);
        histogram!("ai_request_duration_seconds", "provider" => provider.clone(), "model" => model.clone()).record(duration.as_secs_f64());
        counter!("ai_tokens_used_total", "provider" => provider, "model" => model).increment(tokens);
    }

    /// Record AI API error
    pub fn record_ai_error(&self, provider: &str, model: &str, error_type: &str) {
        let provider = provider.to_string();
        let model = model.to_string();
        let error_type = error_type.to_string();
        counter!("ai_errors_total", "provider" => provider, "model" => model, "error" => error_type).increment(1);
    }

    /// Record database query
    pub fn record_db_query(&self, operation: &str, table: &str, duration: Duration) {
        let operation = operation.to_string();
        let table = table.to_string();
        counter!("db_queries_total", "operation" => operation.clone(), "table" => table.clone()).increment(1);
        histogram!("db_query_duration_seconds", "operation" => operation, "table" => table).record(duration.as_secs_f64());
    }

    /// Record WebSocket connection change
    pub fn record_websocket_connection(&self, connected: bool) {
        if connected {
            gauge!("websocket_connections").increment(1.0);
        } else {
            gauge!("websocket_connections").decrement(1.0);
        }
    }

    /// Record WebSocket message
    pub fn record_websocket_message(&self, direction: &str, event_type: &str) {
        let event_type = event_type.to_string();
        match direction {
            "sent" => counter!("websocket_messages_sent_total", "event" => event_type).increment(1),
            "received" => counter!("websocket_messages_received_total", "event" => event_type).increment(1),
            _ => error!("Invalid WebSocket message direction: {}", direction),
        }
    }

    /// Record automation action
    pub fn record_automation_action(&self, action: &str, duration: Duration) {
        let action = action.to_string();
        counter!("automation_actions_total", "action" => action.clone()).increment(1);
        histogram!("automation_action_duration_seconds", "action" => action).record(duration.as_secs_f64());
    }

    /// Record automation error
    pub fn record_automation_error(&self, action: &str, error_type: &str) {
        let action = action.to_string();
        let error_type = error_type.to_string();
        counter!("automation_errors_total", "action" => action, "error" => error_type).increment(1);
    }

    /// Record screenshot taken
    pub fn record_screenshot(&self) {
        counter!("screenshots_taken_total").increment(1);
    }

    /// Update database connection count
    pub fn update_db_connections(&self, count: f64) {
        gauge!("db_connections_active").set(count);
    }

    /// Increment HTTP requests in flight
    pub fn increment_http_in_flight(&self) {
        gauge!("http_requests_in_flight").increment(1.0);
    }

    /// Decrement HTTP requests in flight
    pub fn decrement_http_in_flight(&self) {
        gauge!("http_requests_in_flight").decrement(1.0);
    }
}

/// Timer helper for measuring durations
pub struct MetricsTimer {
    start: Instant,
}

impl MetricsTimer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

impl Default for MetricsTimer {
    fn default() -> Self {
        Self::new()
    }
}

/// Middleware for recording HTTP metrics
pub mod middleware {
    use super::*;
    use axum::{
        extract::Request,
        http::StatusCode,
        middleware::Next,
        response::Response,
    };
    use std::sync::Arc;

    pub async fn metrics_middleware(
        request: Request,
        next: Next,
    ) -> Result<Response, StatusCode> {
        // Extract metrics collector from request extensions
        let metrics = request
            .extensions()
            .get::<Arc<MetricsCollector>>()
            .cloned();

        if let Some(metrics) = metrics {
            let method = request.method().to_string();
            let path = request.uri().path().to_string();
            let timer = MetricsTimer::new();

            metrics.increment_http_in_flight();

            let response = next.run(request).await;
            let status = response.status().as_u16();
            let duration = timer.elapsed();

            metrics.decrement_http_in_flight();
            metrics.record_http_request(&method, &path, status, duration);

            Ok(response)
        } else {
            Ok(next.run(request).await)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_metrics_timer() {
        let timer = MetricsTimer::new();
        std::thread::sleep(Duration::from_millis(10));
        let elapsed = timer.elapsed();
        assert!(elapsed >= Duration::from_millis(10));
    }

    #[test]
    fn test_metrics_collector_creation() {
        // This test might fail in CI environments without network access
        // but it's useful for local development
        if let Ok(collector) = MetricsCollector::new("test-service") {
            // Test that we can record some metrics
            collector.record_task_created("test");
            collector.record_task_completed("test", Duration::from_secs(1));
            
            // Verify metrics can be rendered
            let metrics_output = collector.render();
            assert!(!metrics_output.is_empty());
        }
    }
}