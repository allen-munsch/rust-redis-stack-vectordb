use redis::RedisError;
use std::error::Error;
use std::fmt;

/// Custom error type for the vector store
#[derive(Debug)]
pub enum VectorStoreError {
    /// Errors from the Redis client
    RedisError(RedisError),
    /// Errors from JSON serialization/deserialization
    SerializationError(String),
    /// Errors from JSON deserialization
    DeserializationError(String),
    /// Errors from GCS operations
    GcsError(String),
    /// Other general errors
    Other(String),
}

impl fmt::Display for VectorStoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VectorStoreError::RedisError(e) => write!(f, "Redis error: {}", e),
            VectorStoreError::SerializationError(e) => write!(f, "Serialization error: {}", e),
            VectorStoreError::DeserializationError(e) => write!(f, "Deserialization error: {}", e),
            VectorStoreError::GcsError(e) => write!(f, "GCS error: {}", e),
            VectorStoreError::Other(e) => write!(f, "Error: {}", e),
        }
    }
}

impl Error for VectorStoreError {}

impl From<RedisError> for VectorStoreError {
    fn from(err: RedisError) -> Self {
        VectorStoreError::RedisError(err)
    }
}

impl From<serde_json::Error> for VectorStoreError {
    fn from(err: serde_json::Error) -> Self {
        VectorStoreError::SerializationError(err.to_string())
    }
}

impl From<std::io::Error> for VectorStoreError {
    fn from(err: std::io::Error) -> Self {
        VectorStoreError::Other(err.to_string())
    }
}

impl From<String> for VectorStoreError {
    fn from(err: String) -> Self {
        VectorStoreError::Other(err)
    }
}

impl From<&str> for VectorStoreError {
    fn from(err: &str) -> Self {
        VectorStoreError::Other(err.to_string())
    }
}
