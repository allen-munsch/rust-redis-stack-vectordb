// src/redis_vector_store_driver.rs
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use async_trait::async_trait;

use crate::{
    RedisConfig, PointStruct, Payload, Metadata,
    create_collection, get_collection, get_vector, 
    add_vector_and_metadata, delete_vector_and_metadata,
    serialize_vector, VectorStoreError,
    redis_engine::RedisEngine,
};

const DEFAULT_DISTANCE: &str = "Cosine";
const CONTENT_PAYLOAD_KEY: &str = "data";

/// Convert string to float, handling errors and "nan" values
fn convert_to_float(value: &str) -> f64 {
    if value == "nan" {
        return 0.0;
    }
    value.parse::<f64>().unwrap_or(0.0)
}

/// Entry structure for vector store results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// Unique identifier for the vector entry
    pub id: String,
    
    /// The vector data
    pub vector: Vec<f64>,
    
    /// Similarity score from searches
    pub score: f64,
    
    /// Metadata associated with the vector
    pub meta: serde_json::Value,
}

impl Entry {
    /// Create a new Entry
    pub fn new(id: &str, vector: Vec<f64>, score: f64, meta: serde_json::Value) -> Self {
        Self {
            id: id.to_string(),
            vector,
            score,
            meta,
        }
    }
}

/// Trait defining the vector store driver interface
#[async_trait]
pub trait VectorStoreDriver: Send + Sync {
    /// Delete a vector by its ID
    async fn delete_vector(&self, vector_id: &str) -> Result<(), VectorStoreError>;
    
    /// Insert or update a vector with metadata
    async fn upsert_vector(
        &self,
        vector: Vec<f64>,
        vector_id: Option<&str>,
        namespace: Option<&str>,
        meta: Option<serde_json::Value>,
        content: Option<&str>,
    ) -> Result<String, VectorStoreError>;
    
    /// Query for similar vectors
    async fn query(
        &self,
        query: &str,
        count: Option<usize>,
        include_vectors: bool,
        namespace: Option<&str>,
        query_vector: Option<Vec<f64>>,
    ) -> Result<Vec<Entry>, VectorStoreError>;
    
    /// Load a single entry by ID
    async fn load_entry(&self, vector_id: &str, _namespace: Option<&str>) -> Result<Option<Entry>, VectorStoreError>;
    
    /// Load multiple entries by IDs
    async fn load_entries(&self, namespace: Option<&str>, ids: Option<Vec<String>>) -> Result<Vec<Entry>, VectorStoreError>;
}

/// Embedding driver trait for vector embedding
#[async_trait]
pub trait EmbeddingDriver: Send + Sync {
    /// Embed a string into a vector
    async fn embed_string(&self, text: &str) -> Result<Vec<f64>, VectorStoreError>;
}

/// Redis Stack Vector Store Driver implementation
pub struct RedisStackVectorStoreDriver {
    /// Redis configuration
    redis_config: RedisConfig,
    
    /// Collection name in Redis
    collection_name: String,
    
    /// Embedding driver for generating vectors from text
    embedding_driver: Arc<dyn EmbeddingDriver>,
    
    /// Key name for content in payload
    content_payload_key: String,
}

impl RedisStackVectorStoreDriver {
    /// Create a new Redis Stack Vector Store Driver
    pub fn new(
        redis_config: RedisConfig,
        collection_name: &str,
        embedding_driver: Arc<dyn EmbeddingDriver>,
        content_payload_key: Option<&str>,
    ) -> Self {
        Self {
            redis_config,
            collection_name: collection_name.to_string(),
            embedding_driver,
            content_payload_key: content_payload_key.unwrap_or(CONTENT_PAYLOAD_KEY).to_string(),
        }
    }
    
    /// Initialize the collection if it doesn't exist
    pub async fn initialize(&self) -> Result<(), VectorStoreError> {
        create_collection(&self.redis_config, &self.collection_name)
    }
    
    /// Get the redis engine instance
    fn get_engine(&self) -> Result<RedisEngine, VectorStoreError> {
        RedisEngine::new(&self.redis_config, &self.collection_name)
    }
}

#[async_trait]
impl VectorStoreDriver for RedisStackVectorStoreDriver {
    async fn delete_vector(&self, vector_id: &str) -> Result<(), VectorStoreError> {
        delete_vector_and_metadata(&self.redis_config, vector_id, &self.collection_name)
    }
    
    async fn upsert_vector(
        &self,
        vector: Vec<f64>,
        vector_id: Option<&str>,
        namespace: Option<&str>,
        meta: Option<serde_json::Value>,
        content: Option<&str>,
    ) -> Result<String, VectorStoreError> {
        let mut metadata_map = match meta {
            Some(value) => {
                if let serde_json::Value::Object(map) = value {
                    let mut result = std::collections::HashMap::new();
                    for (k, v) in map {
                        result.insert(k, v);
                    }
                    result
                } else {
                    std::collections::HashMap::new()
                }
            },
            None => std::collections::HashMap::new(),
        };
        
        // Set default values if not present
        if !metadata_map.contains_key("uri") {
            metadata_map.insert(
                "uri".to_string(), 
                serde_json::Value::String("".to_string())
            );
        }
        
        if !metadata_map.contains_key("chunk_id") {
            metadata_map.insert(
                "chunk_id".to_string(), 
                serde_json::Value::Number(serde_json::Number::from(0))
            );
        }
        
        // Add namespace if provided
        if let Some(ns) = namespace {
            metadata_map.insert(
                "namespace".to_string(), 
                serde_json::Value::String(ns.to_string())
            );
        }
        
        // Extract required fields for Metadata struct
        let uri = metadata_map
            .get("uri")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
            
        let chunk_id = metadata_map
            .get("chunk_id")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
            
        let source = metadata_map
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        
        // Create metadata and payload
        let metadata = Metadata::new(&uri, chunk_id, &source);
        let content_str = content.unwrap_or("").to_string();
        let payload = Payload::new(&content_str, metadata);
        
        // Create point with optional ID
        let point = match vector_id {
            Some(id) => PointStruct::new(id, vector, payload),
            None => {
                let uuid = crate::get_uuid(&vector);
                PointStruct::new(&uuid, vector, payload)
            }
        };
        
        // Add to Redis
        let (vector_id, _) = add_vector_and_metadata(&self.redis_config, &point, &self.collection_name)?;
        Ok(vector_id)
    }
    
    async fn query(
        &self,
        query: &str,
        count: Option<usize>,
        include_vectors: bool,
        namespace: Option<&str>,
        query_vector: Option<Vec<f64>>,
    ) -> Result<Vec<Entry>, VectorStoreError> {
        // First, check if collection exists
        match get_collection(&self.redis_config, &self.collection_name) {
            Ok(_) => {}, // Collection exists
            Err(_) => {
                // Try to initialize collection
                self.initialize().await?;
            }
        }
        
        // Get query vector either from parameter or by embedding the query string
        let vector = match query_vector {
            Some(v) => v,
            None => self.embedding_driver.embed_string(query).await?,
        };
        
        // Get redis engine
        let engine = self.get_engine()?;
        let mut conn = engine.get_connection()?;
        
        // Create the filter expression for namespace if needed
        let filter_expr = if let Some(ns) = namespace {
            format!("(@namespace:{{{}}})", ns)
        } else {
            "*".to_string()
        };
        
        // Build KNN query
        let knn_query = format!("({})=>[KNN {} @vector $vec AS vector_score]", 
            filter_expr, 
            count.unwrap_or(10)
        );
        
        // Prepare query parameters
        let vector_bytes = serialize_vector(&vector);
        
        // Execute the search query
        let mut entries = Vec::new();
        
        // Execute RediSearch query
        let cmd_result = redis::cmd("FT.SEARCH")
            .arg(&self.collection_name)
            .arg(knn_query)
            .arg("PARAMS")
            .arg("2")
            .arg("vec")
            .arg(vector_bytes)
            .arg("RETURN")
            .arg(if include_vectors { "2" } else { "1" })
            .arg("vector_score")
            .arg(if include_vectors { "vector" } else { "" })
            .arg("SORTBY")
            .arg("vector_score")
            .arg("LIMIT")
            .arg("0")
            .arg(count.unwrap_or(10).to_string())
            .query(&mut conn);
        
        match cmd_result {
            Ok(result) => {
                // Parse the search results
                // RediSearch returns a structure like:
                // [total_results, "doc_id1", ["field1", "value1", "field2", "value2"], "doc_id2", [...], ...]
                let search_results: Vec<redis::Value> = result;
                
                if search_results.len() < 2 {
                    return Ok(entries); // No results found
                }
                
                // Skip the first element (total count) and process the rest
                for i in (1..search_results.len()).step_by(2) {
                    if i + 1 >= search_results.len() {
                        break;
                    }
                    
                    // Extract document ID
                    let doc_id = match &search_results[i] {
                        redis::Value::Data(bytes) => {
                            String::from_utf8_lossy(bytes).to_string()
                        },
                        redis::Value::Status(s) => s.clone(),
                        _ => continue,
                    };
                    
                    // Extract ID without collection prefix
                    let id_parts: Vec<&str> = doc_id.split(':').collect();
                    let id = if id_parts.len() > 1 { id_parts[1] } else { &doc_id }.to_string();
                    
                    // Extract fields
                    let fields = match &search_results[i + 1] {
                        redis::Value::Bulk(field_values) => field_values,
                        _ => continue,
                    };
                    
                    // Process fields to extract score and vector
                    let mut score = 0.0;
                    let mut vector_data = Vec::new();
                    
                    for j in (0..fields.len()).step_by(2) {
                        if j + 1 >= fields.len() {
                            break;
                        }
                        
                        let field_name = match &fields[j] {
                            redis::Value::Data(bytes) => String::from_utf8_lossy(bytes).to_string(),
                            redis::Value::Status(s) => s.clone(),
                            _ => continue,
                        };
                        
                        if field_name == "vector_score" {
                            // Parse the score
                            score = match &fields[j + 1] {
                                redis::Value::Data(bytes) => {
                                    let score_str = String::from_utf8_lossy(bytes);
                                    convert_to_float(&score_str)
                                },
                                redis::Value::Status(s) => convert_to_float(s),
                                _ => 0.0,
                            };
                        } else if field_name == "vector" && include_vectors {
                            // Parse the vector bytes
                            match &fields[j + 1] {
                                redis::Value::Data(bytes) => {
                                    vector_data = RedisEngine::deserialize_vector(bytes);
                                },
                                _ => {},
                            }
                        }
                    }
                    
                    // Load the full document to get metadata
                    if let Ok(Some(point)) = get_vector(&self.redis_config, &id, Some(&self.collection_name)) {
                        // Use the vector from the point if we didn't get it from the search results
                        let entry_vector = if include_vectors && !vector_data.is_empty() {
                            vector_data
                        } else if include_vectors {
                            point.vector.clone()
                        } else {
                            Vec::new()
                        };
                        
                        // Convert payload to serde_json::Value
                        let meta = serde_json::to_value(&point.payload)?;
                        
                        // Create entry
                        let entry = Entry::new(&id, entry_vector, score, meta);
                        entries.push(entry);
                    }
                }
                
                Ok(entries)
            },
            Err(e) => {
                eprintln!("Error executing search query: {}", e);
                // If we got an error, try using individual document lookups as a fallback
                // This covers cases where RediSearch isn't fully set up or there are syntax issues
                
                // For this fallback, we'll take a naive approach - just get the most recent documents
                // In a real implementation, you might want a better approach
                let max_docs = count.unwrap_or(10);
                let keys_pattern = format!("{}:*", self.collection_name);
                
                let key_result: Result<Vec<String>, redis::RedisError> = redis::cmd("SCAN")
                    .arg("0")
                    .arg("MATCH")
                    .arg(keys_pattern)
                    .arg("COUNT")
                    .arg(max_docs)
                    .query(&mut conn);
                
                if let Ok(keys) = key_result {
                    // Skip the cursor value (first element) and use the second element (array of keys)
                    if keys.len() >= 2 {
                        // Access keys[1] directly instead of trying to create a redis::Value
                        if let Ok(key_bulk) = redis::cmd("SCAN")
                            .arg("0")
                            .arg("MATCH")
                            .arg(format!("{}:{}", self.collection_name, keys[1]))
                            .arg("COUNT")
                            .arg(max_docs)
                            .query::<(String, Vec<String>)>(&mut conn)
                        {
                            for key in key_bulk.1 {
                                let key_parts: Vec<&str> = key.split(':').collect();
                                if key_parts.len() > 1 {
                                    let id = key_parts[1].to_string();
                                    if let Ok(Some(entry)) = self.load_entry(&id, namespace).await {
                                        entries.push(entry);
                                        if entries.len() >= max_docs {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                Ok(entries)
            }
        }
    }
    
    async fn load_entry(&self, vector_id: &str, _namespace: Option<&str>) -> Result<Option<Entry>, VectorStoreError> {
        // Load vector entry from Redis
        match get_vector(&self.redis_config, vector_id, Some(&self.collection_name)) {
            Ok(Some(data)) => {
                // Convert payload to serde_json::Value
                let meta = serde_json::to_value(&data.payload)?;
                
                // Create and return the entry
                Ok(Some(Entry::new(&data.id, data.vector, 0.0, meta)))
            },
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
    
    async fn load_entries(&self, namespace: Option<&str>, ids: Option<Vec<String>>) -> Result<Vec<Entry>, VectorStoreError> {
        let vector_ids = ids.unwrap_or_default();
        let mut entries = Vec::new();
        
        // Load each vector entry
        for id in vector_ids {
            if let Ok(Some(entry)) = self.load_entry(&id, namespace).await {
                entries.push(entry);
            }
        }
        
        Ok(entries)
    }
}

/// Helper function to create a new Redis Vector Store Driver
pub fn get_redis_vector_store_driver(
    redis_config: RedisConfig,
    collection_name: &str,
    embedding_driver: Arc<dyn EmbeddingDriver>,
) -> RedisStackVectorStoreDriver {
    RedisStackVectorStoreDriver::new(
        redis_config,
        collection_name,
        embedding_driver,
        None,
    )
}