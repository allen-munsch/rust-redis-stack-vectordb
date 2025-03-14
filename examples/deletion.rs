use redis_vector_store::{
    RedisConfig, PointStruct, Payload, Metadata,
    get_collection, get_vector,
    delete_vector_and_metadata, delete_collection
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    
    // Get Redis config from environment variables
    let redis_config = RedisConfig::from_env();
    println!("Connected to Redis at {}:{}", redis_config.hostname, redis_config.port);
    
    let collection_name = "test_collection";
    
    // First, check if the collection exists
    println!("\n--- Checking collection ---");
    match get_collection(&redis_config, collection_name) {
        Ok(info) => {
            println!("Collection found:");
            println!("  Name: {}", info["collection_name"]);
            println!("  Document count: {}", info["document_count"]);
        },
        Err(e) => {
            println!("Error finding collection: {}", e);
            println!("Please run the basic example first to create a collection.");
            return Ok(());
        }
    }
    
    // Example 1: Delete a specific vector by ID
    println!("\n--- Deleting specific vector ---");
    let vector_id = "test_id"; // ID used in the basic example
    
    // First, verify the vector exists
    match get_vector(&redis_config, vector_id, Some(collection_name)) {
        Ok(Some(_)) => {
            println!("Vector found, proceeding with deletion");
            
            // Now delete it
            match delete_vector_and_metadata(&redis_config, vector_id, collection_name) {
                Ok(_) => println!("Vector and its metadata deleted successfully"),
                Err(e) => println!("Error deleting vector: {}", e),
            }
        },
        Ok(None) => println!("Vector with ID '{}' not found, skipping deletion", vector_id),
        Err(e) => println!("Error retrieving vector: {}", e),
    }
    
    // Example 2: Delete the entire collection
    println!("\n--- Deleting collection (uncomment to enable) ---");
    println!("WARNING: This will delete the entire '{}' collection!", collection_name);
    
    // Uncomment the following block to actually delete the collection
    /*
    println!("Proceeding with collection deletion...");
    match delete_collection(&redis_config, collection_name) {
        Ok(_) => println!("Collection deleted successfully"),
        Err(e) => println!("Error deleting collection: {}", e),
    }
    */
    
    println!("\nDeletion examples completed!");
    println!("Note: Collection deletion is commented out by default for safety.");
    println!("      Uncomment the code to perform actual collection deletion.");
    Ok(())
}