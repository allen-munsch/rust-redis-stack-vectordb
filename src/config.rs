use std::env;

/// Redis configuration structure that holds connection details
#[derive(Clone, Debug)]
pub struct RedisConfig {
    pub url: String,
    pub hostname: String,
    pub port: u16,
    pub password: Option<String>,
}

impl RedisConfig {
    /// Create a new Redis configuration with the given parameters
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
    
    /// Create a Redis configuration from environment variables
    pub fn from_env() -> Self {
        let hostname = env::var("REDIS_HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
        let port = env::var("REDIS_PORT")
            .unwrap_or_else(|_| "6379".to_string())
            .parse::<u16>()
            .unwrap_or(6379);
        let password = env::var("REDIS_PASSWORD").ok();
        
        Self::new(&hostname, port, password.as_deref())
    }
    
    /// Get the Redis URL formatted for connection
    pub fn get_url(&self) -> &str {
        &self.url
    }
}

/// Utility function to get Redis configuration from environment
pub fn get_redis_config() -> (String, String, u16, Option<String>) {
    let config = RedisConfig::from_env();
    (
        config.url,
        config.hostname,
        config.port,
        config.password,
    )
}
