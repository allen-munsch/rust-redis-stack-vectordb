use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Arbitrary key-value metadata attached to each vector.
///
/// `extra` carries any additional fields beyond the three standard ones.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Metadata {
    /// Source URI (e.g. gs://bucket/file.pdf, https://example.com/doc).
    pub uri: String,
    /// Chunk index within the source document.
    pub chunk_id: usize,
    /// Name or type of the source (e.g. "pdf_parser", "web_scraper").
    pub source: String,
    /// Additional free-form metadata fields.
    #[serde(default)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl Metadata {
    pub fn new(uri: &str, chunk_id: usize, source: &str) -> Self {
        Metadata {
            uri: uri.to_string(),
            chunk_id,
            source: source.to_string(),
            extra: HashMap::new(),
        }
    }

    pub fn with_extra(mut self, key: &str, value: serde_json::Value) -> Self {
        self.extra.insert(key.to_string(), value);
        self
    }
}

/// The full document payload stored alongside a vector.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Payload {
    /// Original text content that was embedded.
    pub content: String,
    /// Metadata about the source document.
    pub metadata: Metadata,
}

impl Payload {
    pub fn new(content: &str, metadata: Metadata) -> Self {
        Payload {
            content: content.to_string(),
            metadata,
        }
    }
}

/// A vector with its ID and associated payload.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PointStruct {
    /// Unique identifier for this point.
    pub id: String,
    /// The embedding vector.
    pub vector: Vec<f64>,
    /// The content and metadata.
    pub payload: Payload,
}

impl PointStruct {
    pub fn new(id: &str, vector: Vec<f64>, payload: Payload) -> Self {
        PointStruct {
            id: id.to_string(),
            vector,
            payload,
        }
    }

    /// Create a point with an auto-generated (deterministic) UUID based on the vector content.
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
}
