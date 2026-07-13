//! HTTP contract and post-write-tail coverage for subtree relocation.

#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use futures::StreamExt;
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio_tungstenite::tungstenite::Message as TMessage;

#[path = "common/mod.rs"]
mod common;
use common::ServerGuard;

const ROOT_BID: &str = "01010101-0101-4101-8101-010101010101";
const CHILD_BID: &str = "02020202-0202-4202-8202-020202020202";
const STAY_BID: &str = "03030303-0303-4303-8303-030303030303";
const TARGET_BID: &str = "04040404-0404-4404-8404-040404040404";
const MOVE_ID: &str = "11111111-1111-4111-8111-111111111111";

fn make_fixture_mosaic(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join("notes"))?;
    fs::create_dir_all(root.join("attachments"))?;
    fs::create_dir_all(root.join(".tesela"))?;
    fs::write(
        root.join(".tesela/config.toml"),
        "[backup]\nauto_on_quit = false\n",
    )?;
    Ok(())
}

fn spawn_server_child(mosaic: &Path, addr: &str) -> Child {
    Command::new(common::binary_path())
        .current_dir(mosaic)
        .env("TESELA_SERVER_BIND", addr)
        .env("TESELA_DISABLE_MDNS", "1")
        .env("TESELA_DISABLE_PEER_SYNC", "1")
        .env("TESELA_GROUP_KEY_FILE_STORE", "1")
        .env("TESELA_BACKUP_ON_START", "0")
        .env("TESELA_BACKUP_INTERVAL_SECS", "0")
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tesela-server")
}

struct Harness {
    base: String,
    mosaic: PathBuf,
    _server: ServerGuard,
    _temp: TempDir,
}

fn boot() -> Harness {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();
    let (child, _addr, base) = common::spawn_with_retry(Duration::from_secs(15), |addr| {
        spawn_server_child(&mosaic, addr)
    });
    Harness {
        base,
        mosaic,
        _server: ServerGuard(Some(child)),
        _temp: temp,
    }
}

async fn create_note(client: &reqwest::Client, base: &str, title: &str, content: String) -> Value {
    client
        .post(format!("{base}/notes"))
        .json(&json!({ "title": title, "content": content, "tags": [] }))
        .send()
        .await
        .expect("POST /notes")
        .error_for_status()
        .expect("note created")
        .json()
        .await
        .expect("create response json")
}

async fn get_note(client: &reqwest::Client, base: &str, id: &str) -> Value {
    client
        .get(format!("{base}/notes/{id}"))
        .send()
        .await
        .expect("GET /notes/:id")
        .error_for_status()
        .expect("note exists")
        .json()
        .await
        .expect("note json")
}

async fn version_count(client: &reqwest::Client, base: &str, id: &str) -> usize {
    client
        .get(format!("{base}/notes/{id}/versions"))
        .send()
        .await
        .expect("GET versions")
        .error_for_status()
        .expect("versions response")
        .json::<Vec<Value>>()
        .await
        .expect("versions json")
        .len()
}

async fn set_property(client: &reqwest::Client, base: &str, note_id: &str, key: &str, value: &str) {
    client
        .post(format!("{base}/blocks/set-property"))
        .json(&json!({
            "block_id": format!("{note_id}:{ROOT_BID}"),
            "key": key,
            "value": value,
        }))
        .send()
        .await
        .expect("POST /blocks/set-property")
        .error_for_status()
        .expect("property set");
}

fn move_json(
    move_id: &str,
    source_note_id: &str,
    destination_note_id: &str,
    target_bid: Option<&str>,
    placement: &str,
) -> Value {
    json!({
        "move_id": move_id,
        "source_note_id": source_note_id,
        "root_bid": ROOT_BID,
        "destination_note_id": destination_note_id,
        "target_bid": target_bid,
        "placement": placement,
    })
}

type TestWs =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect_ws(base: &str) -> TestWs {
    let url = format!("ws://{}/ws", base.trim_start_matches("http://"));
    let (mut ws, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect /ws");
    settle_ws(&mut ws).await;
    ws
}

async fn settle_ws(ws: &mut TestWs) {
    tokio::time::sleep(Duration::from_millis(600)).await;
    loop {
        match tokio::time::timeout(Duration::from_millis(50), ws.next()).await {
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(error))) => panic!("WebSocket error while settling: {error}"),
            Ok(None) => panic!("WebSocket closed while settling"),
            Err(_) => break,
        }
    }
}

async fn collect_tail(ws: &mut TestWs, expected_slugs: &[&str]) {
    let mut note_events = Vec::new();
    let mut delta_docs = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);

    while (note_events.len() < expected_slugs.len() || delta_docs.len() < expected_slugs.len())
        && tokio::time::Instant::now() < deadline
    {
        match tokio::time::timeout(Duration::from_millis(750), ws.next()).await {
            Ok(Some(Ok(TMessage::Text(text)))) => {
                let body: Value = serde_json::from_str(&text).expect("WS text JSON");
                if body["event"] == "note_updated" {
                    let note_id = body["note"]["id"]
                        .as_str()
                        .expect("note_updated carries note id")
                        .to_string();
                    if expected_slugs.contains(&note_id.as_str()) && !note_events.contains(&note_id)
                    {
                        note_events.push(note_id);
                    }
                }
            }
            Ok(Some(Ok(TMessage::Binary(bytes)))) => {
                if let Some(updates) =
                    tesela_sync::decode_loro_relay_payload(&bytes).expect("valid TLR2 frame")
                {
                    assert_eq!(updates.len(), 1, "one note per relocation delta");
                    delta_docs.push(updates[0].doc);
                }
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(error))) => panic!("WebSocket error: {error}"),
            Ok(None) => panic!("WebSocket closed before relocation tail"),
            Err(_) => {}
        }
    }

    let mut expected_events: Vec<String> = expected_slugs
        .iter()
        .map(|slug| (*slug).to_string())
        .collect();
    expected_events.sort();
    note_events.sort();
    assert_eq!(
        note_events, expected_events,
        "one distinct note_updated event per affected note"
    );
    let expected_docs: Vec<[u8; 16]> = expected_slugs
        .iter()
        .map(|slug| tesela_core::stable_uuid_from_slug(slug))
        .collect();
    assert_eq!(
        delta_docs, expected_docs,
        "one ordered cursor-free TLR2 delta per affected note"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn validation_rejects_bad_locators_targets_and_missing_notes_without_mutation() {
    let h = boot();
    let client = reqwest::Client::new();
    let source = create_note(
        &client,
        &h.base,
        "2026-07-12",
        format!(
            "- root <!-- bid:{ROOT_BID} -->\n  - child <!-- bid:{CHILD_BID} -->\n- stay <!-- bid:{STAY_BID} -->\n"
        ),
    )
    .await;
    let source_id = source["id"].as_str().unwrap();
    let original = source["content"].as_str().unwrap().to_string();

    let bad_move_id = client
        .post(format!("{}/blocks/move-subtree", h.base))
        .json(&json!({
            "move_id": "not-a-uuid",
            "source_note_id": source_id,
            "root_bid": ROOT_BID,
            "destination_note_id": source_id,
            "target_bid": STAY_BID,
            "placement": "before",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        bad_move_id.status(),
        reqwest::StatusCode::UNPROCESSABLE_ENTITY
    );

    let bad_target_id = client
        .post(format!("{}/blocks/move-subtree", h.base))
        .json(&json!({
            "move_id": "12121212-1212-4212-8212-121212121212",
            "source_note_id": source_id,
            "root_bid": ROOT_BID,
            "destination_note_id": source_id,
            "target_bid": "not-a-uuid",
            "placement": "before",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        bad_target_id.status(),
        reqwest::StatusCode::UNPROCESSABLE_ENTITY
    );

    for (move_id, target, placement) in [
        ("13131313-1313-4313-8313-131313131313", None, "before"),
        (
            "14141414-1414-4414-8414-141414141414",
            Some(STAY_BID),
            "append",
        ),
        (
            "15151515-1515-4515-8515-151515151515",
            Some(ROOT_BID),
            "before",
        ),
        (
            "16161616-1616-4616-8616-161616161616",
            Some(CHILD_BID),
            "inside",
        ),
    ] {
        let response = client
            .post(format!("{}/blocks/move-subtree", h.base))
            .json(&move_json(move_id, source_id, source_id, target, placement))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            reqwest::StatusCode::BAD_REQUEST,
            "invalid target combination {placement:?}/{target:?}"
        );
    }

    let missing_non_daily = client
        .post(format!("{}/blocks/move-subtree", h.base))
        .json(&move_json(
            "17171717-1717-4717-8717-171717171717",
            source_id,
            "not-a-daily-note",
            None,
            "append",
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(missing_non_daily.status(), reqwest::StatusCode::NOT_FOUND);

    let absent_daily = "2026-07-11";
    let rejected_daily = client
        .post(format!("{}/blocks/move-subtree", h.base))
        .json(&move_json(
            "18181818-1818-4818-8818-181818181818",
            source_id,
            absent_daily,
            Some(TARGET_BID),
            "before",
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(rejected_daily.status(), reqwest::StatusCode::BAD_REQUEST);
    assert!(
        !h.mosaic
            .join("notes")
            .join(format!("{absent_daily}.md"))
            .exists(),
        "rejected relocation must not create an empty daily"
    );

    let after = get_note(&client, &h.base, source_id).await;
    assert_eq!(after["content"], original, "all rejections are immutable");
}

#[tokio::test(flavor = "current_thread")]
async fn absent_daily_append_is_created_inside_move_without_blank_seed_sibling() {
    let h = boot();
    let client = reqwest::Client::new();
    let source = create_note(
        &client,
        &h.base,
        "2026-07-12",
        format!(
            "- root <!-- bid:{ROOT_BID} -->\n  - child <!-- bid:{CHILD_BID} -->\n- stay <!-- bid:{STAY_BID} -->\n"
        ),
    )
    .await;
    let source_id = source["id"].as_str().unwrap();
    let destination = "2026-07-11";

    let response = client
        .post(format!("{}/blocks/move-subtree", h.base))
        .json(&move_json(
            "21212121-2121-4121-8121-212121212121",
            source_id,
            destination,
            None,
            "append",
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    let notes = body["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0]["id"], source_id);
    assert_eq!(notes[1]["id"], destination);

    let source_render = notes[0]["content"].as_str().unwrap();
    assert!(!source_render.contains("root"));
    assert!(!source_render.contains("child"));
    assert!(source_render.contains("stay"));

    let destination_render = notes[1]["content"].as_str().unwrap();
    assert!(destination_render.contains(&format!("bid:{ROOT_BID}")));
    assert!(destination_render.contains(&format!("bid:{CHILD_BID}")));
    assert_eq!(
        destination_render
            .lines()
            .filter(|line| line.trim_start().starts_with("- "))
            .count(),
        2,
        "trusted daily seed's blank block is removed when the subtree is appended"
    );
    assert!(
        !destination_render
            .lines()
            .any(|line| line.trim() == "-" || line.trim() == "- "),
        "no blank seed sibling remains"
    );
    assert!(h
        .mosaic
        .join("notes")
        .join(format!("{destination}.md"))
        .exists());
    assert_eq!(
        version_count(&client, &h.base, destination).await,
        0,
        "a newly created destination has no synthetic history row"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn cross_note_move_repairs_indexes_history_events_deltas_and_replays_without_history() {
    let h = boot();
    let client = reqwest::Client::new();
    create_note(
        &client,
        &h.base,
        "linked-page",
        "- link target\n".to_string(),
    )
    .await;
    let source = create_note(
        &client,
        &h.base,
        "2026-07-12",
        format!(
            "- moving-needle [[linked-page]] <!-- bid:{ROOT_BID} -->\n  - nested child <!-- bid:{CHILD_BID} -->\n- source stays <!-- bid:{STAY_BID} -->\n"
        ),
    )
    .await;
    let destination = create_note(
        &client,
        &h.base,
        "2026-07-11",
        format!("- destination target <!-- bid:{TARGET_BID} -->\n"),
    )
    .await;
    let source_id = source["id"].as_str().unwrap();
    let destination_id = destination["id"].as_str().unwrap();

    set_property(
        &client,
        &h.base,
        source_id,
        "tags",
        "relocated-tag, second-tag",
    )
    .await;
    set_property(&client, &h.base, source_id, "status", "doing").await;
    assert_eq!(
        client
            .get(format!("{}/notes/relocated-tag", h.base))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::NOT_FOUND,
        "set-property does not itself run the tag-page tail"
    );

    let source_versions_before = version_count(&client, &h.base, source_id).await;
    let destination_versions_before = version_count(&client, &h.base, destination_id).await;
    let mut ws = connect_ws(&h.base).await;
    let request = move_json(
        MOVE_ID,
        source_id,
        destination_id,
        Some(TARGET_BID),
        "inside",
    );
    let response = client
        .post(format!("{}/blocks/move-subtree", h.base))
        .json(&request)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["move_id"], MOVE_ID);
    let notes = body["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0]["id"], source_id);
    assert_eq!(notes[1]["id"], destination_id);
    collect_tail(&mut ws, &[source_id, destination_id]).await;

    let source_render = notes[0]["content"].as_str().unwrap();
    let destination_render = notes[1]["content"].as_str().unwrap();
    assert!(!source_render.contains("moving-needle"));
    assert!(source_render.contains("source stays"));
    for needle in [
        "moving-needle",
        "nested child",
        &format!("bid:{ROOT_BID}"),
        &format!("bid:{CHILD_BID}"),
        "status:: doing",
        "tags:: relocated-tag, second-tag",
    ] {
        assert!(
            destination_render.contains(needle),
            "destination preserves subtree identity/property {needle:?}:\n{destination_render}"
        );
    }

    let hits: Vec<Value> = client
        .get(format!("{}/search", h.base))
        .query(&[("q", "moving-needle")])
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["note_id"], destination_id);

    let source_links: Vec<Value> = client
        .get(format!("{}/notes/{source_id}/links", h.base))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(source_links.is_empty());
    let destination_links: Vec<Value> = client
        .get(format!("{}/notes/{destination_id}/links", h.base))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(destination_links.len(), 1);
    assert_eq!(destination_links[0]["target"], "linked-page");
    let backlinks: Vec<Value> = client
        .get(format!("{}/notes/linked-page/backlinks", h.base))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0]["target"], destination_id);
    assert_eq!(
        version_count(&client, &h.base, source_id).await,
        source_versions_before + 1
    );
    assert_eq!(
        version_count(&client, &h.base, destination_id).await,
        destination_versions_before + 1
    );
    assert_eq!(
        client
            .get(format!("{}/notes/relocated-tag", h.base))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::OK,
        "relocation runs ensure_tag_pages on the refreshed destination"
    );

    settle_ws(&mut ws).await;
    let replay = client
        .post(format!("{}/blocks/move-subtree", h.base))
        .json(&request)
        .send()
        .await
        .unwrap();
    assert_eq!(replay.status(), reqwest::StatusCode::OK);
    let replay_body: Value = replay.json().await.unwrap();
    assert_eq!(replay_body["notes"].as_array().unwrap().len(), 2);
    collect_tail(&mut ws, &[source_id, destination_id]).await;
    assert_eq!(
        version_count(&client, &h.base, source_id).await,
        source_versions_before + 1,
        "replay does not duplicate source history"
    );
    assert_eq!(
        version_count(&client, &h.base, destination_id).await,
        destination_versions_before + 1,
        "replay does not duplicate destination history"
    );

    let conflict = client
        .post(format!("{}/blocks/move-subtree", h.base))
        .json(&move_json(
            MOVE_ID,
            source_id,
            destination_id,
            Some(TARGET_BID),
            "after",
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(conflict.status(), reqwest::StatusCode::CONFLICT);
}

#[tokio::test(flavor = "current_thread")]
async fn same_note_move_returns_and_broadcasts_one_deduplicated_note() {
    let h = boot();
    let client = reqwest::Client::new();
    let note = create_note(
        &client,
        &h.base,
        "same-note",
        format!(
            "- root <!-- bid:{ROOT_BID} -->\n  - child <!-- bid:{CHILD_BID} -->\n- target <!-- bid:{TARGET_BID} -->\n"
        ),
    )
    .await;
    let note_id = note["id"].as_str().unwrap();
    let versions_before = version_count(&client, &h.base, note_id).await;
    let mut ws = connect_ws(&h.base).await;

    let response = client
        .post(format!("{}/blocks/move-subtree", h.base))
        .json(&move_json(
            "31313131-3131-4131-8131-313131313131",
            note_id,
            note_id,
            Some(TARGET_BID),
            "after",
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    let notes = body["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["id"], note_id);
    collect_tail(&mut ws, &[note_id]).await;

    let render = notes[0]["content"].as_str().unwrap();
    let target_pos = render.find("target").unwrap();
    let root_pos = render.find("root").unwrap();
    let child_pos = render.find("child").unwrap();
    assert!(target_pos < root_pos && root_pos < child_pos);
    assert_eq!(
        version_count(&client, &h.base, note_id).await,
        versions_before + 1
    );
}
