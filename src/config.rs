use std::env;

/// Redis connection configuration.
///
/// Construct via `RedisConfig::from_env()` for the standard workflow,
/// or `RedisConfig::new()` for programmatic setup.
#[derive(Clone, Debug)]
pub struct RedisConfig {
    /// Full Redis connection URL (e.g. `redis://localhost:6379`).
    pub url: String,
    /// Redis hostname.
    pub hostname: String,
    /// Redis port.
    pub port: u16,
    /// Optional password for AUTH.
    pub password: Option<String>,
}

impl RedisConfig {
    /// Create a new configuration with explicit parameters.
    pub fn new(hostname: &str, port: u16, password: Option<&str>) -> Self {
        let url = match &password {
            Some(pass) => format!("redis://:{}@{}:{}", pass, hostname, port),
            None => format!("redis://{}:{}", hostname, port),
        };

        RedisConfig {
            url,
            hostname: hostname.to_string(),
            port,
            password: password.map(String::from),
        }
    }

    /// Load configuration from environment variables:
    /// - `REDIS_HOSTNAME` (default: `localhost`)
    /// - `REDIS_PORT` (default: `6379`)
    /// - `REDIS_PASSWORD` (optional)
    pub fn from_env() -> Self {
        let hostname = env::var("REDIS_HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
        let port = env::var("REDIS_PORT")
            .unwrap_or_else(|_| "6379".to_string())
            .parse::<u16>()
            .unwrap_or(6379);
        let password = env::var("REDIS_PASSWORD").ok();

        Self::new(&hostname, port, password.as_deref())
    }

    /// Get the Redis connection URL.
    pub fn get_url(&self) -> &str {
        &self.url
    }
}
