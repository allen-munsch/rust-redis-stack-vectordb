use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Metadata associated with a vector
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Metadata {
    pub uri: String,
    pub chunk_id: usize,
    pub source: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl Metadata {
    /// Create a new metadata object
    pub fn new(uri: &str, chunk_id: usize, source: &str) -> Self {
        Metadata {
            uri: uri.to_string(),
            chunk_id,
            source: source.to_string(),
            extra: HashMap::new(),
        }
    }
    
    /// Add extra metadata fields
    pub fn with_extra(mut self, key: &str, value: serde_json::Value) -> Self {
        self.extra.insert(key.to_string(), value);
        self
    }
    
    /// Set the URI
    pub fn with_uri(mut self, uri: &str) -> Self {
        self.uri = uri.to_string();
        self
    }
    
    /// Set the chunk ID
    pub fn with_chunk_id(mut self, chunk_id: usize) -> Self {
        self.chunk_id = chunk_id;
        self
    }
    
    /// Set the source
    pub fn with_source(mut self, source: &str) -> Self {
        self.source = source.to_string();
        self
    }
}

/// Payload containing content and metadata
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Payload {
    pub content: String,
    pub metadata: Metadata,
}

impl Payload {
    /// Create a new payload
    pub fn new(content: &str, metadata: Metadata) -> Self {
        Payload {
            content: content.to_string(),
            metadata,
        }
    }
    
    /// Set the content
    pub fn with_content(mut self, content: &str) -> Self {
        self.content = content.to_string();
        self
    }
    
    /// Set the metadata
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Point structure representing a vector and its metadata
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PointStruct {
    pub id: String,
    pub vector: Vec<f64>,
    pub payload: Payload,
}

impl PointStruct {
    /// Create a new point
    pub fn new(id: &str, vector: Vec<f64>, payload: Payload) -> Self {
        PointStruct {
            id: id.to_string(),
            vector,
            payload,
        }
    }
    
    /// Create a point from a vector, content, and metadata
    pub fn create(vector: Vec<f64>, content: &str, metadata: Metadata) -> Self {
        let vector_str = format!("{:?}", vector);
        let vector_id = Uuid::new_v5(&Uuid::NAMESPACE_DNS, vector_str.as_bytes()).to_string();
        
        let payload = Payload::new(content, metadata);
        
        PointStruct {
            id: vector_id,
            vector,
            payload,
        }
    }
    
    /// Set the vector
    pub fn with_vector(mut self, vector: Vec<f64>) -> Self {
        self.vector = vector;
        self
    }
    
    /// Set the payload
    pub fn with_payload(mut self, payload: Payload) -> Self {
        self.payload = payload;
        self
    }
}
