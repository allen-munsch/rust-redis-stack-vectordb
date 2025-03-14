use std::sync::Arc;
use tokio::task;
use futures::future::join_all;
use serde_json::from_slice;

use crate::error::VectorStoreError;
use crate::models::PointStruct;
use crate::config::RedisConfig;
use crate::redis_engine::RedisEngine;

/// GCS operations class
pub struct GcsOperations {
    bucket_name: String,
}

impl GcsOperations {
    /// Create a new GCS operations instance
    pub fn new(bucket_name: &str) -> Self {
        GcsOperations {
            bucket_name: bucket_name.to_string(),
        }
    }
    
    /// List blobs with a given prefix
    pub async fn list_blobs(&self, prefix: &str) -> Result<Vec<String>, VectorStoreError> {
        // This is a stub implementation
        // In a real implementation, you would use the Google Cloud Storage API to list blobs
        println!("Listing blobs in bucket {} with prefix {}", self.bucket_name, prefix);
        Ok(Vec::new())
    }
    
    /// Get blob data
    pub async fn get_blob(&self, blob_name: &str) -> Result<Vec<u8>, VectorStoreError> {
        // This is a stub implementation
        // In a real implementation, you would use the Google Cloud Storage API to fetch blob data
        println!("Getting blob {} from bucket {}", blob_name, self.bucket_name);
        Ok(Vec::new())
    }
}

/// Load vectors from GCS
pub async fn load_vectors_from_gcs(
    redis_config: &RedisConfig,
    collection_name: &str,
    gcs_bucket: &str
) -> Result<(), VectorStoreError> {
    // Create Redis engine
    let redis_engine = RedisEngine::new(redis_config, collection_name)?;
    
    // Ensure the collection exists
    redis_engine.create_collection()?;
    
    // Create GCS operations
    let gcs_ops = GcsOperations::new(gcs_bucket);
    
    // List blobs with the collection name as prefix
    let gcs_prefix = format!("{}/", collection_name);
    let blob_names = gcs_ops.list_blobs(&gcs_prefix).await?;
    
    println!("Found {} files in GCS bucket", blob_names.len());
    
    // Create thread-safe versions of the engines
    let redis_engine = Arc::new(tokio::sync::Mutex::new(redis_engine));
    let gcs_ops = Arc::new(gcs_ops);
    
    // Process files concurrently
    let mut tasks = Vec::new();
    
    for blob_name in blob_names {
        let gcs_ops_clone = Arc::clone(&gcs_ops);
        let redis_engine_clone = Arc::clone(&redis_engine);
        let blob_name_clone = blob_name.clone();
        
        let task = task::spawn(async move {
            println!("Loading embeddings from {}...", blob_name_clone);
            
            // Get blob data
            match gcs_ops_clone.get_blob(&blob_name_clone).await {
                Ok(data) => {
                    // Parse JSON to PointStruct
                    match from_slice::<PointStruct>(&data) {
                        Ok(point) => {
                            // Add vector to Redis
                            let engine = redis_engine_clone.lock().await;
                            match engine.add_vector_and_metadata(&point) {
                                Ok((id, _)) => {
                                    println!("Inserted vector and metadata for {} into Redis", id);
                                    Ok(())
                                },
                                Err(e) => {
                                    println!("Error inserting vector: {}", e);
                                    Err(e)
                                }
                            }
                        },
                        Err(e) => {
                            println!("Error parsing JSON from {}: {}", blob_name_clone, e);
                            Err(VectorStoreError::DeserializationError(e.to_string()))
                        }
                    }
                },
                Err(e) => {
                    println!("Error downloading {}: {}", blob_name_clone, e);
                    Err(e)
                }
            }
        });
        
        tasks.push(task);
    }
    
    // Wait for all tasks to complete
    let results = join_all(tasks).await;
    
    // Check for errors
    for result in results {
        match result {
            Ok(task_result) => {
                if let Err(e) = task_result {
                    println!("Task error: {}", e);
                }
            },
            Err(e) => {
                println!("Task join error: {}", e);
            }
        }
    }
    
    Ok(())
}