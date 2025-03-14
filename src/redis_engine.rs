// src/redis_engine.rs
use redis::{Client, Connection, Commands, RedisError, RedisResult};
use std::collections::HashMap;
use byteorder::{ByteOrder, LittleEndian};

use crate::error::VectorStoreError;
use crate::config::RedisConfig;
use crate::models::PointStruct;

/// RedisEngine handles all interactions with the Redis database
pub struct RedisEngine {
    client: Client,
    collection_name: String,
}

impl RedisEngine {
    /// Create a new Redis engine with the given configuration and collection name
    pub fn new(config: &RedisConfig, collection_name: &str) -> Result<Self, VectorStoreError> {
        let client = Client::open(config.url.clone())?;
        
        Ok(RedisEngine {
            client,
            collection_name: collection_name.to_string(),
        })
    }
    
    /// Get a Redis connection
    pub fn get_connection(&self) -> Result<Connection, VectorStoreError> {
        let conn = self.client.get_connection()?;
        Ok(conn)
    }
    
    /// Serialize a vector to bytes
    pub fn serialize_vector(vector: &[f64]) -> Vec<u8> {
        let mut bytes = vec![0u8; vector.len() * 8];
        for (i, &val) in vector.iter().enumerate() {
            LittleEndian::write_f64(&mut bytes[i * 8..(i + 1) * 8], val);
        }
        bytes
    }
    
    /// Deserialize bytes to a vector
    pub fn deserialize_vector(bytes: &[u8]) -> Vec<f64> {
        let mut vector = Vec::with_capacity(bytes.len() / 8);
        for chunk in bytes.chunks(8) {
            if chunk.len() == 8 {
                vector.push(LittleEndian::read_f64(chunk));
            }
        }
        vector
    }
    
    /// Create a collection (index) in Redis
    pub fn create_collection(&self) -> Result<(), VectorStoreError> {
        let mut conn = self.get_connection()?;
        
        // Check if the index already exists
        let index_exists: Result<String, RedisError> = redis::cmd("FT.INFO")
            .arg(&self.collection_name)
            .query(&mut conn);
            
        if index_exists.is_ok() {
            println!("Collection '{}' already exists.", self.collection_name);
            return Ok(());
        }
        
        println!("Collection '{}' does not exist. Creating collection...", self.collection_name);
        
        // Create the index with vector search capabilities
        let create_result: RedisResult<String> = redis::cmd("FT.CREATE")
            .arg(&self.collection_name)
            .arg("ON")
            .arg("HASH")
            .arg("PREFIX")
            .arg("1")
            .arg(format!("{}:", self.collection_name))
            .arg("SCHEMA")
            .arg("id")
            .arg("TAG")
            .arg("vector")
            .arg("VECTOR")
            .arg("FLAT")
            .arg("6")
            .arg("TYPE")
            .arg("FLOAT64")
            .arg("DIM")
            .arg("768")
            .arg("DISTANCE_METRIC")
            .arg("COSINE")
            .arg("metadata_json_id")
            .arg("TAG")
            .query(&mut conn);
            
        match create_result {
            Ok(_) => {
                println!("Collection '{}' created successfully.", self.collection_name);
                
                // Create an empty metadata document in RedisJSON
                let empty_metadata = r#"{"content":"","metadata":{"uri":"","chunk_id":0,"source":""}}"#;
                
                let metadata_id = format!("metadata:{}:empty", self.collection_name);
                
                let json_set_result: RedisResult<String> = redis::cmd("JSON.SET")
                    .arg(&metadata_id)
                    .arg("$")
                    .arg(empty_metadata)
                    .query(&mut conn);
                    
                match json_set_result {
                    Ok(_) => {
                        println!("Empty metadata document created with ID: {}", metadata_id);
                        
                        // Create a zero vector as placeholder
                        let vector_id = format!("{}:empty", self.collection_name);
                        let zero_vector = vec![0.0; 768];
                        let vector_bytes = Self::serialize_vector(&zero_vector);
                        
                        let mut hash_map = HashMap::new();
                        hash_map.insert("vector".to_string(), vector_bytes);
                        // Clone metadata_id before consuming it with into_bytes()
                        hash_map.insert("metadata_json_id".to_string(), metadata_id.clone().into_bytes());
                        
                        // Convert HashMap to Vec of tuples for hset_multiple
                        let hash_vec: Vec<(String, Vec<u8>)> = hash_map.into_iter().collect();
                        let _: () = conn.hset_multiple(&vector_id, &hash_vec)?;
                        
                        println!("Metadata pointer set for the collection '{}' to {}", self.collection_name, metadata_id);
                        Ok(())
                    },
                    Err(e) => {
                        println!("Error creating empty metadata: {}", e);
                        Err(VectorStoreError::from(e))
                    }
                }
            },
            Err(e) => {
                println!("Error creating collection: {}", e);
                Err(VectorStoreError::from(e))
            }
        }
    }
    
    /// Delete a collection
    pub fn delete_collection(&self) -> Result<(), VectorStoreError> {
        let mut conn = self.get_connection()?;
        
        // Drop the index
        let drop_result: RedisResult<String> = redis::cmd("FT.DROPINDEX")
            .arg(&self.collection_name)
            .arg("DD") // DELETE_DOCUMENTS
            .query(&mut conn);
            
        match drop_result {
            Ok(_) => {
                println!("Collection '{}' index deleted successfully.", self.collection_name);
                
                // Delete the empty metadata
                let metadata_id = format!("metadata:{}:empty", self.collection_name);
                let _: () = redis::cmd("JSON.DEL")
                    .arg(&metadata_id)
                    .arg("$")
                    .query(&mut conn)?;
                    
                println!("Metadata for collection '{}' deleted successfully.", self.collection_name);
                
                Ok(())
            },
            Err(e) => {
                println!("Error deleting RediSearch index for collection '{}': {}", self.collection_name, e);
                Err(VectorStoreError::from(e))
            }
        }
    }
    
    /// Get vector by ID
    pub fn get_vector(&self, vector_id: &str) -> Result<Option<PointStruct>, VectorStoreError> {
        let mut conn = self.get_connection()?;
        
        let full_id = format!("{}:{}", self.collection_name, vector_id);
        
        // Get the vector data
        let vector_data: HashMap<String, Vec<u8>> = conn.hgetall(&full_id)?;
        
        if vector_data.is_empty() {
            return Ok(None);
        }
        
        // Deserialize the vector
        let vector_bytes = vector_data.get("vector")
            .ok_or_else(|| VectorStoreError::Other("Vector not found in Redis hash".to_string()))?;
        let vector = Self::deserialize_vector(vector_bytes);
        
        // Get the metadata JSON ID
        let metadata_json_id = String::from_utf8(
            vector_data.get("metadata_json_id")
                .ok_or_else(|| VectorStoreError::Other("Metadata ID not found in Redis hash".to_string()))?
                .clone()
        ).map_err(|e| VectorStoreError::Other(format!("Invalid UTF-8 in metadata ID: {}", e)))?;
        
        // Get the metadata JSON
        let metadata_json: String = redis::cmd("JSON.GET")
            .arg(&metadata_json_id)
            .arg("$")
            .query(&mut conn)?;
        
        // Parse the metadata
        let payload: crate::models::Payload = serde_json::from_str(&metadata_json)?;
        
        Ok(Some(PointStruct {
            id: vector_id.to_string(),
            vector,
            payload,
        }))
    }
    
    /// Get collection info
    pub fn get_collection_info(&self) -> Result<HashMap<String, serde_json::Value>, VectorStoreError> {
        let mut conn = self.get_connection()?;
        
        let mut result = HashMap::new();
        result.insert("collection_name".to_string(), serde_json::Value::String(self.collection_name.clone()));
        
        // Check if the index exists
        let index_exists: Result<String, RedisError> = redis::cmd("FT.INFO")
            .arg(&self.collection_name)
            .query(&mut conn);
            
        result.insert("index_exists".to_string(), serde_json::Value::Bool(index_exists.is_ok()));
        
        // Check if the metadata exists
        let metadata_id = format!("metadata:{}:empty", self.collection_name);
        let metadata_exists: Result<String, RedisError> = redis::cmd("JSON.GET")
            .arg(&metadata_id)
            .arg("$")
            .query(&mut conn);
            
        result.insert("metadata_exists".to_string(), serde_json::Value::Bool(metadata_exists.is_ok()));
        
        // Count documents
        let document_count = if index_exists.is_ok() {
            // Use FT.SEARCH with LIMIT 0 0 to just get the count
            let search_result: String = redis::cmd("FT.SEARCH")
                .arg(&self.collection_name)
                .arg("*")
                .arg("LIMIT")
                .arg("0")
                .arg("0")
                .query(&mut conn)?;
                
            // Parse the first line of the result which should be the count
            search_result.lines().next()
                .and_then(|line| line.parse::<i64>().ok())
                .unwrap_or(0)
        } else {
            0
        };
        
        result.insert("document_count".to_string(), serde_json::Value::Number(serde_json::Number::from(document_count)));
        
        Ok(result)
    }
    
    /// Add vector and metadata
    pub fn add_vector_and_metadata(&self, point: &PointStruct) -> Result<(String, String), VectorStoreError> {
        // Create the collection if it doesn't exist
        self.create_collection()?;
        
        let mut conn = self.get_connection()?;
        
        // Generate key names
        let vector_id = point.id.clone();
        let metadata_id = format!("metadata:{}", vector_id);
        let vector_key = format!("{}:{}", self.collection_name, vector_id);
        
        // Store the vector
        let vector_bytes = Self::serialize_vector(&point.vector);
        
        let mut hash_map = HashMap::new();
        hash_map.insert("vector".to_string(), vector_bytes);
        hash_map.insert("metadata_json_id".to_string(), metadata_id.clone().into_bytes());
        
        // Convert HashMap to Vec of tuples for hset_multiple
        let hash_vec: Vec<(String, Vec<u8>)> = hash_map.into_iter().collect();
        let _: () = conn.hset_multiple(&vector_key, &hash_vec)?;
        
        // Store the metadata as JSON
        let metadata_json = serde_json::to_string(&point.payload)?;
        
        let _: () = redis::cmd("JSON.SET")
            .arg(&metadata_id)
            .arg("$")
            .arg(metadata_json)
            .query(&mut conn)?;
            
        Ok((vector_id, metadata_id))
    }
    
    /// Delete vector and metadata
    pub fn delete_vector_and_metadata(&self, vector_id: &str) -> Result<(), VectorStoreError> {
        let mut conn = self.get_connection()?;
        
        // Delete the vector
        let vector_key = format!("{}:{}", self.collection_name, vector_id);
        let _: () = conn.del(&vector_key)?;
        
        // Delete the metadata
        let metadata_id = format!("metadata:{}", vector_id);
        let _: () = redis::cmd("JSON.DEL")
            .arg(&metadata_id)
            .arg("$")
            .query(&mut conn)?;
            
        Ok(())
    }
}

// Helper functions

/// Get UUID from vector
pub fn get_uuid(vector: &[f64]) -> String {
    use uuid::Uuid;
    let vector_str = format!("{:?}", vector);
    Uuid::new_v5(&Uuid::NAMESPACE_DNS, vector_str.as_bytes()).to_string()
}

/// Serialize a vector to bytes
pub fn serialize_vector(vector: &[f64]) -> Vec<u8> {
    RedisEngine::serialize_vector(vector)
}

/// Deserialize bytes to a vector
pub fn deserialize_vector(bytes: &[u8]) -> Vec<f64> {
    RedisEngine::deserialize_vector(bytes)
}