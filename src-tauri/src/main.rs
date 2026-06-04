// Tesela desktop — a native (Tauri) window around the `/g` web UI, with an
// embedded `tesela-server` child bound to LOOPBACK ONLY. The child serves both
// the API and the static UI on one origin, so the webview just loads
// `http://127.0.0.1:<port>/g` — same-origin, no CORS, and the UI's existing
// host-derived WS URL works unchanged.
//
// Design rule (sync model): the embedded server is a LOOPBACK Loro-replica
// node, NOT a hub. It binds 127.0.0.1 only and mDNS is disabled — other devices
// never point at it; cross-device sync flows through the spine (relay/LAN), the
// same transport as iOS. Do not let this bind 0.0.0.0 / become a hub.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

/// Holds the embedded server child so the app can reap it on exit.
struct ServerChild(Mutex<Option<Child>>);

/// Ask the OS for a free loopback port by binding `:0` then dropping it. A tiny
/// TOCTOU window exists before the child re-binds; acceptable for a local app.
fn pick_free_loopback_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral loopback port")
        .local_addr()
        .expect("local_addr")
        .port()
}

/// Workspace root = parent of this crate's dir (`src-tauri/`).
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Locate the `tesela-server` binary. `TESELA_SERVER_BIN` overrides; otherwise
/// the dev build under `target/debug/`. (A bundled release will ship it as a
/// Tauri sidecar resource — that's a packaging follow-up.)
fn tesela_server_bin() -> PathBuf {
    if let Ok(p) = std::env::var("TESELA_SERVER_BIN") {
        return PathBuf::from(p);
    }
    let root = workspace_root();
    let release = root.join("target").join("release").join("tesela-server");
    if release.exists() {
        return release;
    }
    root.join("target").join("debug").join("tesela-server")
}

/// Resolve the mosaic the embedded server should open. `TESELA_MOSAIC` wins;
/// otherwise, when launched from Finder (no env), default to the user's primary
/// mosaic so the bundled app opens real notes instead of auto-initializing a
/// blank one. Returns None to let the server's own `find_mosaic` decide.
fn resolve_mosaic() -> Option<PathBuf> {
    if let Ok(m) = std::env::var("TESELA_MOSAIC") {
        return Some(PathBuf::from(m));
    }
    let home = std::env::var_os("HOME")?;
    let primary = PathBuf::from(home).join("Library/Application Support/tesela/logseq");
    primary.join(".tesela").exists().then_some(primary)
}

/// The built static `/g` bundle the embedded server serves. `TESELA_STATIC_DIR`
/// overrides; otherwise `web/build`.
fn static_dir() -> PathBuf {
    if let Ok(p) = std::env::var("TESELA_STATIC_DIR") {
        return PathBuf::from(p);
    }
    workspace_root().join("web").join("build")
}

/// Block until the child is accepting connections on `port`, or `timeout`.
fn wait_for_port(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn spawn_server(bind: &str) -> std::io::Result<Child> {
    let mut cmd = Command::new(tesela_server_bin());
    cmd.env("TESELA_SERVER_BIND", bind)
        .env("TESELA_STATIC_DIR", static_dir())
        // Loopback node — never advertise on the LAN (we are not a hub).
        .env("TESELA_DISABLE_MDNS", "1")
        // ...and never participate as a relay/LAN-peer writer: the embed is a
        // loopback Loro-replica node; cross-device sync flows through the spine.
        // Prevents a second writer under the shared device_id alongside any
        // standalone server.
        .env("TESELA_DISABLE_RELAY", "1")
        .env("TESELA_DISABLE_PEER_SYNC", "1")
        // Belt to the shell's suspenders: if THIS process dies non-gracefully
        // (crash/SIGKILL, where our Exit handler never runs), the server exits
        // itself rather than orphaning. The explicit pid closes the spawn-race
        // where the parent dies before the child observes its ppid.
        .env("TESELA_EXIT_WITH_PARENT", "1")
        .env("TESELA_PARENT_PID", std::process::id().to_string())
        .env(
            "RUST_LOG",
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,loro=warn".to_string()),
        );
    // Explicit `TESELA_MOSAIC`, else the user's primary mosaic when launched
    // from Finder, else let the server's find_mosaic decide.
    if let Some(mosaic) = resolve_mosaic() {
        cmd.env("TESELA_DEFAULT_MOSAIC", mosaic);
    }
    cmd.spawn()
}

fn main() {
    let port = pick_free_loopback_port();
    let bind = format!("127.0.0.1:{port}");

    let child = match spawn_server(&bind) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "tesela-desktop: failed to spawn {} ({e}). Build it first: `cargo build -p tesela-server`.",
                tesela_server_bin().display()
            );
            std::process::exit(1);
        }
    };

    if !wait_for_port(port, Duration::from_secs(20)) {
        // The server never came up. The likeliest cause is the single-writer
        // lock being held — another tesela-server (a second app launch, or a
        // standalone server) already owns this mosaic. Don't open a window onto
        // a dead port (a blank "connection refused" page); reap + exit clearly.
        let mut child = child;
        let _ = child.kill();
        let _ = child.wait();
        eprintln!(
            "tesela-desktop: the embedded tesela-server didn't start on {bind} \
             within 20s — another instance may already be running on this mosaic. Exiting."
        );
        std::process::exit(1);
    }

    let url = format!("http://127.0.0.1:{port}/g");

    tauri::Builder::default()
        .manage(ServerChild(Mutex::new(Some(child))))
        .setup(move |app| {
            WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::External(url.parse().expect("valid loopback url")),
            )
            .title("Tesela")
            .inner_size(1280.0, 860.0)
            .min_inner_size(900.0, 600.0)
            // Runs before the served bundle: tells the UI to use same-origin
            // (the embedded server serves API + UI on this one origin).
            .initialization_script("window.__TESELA_API_BASE__ = '';")
            .build()?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tesela-desktop")
        .run(|app, event| {
            // Reap the embedded server when the app exits so it doesn't outlive
            // the window (and free its loopback port). Prefer SIGTERM so the
            // server runs its graceful shutdown (drain + auto-backup); fall back
            // to SIGKILL if it doesn't exit promptly.
            if let RunEvent::Exit = event {
                if let Some(state) = app.try_state::<ServerChild>() {
                    if let Some(mut child) = state.0.lock().unwrap().take() {
                        #[cfg(unix)]
                        // SAFETY: child.id() is this process's direct child pid.
                        unsafe {
                            libc::kill(child.id() as libc::pid_t, libc::SIGTERM);
                        }
                        let mut exited = false;
                        for _ in 0..50 {
                            if matches!(child.try_wait(), Ok(Some(_))) {
                                exited = true;
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        if !exited {
                            let _ = child.kill();
                        }
                        let _ = child.wait();
                    }
                }
            }
        });
}
