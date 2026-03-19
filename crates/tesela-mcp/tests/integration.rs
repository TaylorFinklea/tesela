use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;
use tesela_core::{
    config::StorageConfig, db::SqliteIndex, storage::filesystem::FsNoteStore,
    traits::plugin::PluginRegistry,
};
use tesela_mcp::tools::ToolRegistry;

async fn setup_registry(dir: &TempDir) -> ToolRegistry {
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join(".tesela")).unwrap();
    std::fs::create_dir_all(root.join("notes")).unwrap();

    let db_path = root.join(".tesela").join("tesela.db");

    let store = Arc::new(FsNoteStore::new(root, StorageConfig::default()));
    let index = Arc::new(SqliteIndex::open(&db_path).await.unwrap());
    let registry = Arc::new(PluginRegistry::new());

    ToolRegistry::new(store, index, registry)
}

#[tokio::test]
async fn test_create_and_search() {
    let tmp = TempDir::new().unwrap();
    let registry = setup_registry(&tmp).await;

    // Create a note
    let result = registry
        .call(
            "create_note",
            Some(json!({
                "title": "Test Note",
                "content": "unique-test-keyword-789",
                "tags": ["test"]
            })),
        )
        .await
        .unwrap();

    assert!(result["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Test Note"));

    // Search for it
    let result = registry
        .call(
            "search_notes",
            Some(json!({
                "query": "unique-test-keyword-789"
            })),
        )
        .await
        .unwrap();

    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("Test Note"),
        "Expected 'Test Note' in: {}",
        text
    );
}

#[tokio::test]
async fn test_list_notes() {
    let tmp = TempDir::new().unwrap();
    let registry = setup_registry(&tmp).await;

    registry
        .call(
            "create_note",
            Some(json!({ "title": "Note A", "tags": ["alpha"] })),
        )
        .await
        .unwrap();
    registry
        .call(
            "create_note",
            Some(json!({ "title": "Note B", "tags": ["beta"] })),
        )
        .await
        .unwrap();

    let result = registry.call("list_notes", Some(json!({}))).await.unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Note A") && text.contains("Note B"));
}

#[tokio::test]
async fn test_get_daily_note() {
    let tmp = TempDir::new().unwrap();
    let registry = setup_registry(&tmp).await;

    let result = registry
        .call(
            "get_daily_note",
            Some(json!({
                "date": "2025-01-15"
            })),
        )
        .await
        .unwrap();

    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("2025-01-15"));
}

#[tokio::test]
async fn test_get_note_not_found() {
    let tmp = TempDir::new().unwrap();
    let registry = setup_registry(&tmp).await;

    let result = registry
        .call(
            "get_note",
            Some(json!({
                "id": "nonexistent-note"
            })),
        )
        .await
        .unwrap();

    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("not found") || result.get("isError").is_some(),
        "Expected 'not found' or isError in response, got: {}",
        text
    );
}
