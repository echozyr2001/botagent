use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use bytebot_shared_rs::{MetricsCollector, MetricsTimer};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{error, info, warn};

/// Application state for health checks
#[derive(Clone)]
pub struct HealthState {
    pub metrics: Arc<MetricsCollector>,
    pub start_time: DateTime<Utc>,
}

/// Basic health check endpoint
pub async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "bytebotd-rs",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": Utc::now(),
        "capabilities": [
            "screenshot",
            "mouse_control",
            "keyboard_control",
            "file_operations",
            "application_switching"
        ]
    }))
}

/// Detailed health check with service status
pub async fn health_detailed(State(state): State<HealthState>) -> Result<Json<Value>, StatusCode> {
    let timer = MetricsTimer::new();
    
    // Check desktop automation capabilities
    let automation_status = check_automation_health().await;
    
    // Calculate uptime
    let uptime_seconds = (Utc::now() - state.start_time).num_seconds();
    
    // Get system info
    let system_info = get_system_info();
    
    // Check display availability
    let display_status = check_display_health();
    
    let health_status = if automation_status.is_healthy && display_status.is_healthy {
        "healthy"
    } else if automation_status.is_healthy || display_status.is_healthy {
        "degraded"
    } else {
        "unhealthy"
    };

    let response = json!({
        "status": health_status,
        "service": "bytebotd-rs",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": Utc::now(),
        "uptime_seconds": uptime_seconds,
        "checks": {
            "automation": {
                "status": if automation_status.is_healthy { "healthy" } else { "unhealthy" },
                "details": automation_status.details
            },
            "display": {
                "status": if display_status.is_healthy { "healthy" } else { "unhealthy" },
                "details": display_status.details
            },
            "system": system_info
        }
    });

    // Record health check duration
    let duration = timer.elapsed();
    state.metrics.record_automation_action("health_check", duration);

    match health_status {
        "healthy" => Ok(Json(response)),
        "degraded" => {
            warn!("Desktop automation service is in degraded state");
            Ok(Json(response))
        },
        _ => {
            error!("Desktop automation service is unhealthy");
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

/// Readiness probe for Kubernetes/Docker
pub async fn readiness(State(state): State<HealthState>) -> Result<Json<Value>, StatusCode> {
    // Check if we can take a screenshot (basic automation test)
    let automation_check = check_automation_health().await;
    
    if automation_check.is_healthy {
        Ok(Json(json!({
            "status": "ready",
            "timestamp": Utc::now()
        })))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

/// Liveness probe for Kubernetes/Docker
pub async fn liveness() -> Json<Value> {
    Json(json!({
        "status": "alive",
        "timestamp": Utc::now()
    }))
}

/// Prometheus metrics endpoint
pub async fn metrics(State(state): State<HealthState>) -> String {
    state.metrics.render()
}

#[derive(Debug)]
struct AutomationHealth {
    is_healthy: bool,
    details: Value,
}

async fn check_automation_health() -> AutomationHealth {
    let mut checks = Vec::new();
    
    // Test screenshot capability
    let screenshot_ok = test_screenshot_capability().await;
    checks.push(("screenshot", screenshot_ok));
    
    // Test mouse capability (basic check)
    let mouse_ok = test_mouse_capability();
    checks.push(("mouse", mouse_ok));
    
    // Test keyboard capability (basic check)
    let keyboard_ok = test_keyboard_capability();
    checks.push(("keyboard", keyboard_ok));
    
    let all_healthy = checks.iter().all(|(_, ok)| *ok);
    
    AutomationHealth {
        is_healthy: all_healthy,
        details: json!({
            "capabilities": checks.into_iter().map(|(name, ok)| {
                json!({
                    "name": name,
                    "status": if ok { "healthy" } else { "unhealthy" }
                })
            }).collect::<Vec<_>>()
        })
    }
}

async fn test_screenshot_capability() -> bool {
    // Try to initialize screenshot capability without actually taking one
    match screenshots::Screen::all() {
        Ok(screens) => !screens.is_empty(),
        Err(e) => {
            error!("Screenshot capability check failed: {}", e);
            false
        }
    }
}

fn test_mouse_capability() -> bool {
    // Basic check - can we create an Enigo instance for mouse control
    match enigo::Enigo::new(&enigo::Settings::default()) {
        Ok(_) => true,
        Err(e) => {
            error!("Mouse capability check failed: {}", e);
            false
        }
    }
}

fn test_keyboard_capability() -> bool {
    // Basic check - can we create an Enigo instance for keyboard control
    match enigo::Enigo::new(&enigo::Settings::default()) {
        Ok(_) => true,
        Err(e) => {
            error!("Keyboard capability check failed: {}", e);
            false
        }
    }
}

#[derive(Debug)]
struct DisplayHealth {
    is_healthy: bool,
    details: Value,
}

fn check_display_health() -> DisplayHealth {
    // Check if we have access to a display
    let display_available = check_display_available();
    
    DisplayHealth {
        is_healthy: display_available,
        details: json!({
            "display_available": display_available,
            "display_env": std::env::var("DISPLAY").unwrap_or_else(|_| "not_set".to_string())
        })
    }
}

#[cfg(unix)]
fn check_display_available() -> bool {
    std::env::var("DISPLAY").is_ok()
}

#[cfg(not(unix))]
fn check_display_available() -> bool {
    // On non-Unix systems, assume display is available
    true
}

fn get_system_info() -> Value {
    json!({
        "memory_usage": get_memory_usage(),
        "cpu_count": num_cpus::get(),
        "process_id": std::process::id(),
        "platform": std::env::consts::OS,
        "architecture": std::env::consts::ARCH,
    })
}

fn get_memory_usage() -> Value {
    json!({
        "rss_bytes": get_rss_memory(),
    })
}

#[cfg(target_os = "linux")]
fn get_rss_memory() -> u64 {
    use std::fs;
    
    if let Ok(contents) = fs::read_to_string("/proc/self/status") {
        for line in contents.lines() {
            if line.starts_with("VmRSS:") {
                if let Some(kb_str) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = kb_str.parse::<u64>() {
                        return kb * 1024; // Convert KB to bytes
                    }
                }
            }
        }
    }
    0
}

#[cfg(not(target_os = "linux"))]
fn get_rss_memory() -> u64 {
    // Fallback for non-Linux systems
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await;
        let value: Value = response.0;
        
        assert_eq!(value["status"], "healthy");
        assert_eq!(value["service"], "bytebotd-rs");
        assert!(value["capabilities"].is_array());
        assert!(value["timestamp"].is_string());
    }

    #[tokio::test]
    async fn test_liveness() {
        let response = liveness().await;
        let value: Value = response.0;
        
        assert_eq!(value["status"], "alive");
        assert!(value["timestamp"].is_string());
    }

    #[tokio::test]
    async fn test_automation_health_check() {
        let health = check_automation_health().await;
        // Should have some capabilities even if they fail
        assert!(health.details["capabilities"].is_array());
    }

    #[test]
    fn test_get_system_info() {
        let info = get_system_info();
        assert!(info["cpu_count"].is_number());
        assert!(info["process_id"].is_number());
        assert!(info["platform"].is_string());
        assert!(info["architecture"].is_string());
    }

    #[test]
    fn test_display_health_check() {
        let health = check_display_health();
        assert!(health.details["display_env"].is_string());
    }
}
