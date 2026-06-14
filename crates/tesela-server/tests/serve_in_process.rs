//! L4 Phase A: the in-process `tesela_server::serve` path — the contract the
//! desktop Tauri embed (Phase B) relies on. Unlike the other integration tests
//! (which spawn the `tesela-server` BINARY as a child), this drives the LIBRARY
//! `serve` on the test's own tokio runtime, exactly as the embedder will:
//!   * binds `127.0.0.1:0` and hands the real port back via `on_bound`,
//!   * serves `/health` while running,
//!   * returns cleanly when the caller-supplied `shutdown` future resolves,
//!   * and — booting a SECOND time on the same mosaic — proves the single-
//!     writer flock was released when the first `serve` returned (the embedder-
//!     restart / clean-quit invariant).

#![cfg(unix)]

use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use tempfile::TempDir;
use tesela_server::{serve, ServeConfig};

fn make_fixture_mosaic(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join("notes"))?;
    fs::create_dir_all(root.join("attachments"))?;
    fs::create_dir_all(root.join(".tesela"))?;
    // auto_on_quit=false keeps the graceful-shutdown path fast (no VACUUM).
    fs::write(
        root.join(".tesela/config.toml"),
        "[backup]\nauto_on_quit = false\n",
    )?;
    Ok(())
}

/// Boot `serve` in-process, GET `/health`, then fire the shutdown future and
/// wait for `serve` to return. Returns the bound port (proves a fresh one is
/// allocated each time from `:0`).
async fn boot_health_shutdown(mosaic: &Path) -> u16 {
    // Bind an ephemeral loopback port; keep the run hermetic (no mDNS/peers).
    std::env::set_var("TESELA_SERVER_BIND", "127.0.0.1:0");
    std::env::set_var("TESELA_DISABLE_MDNS", "1");
    std::env::set_var("TESELA_DISABLE_PEER_SYNC", "1");

    let config = ServeConfig::resolve(Some(mosaic.to_path_buf())).expect("resolve mosaic");

    let (bound_tx, bound_rx) = tokio::sync::oneshot::channel::<SocketAddr>();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let handle = tokio::spawn(async move {
        serve(
            config,
            async move {
                let _ = shutdown_rx.await;
            },
            move |addr| {
                let _ = bound_tx.send(addr);
            },
        )
        .await
    });

    // The embedder reads the real bound address here to build its webview URL.
    let addr = tokio::time::timeout(Duration::from_secs(20), bound_rx)
        .await
        .expect("serve bound within 20s")
        .expect("on_bound fired with the address");

    // /health must answer while serve runs.
    let body = reqwest::get(format!("http://{addr}/health"))
        .await
        .expect("GET /health connects")
        .error_for_status()
        .expect("/health is 200");
    drop(body);

    // Caller-driven graceful shutdown — serve must drain and return Ok.
    shutdown_tx.send(()).expect("shutdown receiver still alive");
    let result = tokio::time::timeout(Duration::from_secs(20), handle)
        .await
        .expect("serve returns within 20s of shutdown")
        .expect("serve task did not panic");
    result.expect("serve returned Ok");

    addr.port()
}

#[tokio::test(flavor = "multi_thread")]
async fn serve_boots_in_process_serves_health_and_shuts_down_releasing_the_flock() {
    let dir = TempDir::new().expect("temp mosaic");
    make_fixture_mosaic(dir.path()).expect("fixture mosaic");

    // First boot: prove the in-process serve path works end to end.
    let port1 = boot_health_shutdown(dir.path()).await;
    assert_ne!(port1, 0, "a real ephemeral port was bound");

    // Second boot on the SAME mosaic: only succeeds if the first serve released
    // the single-writer flock on return. This is the embedder restart / clean-
    // quit invariant — if the flock leaked, acquire_mosaic_lock would bail and
    // serve would error, failing this call.
    let _port2 = boot_health_shutdown(dir.path()).await;
}
