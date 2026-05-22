# Redis Vector Store

A production-ready Rust vector store backed by Redis Stack. Uses RediSearch KNN for similarity search, RedisJSON for metadata, and an async driver interface with pluggable embedding backends.

## Features

- Store and retrieve 768-dimensional (configurable) vectors with rich JSON metadata
- KNN similarity search with namespace filtering via RediSearch tags
- Async driver interface (`VectorStoreDriver` trait) — easy to mock or swap backends
- Pluggable embedding driver (`EmbeddingDriver` trait) — bring your own model
- Built-in Google Generative Language API embedding client (with deterministic fallback for testing)
- Batch upsert, batch delete, and batch metadata fetching
- Deterministic vector ID generation (UUID v5 from vector bytes)
- Redis Stack compatible (RediSearch + RedisJSON)

## Requirements

- Rust 2021 edition (MSRV: 1.60+)
- Redis Stack with RediSearch and RedisJSON modules

## Redis Setup

```bash
docker compose up -d
```

This starts `redis/redis-stack:latest` on port 6379 and RedisInsight on port 8001.

## Installation

```toml
[dependencies]
redis_vector_store = "0.1.0"
```

## Quick Start

### Low-level API

```rust
use redis_vector_store::*;

#[tokio::main]
async fn main() -> Result<(), VectorStoreError> {
    let config = RedisConfig::from_env();

    create_collection(&config, "my_collection").await?;

    let vector = vec![0.0; 768]; // 768-dim embedding
    let point = PointStruct::new(
        "doc1",
        vector,
        Payload::new("hello world", Metadata::new("gs://bucket/doc.txt", 0, "pdf_parser")),
    );
    add_vector_and_metadata(&config, &point, "my_collection", Some("my_ns")).await?;

    let doc = get_vector(&config, "doc1", Some("my_collection")).await?;
    assert!(doc.is_some());

    delete_collection(&config, "my_collection").await?;
    Ok(())
}
```

### High-level Driver API

```rust
use redis_vector_store::*;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), VectorStoreError> {
    let config = RedisConfig::from_env();

    let driver = get_redis_vector_store_driver(
        config,
        "my_collection",
        Arc::new(get_embedding_driver("models/text-embedding-004", None)),
    );
    driver.initialize().await?;

    // Upsert
    let id = driver.upsert_vector(
        vec![0.0; 768],
        None,                    // auto-generate ID
        Some("my_namespace"),
        Some(serde_json::json!({"source": "doc.txt", "page": 1})),
        Some("hello world"),
    ).await?;

    // Search by text (embedded via the driver) or by raw vector
    let results = driver.query(
        "hello world",
        Some(5),
        false,                   // don't include vectors in response
        Some("my_namespace"),    // filter by namespace
        None,                    // let the driver embed the query
    ).await?;

    for entry in results {
        println!("{}: score={:.6}", entry.id, entry.score);
    }

    // Batch operations
    driver.delete_vectors_batch(&[id]).await?;
    delete_collection(&config, "my_collection").await?;
    Ok(())
}
```

### Custom Embedding Driver

```rust
use async_trait::async_trait;
use redis_vector_store::{EmbeddingDriver, VectorStoreError};

struct MyEmbedder;

#[async_trait]
impl EmbeddingDriver for MyEmbedder {
    async fn embed_string(&self, text: &str) -> Result<Vec<f64>, VectorStoreError> {
        // Call your model / API
        Ok(vec![0.0; 768])
    }
}
```

## Running Examples

```bash
cargo run --example distributed_vector_search   # low-level API: create, insert, get, delete
cargo run --example deletion                    # delete vectors and collections
cargo run --example embedding_driver            # driver API: upsert, query, namespace filtering
```

## Running Tests

```bash
cargo test
```

Integration tests require a running Redis Stack instance on localhost:6379.

## Environment Variables

- `REDIS_HOSTNAME` — Redis host (default: `localhost`)
- `REDIS_PORT` — Redis port (default: `6379`)
- `REDIS_PASSWORD` — Redis AUTH password (optional)
- `GOOGLE_API_KEY` — Google API key for the embedding driver (optional; falls back to deterministic pseudo-embeddings)

## Inspecting Data in Redis

```bash
docker exec redis-server redis-cli FT.INFO my_collection    # index info
docker exec redis-server redis-cli KEYS 'my_collection:*'   # list vectors
docker exec redis-server redis-cli HGETALL my_collection:doc1  # view hash
docker exec redis-server redis-cli JSON.GET metadata:doc1   # view metadata
```

## Architecture

```
VectorStoreDriver trait    EmbeddingDriver trait
        │                       │
RedisStackVectorStoreDriver    GoogleEmbeddingDriver (or your own)
        │                       │
    Redis Engine            reqwest → Google API
        │
   Redis Stack
   ├── RediSearch (KNN index)
   └── RedisJSON (metadata)
```

Each collection maps to a RediSearch index with:
- `vector` — FLOAT64 VECTOR field (FLAT, COSINE distance)
- `namespace` — TAG field with separator for filtering
- `metadata_json_id` — TAG field pointing to a separate RedisJSON key

## License

MIT — see [LICENSE](./LICENSE).
