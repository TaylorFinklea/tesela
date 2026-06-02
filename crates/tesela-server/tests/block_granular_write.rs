//! HTTP-level proof of the block-granular write endpoint (Stage 0,
//! 2026-06-02 block-granular-writes spec).
//!
//! Trust artifact: proves the `POST /notes/{id}/blocks` endpoint kills the
//! concurrent-edit CLOBBER end-to-end through the real router + handler +
//! resident engine + on-disk materialization, mirroring the engine-level
//! `concurrent_whole_body_clobber.rs::block_granular_write_preserves_both_edits`.
//!
//! Scenario (the Stage-0 acceptance):
//!   1. Create a note with two blocks alpha + beta, each carrying an
//!      explicit `<!-- bid:UUID -->` marker so the engine keys its nodes to
//!      ids the client can address.
//!   2. A PEER edit lands on the server's authoritative engine first:
//!      `POST /blocks` upserting ONLY beta -> "beta PEER". (A peer's
//!      block-granular write is itself just a POST /blocks call; this is the
//!      same thing the engine-level repro does with a direct `record_local`.)
//!   3. The STALE client submits a block-granular write touching ONLY the
//!      block it actually changed: `POST /blocks` upserting alpha ->
//!      "alpha CHANGED". It sends NO op for beta.
//! Because no op for beta is submitted, beta's peer edit can never be
//! re-asserted stale, so BOTH survive — in the returned render AND in the
//! materialized `<slug>.md`. The cursor-free WS delta fan-out is covered by
//! reusing `update_note`'s verbatim tail (the binary frame path it shares).
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

#[tokio::test(flavor = "current_thread")]
async fn post_blocks_preserves_concurrent_peer_edit() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();

    let port = pick_free_port();
    let addr = format!("127.0.0.1:{}", port);
    let base = format!("http://{}", addr);
    // _server is the RAII guard — SIGTERMs the child on drop, including on
    // any assertion panic below.
    let _server = spawn_server(&mosaic, &addr);

    assert!(
        wait_for_port(&addr, Duration::from_secs(15)),
        "server never bound to {}",
        addr
    );

    let client = reqwest::Client::new();

    // 1. Create a note with alpha + beta, each carrying an explicit bid so
    //    the engine keys its nodes to ids we can address by block-granular op.
    let seed_body =
        format!("- alpha <!-- bid:{ALPHA_BID} -->\n- beta <!-- bid:{BETA_BID} -->\n");
    let created: serde_json::Value = client
        .post(format!("{}/notes", base))
        .json(&serde_json::json!({
            "title": "Granular Block Note",
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
    let note_id = created["id"]
        .as_str()
        .expect("note id in response")
        .to_string();

    // 2. PEER edit lands on the authoritative server engine first: upsert
    //    ONLY beta -> "beta PEER" via the block-granular endpoint.
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

    // 3. STALE client submits a block-granular write touching ONLY alpha.
    //    It sends NO op for beta, so beta's peer edit can't be clobbered.
    let after: serde_json::Value = client
        .post(format!("{}/notes/{}/blocks", base, note_id))
        .json(&serde_json::json!({
            "ops": [
                {
                    "kind": "upsert",
                    "bid": ALPHA_BID,
                    "text": "alpha CHANGED",
                    "parent_bid": null,
                    "indent_level": 0,
                }
            ]
        }))
        .send()
        .await
        .expect("POST /blocks (alpha)")
        .error_for_status()
        .expect("alpha block write ok")
        .json()
        .await
        .expect("alpha block write json");

    // Both edits survive in the handler's returned render (NO clobber).
    let render = after["content"].as_str().expect("content in response");
    assert!(
        render.contains("alpha CHANGED"),
        "client's own edit (alpha) should land; got:\n{render}"
    );
    assert!(
        render.contains("beta PEER"),
        "peer's concurrent beta edit MUST survive a block-granular write; got:\n{render}"
    );
    // No stale pre-peer-edit beta ghost bullet remains.
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

    // 4. The materialized `<slug>.md` on disk shows BOTH edits.
    let file = read_note_file_containing(&mosaic, "alpha CHANGED")
        .expect("a notes/*.md should hold 'alpha CHANGED'");
    assert!(
        file.contains("alpha CHANGED") && file.contains("beta PEER"),
        "materialized file must hold BOTH edits; got:\n{file}"
    );

    // Touch the tempdir so the compiler keeps it alive until here.
    let _ = temp.path();
}

/// HTTP-level proof of the positional-insert hint: a `POST /blocks` upsert
/// carrying `after_bid` inserts the new block IMMEDIATELY AFTER its
/// predecessor instead of appending at document end. This is the
/// mid-note-split fix (engine `create_at`) exercised through the real
/// router → handler → engine → materialization.
#[tokio::test(flavor = "current_thread")]
async fn post_blocks_after_bid_inserts_adjacent() {
    const A_BID: &str = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    const C_BID: &str = "0c0c0c0c-0c0c-0c0c-0c0c-0c0c0c0c0c0c";
    const B_BID: &str = "0b0b0b0b-0b0b-0b0b-0b0b-0b0b0b0b0b0b";

    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();

    let port = pick_free_port();
    let addr = format!("127.0.0.1:{}", port);
    let base = format!("http://{}", addr);
    let _server = spawn_server(&mosaic, &addr);
    assert!(
        wait_for_port(&addr, Duration::from_secs(15)),
        "server never bound to {addr}"
    );

    let client = reqwest::Client::new();

    // Seed a note with A, C (two top-level bullets).
    let seed_body = format!("- A <!-- bid:{A_BID} -->\n- C <!-- bid:{C_BID} -->\n");
    let created: serde_json::Value = client
        .post(format!("{}/notes", base))
        .json(&serde_json::json!({
            "title": "Positional Insert Note",
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

    // Insert a NEW block B AFTER A via after_bid. Expect A, B, C — NOT A, C, B.
    let after: serde_json::Value = client
        .post(format!("{}/notes/{}/blocks", base, note_id))
        .json(&serde_json::json!({
            "ops": [
                {
                    "kind": "upsert",
                    "bid": B_BID,
                    "text": "B",
                    "parent_bid": null,
                    "indent_level": 0,
                    "after_bid": A_BID,
                }
            ]
        }))
        .send()
        .await
        .expect("POST /blocks (insert B after A)")
        .error_for_status()
        .expect("insert ok")
        .json()
        .await
        .expect("insert json");

    let render = after["content"].as_str().expect("content");
    // Extract the order of the single-letter bullets.
    let order: Vec<&str> = render
        .lines()
        .filter_map(|l| {
            let t = l.trim_start_matches([' ', '\t', '-']).trim();
            let t = t.split(" <!-- bid:").next().unwrap_or(t).trim();
            (t == "A" || t == "B" || t == "C").then_some(t)
        })
        .collect();
    assert_eq!(
        order,
        vec!["A", "B", "C"],
        "B must land adjacent to A (after_bid), not at document end; got render:\n{render}"
    );

    let _ = temp.path();
}
