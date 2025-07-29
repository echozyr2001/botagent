use std::net::SocketAddr;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub automation: AutomationConfig,
    pub log_level: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AutomationConfig {
    pub screenshot_quality: Option<u8>,
    pub input_delay_ms: Option<u64>,
    pub max_file_size_mb: Option<u64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 9990,
            },
            automation: AutomationConfig {
                screenshot_quality: Some(80),
                input_delay_ms: Some(10),
                max_file_size_mb: Some(10),
            },
            log_level: Some("info".to_string()),
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self, envy::Error> {
        let mut config = envy::from_env::<Config>().unwrap_or_default();

        // Override with environment variables if present
        if let Ok(host) = std::env::var("BYTEBOTD_HOST") {
            config.server.host = host;
        }
        if let Ok(port) = std::env::var("BYTEBOTD_PORT") {
            config.server.port = port.parse().unwrap_or(9990);
        }

        Ok(config)
    }

    pub fn socket_addr(&self) -> SocketAddr {
        format!("{}:{}", self.server.host, self.server.port)
            .parse()
            .expect("Invalid socket address")
    }
}
