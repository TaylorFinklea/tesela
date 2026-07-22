#![cfg(unix)]

use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;
use common::ServerGuard;
use tesela_core::stable_uuid_from_slug;
use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, PropOp, PropScalar, SyncEngine};

fn make_mosaic(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join("notes"))?;
    fs::create_dir_all(root.join("attachments"))?;
    fs::create_dir_all(root.join(".tesela"))?;
    fs::write(
        root.join(".tesela/config.toml"),
        "[backup]\nauto_on_quit = false\n",
    )
}

fn spawn(mosaic: &Path, addr: &str) -> Child {
    Command::new(common::binary_path())
        .current_dir(mosaic)
        .env("TESELA_SERVER_BIND", addr)
        .env("RUST_LOG", "warn")
        .env("TESELA_DISABLE_MDNS", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn tesela-server")
}

#[tokio::test(flavor = "current_thread")]
async fn node_property_backlink_is_pageid_based() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_mosaic(&mosaic).unwrap();
    let (child, _addr, base) =
        common::spawn_with_retry(Duration::from_secs(15), |addr| spawn(&mosaic, addr));
    let _server = ServerGuard(Some(child));
    let client = reqwest::Client::new();

    let target_response = client
        .post(format!("{base}/notes"))
        .json(&serde_json::json!({"title": "Target", "content": "- target\n", "tags": []}))
        .send()
        .await
        .unwrap();
    assert!(
        target_response.status().is_success(),
        "target create failed: {}",
        target_response.text().await.unwrap()
    );
    let target: serde_json::Value = target_response.json().await.unwrap();
    let target_slug = target["id"].as_str().unwrap();
    let property_response = client
        .post(format!("{base}/notes"))
        .json(&serde_json::json!({
            "title": "relation_project_test",
            "content": "---\ntitle: relation_project_test\ntype: Property\nvalue_type: node\n---\n- property\n",
            "tags": []
        }))
        .send().await.unwrap();
    assert!(
        property_response.status().is_success(),
        "property create failed: {}",
        property_response.text().await.unwrap()
    );
    let source: serde_json::Value = client
        .post(format!("{base}/notes"))
        .json(&serde_json::json!({
            "title": "Source",
            "content": "- source <!-- bid:01010101-0101-0101-0101-010101010101 -->\n",
            "tags": []
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let source_slug = source["id"].as_str().unwrap().to_string();
    let directory: Vec<serde_json::Value> = client
        .get(format!("{base}/loro/page-directory"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let target_page_id = directory
        .iter()
        .find(|entry| entry["slug"] == target_slug)
        .and_then(|entry| entry["page_id"].as_str())
        .expect("target PageId")
        .to_string();

    client
        .post(format!("{base}/blocks/set-property"))
        .json(&serde_json::json!({
            "block_id": format!("{source_slug}:0"),
            "key": "relation_project_test",
            "value": target_page_id,
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let backlinks: Vec<serde_json::Value> = client
        .get(format!("{base}/relations/{target_page_id}/backlinks"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0]["source_slug"], source_slug);
}

/// A restart can materialize the canonical Loro snapshots *after* the server
/// has built its SQLite cache from the empty notes directory. The watcher must
/// rebuild relation edges once the materialized property definition is indexed.
#[tokio::test(flavor = "current_thread")]
async fn node_relation_rebuilds_after_snapshot_only_startup() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_mosaic(&mosaic).unwrap();
    let snapshot_dir = mosaic.join(".tesela/loro");
    let notes_dir = mosaic.join("notes");
    let device = DeviceId::from_bytes([0x31; 16]);
    let target_doc = stable_uuid_from_slug("startup-relation-target");
    let property_doc = stable_uuid_from_slug("startup-relation-property");
    let source_doc = stable_uuid_from_slug("startup-relation-source");
    let source_block = [0x35; 16];

    let engine = LoroEngine::with_dirs(
        device,
        Arc::new(Hlc::new(device)),
        snapshot_dir,
        Some(notes_dir.clone()),
    )
    .await
    .unwrap();
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: property_doc,
            display_alias: Some("startup-relation-property".into()),
            title: "startup_relation_property".into(),
            content: "---\ntitle: startup_relation_property\ntype: Property\nvalue_type: node\n---\n- property\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: target_doc,
            display_alias: Some("startup-relation-target".into()),
            title: "Startup Relation Target".into(),
            content: "- target\n".into(),
            created_at_millis: 2,
        })
        .await
        .unwrap();
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: source_doc,
            display_alias: Some("startup-relation-source".into()),
            title: "Startup Relation Source".into(),
            content: "- source <!-- bid:35353535-3535-3535-3535-353535353535 -->\n".into(),
            created_at_millis: 3,
        })
        .await
        .unwrap();
    let target_page_id = engine
        .page_directory_list()
        .await
        .into_iter()
        .find(|entry| entry.loro_doc_id == hex::encode(target_doc))
        .expect("target directory binding")
        .page_id;
    engine
        .record_local(OpPayload::BlockPropertySet {
            note_id: source_doc,
            block_id: source_block,
            key: "startup_relation_property".into(),
            value: PropOp::SetScalar(PropScalar::Text(target_page_id.to_string())),
        })
        .await
        .unwrap();
    drop(engine);

    // Simulate restore/relaunch with only the authoritative snapshots. The
    // server needs to materialize and index these files itself.
    fs::remove_dir_all(&notes_dir).unwrap();
    fs::create_dir_all(&notes_dir).unwrap();

    let (child, _addr, base) =
        common::spawn_with_retry(Duration::from_secs(15), |addr| spawn(&mosaic, addr));
    let _server = ServerGuard(Some(child));
    let client = reqwest::Client::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let backlinks: Vec<serde_json::Value> = client
            .get(format!("{base}/relations/{target_page_id}/backlinks"))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        if backlinks.len() == 1 {
            assert_eq!(backlinks[0]["source_slug"], "startup-relation-source");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "snapshot-only startup never rebuilt relation backlink: {backlinks:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn node_property_writes_normalize_compact_page_id() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_mosaic(&mosaic).unwrap();
    let (child, _addr, base) =
        common::spawn_with_retry(Duration::from_secs(15), |addr| spawn(&mosaic, addr));
    let _server = ServerGuard(Some(child));
    let client = reqwest::Client::new();

    let target: serde_json::Value = client
        .post(format!("{base}/notes"))
        .json(&serde_json::json!({"title": "Target", "content": "- target\n", "tags": []}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let target_slug = target["id"].as_str().unwrap();
    let property: serde_json::Value = client
        .post(format!("{base}/notes"))
        .json(&serde_json::json!({
            "title": "canonical_node_property",
            "content": "---\ntitle: canonical_node_property\ntype: Property\nvalue_type: node\n---\n- property\n",
            "tags": []
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(property["id"].is_string());
    let source: serde_json::Value = client
        .post(format!("{base}/notes"))
        .json(&serde_json::json!({
            "title": "Source",
            "content": "- source <!-- bid:11111111-1111-1111-1111-111111111112 -->\n",
            "tags": []
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let source_slug = source["id"].as_str().unwrap();
    let directory: Vec<serde_json::Value> = client
        .get(format!("{base}/loro/page-directory"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let canonical_page_id = directory
        .iter()
        .find(|entry| entry["slug"] == target_slug)
        .and_then(|entry| entry["page_id"].as_str())
        .expect("target PageId");
    let compact_page_id = canonical_page_id.replace('-', "");

    client
        .post(format!("{base}/blocks/set-property"))
        .json(&serde_json::json!({
            "block_id": format!("{source_slug}:0"),
            "key": "canonical_node_property",
            "value": compact_page_id,
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let stored: serde_json::Value = client
        .get(format!("{base}/notes/{source_slug}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let content = stored["content"].as_str().expect("note content");
    assert!(
        content.contains(&format!("canonical_node_property:: {canonical_page_id}")),
        "the stored property must use the canonical hyphenated PageId"
    );
    assert!(
        !content.contains(&compact_page_id),
        "the compact compatibility input must not be persisted"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn node_relation_survives_tag_rename_by_page_id() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_mosaic(&mosaic).unwrap();
    let (child, _addr, base) =
        common::spawn_with_retry(Duration::from_secs(15), |addr| spawn(&mosaic, addr));
    let _server = ServerGuard(Some(child));
    let client = reqwest::Client::new();

    let target: serde_json::Value = client
        .post(format!("{base}/notes"))
        .json(&serde_json::json!({
            "title": "Old Project",
            "content": "---\ntitle: Old Project\ntype: Tag\n---\n- target\n",
            "tags": []
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let old_slug = target["id"].as_str().unwrap().to_string();
    let property: serde_json::Value = client
        .post(format!("{base}/notes"))
        .json(&serde_json::json!({
            "title": "rename_relation_project",
            "content": "---\ntitle: rename_relation_project\ntype: Property\nvalue_type: node\n---\n- property\n",
            "tags": []
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(property["id"].is_string());
    let source: serde_json::Value = client
        .post(format!("{base}/notes"))
        .json(&serde_json::json!({
            "title": "Relation Source",
            "content": "- source <!-- bid:02020202-0202-0202-0202-020202020202 -->\n",
            "tags": []
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let source_slug = source["id"].as_str().unwrap().to_string();

    let directory: Vec<serde_json::Value> = client
        .get(format!("{base}/loro/page-directory"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let page_id = directory
        .iter()
        .find(|entry| entry["slug"] == old_slug)
        .and_then(|entry| entry["page_id"].as_str())
        .expect("old target PageId")
        .to_string();
    client
        .post(format!("{base}/blocks/set-property"))
        .json(&serde_json::json!({
            "block_id": format!("{source_slug}:0"),
            "key": "rename_relation_project",
            "value": page_id,
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    client
        .post(format!("{base}/tags/rename"))
        .json(&serde_json::json!({
            "from_slug": old_slug,
            "to_slug": "renamed-project",
            "commit": true,
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let renamed_directory: Vec<serde_json::Value> = client
        .get(format!("{base}/loro/page-directory"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let live = renamed_directory
        .iter()
        .find(|entry| entry["page_id"] == page_id && entry["deleted"] == false)
        .expect("one live binding after rename");
    assert_eq!(live["slug"], "renamed-project");
    assert_eq!(live["conflict"], false);
    assert!(
        renamed_directory.iter().any(|entry| {
            entry["page_id"] == page_id
                && entry["slug"] == old_slug
                && entry["deleted"] == true
                && entry["forward_to_loro_doc_id"].is_string()
        }),
        "old binding remains a tombstone with forwarding provenance"
    );

    let backlinks: Vec<serde_json::Value> = client
        .get(format!("{base}/relations/{page_id}/backlinks"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0]["source_slug"], source_slug);
    assert_eq!(
        backlinks[0]["edge"]["property_key"],
        "rename_relation_project"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn renamed_relation_source_rebuilds_after_source_tombstone() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_mosaic(&mosaic).unwrap();
    let (child, _addr, base) =
        common::spawn_with_retry(Duration::from_secs(15), |addr| spawn(&mosaic, addr));
    let _server = ServerGuard(Some(child));
    let client = reqwest::Client::new();

    let target: serde_json::Value = client
        .post(format!("{base}/notes"))
        .json(&serde_json::json!({
            "title": "Relation Target",
            "content": "- target\n",
            "tags": []
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let target_slug = target["id"].as_str().unwrap().to_string();
    let property: serde_json::Value = client
        .post(format!("{base}/notes"))
        .json(&serde_json::json!({
            "title": "rename_source_relation_project",
            "content": "---\ntitle: rename_source_relation_project\ntype: Property\nvalue_type: node\n---\n- property\n",
            "tags": []
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(property["id"].is_string());
    let source: serde_json::Value = client
        .post(format!("{base}/notes"))
        .json(&serde_json::json!({
            "title": "Old Relation Source",
            "content": "---\ntitle: Old Relation Source\ntype: Tag\n---\n- source <!-- bid:03030303-0303-0303-0303-030303030303 -->\n",
            "tags": []
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let source_slug = source["id"].as_str().unwrap().to_string();
    let directory: Vec<serde_json::Value> = client
        .get(format!("{base}/loro/page-directory"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let target_page_id = directory
        .iter()
        .find(|entry| entry["slug"] == target_slug)
        .and_then(|entry| entry["page_id"].as_str())
        .expect("target PageId")
        .to_string();

    client
        .post(format!("{base}/blocks/set-property"))
        .json(&serde_json::json!({
            "block_id": format!("{source_slug}:0"),
            "key": "rename_source_relation_project",
            "value": target_page_id,
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    client
        .post(format!("{base}/tags/rename"))
        .json(&serde_json::json!({
            "from_slug": source_slug,
            "to_slug": "renamed-relation-source",
            "commit": true,
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let backlinks: Vec<serde_json::Value> = client
        .get(format!("{base}/relations/{target_page_id}/backlinks"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0]["source_slug"], "renamed-relation-source");
    assert_eq!(
        backlinks[0]["edge"]["property_key"],
        "rename_source_relation_project"
    );

    client
        .post(format!("{base}/tags/rename"))
        .json(&serde_json::json!({
            "from_slug": "renamed-relation-source",
            "to_slug": "renamed-again-relation-source",
            "commit": true,
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let directory: Vec<serde_json::Value> = client
        .get(format!("{base}/loro/page-directory"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let renamed_again = directory
        .iter()
        .find(|entry| entry["slug"] == "renamed-again-relation-source")
        .expect("live renamed target");
    let aliases = renamed_again["aliases"].as_array().expect("alias array");
    assert!(aliases.iter().any(|alias| alias == &source_slug));
    assert!(aliases
        .iter()
        .any(|alias| alias == "renamed-relation-source"));
}
