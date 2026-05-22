use thiserror::Error;

#[derive(Error, Debug)]
pub enum VectorStoreError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
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
