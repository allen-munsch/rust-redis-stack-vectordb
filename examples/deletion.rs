use redis_vector_store::{
    RedisConfig, PointStruct, Payload, Metadata,
    create_collection, get_collection,
    add_vector_and_metadata, get_vector,
    delete_vector_and_metadata, delete_collection,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let redis_config = RedisConfig::from_env();
    println!("Connected to Redis at {}:{}", redis_config.hostname, redis_config.port);

    let collection_name = "test_deletion_collection";

    let _ = delete_collection(&redis_config, collection_name).await;

    println!("\n--- Creating collection and adding test data ---");
    create_collection(&redis_config, collection_name).await?;

    let vector = vec![0.0f64; 768];
    let metadata = Metadata::new("test_uri", 0, "test_source");
    let payload = Payload::new("test content", metadata);
    let point = PointStruct::new("test_id", vector, payload);

    add_vector_and_metadata(&redis_config, &point, collection_name, None).await?;
    println!("Test data added");

    match get_collection(&redis_config, collection_name).await {
        Ok(info) => {
            println!("Collection found:");
            println!("  Name: {}", info["collection_name"]);
            println!("  Document count: {}", info["document_count"]);
        },
        Err(e) => {
            println!("Error finding collection: {}", e);
            return Ok(());
        }
    }

    println!("\n--- Deleting specific vector ---");
    let vector_id = "test_id";
    match get_vector(&redis_config, vector_id, Some(collection_name)).await {
        Ok(Some(_)) => {
            println!("Vector found, proceeding with deletion");
            delete_vector_and_metadata(&redis_config, vector_id, collection_name).await?;
            println!("Vector and its metadata deleted successfully");

            match get_vector(&redis_config, vector_id, Some(collection_name)).await {
                Ok(None) => println!("Verified: vector no longer exists"),
                Ok(Some(_)) => println!("WARNING: vector still exists after deletion!"),
                Err(e) => println!("Error checking deletion: {}", e),
            }
        },
        Ok(None) => println!("Vector with ID '{}' not found, skipping deletion", vector_id),
        Err(e) => println!("Error retrieving vector: {}", e),
    }

    println!("\n--- Deleting entire collection ---");
    delete_collection(&redis_config, collection_name).await?;
    println!("Collection deleted successfully");

    match get_collection(&redis_config, collection_name).await {
        Ok(info) => {
            if info["index_exists"].as_bool().unwrap_or(true) {
                println!("WARNING: collection index still exists!");
            } else {
                println!("Verified: collection index no longer exists");
            }
        },
        Err(_) => println!("Verified: collection no longer accessible"),
    }

    println!("\nDeletion examples completed!");
    Ok(())
}
