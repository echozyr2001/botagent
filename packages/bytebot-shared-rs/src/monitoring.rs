use std::sync::Arc;

use serde_json::{json, Value};
use tracing::{info, warn};

use crate::MetricsCollector;

/// Monitoring integration for deployment environments
pub struct MonitoringIntegration {
    metrics: Arc<MetricsCollector>,
    service_name: String,
}

impl MonitoringIntegration {
    pub fn new(metrics: Arc<MetricsCollector>, service_name: String) -> Self {
        Self {
            metrics,
            service_name,
        }
    }

    /// Generate monitoring configuration for Prometheus
    pub fn generate_prometheus_config(&self) -> Value {
        json!({
            "global": {
                "scrape_interval": "15s",
                "evaluation_interval": "15s"
            },
            "scrape_configs": [
                {
                    "job_name": format!("{}-metrics", self.service_name),
                    "static_configs": [
                        {
                            "targets": ["localhost:9090"]
                        }
                    ],
                    "scrape_interval": "5s",
                    "metrics_path": "/metrics"
                }
            ],
            "rule_files": [
                format!("{}_alerts.yml", self.service_name)
            ]
        })
    }

    /// Generate alerting rules for the service
    pub fn generate_alert_rules(&self) -> Value {
        json!({
            "groups": [
                {
                    "name": format!("{}.rules", self.service_name),
                    "rules": [
                        {
                            "alert": "HighErrorRate",
                            "expr": format!("rate(http_requests_total{{status=~\"5..\",service=\"{}\"}}[5m]) > 0.1", self.service_name),
                            "for": "5m",
                            "labels": {
                                "severity": "warning",
                                "service": self.service_name
                            },
                            "annotations": {
                                "summary": format!("High error rate detected for {}", self.service_name),
                                "description": "Error rate is above 10% for 5 minutes"
                            }
                        },
                        {
                            "alert": "HighResponseTime",
                            "expr": format!("histogram_quantile(0.95, rate(http_request_duration_seconds_bucket{{service=\"{}\"}}[5m])) > 1", self.service_name),
                            "for": "5m",
                            "labels": {
                                "severity": "warning",
                                "service": self.service_name
                            },
                            "annotations": {
                                "summary": format!("High response time detected for {}", self.service_name),
                                "description": "95th percentile response time is above 1 second for 5 minutes"
                            }
                        },
                        {
                            "alert": "ServiceDown",
                            "expr": format!("up{{job=\"{}-metrics\"}} == 0", self.service_name),
                            "for": "1m",
                            "labels": {
                                "severity": "critical",
                                "service": self.service_name
                            },
                            "annotations": {
                                "summary": format!("{} service is down", self.service_name),
                                "description": "Service has been down for more than 1 minute"
                            }
                        },
                        {
                            "alert": "HighMemoryUsage",
                            "expr": format!("process_resident_memory_bytes{{service=\"{}\"}} > 1073741824", self.service_name), // 1GB
                            "for": "10m",
                            "labels": {
                                "severity": "warning",
                                "service": self.service_name
                            },
                            "annotations": {
                                "summary": format!("High memory usage for {}", self.service_name),
                                "description": "Memory usage is above 1GB for 10 minutes"
                            }
                        }
                    ]
                }
            ]
        })
    }

    /// Generate Grafana dashboard configuration
    pub fn generate_grafana_dashboard(&self) -> Value {
        json!({
            "dashboard": {
                "id": null,
                "title": format!("{} Dashboard", self.service_name),
                "tags": ["bytebot", self.service_name],
                "timezone": "browser",
                "panels": [
                    {
                        "id": 1,
                        "title": "HTTP Request Rate",
                        "type": "graph",
                        "targets": [
                            {
                                "expr": format!("rate(http_requests_total{{service=\"{}\"}}[5m])", self.service_name),
                                "legendFormat": "{{method}} {{status}}"
                            }
                        ],
                        "yAxes": [
                            {
                                "label": "Requests/sec"
                            }
                        ]
                    },
                    {
                        "id": 2,
                        "title": "Response Time",
                        "type": "graph",
                        "targets": [
                            {
                                "expr": format!("histogram_quantile(0.95, rate(http_request_duration_seconds_bucket{{service=\"{}\"}}[5m]))", self.service_name),
                                "legendFormat": "95th percentile"
                            },
                            {
                                "expr": format!("histogram_quantile(0.50, rate(http_request_duration_seconds_bucket{{service=\"{}\"}}[5m]))", self.service_name),
                                "legendFormat": "50th percentile"
                            }
                        ],
                        "yAxes": [
                            {
                                "label": "Seconds"
                            }
                        ]
                    },
                    {
                        "id": 3,
                        "title": "Error Rate",
                        "type": "singlestat",
                        "targets": [
                            {
                                "expr": format!("rate(http_requests_total{{status=~\"5..\",service=\"{}\"}}[5m]) / rate(http_requests_total{{service=\"{}\"}}[5m]) * 100", self.service_name, self.service_name),
                                "legendFormat": "Error Rate %"
                            }
                        ]
                    },
                    {
                        "id": 4,
                        "title": "Memory Usage",
                        "type": "graph",
                        "targets": [
                            {
                                "expr": format!("process_resident_memory_bytes{{service=\"{}\"}}", self.service_name),
                                "legendFormat": "RSS Memory"
                            }
                        ],
                        "yAxes": [
                            {
                                "label": "Bytes"
                            }
                        ]
                    }
                ],
                "time": {
                    "from": "now-1h",
                    "to": "now"
                },
                "refresh": "5s"
            }
        })
    }

    /// Generate Docker Compose monitoring stack
    pub fn generate_docker_compose_monitoring(&self) -> Value {
        json!({
            "version": "3.8",
            "services": {
                "prometheus": {
                    "image": "prom/prometheus:latest",
                    "container_name": format!("{}-prometheus", self.service_name),
                    "ports": ["9090:9090"],
                    "volumes": [
                        "./prometheus.yml:/etc/prometheus/prometheus.yml",
                        format!("./{}_alerts.yml:/etc/prometheus/{}_alerts.yml", self.service_name, self.service_name)
                    ],
                    "command": [
                        "--config.file=/etc/prometheus/prometheus.yml",
                        "--storage.tsdb.path=/prometheus",
                        "--web.console.libraries=/etc/prometheus/console_libraries",
                        "--web.console.templates=/etc/prometheus/consoles",
                        "--storage.tsdb.retention.time=200h",
                        "--web.enable-lifecycle"
                    ]
                },
                "grafana": {
                    "image": "grafana/grafana:latest",
                    "container_name": format!("{}-grafana", self.service_name),
                    "ports": ["3000:3000"],
                    "environment": {
                        "GF_SECURITY_ADMIN_PASSWORD": "admin"
                    },
                    "volumes": [
                        "grafana-storage:/var/lib/grafana"
                    ]
                },
                "alertmanager": {
                    "image": "prom/alertmanager:latest",
                    "container_name": format!("{}-alertmanager", self.service_name),
                    "ports": ["9093:9093"],
                    "volumes": [
                        "./alertmanager.yml:/etc/alertmanager/alertmanager.yml"
                    ]
                }
            },
            "volumes": {
                "grafana-storage": {}
            }
        })
    }

    /// Generate Kubernetes monitoring manifests
    pub fn generate_k8s_monitoring_manifests(&self) -> Vec<Value> {
        vec![
            // ServiceMonitor for Prometheus Operator
            json!({
                "apiVersion": "monitoring.coreos.com/v1",
                "kind": "ServiceMonitor",
                "metadata": {
                    "name": format!("{}-metrics", self.service_name),
                    "labels": {
                        "app": self.service_name
                    }
                },
                "spec": {
                    "selector": {
                        "matchLabels": {
                            "app": self.service_name
                        }
                    },
                    "endpoints": [
                        {
                            "port": "metrics",
                            "path": "/metrics",
                            "interval": "30s"
                        }
                    ]
                }
            }),
            // PrometheusRule for alerting
            json!({
                "apiVersion": "monitoring.coreos.com/v1",
                "kind": "PrometheusRule",
                "metadata": {
                    "name": format!("{}-alerts", self.service_name),
                    "labels": {
                        "app": self.service_name
                    }
                },
                "spec": self.generate_alert_rules()
            }),
        ]
    }

    /// Log monitoring setup information
    pub fn log_monitoring_setup(&self) {
        info!(
            service = %self.service_name,
            metrics_port = 9090,
            "Monitoring setup completed"
        );

        info!(
            prometheus_endpoint = "http://localhost:9090/metrics",
            grafana_endpoint = "http://localhost:3000",
            "Monitoring endpoints available"
        );
    }

    /// Validate monitoring configuration
    pub fn validate_monitoring_setup(&self) -> Result<(), String> {
        // Check if metrics collector is working
        let metrics_output = self.metrics.render();
        if metrics_output.is_empty() {
            return Err("Metrics collector is not producing output".to_string());
        }

        // Check if required environment variables are set for monitoring
        let required_vars = vec!["PROMETHEUS_ENABLED", "GRAFANA_ENABLED"];
        for var in required_vars {
            if std::env::var(var).is_err() {
                warn!(
                    env_var = %var,
                    "Optional monitoring environment variable not set"
                );
            }
        }

        info!(
            service = %self.service_name,
            "Monitoring configuration validation completed"
        );

        Ok(())
    }

    /// Export monitoring configuration files
    pub fn export_monitoring_configs(
        &self,
        output_dir: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::{fs, path::Path};

        let output_path = Path::new(output_dir);
        fs::create_dir_all(output_path)?;

        // Export Prometheus config
        let prometheus_config = self.generate_prometheus_config();
        fs::write(
            output_path.join("prometheus.yml"),
            serde_yaml::to_string(&prometheus_config)?,
        )?;

        // Export alert rules
        let alert_rules = self.generate_alert_rules();
        fs::write(
            output_path.join(format!("{}_alerts.yml", self.service_name)),
            serde_yaml::to_string(&alert_rules)?,
        )?;

        // Export Grafana dashboard
        let dashboard = self.generate_grafana_dashboard();
        fs::write(
            output_path.join(format!("{}_dashboard.json", self.service_name)),
            serde_json::to_string_pretty(&dashboard)?,
        )?;

        // Export Docker Compose
        let docker_compose = self.generate_docker_compose_monitoring();
        fs::write(
            output_path.join("docker-compose.monitoring.yml"),
            serde_yaml::to_string(&docker_compose)?,
        )?;

        info!(
            output_dir = %output_dir,
            service = %self.service_name,
            "Monitoring configuration files exported"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn test_monitoring_integration_creation() {
        // Create a mock metrics collector for testing
        if let Ok(metrics) = MetricsCollector::new("test-service") {
            let monitoring =
                MonitoringIntegration::new(Arc::new(metrics), "test-service".to_string());

            // Test configuration generation
            let prometheus_config = monitoring.generate_prometheus_config();
            assert!(prometheus_config["scrape_configs"].is_array());

            let alert_rules = monitoring.generate_alert_rules();
            assert!(alert_rules["groups"].is_array());

            let dashboard = monitoring.generate_grafana_dashboard();
            assert!(dashboard["dashboard"]["panels"].is_array());

            let docker_compose = monitoring.generate_docker_compose_monitoring();
            assert!(docker_compose["services"]["prometheus"].is_object());

            let k8s_manifests = monitoring.generate_k8s_monitoring_manifests();
            assert_eq!(k8s_manifests.len(), 2);
        }
    }

    #[test]
    fn test_monitoring_validation() {
        if let Ok(metrics) = MetricsCollector::new("test-service") {
            let monitoring =
                MonitoringIntegration::new(Arc::new(metrics), "test-service".to_string());

            // Validation should succeed with a working metrics collector
            assert!(monitoring.validate_monitoring_setup().is_ok());
        }
    }
}
