use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::redis_vector_store_driver::EmbeddingDriver;
use crate::error::VectorStoreError;

/// Google Generative Language API embedding driver.
///
/// Uses the `models/text-embedding-004` endpoint (or any compatible model).
/// Falls back to a deterministic pseudo-embedding when no API key is provided,
/// which is useful for testing but NOT suitable for production.
pub struct GoogleEmbeddingDriver {
    model: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct EmbeddingRequest {
    content: EmbeddingContent,
}

#[derive(Serialize)]
struct EmbeddingContent {
    parts: Vec<EmbeddingPart>,
}

#[derive(Serialize)]
struct EmbeddingPart {
    text: String,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    embedding: Option<EmbeddingValues>,
}

#[derive(Deserialize)]
struct EmbeddingValues {
    values: Vec<f64>,
}

impl GoogleEmbeddingDriver {
    /// Create a new Google Embedding Driver.
    ///
    /// `model` should be the full model path, e.g. `"models/text-embedding-004"`.
    /// `api_key` is optional — if not set, the driver produces deterministic pseudo-embeddings.
    pub fn new(model: &str, api_key: Option<&str>) -> Self {
        Self {
            model: model.to_string(),
            api_key: api_key.map(String::from),
            client: reqwest::Client::new(),
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

#[async_trait]
impl EmbeddingDriver for GoogleEmbeddingDriver {
    async fn embed_string(&self, text: &str) -> Result<Vec<f64>, VectorStoreError> {
        let api_key = match &self.api_key {
            Some(key) => key.clone(),
            None => {
                return Ok(deterministic_fallback(text, 768));
            }
        };

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/{}:embedContent?key={}",
            self.model, api_key
        );

        let request_body = EmbeddingRequest {
            content: EmbeddingContent {
                parts: vec![EmbeddingPart {
                    text: text.to_string(),
                }],
            },
        };

        let response = self.client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| VectorStoreError::Other(format!("API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(VectorStoreError::Other(format!(
                "API error {}: {}",
                status, body
            )));
        }

        let embedding_response: EmbeddingResponse = response
            .json()
            .await
            .map_err(|e| VectorStoreError::Other(format!(
                "Failed to parse API response: {}", e
            )))?;

        embedding_response
            .embedding
            .map(|e| e.values)
            .ok_or_else(|| VectorStoreError::Other("API response missing embedding".to_string()))
    }
}

/// Deterministic fallback embedding for testing without an API key.
fn deterministic_fallback(text: &str, dim: usize) -> Vec<f64> {
    let mut vec = Vec::with_capacity(dim);
    let bytes = text.as_bytes();
    for i in 0..dim {
        let idx = i % bytes.len().max(1);
        let seed = bytes[idx] as f64 / 255.0;
        let phase = (i as f64 * 0.0174533) + (seed * std::f64::consts::PI);
        vec.push(phase.sin() * 0.5 + 0.5);
    }
    vec
}

/// Create a Google Embedding Driver with the given model and optional API key.
pub fn get_embedding_driver(model: &str, api_key: Option<&str>) -> GoogleEmbeddingDriver {
    GoogleEmbeddingDriver::new(model, api_key)
}
