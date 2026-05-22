//! # Redis Vector Store
//!
//! A high-performance vector store backed by Redis Stack with RediSearch and RedisJSON.
//!
//! ## Quick start
//!
//! ```no_run
//! use redis_vector_store::*;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), VectorStoreError> {
//! let config = RedisConfig::from_env();
//! let driver = get_redis_vector_store_driver(
//!     config,
//!     "my_collection",
//!     Arc::new(get_embedding_driver("models/text-embedding-004", None)),
//! );
//! driver.initialize().await?;
//!
//! // Insert
//! let id = driver.upsert_vector(
//!     vec![0.0; 768], None, Some("my_ns"), None, Some("hello world")
//! ).await?;
//!
//! // Search
//! let results = driver.query("hello", Some(5), false, Some("my_ns"), None).await?;
//! # Ok(())
//! # }
//! ```

mod config;
mod error;
mod models;
mod redis_engine;
pub mod redis_vector_store_driver;
pub mod google_embedding_driver;

pub use config::RedisConfig;
pub use error::VectorStoreError;
pub use models::{PointStruct, Payload, Metadata};
pub use redis_engine::RedisEngine;
pub use redis_engine::{get_uuid, serialize_vector, deserialize_vector, DEFAULT_VECTOR_DIM};

/// Create a new collection with the default vector dimension (768).
pub async fn create_collection(redis_config: &RedisConfig, collection_name: &str) -> Result<(), VectorStoreError> {
    let engine = RedisEngine::new(redis_config, collection_name).await?;
    engine.create_collection().await
}

/// Create a new collection with a custom vector dimension.
pub async fn create_collection_with_dim(
    redis_config: &RedisConfig,
    collection_name: &str,
    vector_dim: usize,
) -> Result<(), VectorStoreError> {
    let engine = RedisEngine::with_dim(redis_config, collection_name, vector_dim).await?;
    engine.create_collection().await
}

/// Delete a collection and all its vectors.
pub async fn delete_collection(redis_config: &RedisConfig, collection_name: &str) -> Result<(), VectorStoreError> {
    let engine = RedisEngine::new(redis_config, collection_name).await?;
    engine.delete_collection().await
}

/// Get collection metadata (name, index status, document count).
pub async fn get_collection(redis_config: &RedisConfig, collection_name: &str) -> Result<serde_json::Value, VectorStoreError> {
    let engine = RedisEngine::new(redis_config, collection_name).await?;
    let info = engine.get_collection_info().await?;
    Ok(serde_json::to_value(info)?)
}

/// Retrieve a single vector and its payload by ID.
pub async fn get_vector(
    redis_config: &RedisConfig,
    vector_id: &str,
    collection_name: Option<&str>,
) -> Result<Option<PointStruct>, VectorStoreError> {
    let (actual_collection, actual_id) = if vector_id.contains(':') {
        let parts: Vec<&str> = vector_id.split(':').collect();
        (parts[0], parts[1])
    } else {
        (collection_name.unwrap_or("empty"), vector_id)
    };

    let engine = RedisEngine::new(redis_config, actual_collection).await?;
    engine.get_vector(actual_id).await
}

/// Insert a vector and its metadata into the collection.
pub async fn add_vector_and_metadata(
    redis_config: &RedisConfig,
    point: &PointStruct,
    collection_name: &str,
    namespace: Option<&str>,
) -> Result<(String, String), VectorStoreError> {
    let engine = RedisEngine::new(redis_config, collection_name).await?;
    engine.add_vector_and_metadata(point, namespace).await
}

/// Delete a vector and its metadata by ID.
pub async fn delete_vector_and_metadata(
    redis_config: &RedisConfig,
    vector_id: &str,
    collection_name: &str,
) -> Result<(), VectorStoreError> {
    let engine = RedisEngine::new(redis_config, collection_name).await?;
    engine.delete_vector_and_metadata(vector_id).await
}

pub use redis_vector_store_driver::{
    VectorStoreDriver,
    EmbeddingDriver,
    Entry,
    get_redis_vector_store_driver
};

pub use google_embedding_driver::get_embedding_driver;
