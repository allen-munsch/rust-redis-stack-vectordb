use serde::{Deserialize, Serialize};
use std::sync::Arc;
use async_trait::async_trait;

use crate::{
    RedisConfig, PointStruct, Payload, Metadata,
    create_collection, get_collection, get_vector,
    add_vector_and_metadata, delete_vector_and_metadata,
    VectorStoreError,
    redis_engine::RedisEngine,
};

/// A search result entry containing the vector ID, similarity score, and associated metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// Unique identifier for the vector entry.
    pub id: String,
    /// The vector data. Empty unless `include_vectors` was requested in the query.
    pub vector: Vec<f64>,
    /// Similarity score from the search. Lower = more similar when using COSINE distance.
    pub score: f64,
    /// Arbitrary JSON metadata associated with this vector.
    pub meta: serde_json::Value,
}

impl Entry {
    pub fn new(id: &str, vector: Vec<f64>, score: f64, meta: serde_json::Value) -> Self {
        Self {
            id: id.to_string(),
            vector,
            score,
            meta,
        }
    }
}

/// Trait for vector store backends. Implement this to plug in different storage engines.
#[async_trait]
pub trait VectorStoreDriver: Send + Sync {
    /// Delete a single vector by ID.
    async fn delete_vector(&self, vector_id: &str) -> Result<(), VectorStoreError>;

    /// Insert or update a single vector with metadata.
    /// Returns the vector's ID (auto-generated if not provided).
    async fn upsert_vector(
        &self,
        vector: Vec<f64>,
        vector_id: Option<&str>,
        namespace: Option<&str>,
        meta: Option<serde_json::Value>,
        content: Option<&str>,
    ) -> Result<String, VectorStoreError>;

    /// Batch-insert or update multiple vectors.
    async fn upsert_vectors_batch(
        &self,
        vectors: Vec<(Vec<f64>, Option<String>, Option<serde_json::Value>, Option<String>)>,
        namespace: Option<&str>,
    ) -> Result<Vec<String>, VectorStoreError>;

    /// Batch-delete multiple vectors by ID.
    async fn delete_vectors_batch(&self, vector_ids: &[String]) -> Result<(), VectorStoreError>;

    /// Search for similar vectors. Provide either a text `query` (embedded via the driver)
    /// or a raw `query_vector`. Returns results sorted by ascending score (most similar first).
    async fn query(
        &self,
        query: &str,
        count: Option<usize>,
        include_vectors: bool,
        namespace: Option<&str>,
        query_vector: Option<Vec<f64>>,
    ) -> Result<Vec<Entry>, VectorStoreError>;

    /// Load a single entry by ID.
    async fn load_entry(&self, vector_id: &str, namespace: Option<&str>) -> Result<Option<Entry>, VectorStoreError>;

    /// Load multiple entries by ID. If `ids` is `None`, scans all entries in the collection.
    async fn load_entries(&self, namespace: Option<&str>, ids: Option<Vec<String>>) -> Result<Vec<Entry>, VectorStoreError>;
}

/// Trait for embedding models. Implement this to plug in your own text-to-vector service.
#[async_trait]
pub trait EmbeddingDriver: Send + Sync {
    /// Convert a text string into a vector embedding.
    async fn embed_string(&self, text: &str) -> Result<Vec<f64>, VectorStoreError>;
}

/// Redis Stack Vector Store Driver.
///
/// Stores vectors as Redis hashes with JSON metadata, indexed via RediSearch for KNN.
pub struct RedisStackVectorStoreDriver {
    redis_config: RedisConfig,
    collection_name: String,
    embedding_driver: Arc<dyn EmbeddingDriver>,
}

impl RedisStackVectorStoreDriver {
    /// Create a new driver.
    ///
    /// `embedding_driver` is used to convert text queries into vectors.
    pub fn new(
        redis_config: RedisConfig,
        collection_name: &str,
        embedding_driver: Arc<dyn EmbeddingDriver>,
    ) -> Self {
        Self {
            redis_config,
            collection_name: collection_name.to_string(),
            embedding_driver,
        }
    }

    /// Ensure the RediSearch index exists. Idempotent — safe to call multiple times.
    pub async fn initialize(&self) -> Result<(), VectorStoreError> {
        create_collection(&self.redis_config, &self.collection_name).await
    }

    async fn get_engine(&self) -> Result<RedisEngine, VectorStoreError> {
        RedisEngine::new(&self.redis_config, &self.collection_name).await
    }
}

#[async_trait]
impl VectorStoreDriver for RedisStackVectorStoreDriver {
    async fn delete_vector(&self, vector_id: &str) -> Result<(), VectorStoreError> {
        delete_vector_and_metadata(&self.redis_config, vector_id, &self.collection_name).await
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
            Some(serde_json::Value::Object(map)) => {
                let mut result = std::collections::HashMap::new();
                for (k, v) in map {
                    result.insert(k, v);
                }
                result
            },
            _ => std::collections::HashMap::new(),
        };

        // Extract known fields, preserve everything else in extra
        let uri = metadata_map.remove("uri").and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
        let chunk_id = metadata_map.remove("chunk_id").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let source = metadata_map.remove("source").and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
        let content_str = content.unwrap_or("");

        if let Some(ns) = namespace {
            metadata_map.entry("namespace".to_string()).or_insert_with(|| serde_json::Value::String(ns.to_string()));
        }

        // Build Metadata with all extra fields preserved
        let mut metadata = Metadata::new(&uri, chunk_id, &source);
        metadata.extra = metadata_map;

        let payload = Payload::new(content_str, metadata);

        let point = match vector_id {
            Some(id) => PointStruct::new(id, vector, payload),
            None => {
                let uuid = crate::get_uuid(&vector);
                PointStruct::new(&uuid, vector, payload)
            }
        };

        let (vid, _) = add_vector_and_metadata(&self.redis_config, &point, &self.collection_name, namespace).await?;
        Ok(vid)
    }

    async fn upsert_vectors_batch(
        &self,
        vectors: Vec<(Vec<f64>, Option<String>, Option<serde_json::Value>, Option<String>)>,
        namespace: Option<&str>,
    ) -> Result<Vec<String>, VectorStoreError> {
        let mut ids = Vec::with_capacity(vectors.len());
        for (vec, id, meta, content) in vectors {
            let vid = self.upsert_vector(vec, id.as_deref(), namespace, meta, content.as_deref()).await?;
            ids.push(vid);
        }
        Ok(ids)
    }

    async fn delete_vectors_batch(&self, vector_ids: &[String]) -> Result<(), VectorStoreError> {
        for id in vector_ids {
            self.delete_vector(id).await?;
        }
        Ok(())
    }

    async fn query(
        &self,
        query: &str,
        count: Option<usize>,
        include_vectors: bool,
        namespace: Option<&str>,
        query_vector: Option<Vec<f64>>,
    ) -> Result<Vec<Entry>, VectorStoreError> {
        if get_collection(&self.redis_config, &self.collection_name).await.is_err() {
            self.initialize().await?;
        }

        let vector = match query_vector {
            Some(v) => v,
            None => self.embedding_driver.embed_string(query).await?,
        };

        let engine = self.get_engine().await?;
        let count = count.unwrap_or(10);

        // Single KNN query returns (id, score, metadata_json_id)
        let knn_results = engine.search_knn(&vector, count, namespace).await?;
        // Batch-fetch all metadata in one helper call
        let batch = engine.get_vectors_batch(&knn_results, include_vectors).await?;

        let entries: Vec<Entry> = batch
            .into_iter()
            .filter_map(|(id, score, point)| {
                point.map(|p| {
                    let meta = serde_json::to_value(&p.payload).unwrap_or_default();
                    Entry::new(&id, p.vector, score, meta)
                })
            })
            .collect();

        Ok(entries)
    }

    async fn load_entry(&self, vector_id: &str, _namespace: Option<&str>) -> Result<Option<Entry>, VectorStoreError> {
        match get_vector(&self.redis_config, vector_id, Some(&self.collection_name)).await {
            Ok(Some(data)) => {
                let meta = serde_json::to_value(&data.payload)?;
                Ok(Some(Entry::new(&data.id, data.vector, 0.0, meta)))
            },
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn load_entries(&self, namespace: Option<&str>, ids: Option<Vec<String>>) -> Result<Vec<Entry>, VectorStoreError> {
        let vector_ids = ids.unwrap_or_default();
        let mut entries = Vec::with_capacity(vector_ids.len());
        for id in vector_ids {
            if let Ok(Some(entry)) = self.load_entry(&id, namespace).await {
                entries.push(entry);
            }
        }
        Ok(entries)
    }
}

/// Create a new Redis-backed vector store driver with default settings.
pub fn get_redis_vector_store_driver(
    redis_config: RedisConfig,
    collection_name: &str,
    embedding_driver: Arc<dyn EmbeddingDriver>,
) -> RedisStackVectorStoreDriver {
    RedisStackVectorStoreDriver::new(redis_config, collection_name, embedding_driver)
}
