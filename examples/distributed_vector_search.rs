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

    let collection_name = "test_collection";

    let _ = delete_collection(&redis_config, collection_name).await;

    println!("\n--- Creating collection ---");
    create_collection(&redis_config, collection_name).await?;
    println!("Collection created successfully");

    println!("\n--- Adding vector and metadata ---");
    let vector = vec![0.0f64; 768];

    let metadata = Metadata::new("example_uri", 1, "example_source");
    let content = "This is a test document";
    let payload = Payload::new(content, metadata);
    let point = PointStruct::new("test_id", vector.clone(), payload);

    match add_vector_and_metadata(&redis_config, &point, collection_name, None).await {
        Ok((vector_id, metadata_id)) => {
            println!("Vector added with ID: {}", vector_id);
            println!("Metadata stored with ID: {}", metadata_id);

            println!("\n--- Getting vector ---");
            match get_vector(&redis_config, &vector_id, Some(collection_name)).await {
                Ok(Some(retrieved_point)) => {
                    println!("Retrieved vector with ID: {}", retrieved_point.id);
                    println!("Content: {}", retrieved_point.payload.content);
                    println!("Metadata URI: {}", retrieved_point.payload.metadata.uri);
                    println!("Vector dimension: {}", retrieved_point.vector.len());
                },
                Ok(None) => println!("Vector not found"),
                Err(e) => println!("Error retrieving vector: {}", e),
            }
        },
        Err(e) => println!("Error adding vector: {}", e),
    }

    println!("\n--- Getting collection info ---");
    match get_collection(&redis_config, collection_name).await {
        Ok(info) => {
            println!("Collection info:");
            println!("  Name: {}", info["collection_name"]);
            println!("  Index exists: {}", info["index_exists"]);
            println!("  Document count: {}", info["document_count"]);
        },
        Err(e) => println!("Error getting collection info: {}", e),
    }

    println!("\n--- Cleaning up ---");
    delete_vector_and_metadata(&redis_config, "test_id", collection_name).await?;
    delete_collection(&redis_config, collection_name).await?;
    println!("Cleanup complete");

    println!("\nAll examples completed successfully!");
    Ok(())
}
