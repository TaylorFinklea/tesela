//! HTTP-level coverage for the keybinding + leader-tree user config route
//! (tesela-cmdd.4): `GET /keymap-config` defaults to empty, `PUT` persists
//! the whole blob, and a fresh server process reading the SAME mosaic
//! (simulating "a second device pointed at this server") sees the saved
//! config — the acceptance bar for the web surface, where a browser is
//! just a thin HTTP client with no local sync engine of its own.
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
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tesela-server")
}

fn boot(mosaic: &Path) -> (ServerGuard, String) {
    let (child, _addr, base) = common::spawn_with_retry(Duration::from_secs(15), |addr| {
        spawn_server_child(mosaic, addr)
    });
    (ServerGuard(Some(child)), base)
}

#[tokio::test(flavor = "current_thread")]
async fn get_defaults_to_empty_then_put_round_trips() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();
    let (_server, base) = boot(&mosaic);
    let client = reqwest::Client::new();

    let empty = client
        .get(format!("{base}/keymap-config"))
        .send()
        .await
        .expect("GET /keymap-config")
        .error_for_status()
        .expect("GET ok")
        .json::<serde_json::Value>()
        .await
        .expect("json");
    assert_eq!(
        empty,
        serde_json::json!({ "overrides": {}, "group_labels": {} }),
        "fresh mosaic has no saved keymap config"
    );

    let body = serde_json::json!({
        "overrides": {
            "nav.daily": { "shortcut": "⌘D", "chord": ["g", "d"] },
        },
        "group_labels": { "b": "My Blocks" },
    });
    let put_resp = client
        .put(format!("{base}/keymap-config"))
        .json(&body)
        .send()
        .await
        .expect("PUT /keymap-config")
        .error_for_status()
        .expect("PUT ok")
        .json::<serde_json::Value>()
        .await
        .expect("json");
    assert_eq!(put_resp, body, "PUT echoes back the saved config");

    let got = client
        .get(format!("{base}/keymap-config"))
        .send()
        .await
        .expect("GET /keymap-config")
        .error_for_status()
        .expect("GET ok")
        .json::<serde_json::Value>()
        .await
        .expect("json");
    assert_eq!(got, body, "GET after PUT reflects the saved config");
}

/// A second server process reading the SAME mosaic root sees the config the
/// first process saved — the literal "second device" acceptance check for
/// the web surface (a browser hitting a different tesela-server instance
/// pointed at the same mosaic directory, e.g. over Tailscale).
#[tokio::test(flavor = "current_thread")]
async fn config_persists_across_server_restart() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();

    let body = serde_json::json!({
        "overrides": {
            "block.delete": { "chord": ["b", "D"] },
        },
        "group_labels": { "g": "Jump to…" },
    });

    {
        let (_server, base) = boot(&mosaic);
        let client = reqwest::Client::new();
        client
            .put(format!("{base}/keymap-config"))
            .json(&body)
            .send()
            .await
            .expect("PUT /keymap-config")
            .error_for_status()
            .expect("PUT ok");
    }

    // Fresh process, same mosaic root.
    let (_server2, base2) = boot(&mosaic);
    let client = reqwest::Client::new();
    let got = client
        .get(format!("{base2}/keymap-config"))
        .send()
        .await
        .expect("GET /keymap-config")
        .error_for_status()
        .expect("GET ok")
        .json::<serde_json::Value>()
        .await
        .expect("json");
    assert_eq!(got, body, "config saved by the first process is visible to the second");
}
