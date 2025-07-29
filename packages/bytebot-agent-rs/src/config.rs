use std::env;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub redis_url: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub google_api_key: Option<String>,
    pub auth_enabled: bool,
    pub jwt_secret: String,
    pub cors_origins: Vec<String>,
    pub log_level: String,
    pub server: ServerConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: Option<usize>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Environment variable error: {0}")]
    Env(#[from] env::VarError),
    #[error("Configuration parsing error: {0}")]
    Parse(#[from] envy::Error),
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok(); // Load .env file if present

        let config = envy::from_env::<Config>()?;
        Ok(config)
    }

    pub fn database_url(&self) -> &str {
        &self.database_url
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://localhost:5432/bytebot".to_string()),
            redis_url: env::var("REDIS_URL").ok(),
            anthropic_api_key: env::var("ANTHROPIC_API_KEY").ok(),
            openai_api_key: env::var("OPENAI_API_KEY").ok(),
            google_api_key: env::var("GOOGLE_API_KEY").ok(),
            auth_enabled: env::var("AUTH_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "default-jwt-secret-change-in-production".to_string()),
            cors_origins: env::var("CORS_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:3000".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            server: ServerConfig {
                host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
                port: env::var("PORT")
                    .unwrap_or_else(|_| "9991".to_string())
                    .parse()
                    .unwrap_or(9991),
                workers: env::var("WORKERS").ok().and_then(|w| w.parse().ok()),
            },
        }
    }
}
