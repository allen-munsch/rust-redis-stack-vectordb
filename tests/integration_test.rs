use redis_vector_store::{
    RedisConfig, PointStruct, Payload, Metadata,
    create_collection, delete_collection,
    add_vector_and_metadata, get_vector, get_collection,
    delete_vector_and_metadata,
    serialize_vector, deserialize_vector, get_uuid,
    DEFAULT_VECTOR_DIM,
};

fn redis_config() -> RedisConfig {
    RedisConfig::from_env()
}

fn collection(name: &str) -> String {
    format!("test_int_{}", name)
}

async fn cleanup(name: &str) {
    let _ = delete_collection(&redis_config(), &collection(name)).await;
}

#[tokio::test]
async fn test_vector_serialization_roundtrip() {
    let original = vec![1.0f64, -2.5, 42.0, 0.0, -0.0, f64::MAX, f64::MIN, f64::EPSILON];
    let bytes = serialize_vector(&original);
    let deserialized = deserialize_vector(&bytes);
    assert_eq!(original.len(), deserialized.len());
    for (a, b) in original.iter().zip(deserialized.iter()) {
        assert_eq!(a.to_bits(), b.to_bits(), "mismatch at value {}", a);
    }
}

#[tokio::test]
async fn test_collection_lifecycle() {
    let cn = "lifecycle";
    cleanup(cn).await;
    let config = redis_config();
    let name = collection(cn);

    create_collection(&config, &name).await.expect("create_collection");
    create_collection(&config, &name).await.expect("idempotent create_collection");

    let info = get_collection(&config, &name).await.expect("get_collection");
    assert_eq!(info["collection_name"], name);
    assert_eq!(info["index_exists"], true);
    assert_eq!(info["document_count"], 0);

    delete_collection(&config, &name).await.expect("delete_collection");

    let info = get_collection(&config, &name).await.expect("get_collection after delete");
    assert_eq!(info["index_exists"], false);

    cleanup(cn).await;
}

#[tokio::test]
async fn test_add_and_get_vector() {
    let cn = "addget";
    cleanup(cn).await;
    let config = redis_config();
    let name = collection(cn);
    create_collection(&config, &name).await.unwrap();

    let vector: Vec<f64> = (0..DEFAULT_VECTOR_DIM).map(|i| (i as f64 * 0.01).sin()).collect();
    let metadata = Metadata::new("test://uri", 42, "test_source");
    let payload = Payload::new("hello world", metadata);
    let point = PointStruct::new("doc1", vector.clone(), payload);

    let (id, meta_id) = add_vector_and_metadata(&config, &point, &name, Some("ns1")).await.unwrap();
    assert_eq!(id, "doc1");
    assert!(meta_id.starts_with("metadata:"));

    let retrieved = get_vector(&config, "doc1", Some(&name)).await.unwrap().expect("should exist");
    assert_eq!(retrieved.id, "doc1");
    assert_eq!(retrieved.vector.len(), DEFAULT_VECTOR_DIM);
    assert_eq!(retrieved.payload.content, "hello world");
    assert_eq!(retrieved.payload.metadata.uri, "test://uri");
    assert_eq!(retrieved.payload.metadata.chunk_id, 42);
    assert_eq!(retrieved.payload.metadata.source, "test_source");

    assert!(get_vector(&config, "nonexistent", Some(&name)).await.unwrap().is_none());

    cleanup(cn).await;
}

#[tokio::test]
async fn test_delete_vector() {
    let cn = "delete";
    cleanup(cn).await;
    let config = redis_config();
    let name = collection(cn);
    create_collection(&config, &name).await.unwrap();

    let vector = vec![1.0f64; DEFAULT_VECTOR_DIM];
    let payload = Payload::new("delete me", Metadata::new("uri", 0, "src"));
    let point = PointStruct::new("del1", vector, payload);
    add_vector_and_metadata(&config, &point, &name, None).await.unwrap();

    assert!(get_vector(&config, "del1", Some(&name)).await.unwrap().is_some());

    delete_vector_and_metadata(&config, "del1", &name).await.unwrap();
    assert!(get_vector(&config, "del1", Some(&name)).await.unwrap().is_none());

    cleanup(cn).await;
}

#[tokio::test]
async fn test_knn_search_and_namespace_filtering() {
    use redis_vector_store::redis_vector_store_driver::{
        VectorStoreDriver, EmbeddingDriver, get_redis_vector_store_driver
    };
    use std::sync::Arc;
    use async_trait::async_trait;

    let cn = "knn";
    cleanup(cn).await;
    let config = redis_config();
    let name = collection(cn);

    struct IdentityEmbedder;
    #[async_trait]
    impl EmbeddingDriver for IdentityEmbedder {
        async fn embed_string(&self, _text: &str) -> Result<Vec<f64>, redis_vector_store::VectorStoreError> {
            Ok(vec![])
        }
    }

    let driver = get_redis_vector_store_driver(config.clone(), &name, Arc::new(IdentityEmbedder));
    driver.initialize().await.unwrap();

    let v1: Vec<f64> = (0..DEFAULT_VECTOR_DIM).map(|i| (i as f64 * 0.01).sin()).collect();
    let id1 = driver.upsert_vector(v1.clone(), Some("ns_a_1"), Some("ns_a"), None, Some("doc a1")).await.unwrap();

    let v2: Vec<f64> = (0..DEFAULT_VECTOR_DIM).map(|i| (i as f64 * 0.02).cos()).collect();
    let id2 = driver.upsert_vector(v2.clone(), Some("ns_a_2"), Some("ns_a"), None, Some("doc a2")).await.unwrap();

    let v3: Vec<f64> = (0..DEFAULT_VECTOR_DIM).map(|i| (i as f64 * 0.05).sin()).collect();
    let id3 = driver.upsert_vector(v3.clone(), Some("ns_b_1"), Some("ns_b"), None, Some("doc b1")).await.unwrap();

    let query_v: Vec<f64> = (0..DEFAULT_VECTOR_DIM).map(|i| (i as f64 * 0.015).sin()).collect();

    // Search within ns_a
    let results = driver.query("unused", Some(10), false, Some("ns_a"), Some(query_v.clone())).await.unwrap();
    let ids: Vec<&str> = results.iter().map(|e| e.id.as_str()).collect();
    assert!(ids.contains(&id1.as_str()), "should contain ns_a_1");
    assert!(ids.contains(&id2.as_str()), "should contain ns_a_2");
    assert!(!ids.contains(&id3.as_str()), "should NOT contain ns_b_1 in ns_a filter");

    // Search within ns_b
    let results_b = driver.query("unused", Some(10), false, Some("ns_b"), Some(query_v.clone())).await.unwrap();
    let ids_b: Vec<&str> = results_b.iter().map(|e| e.id.as_str()).collect();
    assert!(ids_b.contains(&id3.as_str()), "should contain ns_b_1");
    assert!(!ids_b.contains(&id1.as_str()), "should NOT contain ns_a_1 in ns_b filter");

    // No filter
    let results_all = driver.query("unused", Some(10), false, None, Some(query_v)).await.unwrap();
    let ids_all: Vec<&str> = results_all.iter().map(|e| e.id.as_str()).collect();
    assert!(ids_all.contains(&id1.as_str()));
    assert!(ids_all.contains(&id2.as_str()));
    assert!(ids_all.contains(&id3.as_str()));

    // load_entry / load_entries
    let entry = driver.load_entry(&id1, None).await.unwrap().expect("load_entry");
    assert_eq!(entry.id, id1);
    assert_eq!(entry.vector.len(), DEFAULT_VECTOR_DIM);

    let entries = driver.load_entries(None, Some(vec![id1.clone(), id2.clone(), id3.clone()])).await.unwrap();
    assert_eq!(entries.len(), 3);

    driver.delete_vector(&id1).await.unwrap();
    assert!(driver.load_entry(&id1, None).await.unwrap().is_none());

    cleanup(cn).await;
}

#[tokio::test]
async fn test_get_uuid_determinism() {
    let v1 = vec![1.0, 2.0, 3.0];
    let v2 = vec![1.0, 2.0, 3.0];
    let v3 = vec![1.0, 2.0, 3.1];
    assert_eq!(get_uuid(&v1), get_uuid(&v2), "same vector should produce same UUID");
    assert_ne!(get_uuid(&v1), get_uuid(&v3), "different vector should produce different UUID");
}

#[tokio::test]
async fn test_metadata_serialization_no_flatten() {
    let mut meta = Metadata::new("gs://bucket/file.txt", 5, "pdf_parser");
    meta.extra.insert("author".to_string(), serde_json::Value::String("Alice".to_string()));
    meta.extra.insert("pages".to_string(), serde_json::Value::Number(serde_json::Number::from(10)));

    let json = serde_json::to_string(&meta).unwrap();
    let parsed: Metadata = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.uri, "gs://bucket/file.txt");
    assert_eq!(parsed.chunk_id, 5);
    assert_eq!(parsed.source, "pdf_parser");
    assert_eq!(parsed.extra.get("author").unwrap(), "Alice");
    assert_eq!(parsed.extra.get("pages").unwrap().as_u64().unwrap(), 10);

    let payload = Payload::new("content goes here", meta);
    let json = serde_json::to_string(&payload).unwrap();
    let parsed: Payload = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.content, "content goes here");
    assert_eq!(parsed.metadata.extra.get("author").unwrap(), "Alice");
}

#[tokio::test]
async fn test_dimension_mismatch_error() {
    let cn = "dimerr";
    cleanup(cn).await;
    let config = redis_config();
    let name = collection(cn);
    create_collection(&config, &name).await.unwrap();

    let bad_vector = vec![1.0, 2.0, 3.0];
    let point = PointStruct::new("bad", bad_vector, Payload::new("test", Metadata::new("u", 0, "s")));
    let result = add_vector_and_metadata(&config, &point, &name, None).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("dimension mismatch"));

    cleanup(cn).await;
}
