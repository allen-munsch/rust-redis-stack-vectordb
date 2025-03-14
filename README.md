# Redis Vector Store

A Rust implementation of a vector store using Redis for high-performance distributed vector similarity search and storage, with a convenient driver interface for use in other applications.

Embeddings cold storage, and embeddings hot reloading.

- Related: https://redis.io/blog/benchmarking-results-for-vector-databases/

## Features

- Store and retrieve high-dimensional vectors with associated metadata
- Create and manage Redis collections for vector data
- Serialize/deserialize vectors for efficient storage
- Integration with Google Cloud Storage for loading vector data
- Vector similarity search with KNN queries
- Easy-to-use driver interface for integration with other libraries
- Google embedding API integration (placeholder implementation)
- Built-in Redis Stack compatibility
- Redis Insight
- QdrantDB data compatibility

## Requirements

- Rust 1.60 or higher
- Redis Stack with RediSearch and RedisJSON modules
- For GCS integration: Google Cloud credentials configured

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
redis_vector_store = "0.1.0"
```

## Redis Setup

This library requires Redis with the RediSearch and RedisJSON modules. The easiest way is to use Redis Stack with Docker:

```bash
docker compose up -d
```

With a docker-compose.yml configuration:

```yaml
services:
  redis:
    image: redis/redis-stack:latest
    ports:
      - "6379:6379"  # Redis port
      - "8001:8001"  # RedisInsight (web UI) port
    volumes:
      - redis-data:/data

volumes:
  redis-data:
```

## Usage

### Basic Example

```rust
use redis_vector_store::{
    RedisConfig, PointStruct, Payload, Metadata,
    create_collection, get_collection,
    add_vector_and_metadata, get_vector
};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get Redis config from environment variables
    let redis_config = RedisConfig::from_env();
    
    // Create a collection
    let collection_name = "test_collection";
    create_collection(&redis_config, collection_name)?;
    
    // Create vector and metadata
    let vector = vec![0.1, 0.2, 0.3]; // In real usage, use 768-dimensional vector
    let metadata = Metadata::new("example_uri", 1, "example_source");
    let content = "This is a test document";
    let payload = Payload::new(content, metadata);
    let point = PointStruct::new("test_id", vector.clone(), payload);
    
    // Add vector to Redis
    let (vector_id, metadata_id) = add_vector_and_metadata(&redis_config, &point, collection_name)?;
    println!("Vector added with ID: {}", vector_id);
    
    // Retrieve the vector
    if let Ok(Some(retrieved_point)) = get_vector(&redis_config, &vector_id, Some(collection_name)) {
        println!("Retrieved vector with ID: {}", retrieved_point.id);
        println!("Content: {}", retrieved_point.payload.content);
    }
    
    Ok(())
}
```

### Using the Vector Store Driver

The driver provides a higher-level interface:

```rust
use std::sync::Arc;
use redis_vector_store::{
    RedisConfig,
    get_redis_vector_store_driver,
    get_embedding_driver,
    VectorStoreDriver
};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get Redis configuration
    let redis_config = RedisConfig::from_env();
    
    // Create embedding driver (for converting text to vectors)
    let embedding_driver = Arc::new(get_embedding_driver(
        "models/embedding-001", 
        std::env::var("GOOGLE_API_KEY").ok().as_deref()
    ));
    
    // Create vector store driver
    let collection_name = "my_collection";
    let vector_store = get_redis_vector_store_driver(
        redis_config,
        collection_name,
        embedding_driver
    );
    
    // Initialize the collection
    vector_store.initialize().await?;
    
    // Store a document with metadata
    let meta = json!({
        "source": "document.txt",
        "page": 1,
        "gcs_uri": "gs://bucket/document.txt"
    });
    
    let id = vector_store.upsert_vector(
        vec![0.1, 0.2, 0.3], // In practice, use a 768-dimensional vector
        None, // Auto-generate ID
        Some("my_namespace"),
        Some(meta),
        Some("This is a test document")
    ).await?;
    
    println!("Stored vector with ID: {}", id);
    
    // Query for similar documents
    let results = vector_store.query(
        "test document", // Query text
        Some(5),         // Return top 5 results
        true,            // Include vectors in results
        Some("my_namespace"),
        None             // Let the embedding driver convert the query to a vector
    ).await?;
    
    println!("Found {} results", results.len());
    for result in results {
        println!("ID: {}, Score: {}", result.id, result.score);
    }
    
    Ok(())
}
```

### Implementing a Custom Embedding Driver

You can implement your own embedding driver by implementing the `EmbeddingDriver` trait:

```rust
use async_trait::async_trait;
use redis_vector_store::{EmbeddingDriver, VectorStoreError};

struct MyEmbeddingDriver;

#[async_trait]
impl EmbeddingDriver for MyEmbeddingDriver {
    async fn embed_string(&self, text: &str) -> Result<Vec<f64>, VectorStoreError> {
        // Implement your embedding logic here
        // For example, call an external API or use a local model
        
        // Return a vector
        Ok(vec![0.1, 0.2, 0.3]) // Placeholder
    }
}
```

## Examples

The crate includes several examples that demonstrate different features:

- `basic`: Core functionality for creating collections and storing vectors
- `deletion`: How to delete vectors and collections
- `distributed_vector_search`: Using Redis for distributed vector search
- `embedding_driver`: Working with custom embedding drivers

You can run these examples with:

```bash
cargo run --example basic
cargo run --example deletion
cargo run --example distributed_vector_search
cargo run --example embedding_driver
```

## Environment Variables

The library uses the following environment variables:

- `REDIS_HOSTNAME`: Redis server hostname (default: "localhost")
- `REDIS_PORT`: Redis server port (default: 6379)
- `REDIS_PASSWORD`: Redis server password (optional)
- `GOOGLE_API_KEY`: Google API key for embedding (optional)

## Working with Redis

To inspect your vectors in Redis, you can use the Redis CLI:

```bash
# Connect to Redis
redis-cli

# List all keys in your collection
SCAN 0 MATCH test_collection:* COUNT 100

# View a specific vector
HGETALL test_collection:test_id

# Get metadata JSON
# First get the metadata ID
HGET test_collection:test_id metadata_json_id
# Then use the ID to get the JSON
JSON.GET metadata:test_id
```

You can also use RedisInsight, a web-based UI included with Redis Stack, by accessing http://localhost:8001 in your browser.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
