//! HTTP-level proof that `POST /blocks/set-property` writes the property
//! through the sync engine's typed `props`/`prop_keys` container (P1.10),
//! NOT via the legacy whole-note markdown rewrite + re-diff.
//!
//! Trust artifact: the new route emits `OpPayload::BlockPropertySet` so the
//! property merges INDEPENDENTLY of the block's prose `text_seq`. The
//! load-bearing assertion is the concurrency one: a concurrent block-text
//! edit (`POST /blocks`) to the SAME block must NOT clobber the property and
//! vice-versa — only possible when the property lives in its own container,
//! not spliced into the block's text. The materialized `<slug>.md` then
//! shows BOTH the prose edit and the `key:: value` line (the deterministic
//! container → markdown view).
//!
//! Skipped on non-Unix (spawns the server binary, SIGTERMs to shut down).

#![cfg(unix)]

use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;
use common::ServerGuard;

const TASK_BID: &str = "01010101-0101-0101-0101-010101010101";

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
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn tesela-server")
}

fn read_note_file_containing(mosaic: &Path, needle: &str) -> Option<String> {
    for entry in fs::read_dir(mosaic.join("notes")).ok()? {
        let path = entry.ok()?.path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            if let Ok(content) = fs::read_to_string(&path) {
                if content.contains(needle) {
                    return Some(content);
                }
            }
        }
    }
    None
}

/// A property set via `/blocks/set-property` lands in the engine's typed
/// container: it survives a CONCURRENT prose edit to the same block (only
/// possible when the property merges independently of `text_seq`), and the
/// materialized file carries BOTH the prose edit and the `key:: value` line.
#[tokio::test(flavor = "current_thread")]
async fn set_property_lands_in_engine_container_and_survives_prose_edit() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();

    let (child, _addr, base) = common::spawn_with_retry(Duration::from_secs(15), |addr| {
        spawn_server_child(&mosaic, addr)
    });
    let _server = ServerGuard(Some(child));

    let client = reqwest::Client::new();

    // 1. Create a note with a single block carrying an explicit bid so the
    //    engine keys its node to an id the property op can resolve.
    let seed_body = format!("- a task <!-- bid:{TASK_BID} -->\n");
    let created: serde_json::Value = client
        .post(format!("{base}/notes"))
        .json(&serde_json::json!({
            "title": "Property Note",
            "content": seed_body,
            "tags": [],
        }))
        .send()
        .await
        .expect("POST /notes")
        .error_for_status()
        .expect("note created")
        .json()
        .await
        .expect("create json");
    let note_id = created["id"].as_str().expect("note id").to_string();

    // 2. Set `status:: doing` on the task block (body line 0) via the
    //    re-pointed route.
    let resp = client
        .post(format!("{base}/blocks/set-property"))
        .json(&serde_json::json!({
            "block_id": format!("{note_id}:0"),
            "key": "status",
            "value": "doing",
        }))
        .send()
        .await
        .expect("POST /blocks/set-property")
        .error_for_status()
        .expect("set-property ok");
    let _: serde_json::Value = resp.json().await.expect("set-property json");

    // 3. CONCURRENT prose edit to the SAME block via the block-granular
    //    endpoint: rename its text. If the property were spliced into the
    //    block's text (the old whole-note rewrite path), this BlockUpsert
    //    would carry stale text WITHOUT the property and erase it. Because
    //    the property lives in its own container, the prose update and the
    //    property merge independently.
    let after: serde_json::Value = client
        .post(format!("{base}/notes/{note_id}/blocks"))
        .json(&serde_json::json!({
            "ops": [
                {
                    "kind": "upsert",
                    "bid": TASK_BID,
                    "text": "a renamed task",
                    "parent_bid": null,
                    "indent_level": 0,
                }
            ]
        }))
        .send()
        .await
        .expect("POST /blocks (rename)")
        .error_for_status()
        .expect("rename ok")
        .json()
        .await
        .expect("rename json");

    let render = after["content"].as_str().expect("content");
    assert!(
        render.contains("a renamed task"),
        "prose edit must land; got:\n{render}"
    );
    assert!(
        render.contains("status:: doing"),
        "property set via the engine container MUST survive a concurrent prose \
         edit (proves it is NOT spliced into block text); got:\n{render}"
    );

    // 4. The materialized `<slug>.md` shows BOTH — the deterministic
    //    container → markdown view.
    let file = read_note_file_containing(&mosaic, "a renamed task")
        .expect("a notes/*.md should hold the renamed task");
    assert!(
        file.contains("a renamed task") && file.contains("status:: doing"),
        "materialized file must hold BOTH the prose edit AND the property; got:\n{file}"
    );

    let _ = temp.path();
}

/// PROBE: set a property on a block that ALREADY carries that key as an
/// in-text continuation line (the recurring-task shape). Asserts the
/// materialized markdown does NOT duplicate the key.
#[tokio::test(flavor = "current_thread")]
async fn set_property_on_block_with_intext_prop_does_not_duplicate() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();

    let (child, _addr, base) = common::spawn_with_retry(Duration::from_secs(15), |addr| {
        spawn_server_child(&mosaic, addr)
    });
    let _server = ServerGuard(Some(child));

    let client = reqwest::Client::new();

    let seed_body = format!("- a task <!-- bid:{TASK_BID} -->\n  status:: todo\n");
    let created: serde_json::Value = client
        .post(format!("{base}/notes"))
        .json(&serde_json::json!({ "title": "Dup Note", "content": seed_body, "tags": [] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let note_id = created["id"].as_str().unwrap().to_string();

    // The bullet is body line 0; status:: todo is its continuation line.
    let _ = client
        .post(format!("{base}/blocks/set-property"))
        .json(&serde_json::json!({
            "block_id": format!("{note_id}:0"),
            "key": "status",
            "value": "doing",
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let file = read_note_file_containing(&mosaic, "a task").expect("note file");
    let status_lines = file.matches("status::").count();
    assert_eq!(
        status_lines, 1,
        "status:: must appear exactly once (no text+container duplicate); got:\n{file}"
    );
    assert!(file.contains("status:: doing"), "got:\n{file}");

    let _ = temp.path();
}

/// Setting `status:: done` on a RECURRING block via the route still fires the
/// server-side recurring-roll (post-save reads the just-set property from the
/// re-materialized container view): the deadline advances, `status` resets to
/// `todo`, and the rolled `status::` line is NOT duplicated by a stale
/// container value.
#[tokio::test(flavor = "current_thread")]
async fn set_status_done_on_recurring_block_rolls_via_engine() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();

    let (child, _addr, base) = common::spawn_with_retry(Duration::from_secs(15), |addr| {
        spawn_server_child(&mosaic, addr)
    });
    let _server = ServerGuard(Some(child));

    let client = reqwest::Client::new();

    // A recurring task: daily, with a deadline + status:: todo (all in-text,
    // the production shape from a markdown-seeded note).
    let seed_body = format!(
        "- water plants <!-- bid:{TASK_BID} -->\n  recurring:: daily\n  deadline:: [[2026-05-07]]\n  status:: todo\n"
    );
    let created: serde_json::Value = client
        .post(format!("{base}/notes"))
        .json(&serde_json::json!({ "title": "Recur Note", "content": seed_body, "tags": [] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let note_id = created["id"].as_str().unwrap().to_string();

    // Mark it done via the re-pointed route — should trigger the roll.
    let _ = client
        .post(format!("{base}/blocks/set-property"))
        .json(&serde_json::json!({
            "block_id": format!("{note_id}:0"),
            "key": "status",
            "value": "done",
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let file = read_note_file_containing(&mosaic, "water plants").expect("note file");
    // Exactly one status:: line, and the roll reset it to todo (not done).
    assert_eq!(
        file.matches("status::").count(),
        1,
        "exactly one status:: line after the roll; got:\n{file}"
    );
    assert!(
        file.contains("status:: todo"),
        "recurring roll must reset status to todo; got:\n{file}"
    );
    // The deadline advanced one day (daily recurrence).
    assert!(
        file.contains("deadline:: [[2026-05-08]]"),
        "deadline must advance to the next day; got:\n{file}"
    );
    assert_eq!(
        file.matches("deadline::").count(),
        1,
        "exactly one deadline:: line; got:\n{file}"
    );

    let _ = temp.path();
}

/// A multi-select delta promotes a markdown-seeded continuation line into the
/// engine's mergeable list, applies independent member changes, and remains
/// addressable by the block's stable bid after the note re-materializes.
#[tokio::test(flavor = "current_thread")]
async fn update_property_list_promotes_legacy_members_and_applies_deltas() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();
    fs::write(
        mosaic.join("notes/labels.md"),
        "---\ntitle: \"Labels\"\ntype: \"Property\"\nvalue_type: \"multi-select\"\nchoices: [\"alpha\", \"beta\", \"gamma\", \"delta\"]\ntags: []\n---\n- Labels property.\n",
    )
    .unwrap();

    let (child, _addr, base) = common::spawn_with_retry(Duration::from_secs(15), |addr| {
        spawn_server_child(&mosaic, addr)
    });
    let _server = ServerGuard(Some(child));
    let client = reqwest::Client::new();

    let seed_body = format!("- a task <!-- bid:{TASK_BID} -->\n  labels:: alpha, beta\n");
    let created: serde_json::Value = client
        .post(format!("{base}/notes"))
        .json(&serde_json::json!({
            "title": "List Property Note",
            "content": seed_body,
            "tags": [],
        }))
        .send()
        .await
        .expect("POST /notes")
        .error_for_status()
        .expect("note created")
        .json()
        .await
        .expect("create json");
    let note_id = created["id"].as_str().expect("note id").to_string();

    client
        .post(format!("{base}/blocks/update-property-list"))
        .json(&serde_json::json!({
            "block_id": format!("{note_id}:0"),
            "key": "labels",
            "add": [" gamma ", "gamma"],
            "remove": ["alpha"],
        }))
        .send()
        .await
        .expect("POST /blocks/update-property-list")
        .error_for_status()
        .expect("first list delta ok");

    let first = read_note_file_containing(&mosaic, "a task").expect("materialized note");
    let first_line = first
        .lines()
        .find(|line| line.trim_start().starts_with("labels::"))
        .expect("labels property line");
    assert_eq!(first.matches("labels::").count(), 1, "got:\n{first}");
    assert!(!first_line.contains("alpha"), "got:\n{first}");
    assert!(first_line.contains("beta"), "got:\n{first}");
    assert!(first_line.contains("gamma"), "got:\n{first}");
    assert_eq!(first_line.matches("gamma").count(), 1, "got:\n{first}");

    client
        .post(format!("{base}/blocks/update-property-list"))
        .json(&serde_json::json!({
            "block_id": format!("{note_id}:{TASK_BID}"),
            "key": "labels",
            "add": ["delta"],
            "remove": ["beta"],
        }))
        .send()
        .await
        .expect("POST /blocks/update-property-list by bid")
        .error_for_status()
        .expect("second list delta ok");

    let second = read_note_file_containing(&mosaic, "a task").expect("materialized note");
    let second_line = second
        .lines()
        .find(|line| line.trim_start().starts_with("labels::"))
        .expect("labels property line");
    assert_eq!(second.matches("labels::").count(), 1, "got:\n{second}");
    assert!(!second_line.contains("alpha"), "got:\n{second}");
    assert!(!second_line.contains("beta"), "got:\n{second}");
    assert!(second_line.contains("gamma"), "got:\n{second}");
    assert!(second_line.contains("delta"), "got:\n{second}");

    let _ = temp.path();
}

/// A note discovered from disk at server startup is not yet resident in the
/// sync engine. Its first typed property edit must seed that materialized note
/// into the engine before authoring the list delta, otherwise the request can
/// return 200 while the markdown view remains unchanged.
#[tokio::test(flavor = "current_thread")]
async fn update_property_list_seeds_a_markdown_only_note_before_first_edit() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();
    fs::write(
        mosaic.join("notes/labels.md"),
        "---\ntitle: \"Labels\"\ntype: \"Property\"\nvalue_type: \"multi-select\"\nchoices: [\"alpha\", \"beta\", \"gamma\"]\ntags: []\n---\n- Labels property.\n",
    )
    .unwrap();
    fs::write(
        mosaic.join("notes/cold-list.md"),
        format!(
            "---\ntitle: \"Cold List\"\ntags: []\n---\n- a cold task <!-- bid:{TASK_BID} -->\n  labels:: alpha, beta\n"
        ),
    )
    .unwrap();

    let (child, _addr, base) = common::spawn_with_retry(Duration::from_secs(15), |addr| {
        spawn_server_child(&mosaic, addr)
    });
    let _server = ServerGuard(Some(child));
    let client = reqwest::Client::new();

    client
        .post(format!("{base}/blocks/update-property-list"))
        .json(&serde_json::json!({
            "block_id": format!("cold-list:{TASK_BID}"),
            "key": "labels",
            "add": ["gamma"],
            "remove": ["beta"],
        }))
        .send()
        .await
        .expect("POST /blocks/update-property-list")
        .error_for_status()
        .expect("cold list delta ok");

    let file = fs::read_to_string(mosaic.join("notes/cold-list.md")).unwrap();
    let line = file
        .lines()
        .find(|line| line.trim_start().starts_with("labels::"))
        .expect("labels property line");
    assert!(line.contains("alpha"), "got:\n{file}");
    assert!(!line.contains("beta"), "got:\n{file}");
    assert!(line.contains("gamma"), "got:\n{file}");
}

#[tokio::test(flavor = "current_thread")]
async fn set_property_seeds_a_markdown_only_note_before_first_edit() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();
    fs::write(
        mosaic.join("notes/pinned.md"),
        "---\ntitle: \"Pinned\"\ntype: \"Property\"\nvalue_type: \"checkbox\"\ntags: []\n---\n- Pinned property.\n",
    )
    .unwrap();
    fs::write(
        mosaic.join("notes/cold-scalar.md"),
        format!(
            "---\ntitle: \"Cold Scalar\"\ntags: []\n---\n- a cold task <!-- bid:{TASK_BID} -->\n  pinned:: false\n"
        ),
    )
    .unwrap();

    let (child, _addr, base) = common::spawn_with_retry(Duration::from_secs(15), |addr| {
        spawn_server_child(&mosaic, addr)
    });
    let _server = ServerGuard(Some(child));
    let client = reqwest::Client::new();

    client
        .post(format!("{base}/blocks/set-property"))
        .json(&serde_json::json!({
            "block_id": format!("cold-scalar:{TASK_BID}"),
            "key": "pinned",
            "value": "true",
        }))
        .send()
        .await
        .expect("POST /blocks/set-property")
        .error_for_status()
        .expect("cold scalar set ok");

    let file = fs::read_to_string(mosaic.join("notes/cold-scalar.md")).unwrap();
    assert!(file.contains("pinned:: true"), "got:\n{file}");
    assert_eq!(file.matches("pinned::").count(), 1, "got:\n{file}");
}
