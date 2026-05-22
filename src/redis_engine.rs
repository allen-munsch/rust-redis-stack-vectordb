use redis::{Client, RedisResult};
use redis::aio::ConnectionManager;
use std::collections::HashMap;
use byteorder::{ByteOrder, LittleEndian};

use crate::error::VectorStoreError;
use crate::config::RedisConfig;
use crate::models::PointStruct;

pub const DEFAULT_VECTOR_DIM: usize = 768;

pub struct RedisEngine {
    conn: ConnectionManager,
    collection_name: String,
    vector_dim: usize,
}

impl RedisEngine {
    pub async fn new(config: &RedisConfig, collection_name: &str) -> Result<Self, VectorStoreError> {
        let client = Client::open(config.url.clone())?;
        let conn = ConnectionManager::new(client).await?;
        Ok(RedisEngine {
            conn,
            collection_name: collection_name.to_string(),
            vector_dim: DEFAULT_VECTOR_DIM,
        })
    }

    pub async fn with_dim(config: &RedisConfig, collection_name: &str, vector_dim: usize) -> Result<Self, VectorStoreError> {
        let client = Client::open(config.url.clone())?;
        let conn = ConnectionManager::new(client).await?;
        Ok(RedisEngine {
            conn,
            collection_name: collection_name.to_string(),
            vector_dim,
        })
    }

    pub fn serialize_vector(vector: &[f64]) -> Vec<u8> {
        let mut bytes = vec![0u8; vector.len() * 8];
        for (i, &val) in vector.iter().enumerate() {
            LittleEndian::write_f64(&mut bytes[i * 8..(i + 1) * 8], val);
        }
        bytes
    }

    pub fn deserialize_vector(bytes: &[u8]) -> Vec<f64> {
        let mut vector = Vec::with_capacity(bytes.len() / 8);
        for chunk in bytes.chunks(8) {
            if chunk.len() == 8 {
                vector.push(LittleEndian::read_f64(chunk));
            }
        }
        vector
    }

    /// Create a RediSearch index for the collection with vector search capability.
    /// Schema: vector (FLOAT64), namespace (TAG for filtering), metadata_json_id (TAG).
    pub async fn create_collection(&self) -> Result<(), VectorStoreError> {
        let mut conn = self.conn.clone();

        let index_exists: RedisResult<redis::Value> = redis::cmd("FT.INFO")
            .arg(&self.collection_name)
            .query_async(&mut conn)
            .await;

        if index_exists.is_ok() {
            return Ok(());
        }

        let dim_str = self.vector_dim.to_string();
        redis::cmd("FT.CREATE")
            .arg(&self.collection_name)
            .arg("ON")
            .arg("HASH")
            .arg("PREFIX")
            .arg("1")
            .arg(format!("{}:", self.collection_name))
            .arg("SCHEMA")
            .arg("vector")
            .arg("VECTOR")
            .arg("FLAT")
            .arg("6")
            .arg("TYPE")
            .arg("FLOAT64")
            .arg("DIM")
            .arg(&dim_str)
            .arg("DISTANCE_METRIC")
            .arg("COSINE")
            .arg("namespace")
            .arg("TAG")
            .arg("SEPARATOR")
            .arg("|")
            .arg("metadata_json_id")
            .arg("TAG")
            .query_async::<()>(&mut conn)
            .await?;

        Ok(())
    }

    pub async fn delete_collection(&self) -> Result<(), VectorStoreError> {
        let mut conn = self.conn.clone();

        let drop_result: RedisResult<()> = redis::cmd("FT.DROPINDEX")
            .arg(&self.collection_name)
            .arg("DD")
            .query_async(&mut conn)
            .await;

        if drop_result.is_ok() {
            // Clean up orphaned metadata keys (best effort)
            let metadata_id = format!("metadata:{}:empty", self.collection_name);
            let _: RedisResult<()> = redis::cmd("JSON.DEL")
                .arg(&metadata_id)
                .arg("$")
                .query_async(&mut conn)
                .await;
        }

        drop_result.map_err(VectorStoreError::from)
    }

    /// Get vector and its payload by ID.
    /// JSON.GET without `$` path returns the bare JSON object directly.
    pub async fn get_vector(&self, vector_id: &str) -> Result<Option<PointStruct>, VectorStoreError> {
        let mut conn = self.conn.clone();
        let full_id = format!("{}:{}", self.collection_name, vector_id);

        let exists: bool = redis::cmd("EXISTS")
            .arg(&full_id)
            .query_async(&mut conn)
            .await?;

        if !exists {
            return Ok(None);
        }

        let vector_data: HashMap<String, Vec<u8>> = redis::cmd("HGETALL")
            .arg(&full_id)
            .query_async(&mut conn)
            .await?;

        if vector_data.is_empty() {
            return Ok(None);
        }

        let vector_bytes = vector_data.get("vector")
            .ok_or_else(|| VectorStoreError::Other("Vector field not found in Redis hash".to_string()))?;
        let vector = Self::deserialize_vector(vector_bytes);

        let metadata_json_id_bytes = vector_data.get("metadata_json_id")
            .ok_or_else(|| VectorStoreError::Other("metadata_json_id field not found in Redis hash".to_string()))?;
        let metadata_json_id = String::from_utf8(metadata_json_id_bytes.clone())
            .map_err(|e| VectorStoreError::Other(format!("Invalid UTF-8 in metadata ID: {}", e)))?;

        let metadata_json: String = redis::cmd("JSON.GET")
            .arg(&metadata_json_id)
            .query_async(&mut conn)
            .await?;

        // JSON.GET returns either an array (with $ path) or a bare object (without $)
        let payload: crate::models::Payload = if metadata_json.trim_start().starts_with('[') {
            let arr: Vec<crate::models::Payload> = serde_json::from_str(&metadata_json)?;
            arr.into_iter().next()
                .ok_or_else(|| VectorStoreError::Other("Empty JSON array in metadata".to_string()))?
        } else {
            serde_json::from_str(&metadata_json)?
        };

        Ok(Some(PointStruct {
            id: vector_id.to_string(),
            vector,
            payload,
        }))
    }

    pub async fn get_collection_info(&self) -> Result<HashMap<String, serde_json::Value>, VectorStoreError> {
        let mut conn = self.conn.clone();
        let mut result = HashMap::new();

        result.insert("collection_name".to_string(), serde_json::Value::String(self.collection_name.clone()));

        let index_exists: RedisResult<redis::Value> = redis::cmd("FT.INFO")
            .arg(&self.collection_name)
            .query_async(&mut conn)
            .await;

        result.insert("index_exists".to_string(), serde_json::Value::Bool(index_exists.is_ok()));

        let document_count = if let Ok(search_result) = redis::cmd("FT.SEARCH")
            .arg(&self.collection_name)
            .arg("*")
            .arg("LIMIT")
            .arg("0")
            .arg("0")
            .query_async::<redis::Value>(&mut conn)
            .await
        {
            match search_result {
                redis::Value::Int(count) => count,
                redis::Value::Array(ref items) if !items.is_empty() => {
                    match &items[0] {
                        redis::Value::Int(count) => *count,
                        _ => 0,
                    }
                },
                _ => 0,
            }
        } else {
            0
        };

        result.insert("document_count".to_string(), serde_json::Value::Number(serde_json::Number::from(document_count)));
        Ok(result)
    }

    pub async fn add_vector_and_metadata(&self, point: &PointStruct, namespace: Option<&str>) -> Result<(String, String), VectorStoreError> {
        self.create_collection().await?;

        if point.vector.len() != self.vector_dim {
            return Err(VectorStoreError::Other(format!(
                "Vector dimension mismatch: expected {}, got {}",
                self.vector_dim,
                point.vector.len()
            )));
        }

        let mut conn = self.conn.clone();
        let vector_id = point.id.clone();
        let metadata_id = format!("metadata:{}", vector_id);
        let vector_key = format!("{}:{}", self.collection_name, vector_id);

        let vector_bytes = Self::serialize_vector(&point.vector);

        let mut hash_map: HashMap<String, Vec<u8>> = HashMap::new();
        hash_map.insert("vector".to_string(), vector_bytes);
        hash_map.insert("metadata_json_id".to_string(), metadata_id.clone().into_bytes());
        if let Some(ns) = namespace {
            hash_map.insert("namespace".to_string(), ns.to_string().into_bytes());
        }

        let hash_vec: Vec<(String, Vec<u8>)> = hash_map.into_iter().collect();
        redis::cmd("HSET")
            .arg(&vector_key)
            .arg(&hash_vec)
            .query_async::<()>(&mut conn)
            .await?;

        let metadata_json = serde_json::to_string(&point.payload)?;
        redis::cmd("JSON.SET")
            .arg(&metadata_id)
            .arg("$")
            .arg(&metadata_json)
            .query_async::<()>(&mut conn)
            .await?;

        Ok((vector_id, metadata_id))
    }

    pub async fn delete_vector_and_metadata(&self, vector_id: &str) -> Result<(), VectorStoreError> {
        let mut conn = self.conn.clone();

        let vector_key = format!("{}:{}", self.collection_name, vector_id);
        let _: () = redis::cmd("DEL")
            .arg(&vector_key)
            .query_async(&mut conn)
            .await?;

        let metadata_id = format!("metadata:{}", vector_id);
        let _: RedisResult<()> = redis::cmd("JSON.DEL")
            .arg(&metadata_id)
            .arg("$")
            .query_async(&mut conn)
            .await;

        Ok(())
    }

    /// Execute a KNN vector search query.
    /// Returns (id, score, metadata_json_id) tuples for efficient batch metadata loading.
    pub async fn search_knn(
        &self,
        query_vector: &[f64],
        count: usize,
        namespace_filter: Option<&str>,
    ) -> Result<Vec<(String, f64, String)>, VectorStoreError> {
        let mut conn = self.conn.clone();

        let filter = match namespace_filter {
            Some(ns) if !ns.is_empty() => format!("@namespace:{{{}}}", ns),
            _ => "*".to_string(),
        };

        let knn_query = format!("({})=>[KNN $K @vector $vec AS vector_score]", filter);

        let vector_bytes = Self::serialize_vector(query_vector);
        let k_str = count.to_string();

        let result: redis::Value = redis::cmd("FT.SEARCH")
            .arg(&self.collection_name)
            .arg(&knn_query)
            .arg("PARAMS")
            .arg("4")
            .arg("vec")
            .arg(&vector_bytes)
            .arg("K")
            .arg(&k_str)
            .arg("RETURN")
            .arg("2")
            .arg("vector_score")
            .arg("metadata_json_id")
            .arg("SORTBY")
            .arg("vector_score")
            .arg("ASC")
            .arg("LIMIT")
            .arg("0")
            .arg(&k_str)
            .arg("DIALECT")
            .arg("2")
            .query_async(&mut conn)
            .await?;

        self.parse_knn_results(result)
    }

    fn parse_knn_results(&self, value: redis::Value) -> Result<Vec<(String, f64, String)>, VectorStoreError> {
        let items = match value {
            redis::Value::Array(items) => items,
            _ => return Ok(Vec::new()),
        };

        if items.len() < 2 {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();

        for i in (1..items.len()).step_by(2) {
            if i + 1 >= items.len() {
                break;
            }

            let doc_id = match &items[i] {
                redis::Value::BulkString(bytes) => String::from_utf8_lossy(bytes).to_string(),
                redis::Value::SimpleString(s) => s.clone(),
                _ => continue,
            };

            let prefix = format!("{}:", self.collection_name);
            let id = doc_id.strip_prefix(&prefix).unwrap_or(&doc_id).to_string();

            let fields = match &items[i + 1] {
                redis::Value::Array(field_values) => field_values,
                _ => continue,
            };

            let mut score = f64::NAN;
            let mut metadata_json_id = String::new();
            for j in (0..fields.len()).step_by(2) {
                if j + 1 >= fields.len() {
                    break;
                }
                let field_name = match &fields[j] {
                    redis::Value::BulkString(bytes) => String::from_utf8_lossy(bytes).to_string(),
                    redis::Value::SimpleString(s) => s.clone(),
                    _ => continue,
                };
                match field_name.as_str() {
                    "vector_score" => {
                        score = match &fields[j + 1] {
                            redis::Value::BulkString(bytes) => {
                                String::from_utf8_lossy(bytes).parse::<f64>().unwrap_or(f64::NAN)
                            },
                            redis::Value::SimpleString(s) => s.parse::<f64>().unwrap_or(f64::NAN),
                            redis::Value::Double(d) => *d,
                            _ => f64::NAN,
                        };
                    }
                    "metadata_json_id" => {
                        metadata_json_id = match &fields[j + 1] {
                            redis::Value::BulkString(bytes) => String::from_utf8_lossy(bytes).to_string(),
                            redis::Value::SimpleString(s) => s.clone(),
                            _ => String::new(),
                        };
                    }
                    _ => {}
                }
            }

            if !score.is_nan() {
                results.push((id, score, metadata_json_id));
            }
        }

        Ok(results)
    }

    /// Batch-fetch vectors and their payloads by metadata_json_id.
    /// Much more efficient than individual get_vector calls.
    pub async fn get_vectors_batch(
        &self,
        ids_and_scores: &[(String, f64, String)],
        include_vectors: bool,
    ) -> Result<Vec<(String, f64, Option<PointStruct>)>, VectorStoreError> {
        let mut conn = self.conn.clone();
        let mut results = Vec::with_capacity(ids_and_scores.len());

        for (id, score, meta_id) in ids_and_scores {
            let exists: bool = redis::cmd("EXISTS")
                .arg(format!("{}:{}", self.collection_name, id))
                .query_async(&mut conn)
                .await?;

            if !exists {
                results.push((id.clone(), *score, None));
                continue;
            }

            let vector_data: HashMap<String, Vec<u8>> = redis::cmd("HGETALL")
                .arg(format!("{}:{}", self.collection_name, id))
                .query_async(&mut conn)
                .await?;

            let vector_bytes = match vector_data.get("vector") {
                Some(b) => b,
                None => {
                    results.push((id.clone(), *score, None));
                    continue;
                }
            };

            let vector = if include_vectors {
                Self::deserialize_vector(vector_bytes)
            } else {
                Vec::new()
            };

            let metadata_json: String = redis::cmd("JSON.GET")
                .arg(meta_id)
                .query_async(&mut conn)
                .await?;

            let payload: crate::models::Payload = if metadata_json.trim_start().starts_with('[') {
                let arr: Vec<crate::models::Payload> = serde_json::from_str(&metadata_json)?;
                match arr.into_iter().next() {
                    Some(p) => p,
                    None => {
                        results.push((id.clone(), *score, None));
                        continue;
                    }
                }
            } else {
                serde_json::from_str(&metadata_json)?
            };

            results.push((id.clone(), *score, Some(PointStruct {
                id: id.clone(),
                vector,
                payload,
            })));
        }

        Ok(results)
    }
}

pub fn get_uuid(vector: &[f64]) -> String {
    use uuid::Uuid;
    let vector_str = format!("{:?}", vector);
    Uuid::new_v5(&Uuid::NAMESPACE_DNS, vector_str.as_bytes()).to_string()
}

pub fn serialize_vector(vector: &[f64]) -> Vec<u8> {
    RedisEngine::serialize_vector(vector)
}

pub fn deserialize_vector(bytes: &[u8]) -> Vec<f64> {
    RedisEngine::deserialize_vector(bytes)
}
