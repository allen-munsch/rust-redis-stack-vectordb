use std::sync::Arc;
use redis_vector_store::{
    RedisConfig,
    redis_vector_store_driver::{
        VectorStoreDriver, get_redis_vector_store_driver
    },
    google_embedding_driver::get_embedding_driver,
    delete_collection,
};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let redis_config = RedisConfig::from_env();
    let collection_name = "test_driver_collection";

    let _ = delete_collection(&redis_config, collection_name).await;

    let embedding_driver = Arc::new(get_embedding_driver(
        "models/text-embedding-004",
        std::env::var("GOOGLE_API_KEY").ok().as_deref()
    ));

    let vector_store = get_redis_vector_store_driver(
        redis_config.clone(),
        collection_name,
        embedding_driver
    );

    println!("Initializing collection...");
    vector_store.initialize().await?;

    println!("\nInserting test vectors...");
    let vector1: Vec<f64> = (0..768).map(|i| (i as f64 * 0.01).sin()).collect();
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

    let vector2: Vec<f64> = (0..768).map(|i| (i as f64 * 0.02).sin()).collect();
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

    println!("\nQuerying similar vectors (namespace filtered)...");
    let query_vec: Vec<f64> = (0..768).map(|i| ((i as f64 + 5.0) * 0.01).sin()).collect();
    let results = vector_store.query(
        "unused",
        Some(5),
        false,
        Some("test_namespace"),
        Some(query_vec.clone())
    ).await?;

    println!("Found {} results", results.len());
    for (i, result) in results.iter().enumerate() {
        println!("Result {}: ID={}, Score={:.6}", i + 1, result.id, result.score);
        if let Some(content) = result.meta.get("content").and_then(|c| c.as_str()) {
            println!("  Content: {}", content);
        }
    }

    println!("\nQuerying with wrong namespace (should return empty)...");
    let results_empty = vector_store.query(
        "unused",
        Some(5),
        false,
        Some("nonexistent_namespace"),
        Some(query_vec)
    ).await?;
    println!("Found {} results (expected 0)", results_empty.len());

    println!("\nLoading specific entries...");
    let entry1 = vector_store.load_entry(&id1, None).await?;
    if let Some(entry) = entry1 {
        println!("Loaded entry with ID: {}", entry.id);
        println!("Vector length: {}", entry.vector.len());
    }

    println!("\nLoading multiple entries...");
    let entries = vector_store.load_entries(None, Some(vec![id1.clone(), id2.clone()])).await?;
    println!("Loaded {} entries", entries.len());

    println!("\nDeleting vector...");
    vector_store.delete_vector(&id1).await?;
    println!("Deleted vector with ID: {}", id1);

    let deleted_entry = vector_store.load_entry(&id1, None).await?;
    if deleted_entry.is_none() {
        println!("Vector {} was successfully deleted", id1);
    }

    delete_collection(&redis_config, collection_name).await?;

    println!("\nExample completed successfully!");
    Ok(())
}
