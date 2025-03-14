mod config;
mod error;
mod models;
mod redis_engine;
mod gcs;
pub mod redis_vector_store_driver;
pub mod google_embedding_driver;

pub use config::RedisConfig;
pub use error::VectorStoreError;
pub use models::{PointStruct, Payload, Metadata};
pub use redis_engine::RedisEngine;
pub use gcs::GcsOperations;

// Re-export functions for easier access
pub use redis_engine::{get_uuid, serialize_vector, deserialize_vector};
pub use gcs::load_vectors_from_gcs;

// This function mimics the create_collection function in the Python code
pub fn create_collection(redis_config: &RedisConfig, collection_name: &str) -> Result<(), VectorStoreError> {
    let engine = RedisEngine::new(redis_config, collection_name)?;
    engine.create_collection()
}

// This function mimics the delete_collection function in the Python code
pub fn delete_collection(redis_config: &RedisConfig, collection_name: &str) -> Result<(), VectorStoreError> {
    let engine = RedisEngine::new(redis_config, collection_name)?;
    engine.delete_collection()
}

// This function mimics the get_collection function in the Python code
pub fn get_collection(redis_config: &RedisConfig, collection_name: &str) -> Result<serde_json::Value, VectorStoreError> {
    let engine = RedisEngine::new(redis_config, collection_name)?;
    let info = engine.get_collection_info()?;
    Ok(serde_json::to_value(info)?)
}

// This function mimics the get_vector function in the Python code
pub fn get_vector(redis_config: &RedisConfig, vector_id: &str, collection_name: Option<&str>) -> Result<Option<PointStruct>, VectorStoreError> {
    let (actual_collection, actual_id) = if vector_id.contains(':') {
        let parts: Vec<&str> = vector_id.split(':').collect();
        (parts[0], parts[1])
    } else {
        (collection_name.unwrap_or("empty"), vector_id)
    };
    
    let engine = RedisEngine::new(redis_config, actual_collection)?;
    engine.get_vector(actual_id)
}

// This function mimics the add_vector_and_metadata function in the Python code
pub fn add_vector_and_metadata(redis_config: &RedisConfig, point: &PointStruct, collection_name: &str) -> Result<(String, String), VectorStoreError> {
    let engine = RedisEngine::new(redis_config, collection_name)?;
    engine.add_vector_and_metadata(point)
}

// This function mimics the delete_vector_and_metadata function in the Python code
pub fn delete_vector_and_metadata(redis_config: &RedisConfig, vector_id: &str, collection_name: &str) -> Result<(), VectorStoreError> {
    let engine = RedisEngine::new(redis_config, collection_name)?;
    engine.delete_vector_and_metadata(vector_id)
}

// Re-export key driver components for easier access
pub use redis_vector_store_driver::{
    VectorStoreDriver, 
    EmbeddingDriver,
    Entry,
    get_redis_vector_store_driver
};

pub use google_embedding_driver::get_embedding_driver;