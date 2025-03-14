use std::sync::Arc;
use redis_vector_store::{
    RedisConfig,
    redis_vector_store_driver::{
        RedisStackVectorStoreDriver, VectorStoreDriver, get_redis_vector_store_driver
    },
    google_embedding_driver::get_embedding_driver
};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    
    // Get Redis configuration from environment
    let redis_config = RedisConfig::from_env();
    
    // Create embedding driver
    let embedding_driver = Arc::new(get_embedding_driver(
        "models/embedding-001", 
        std::env::var("GOOGLE_API_KEY").ok().as_deref()
    ));
    
    // Create vector store driver
    let collection_name = "test_collection";
    let vector_store = get_redis_vector_store_driver(
        redis_config.clone(),
        collection_name,
        embedding_driver
    );
    
    println!("Initializing collection...");
    vector_store.initialize().await?;
    
    // Example 1: Insert vectors
    println!("\nInserting test vectors...");
    
    // Insert first vector with metadata
    let vector1 = vec![0.1, 0.2, 0.3, 0.4]; // In practice, this would be 768 dimensions
    let meta1 = json!({
        "source": "document1.txt",
        "page": 1,
        "gcs_uri": "gs://bucket/document1.txt"
    });
    let id1 = vector_store.upsert_vector(
        vector1,
        None,
        Some("test_namespace"),
        Some(meta1),
        Some("This is test document 1")
    ).await?;
    println!("Inserted vector 1 with ID: {}", id1);
    
    // Insert second vector
    let vector2 = vec![0.2, 0.3, 0.4, 0.5];
    let meta2 = json!({
        "source": "document2.txt",
        "page": 2,
        "gcs_uri": "gs://bucket/document2.txt"
    });
    let id2 = vector_store.upsert_vector(
        vector2,
        None,
        Some("test_namespace"),
        Some(meta2),
        Some("This is test document 2")
    ).await?;
    println!("Inserted vector 2 with ID: {}", id2);
    
    // Example 2: Query similar vectors
    println!("\nQuerying similar vectors...");
    let query_text = "test document";
    let results = vector_store.query(
        query_text,
        Some(5),
        true,
        Some("test_namespace"),
        None
    ).await?;
    
    println!("Found {} results for query: '{}'", results.len(), query_text);
    for (i, result) in results.iter().enumerate() {
        println!("Result {}: ID={}, Score={}", i+1, result.id, result.score);
        
        // Print first few vector components if included
        if !result.vector.is_empty() {
            println!("  Vector (first 3 elements): {:?}", &result.vector[0..3.min(result.vector.len())]);
        }
        
        // Get content from metadata if available
        if let Some(content) = result.meta.get("content").and_then(|c| c.as_str()) {
            println!("  Content: {}", content);
        }
    }
    
    // Example 3: Load specific entries
    println!("\nLoading specific entries...");
    let entry1 = vector_store.load_entry(&id1, None).await?;
    if let Some(entry) = entry1 {
        println!("Loaded entry with ID: {}", entry.id);
        println!("Vector length: {}", entry.vector.len());
        println!("Metadata: {}", serde_json::to_string_pretty(&entry.meta)?);
    }
    
    // Example 4: Load multiple entries
    println!("\nLoading multiple entries...");
    let entries = vector_store.load_entries(None, Some(vec![id1.clone(), id2.clone()])).await?;
    println!("Loaded {} entries", entries.len());
    
    // Example 5: Delete a vector
    println!("\nDeleting vector...");
    vector_store.delete_vector(&id1).await?;
    println!("Deleted vector with ID: {}", id1);
    
    // Try to load the deleted vector
    let deleted_entry = vector_store.load_entry(&id1, None).await?;
    if deleted_entry.is_none() {
        println!("Vector {} was successfully deleted", id1);
    }
    
    println!("\nExample completed successfully!");
    Ok(())
}