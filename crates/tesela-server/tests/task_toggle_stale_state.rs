//! Regression coverage for task-status writes staying block-granular.
//!
//! A stale client toggling one task must not reassert old status/text for
//! sibling blocks it never touched. The test exercises the protected HTTP paths:
//! `/blocks/set-property` for task status, addressed in both direct-bid and
//! legacy line-id shapes, plus `/notes/{id}/blocks` for an unrelated text edit.

#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

const ALPHA_BID: &str = "10101010-1010-1010-1010-101010101010";
const BETA_BID: &str = "20202020-2020-2020-2020-202020202020";
const GAMMA_BID: &str = "30303030-3030-3030-3030-303030303030";

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

fn block_section<'a>(content: &'a str, bid: &str) -> Option<&'a str> {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.iter().position(|line| line.contains(bid))?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(idx, line)| line.trim_start().starts_with("- ").then_some(idx))
        .unwrap_or(lines.len());
    Some(&content[byte_offset_for_line(&lines, start)..byte_offset_for_line(&lines, end)])
}

fn byte_offset_for_line(lines: &[&str], line_idx: usize) -> usize {
    lines.iter().take(line_idx).map(|line| line.len() + 1).sum()
}

#[tokio::test(flavor = "current_thread")]
async fn task_toggle_does_not_reassert_stale_state() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();

    let port = pick_free_port();
    let addr = format!("127.0.0.1:{port}");
    let base = format!("http://{addr}");
    let _server = spawn_server(&mosaic, &addr);

    assert!(
        wait_for_port(&addr, Duration::from_secs(60)),
        "server never bound to {addr}"
    );

    let client = reqwest::Client::new();

    let seed_body = format!(
        "- alpha task <!-- bid:{ALPHA_BID} -->\n  tags:: Task\n  status:: todo\n- beta task <!-- bid:{BETA_BID} -->\n  tags:: Task\n  status:: todo\n- gamma note <!-- bid:{GAMMA_BID} -->\n"
    );
    let created: serde_json::Value = client
        .post(format!("{base}/notes"))
        .json(&serde_json::json!({
            "title": "Task Toggle Stale State",
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
        .post(format!("{base}/blocks/set-property"))
        .json(&serde_json::json!({
            "block_id": format!("{note_id}:{BETA_BID}"),
            "key": "status",
            "value": "done",
        }))
        .send()
        .await
        .expect("POST /blocks/set-property beta")
        .error_for_status()
        .expect("beta status write ok");

    client
        .post(format!("{base}/notes/{note_id}/blocks"))
        .json(&serde_json::json!({
            "ops": [
                {
                    "kind": "upsert",
                    "bid": GAMMA_BID,
                    "text": "gamma renamed by peer",
                    "parent_bid": null,
                    "indent_level": 0,
                }
            ]
        }))
        .send()
        .await
        .expect("POST /notes/{id}/blocks gamma")
        .error_for_status()
        .expect("gamma text write ok");

    client
        .post(format!("{base}/blocks/set-property"))
        .json(&serde_json::json!({
            "block_id": format!("{note_id}:0"),
            "key": "status",
            "value": "done",
        }))
        .send()
        .await
        .expect("POST /blocks/set-property alpha")
        .error_for_status()
        .expect("alpha status write ok");

    let after: serde_json::Value = client
        .get(format!("{base}/notes/{note_id}"))
        .send()
        .await
        .expect("GET /notes/{id}")
        .error_for_status()
        .expect("get note ok")
        .json()
        .await
        .expect("get note json");
    let content = after["content"].as_str().expect("content");

    let alpha = block_section(content, ALPHA_BID).expect("alpha block");
    assert!(
        alpha.contains("status:: done"),
        "stale client's intended task toggle must land; got:\n{alpha}"
    );

    let beta = block_section(content, BETA_BID).expect("beta block");
    assert!(
        beta.contains("status:: done"),
        "peer-completed sibling task must stay done; got:\n{beta}"
    );
    assert!(
        !beta.contains("status:: todo"),
        "stale task toggle must not reassert beta's old unchecked state; got:\n{beta}"
    );

    let gamma = block_section(content, GAMMA_BID).expect("gamma block");
    assert!(
        gamma.contains("gamma renamed by peer"),
        "unrelated concurrent text edit must survive task toggle; got:\n{gamma}"
    );

    let _ = temp.path();
}
