use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use bytebot_shared_rs::{MetricsCollector, MetricsTimer};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{error, info};

/// Application state for health checks
#[derive(Clone)]
pub struct HealthState {
    pub db_pool: PgPool,
    pub metrics: Arc<MetricsCollector>,
    pub start_time: DateTime<Utc>,
}

/// Basic health check endpoint
pub async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "bytebot-agent-rs",
        "timestamp": Utc::now(),
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Detailed health check with service status
pub async fn health_detailed(State(state): State<HealthState>) -> Result<Json<Value>, StatusCode> {
    let timer = MetricsTimer::new();
    
    // Check database connectivity
    let db_status = check_database_health(&state.db_pool).await;
    
    // Calculate uptime
    let uptime_seconds = (Utc::now() - state.start_time).num_seconds();
    
    // Get system metrics
    let system_info = get_system_info();
    
    // Check AI service connectivity (basic check)
    let ai_services = check_ai_services().await;
    
    let health_status = if db_status.is_healthy && ai_services.iter().any(|s| s.is_healthy) {
        "healthy"
    } else if db_status.is_healthy {
        "degraded"
    } else {
        "unhealthy"
    };

    let response = json!({
        "status": health_status,
        "service": "bytebot-agent-rs",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": Utc::now(),
        "uptime_seconds": uptime_seconds,
        "checks": {
            "database": {
                "status": if db_status.is_healthy { "healthy" } else { "unhealthy" },
                "response_time_ms": db_status.response_time_ms,
                "details": db_status.details
            },
            "ai_services": ai_services,
            "system": system_info
        }
    });

    // Record health check metrics
    state.metrics.record_db_query("health_check", "system", timer.elapsed());

    match health_status {
        "healthy" => Ok(Json(response)),
        "degraded" => {
            info!("Service is in degraded state");
            Ok(Json(response))
        },
        _ => {
            error!("Service is unhealthy");
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

/// Readiness probe for Kubernetes/Docker
pub async fn readiness(State(state): State<HealthState>) -> Result<Json<Value>, StatusCode> {
    // Check if database is accessible
    let db_check = check_database_health(&state.db_pool).await;
    
    if db_check.is_healthy {
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
struct DatabaseHealth {
    is_healthy: bool,
    response_time_ms: u64,
    details: Value,
}

async fn check_database_health(pool: &PgPool) -> DatabaseHealth {
    let timer = MetricsTimer::new();
    
    match sqlx::query("SELECT 1 as health_check")
        .fetch_one(pool)
        .await
    {
        Ok(_) => {
            let response_time = timer.elapsed().as_millis() as u64;
            DatabaseHealth {
                is_healthy: true,
                response_time_ms: response_time,
                details: json!({
                    "connection_pool_size": pool.size(),
                    "idle_connections": pool.num_idle(),
                })
            }
        }
        Err(e) => {
            error!("Database health check failed: {}", e);
            DatabaseHealth {
                is_healthy: false,
                response_time_ms: timer.elapsed().as_millis() as u64,
                details: json!({
                    "error": e.to_string()
                })
            }
        }
    }
}

#[derive(Debug, serde::Serialize)]
struct AIServiceHealth {
    name: String,
    is_healthy: bool,
    details: Value,
}

async fn check_ai_services() -> Vec<AIServiceHealth> {
    let mut services = Vec::new();
    
    // Check Anthropic
    services.push(AIServiceHealth {
        name: "anthropic".to_string(),
        is_healthy: std::env::var("ANTHROPIC_API_KEY").is_ok(),
        details: json!({
            "configured": std::env::var("ANTHROPIC_API_KEY").is_ok()
        })
    });
    
    // Check OpenAI
    services.push(AIServiceHealth {
        name: "openai".to_string(),
        is_healthy: std::env::var("OPENAI_API_KEY").is_ok(),
        details: json!({
            "configured": std::env::var("OPENAI_API_KEY").is_ok()
        })
    });
    
    // Check Google
    services.push(AIServiceHealth {
        name: "google".to_string(),
        is_healthy: std::env::var("GOOGLE_API_KEY").is_ok(),
        details: json!({
            "configured": std::env::var("GOOGLE_API_KEY").is_ok()
        })
    });
    
    services
}

fn get_system_info() -> Value {
    json!({
        "memory_usage": get_memory_usage(),
        "cpu_count": num_cpus::get(),
        "process_id": std::process::id(),
    })
}

fn get_memory_usage() -> Value {
    // Basic memory usage information
    // In a production system, you might want to use a more sophisticated
    // memory monitoring library
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
    use sqlx::PgPool;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await;
        let value: Value = response.0;
        
        assert_eq!(value["status"], "healthy");
        assert_eq!(value["service"], "bytebot-agent-rs");
        assert!(value["timestamp"].is_string());
    }

    #[tokio::test]
    async fn test_liveness() {
        let response = liveness().await;
        let value: Value = response.0;
        
        assert_eq!(value["status"], "alive");
        assert!(value["timestamp"].is_string());
    }

    #[test]
    fn test_get_system_info() {
        let info = get_system_info();
        assert!(info["cpu_count"].is_number());
        assert!(info["process_id"].is_number());
    }
}