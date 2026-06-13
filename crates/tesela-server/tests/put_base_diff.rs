//! HTTP-level proof of the whole-body `PUT /notes/{id}` BASE-DIFF (the
//! LAST concurrent-edit clobber vector, 2026-06-02 base-diff spec).
//!
//! Trust artifact: proves that when a stale whole-body PUT carries
//! `base_content` (the author's edit base), the server diffs base→new and
//! applies ONLY the author's real block changes — so a concurrent peer
//! edit to a block the author never touched ALWAYS survives, end-to-end
//! through the real router + handler + resident engine + on-disk
//! materialization. Mirrors `block_granular_write.rs`'s harness.
//!
//! Three cases:
//!   1. `put_with_base_preserves_concurrent_peer_edit` (invariant 1):
//!      author changed only alpha; peer edited beta first; the stale PUT
//!      carries `base_content` (its pre-peer view). Both survive.
//!   2. `frontmatter_only_put_with_base_preserves_peer_block_edit`
//!      (invariant 2 — THE LOAD-BEARING TEST): author changed only a
//!      frontmatter field; peer edited beta first; the stale PUT carries
//!      `base_content`. The frontmatter-only NoteUpsert fallback must be
//!      body-preserving so it does NOT reseed the block tree over the
//!      peer's beta edit. beta PEER survives.
//!   3. `put_without_base_still_clobbers` (invariant 3 — backward compat):
//!      a PUT WITHOUT `base_content` behaves exactly as before: the
//!      server-file→new diff re-asserts the stale beta and clobbers the
//!      peer. Documents that the change is additive (clients opt in by
//!      sending the base).
//!
//! Skipped on non-Unix (spawns the server binary, SIGTERMs to shut down).

#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

const ALPHA_BID: &str = "01010101-0101-0101-0101-010101010101";
const BETA_BID: &str = "02020202-0202-0202-0202-020202020202";

fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_tesela-server"))
}

fn pick_free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

fn wait_for_port(addr: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if std::net::TcpStream::connect(addr).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

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

/// Owns the spawned server process and SIGTERMs it on drop so the server is
/// reaped even if the test panics mid-flight.
struct ServerGuard(Option<Child>);

impl Drop for ServerGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let pid = child.id() as i32;
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
            let _ = child.wait();
        }
    }
}

fn spawn_server(mosaic: &Path, addr: &str) -> ServerGuard {
    let child = Command::new(binary_path())
        .current_dir(mosaic)
        .env("TESELA_SERVER_BIND", addr)
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tesela-server");
    ServerGuard(Some(child))
}

/// Find the materialized note file under `notes/` whose content holds
/// `needle`.
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

/// Spin up a server, create a note with alpha + beta (explicit bids), and
/// land a PEER edit to beta -> "beta PEER" via the block-granular endpoint
/// so the authoritative engine holds the peer's edit before the stale PUT.
/// Returns (mosaic dir, server guard, base url, note id, seed body).
async fn setup_with_peer_beta_edit(
    temp: &TempDir,
    client: &reqwest::Client,
) -> (PathBuf, ServerGuard, String, String, String) {
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();

    let port = pick_free_port();
    let addr = format!("127.0.0.1:{}", port);
    let base = format!("http://{}", addr);
    let server = spawn_server(&mosaic, &addr);

    assert!(
        wait_for_port(&addr, Duration::from_secs(60)),
        "server never bound to {}",
        addr
    );

    let seed_body = format!("- alpha <!-- bid:{ALPHA_BID} -->\n- beta <!-- bid:{BETA_BID} -->\n");
    let created: serde_json::Value = client
        .post(format!("{}/notes", base))
        .json(&serde_json::json!({
            "title": "Base Diff Note",
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

    // PEER edit lands first: upsert ONLY beta -> "beta PEER".
    let peer: serde_json::Value = client
        .post(format!("{}/notes/{}/blocks", base, note_id))
        .json(&serde_json::json!({
            "ops": [
                {
                    "kind": "upsert",
                    "bid": BETA_BID,
                    "text": "beta PEER",
                    "parent_bid": null,
                    "indent_level": 0,
                }
            ]
        }))
        .send()
        .await
        .expect("POST /blocks (peer beta)")
        .error_for_status()
        .expect("peer block write ok")
        .json()
        .await
        .expect("peer block write json");
    assert!(
        peer["content"]
            .as_str()
            .unwrap_or_default()
            .contains("beta PEER"),
        "peer write should land beta PEER; got: {:?}",
        peer["content"]
    );

    (mosaic, server, base, note_id, seed_body)
}

/// Invariant 1: a stale whole-body PUT carrying `base_content` re-asserts
/// ONLY the blocks the author actually changed. The author changed alpha
/// but its beta is the OLD (pre-peer) text — yet because beta is identical
/// base→new, NO op is emitted for it, so the peer's "beta PEER" survives.
#[tokio::test(flavor = "current_thread")]
async fn put_with_base_preserves_concurrent_peer_edit() {
    let temp = TempDir::new().unwrap();
    let client = reqwest::Client::new();
    let (mosaic, _server, base, note_id, seed_body) =
        setup_with_peer_beta_edit(&temp, &client).await;

    // The STALE author's whole-body PUT: it changed alpha ("alpha
    // CHANGED") but carries the OLD beta. It sends `base_content` = the
    // body it started from (the pre-peer seed, with old beta).
    let stale_new =
        format!("- alpha CHANGED <!-- bid:{ALPHA_BID} -->\n- beta <!-- bid:{BETA_BID} -->\n");
    let after: serde_json::Value = client
        .put(format!("{}/notes/{}", base, note_id))
        .json(&serde_json::json!({
            "content": stale_new,
            "base_content": seed_body,
        }))
        .send()
        .await
        .expect("PUT /notes")
        .error_for_status()
        .expect("PUT ok")
        .json()
        .await
        .expect("PUT json");

    let render = after["content"].as_str().expect("content in response");
    assert!(
        render.contains("alpha CHANGED"),
        "author's own edit (alpha) should land; got:\n{render}"
    );
    assert!(
        render.contains("beta PEER"),
        "peer's concurrent beta edit MUST survive a base-diff PUT; got:\n{render}"
    );
    // No stale pre-peer-edit beta ghost.
    assert_no_stale_beta(render);

    // Materialized file on disk shows BOTH edits.
    let file = read_note_file_containing(&mosaic, "alpha CHANGED")
        .expect("a notes/*.md should hold 'alpha CHANGED'");
    assert!(
        file.contains("alpha CHANGED") && file.contains("beta PEER"),
        "materialized file must hold BOTH edits; got:\n{file}"
    );
}

/// Invariant 2 (THE LOAD-BEARING TEST): a stale whole-body PUT that
/// changes ONLY a frontmatter field (no block changed) yields an empty
/// block diff → NoteUpsert fallback. With a base present that fallback is
/// BODY-PRESERVING: it must NOT reseed the block tree from the author's
/// stale body, so the peer's concurrent "beta PEER" survives.
#[tokio::test(flavor = "current_thread")]
async fn frontmatter_only_put_with_base_preserves_peer_block_edit() {
    let temp = TempDir::new().unwrap();
    let client = reqwest::Client::new();
    let (mosaic, _server, base, note_id, _seed_body) =
        setup_with_peer_beta_edit(&temp, &client).await;

    // The author's edit BASE: a body with frontmatter + the two blocks,
    // beta still the OLD pre-peer text (the author hasn't seen "beta PEER").
    let base_with_fm = format!(
        "---\ntitle: \"Old Title\"\n---\n\
         - alpha <!-- bid:{ALPHA_BID} -->\n- beta <!-- bid:{BETA_BID} -->\n"
    );
    // The author changes ONLY the frontmatter title. Blocks are byte-for-
    // byte the base's (stale beta included) — so the base→new BLOCK diff is
    // empty and we hit the frontmatter-only NoteUpsert fallback.
    let new_with_fm = format!(
        "---\ntitle: \"New Title\"\n---\n\
         - alpha <!-- bid:{ALPHA_BID} -->\n- beta <!-- bid:{BETA_BID} -->\n"
    );
    let after: serde_json::Value = client
        .put(format!("{}/notes/{}", base, note_id))
        .json(&serde_json::json!({
            "content": new_with_fm,
            "base_content": base_with_fm,
        }))
        .send()
        .await
        .expect("PUT /notes (frontmatter-only)")
        .error_for_status()
        .expect("PUT ok")
        .json()
        .await
        .expect("PUT json");

    let render = after["content"].as_str().expect("content in response");
    assert!(
        render.contains("beta PEER"),
        "peer's concurrent beta edit MUST survive a frontmatter-only base-diff PUT \
         (no body reseed clobber); got:\n{render}"
    );
    assert_no_stale_beta(render);

    // Materialized file on disk still holds the peer's beta edit.
    let file = read_note_file_containing(&mosaic, "beta PEER")
        .expect("a notes/*.md should still hold 'beta PEER'");
    assert!(
        file.contains("beta PEER"),
        "materialized file must still hold the peer's beta edit; got:\n{file}"
    );
}

/// Invariant 3 (backward compat): a PUT WITHOUT `base_content` behaves
/// exactly as before — the server-file→new diff re-asserts the stale beta
/// and clobbers the peer. Pinned as documentation so a future change that
/// flips it forces a deliberate revisit of the legacy no-base path.
#[tokio::test(flavor = "current_thread")]
async fn put_without_base_still_clobbers() {
    let temp = TempDir::new().unwrap();
    let client = reqwest::Client::new();
    let (_mosaic, _server, base, note_id, _seed_body) =
        setup_with_peer_beta_edit(&temp, &client).await;

    // Same stale PUT, but NO `base_content` field — the legacy path.
    let stale_new =
        format!("- alpha CHANGED <!-- bid:{ALPHA_BID} -->\n- beta <!-- bid:{BETA_BID} -->\n");
    let after: serde_json::Value = client
        .put(format!("{}/notes/{}", base, note_id))
        .json(&serde_json::json!({
            "content": stale_new,
        }))
        .send()
        .await
        .expect("PUT /notes (no base)")
        .error_for_status()
        .expect("PUT ok")
        .json()
        .await
        .expect("PUT json");

    let render = after["content"].as_str().expect("content in response");
    assert!(
        render.contains("alpha CHANGED"),
        "author's own edit (alpha) should land; got:\n{render}"
    );
    // THE LEGACY BUG, asserted as documentation: without a base the stale
    // whole-body PUT re-asserts old "beta", clobbering "beta PEER".
    assert!(
        !render.contains("beta PEER"),
        "expected the legacy no-base PUT to clobber the peer's beta edit \
         (documents backward compat); got:\n{render}"
    );
}

/// Spin up a server and create a note with frontmatter + alpha/beta blocks
/// (explicit bids). NO peer edit. Returns (mosaic, server, base url, note id,
/// the seed body that was POSTed). The seed body carries a `title` + `tags`
/// frontmatter so a bundled PUT can change both a block AND the frontmatter.
async fn setup_with_frontmatter(
    temp: &TempDir,
    client: &reqwest::Client,
) -> (PathBuf, ServerGuard, String, String, String) {
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();

    let port = pick_free_port();
    let addr = format!("127.0.0.1:{}", port);
    let base = format!("http://{}", addr);
    let server = spawn_server(&mosaic, &addr);

    assert!(
        wait_for_port(&addr, Duration::from_secs(60)),
        "server never bound to {}",
        addr
    );

    let seed_body = format!(
        "---\ntitle: \"Old Title\"\ntags: [a]\n---\n\
         - alpha <!-- bid:{ALPHA_BID} -->\n- beta <!-- bid:{BETA_BID} -->\n"
    );
    let created: serde_json::Value = client
        .post(format!("{}/notes", base))
        .json(&serde_json::json!({
            "title": "Bundled Note",
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

    (mosaic, server, base, note_id, seed_body)
}

/// THE REGRESSION TEST: a single PUT that changes BOTH a block's text AND a
/// frontmatter field must land BOTH. Before the fix the non-empty block diff
/// short-circuited and the bundled frontmatter change was silently dropped.
#[tokio::test(flavor = "current_thread")]
async fn bundled_block_and_frontmatter_put_lands_both() {
    let temp = TempDir::new().unwrap();
    let client = reqwest::Client::new();
    let (mosaic, _server, base, note_id, seed_body) = setup_with_frontmatter(&temp, &client).await;

    // ONE PUT: change alpha's text AND the frontmatter (title + tags),
    // carrying `base_content` = the original seed so the diff is base→new.
    let new_body = format!(
        "---\ntitle: \"New Title\"\ntags: [a, b]\n---\n\
         - alpha CHANGED <!-- bid:{ALPHA_BID} -->\n- beta <!-- bid:{BETA_BID} -->\n"
    );
    let after: serde_json::Value = client
        .put(format!("{}/notes/{}", base, note_id))
        .json(&serde_json::json!({
            "content": new_body,
            "base_content": seed_body,
        }))
        .send()
        .await
        .expect("PUT /notes (bundled)")
        .error_for_status()
        .expect("PUT ok")
        .json()
        .await
        .expect("PUT json");

    let render = after["content"].as_str().expect("content in response");
    assert!(
        render.contains("alpha CHANGED"),
        "the block change must land; got:\n{render}"
    );
    assert!(
        render.contains("New Title"),
        "the bundled frontmatter title change must land (was silently dropped \
         before the fix); got:\n{render}"
    );
    assert!(
        render.contains("a, b") || render.contains("a,b"),
        "the bundled frontmatter tags change must land; got:\n{render}"
    );

    // Materialized file on disk shows BOTH the block AND the frontmatter edit.
    let file = read_note_file_containing(&mosaic, "alpha CHANGED")
        .expect("a notes/*.md should hold 'alpha CHANGED'");
    assert!(
        file.contains("alpha CHANGED"),
        "materialized file must hold the block edit; got:\n{file}"
    );
    assert!(
        file.contains("New Title"),
        "materialized file must hold the frontmatter edit; got:\n{file}"
    );
}

/// CONCURRENT-SAFETY VARIANT: a peer edited a DIFFERENT block (beta) first;
/// the author then PUTs a bundled change (alpha block + frontmatter) carrying
/// its pre-peer base. The bundled frontmatter NoteUpsert must be
/// body-preserving — it applies AFTER the alpha block op over the server's
/// CURRENT (post-peer, post-alpha) blocks — so the peer's "beta PEER"
/// survives, alpha lands, and the frontmatter lands.
#[tokio::test(flavor = "current_thread")]
async fn bundled_block_and_frontmatter_put_preserves_peer_block_edit() {
    let temp = TempDir::new().unwrap();
    let client = reqwest::Client::new();
    let (mosaic, _server, base, note_id, _seed_body) =
        setup_with_peer_beta_edit(&temp, &client).await;

    // The author's base: frontmatter + the two blocks, beta still the OLD
    // pre-peer text (the author never saw "beta PEER").
    let base_with_fm = format!(
        "---\ntitle: \"Old Title\"\n---\n\
         - alpha <!-- bid:{ALPHA_BID} -->\n- beta <!-- bid:{BETA_BID} -->\n"
    );
    // The author changes alpha's text AND the frontmatter title in one PUT,
    // but carries the stale beta. base→new block diff = alpha only; the
    // frontmatter change is bundled alongside it.
    let new_with_fm = format!(
        "---\ntitle: \"New Title\"\n---\n\
         - alpha CHANGED <!-- bid:{ALPHA_BID} -->\n- beta <!-- bid:{BETA_BID} -->\n"
    );
    let after: serde_json::Value = client
        .put(format!("{}/notes/{}", base, note_id))
        .json(&serde_json::json!({
            "content": new_with_fm,
            "base_content": base_with_fm,
        }))
        .send()
        .await
        .expect("PUT /notes (bundled + concurrent)")
        .error_for_status()
        .expect("PUT ok")
        .json()
        .await
        .expect("PUT json");

    let render = after["content"].as_str().expect("content in response");
    assert!(
        render.contains("alpha CHANGED"),
        "author's alpha block edit must land; got:\n{render}"
    );
    assert!(
        render.contains("New Title"),
        "bundled frontmatter change must land; got:\n{render}"
    );
    assert!(
        render.contains("beta PEER"),
        "peer's concurrent beta block edit MUST survive the bundled \
         frontmatter NoteUpsert (no body reseed clobber); got:\n{render}"
    );
    assert_no_stale_beta(render);

    // Materialized file holds all three: alpha edit, frontmatter, peer beta.
    let file = read_note_file_containing(&mosaic, "alpha CHANGED")
        .expect("a notes/*.md should hold 'alpha CHANGED'");
    assert!(
        file.contains("alpha CHANGED") && file.contains("New Title") && file.contains("beta PEER"),
        "materialized file must hold the block edit, frontmatter edit, AND \
         the peer's beta edit; got:\n{file}"
    );
}

/// Assert no stale pre-peer-edit `beta` bullet remains (only "beta PEER").
fn assert_no_stale_beta(render: &str) {
    let stale_beta = render
        .lines()
        .filter(|l| {
            let t = l.trim_start_matches([' ', '\t', '-']).trim();
            t.starts_with("beta") && !t.starts_with("beta PEER")
        })
        .count();
    assert_eq!(
        stale_beta, 0,
        "no stale 'beta' bullet should remain; got:\n{render}"
    );
}
