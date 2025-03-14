use redis_vector_store::{
    RedisConfig, PointStruct, Payload, Metadata,
    create_collection, get_collection,
    add_vector_and_metadata, get_vector,
    load_vectors_from_gcs
};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    
    // Get Redis config from environment variables
    let redis_config = RedisConfig::from_env();
    println!("Connected to Redis at {}:{}", redis_config.hostname, redis_config.port);
    
    // Example 1: Create a collection
    let collection_name = "test_collection";
    println!("\n--- Creating collection ---");
    match create_collection(&redis_config, collection_name) {
        Ok(_) => println!("Collection created successfully"),
        Err(e) => println!("Error creating collection: {}", e),
    }
    
    // Example 2: Add a vector and metadata
    println!("\n--- Adding vector and metadata ---");
    let vector = vec![0.1, 0.2, 0.3]; // In real usage, this would be a 768-dimensional vector
    
    // Create metadata
    let metadata = Metadata::new("example_uri", 1, "example_source");
    let mut extra = HashMap::new();
    extra.insert("custom_field".to_string(), serde_json::Value::String("custom_value".to_string()));
    
    // Create point
    let content = "This is a test document";
    let payload = Payload::new(content, metadata);
    let point = PointStruct::new("test_id", vector.clone(), payload);
    
    match add_vector_and_metadata(&redis_config, &point, collection_name) {
        Ok((vector_id, metadata_id)) => {
            println!("Vector added with ID: {}", vector_id);
            println!("Metadata stored with ID: {}", metadata_id);
            
            // Example 3: Get the vector by ID
            println!("\n--- Getting vector ---");
            match get_vector(&redis_config, &vector_id, Some(collection_name)) {
                Ok(Some(retrieved_point)) => {
                    println!("Retrieved vector with ID: {}", retrieved_point.id);
                    println!("Content: {}", retrieved_point.payload.content);
                    println!("Metadata URI: {}", retrieved_point.payload.metadata.uri);
                    println!("Vector (first 3 elements): {:?}", &retrieved_point.vector[0..3.min(retrieved_point.vector.len())]);
                },
                Ok(None) => println!("Vector not found"),
                Err(e) => println!("Error retrieving vector: {}", e),
            }
        },
        Err(e) => println!("Error adding vector: {}", e),
    }
    
    // Example 4: Get collection info
    println!("\n--- Getting collection info ---");
    match get_collection(&redis_config, collection_name) {
        Ok(info) => {
            println!("Collection info:");
            println!("  Name: {}", info["collection_name"]);
            println!("  Index exists: {}", info["index_exists"]);
            println!("  Metadata exists: {}", info["metadata_exists"]);
            println!("  Document count: {}", info["document_count"]);
        },
        Err(e) => println!("Error getting collection info: {}", e),
    }
    
    // Example 5: Using the GCS loader (requires GCP credentials)
    println!("\n--- Loading vectors from GCS (skipped in this example) ---");
    println!("To load vectors from GCS, you would use:");
    println!("load_vectors_from_gcs(&redis_config, collection_name, \"your-gcs-bucket\").await?;");
    
    println!("\nAll examples completed successfully!");
    println!("Note: This example does not delete any data. Use the deletion_examples.rs to clean up when needed.");
    Ok(())
}