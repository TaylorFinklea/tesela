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
//
// REMOTE MODE: when a remote URL is configured (env `TESELA_DESKTOP_REMOTE_URL`
// or `remote_url` in `~/Library/Application Support/tesela/desktop.toml`), the
// desktop does NOT embed a server — it just wraps that external server's `/g`
// (a LAN/Tailscale hub, or the cloud relay). The native window keeps the full
// Vim experience while sharing one server with iOS. This is the desktop acting
// as a thin client of the spine, the multi-device endgame.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::menu::{Menu, MenuItem, Submenu};
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
/// pick whichever of `target/release/` or `target/debug/` was built MOST
/// RECENTLY. (A bundled release will ship it as a Tauri sidecar resource —
/// that's a packaging follow-up.)
///
/// Newest-by-mtime, not release-preferred: a stale release binary built before
/// a server feature landed must never shadow a fresher debug build. That exact
/// mismatch — a release `tesela-server` predating `TESELA_STATIC_DIR` support —
/// served a static-less server to the desktop, so `/g` 404'd and the window
/// rendered blank. mtime tracks "what I last compiled," which is what should run.
fn tesela_server_bin() -> PathBuf {
    if let Ok(p) = std::env::var("TESELA_SERVER_BIN") {
        return PathBuf::from(p);
    }
    let root = workspace_root();
    let release = root.join("target").join("release").join("tesela-server");
    let debug = root.join("target").join("debug").join("tesela-server");
    let mtime = |p: &std::path::Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    match (mtime(&release), mtime(&debug)) {
        (Some(r), Some(d)) => {
            if r >= d {
                release
            } else {
                debug
            }
        }
        (Some(_), None) => release,
        // Neither present → return the debug path so the spawn error names a
        // concrete build target.
        (None, _) => debug,
    }
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

/// If the desktop should connect to an EXTERNAL tesela-server (a LAN/Tailscale
/// hub or the relay) instead of embedding its own loopback node, return its base
/// URL. Source: `TESELA_DESKTOP_REMOTE_URL` env (wins, for terminal launches),
/// else a `remote_url = "..."` line in
/// `~/Library/Application Support/tesela/desktop.toml` (works for Finder/Dock
/// launches, which don't inherit shell env). `None` → embed (default).
fn resolve_remote_url() -> Option<String> {
    desktop_config_value("TESELA_DESKTOP_REMOTE_URL", "remote_url")
}

/// If the EMBEDDED server should JOIN the relay (the spine) directly — instead
/// of the default loopback-only Loro-replica — return the relay base URL.
/// Source: `TESELA_EMBED_RELAY_URL` env, else `relay_url = "..."` in
/// desktop.toml. When set, `spawn_server` drops `TESELA_DISABLE_RELAY` and
/// points the child at this URL (mDNS stays off — it's a relay participant, not
/// a LAN hub). `None` → embed stays loopback-only (default).
/// ⚠ Only opt in when this embed is the SOLE writer for the mosaic — never
/// alongside a standalone server, or two relay participants share one device_id.
fn resolve_embed_relay_url() -> Option<String> {
    desktop_config_value("TESELA_EMBED_RELAY_URL", "relay_url")
}

/// Read a `key = "value"` line from `desktop.toml`, with an `env_var` override
/// (env wins — terminal launches; the file works for Finder/Dock launches that
/// don't inherit shell env). A hand-rolled scan, not a TOML parse — the file is
/// a handful of flat keys.
fn desktop_config_value(env_var: &str, key: &str) -> Option<String> {
    if let Ok(v) = std::env::var(env_var) {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return Some(v);
        }
    }
    let home = std::env::var_os("HOME")?;
    let cfg = PathBuf::from(home).join("Library/Application Support/tesela/desktop.toml");
    let text = std::fs::read_to_string(&cfg).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix(key) {
            // Require a whole-key match (next char is `=` or whitespace) so
            // `relay_url` can't match a longer key like `relay_url_extra`.
            if rest
                .chars()
                .next()
                .is_some_and(|c| c != '=' && !c.is_whitespace())
            {
                continue;
            }
            let raw = rest
                .trim_start_matches(|c: char| c == '=' || c.is_whitespace())
                .trim();
            let mut in_quotes = false;
            let mut escaped = false;
            let mut comment_start = raw.len();
            for (idx, ch) in raw.char_indices() {
                if escaped {
                    escaped = false;
                    continue;
                }
                match ch {
                    '\\' if in_quotes => escaped = true,
                    '"' => in_quotes = !in_quotes,
                    '#' if !in_quotes => {
                        comment_start = idx;
                        break;
                    }
                    _ => {}
                }
            }
            let val = raw[..comment_start].trim().trim_matches('"').trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
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
/// Returns `Err(reason)` early if the child has already exited — listening
/// on a dead child for the full timeout would mask the real failure (flock
/// conflict, bad binary, panic) behind a misleading "20s timeout" error.
fn wait_for_port(child: &mut Child, port: u16, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Ok(());
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!(
                "server child exited before binding (status: {status})"
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(format!("server did not start within {timeout:?}"))
}

fn spawn_server(bind: &str) -> std::io::Result<Child> {
    let mut cmd = Command::new(tesela_server_bin());
    cmd.env("TESELA_SERVER_BIND", bind)
        .env("TESELA_STATIC_DIR", static_dir())
        // Loopback node — never advertise on the LAN (we are not a hub). This
        // stays set REGARDLESS of relay opt-in: the embed binds 127.0.0.1 and
        // must never mDNS-advertise.
        .env("TESELA_DISABLE_MDNS", "1")
        // The LAN peer data-plane is retired; the embed never LAN-peer-writes.
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
    // Relay participation. DEFAULT: the embed is a loopback-only Loro-replica
    // (cross-device sync flows through the spine, reached via a standalone hub
    // or remote mode) — so it sets TESELA_DISABLE_RELAY to avoid being a second
    // writer under the shared device_id. OPTED IN (relay_url in desktop.toml /
    // TESELA_EMBED_RELAY_URL): the embed becomes a first-class relay participant
    // — drop the disable + point it at the relay. mDNS stays off; it's still not
    // a LAN hub. ⚠ Never run an opted-in embed AND a standalone server on the
    // same mosaic (two relay participants under one device_id corrupts cursors).
    match resolve_embed_relay_url() {
        Some(url) => {
            cmd.env_remove("TESELA_DISABLE_RELAY");
            cmd.env("TESELA_RELAY_URL", url);
        }
        None => {
            cmd.env("TESELA_DISABLE_RELAY", "1");
        }
    }
    // Explicit `TESELA_MOSAIC`, else the user's primary mosaic when launched
    // from Finder, else let the server's find_mosaic decide.
    if let Some(mosaic) = resolve_mosaic() {
        cmd.env("TESELA_DEFAULT_MOSAIC", mosaic);
    }
    cmd.spawn()
}

fn main() {
    // Remote mode: wrap an external hub/relay's `/g` instead of embedding a
    // loopback server. No child to spawn, no single-writer lock to take — the
    // hub owns the mosaic; this window is just a native client of it.
    if let Some(remote) = resolve_remote_url() {
        let url = format!("{}/g", remote.trim_end_matches('/'));
        run_app(url, None);
        return;
    }

    // Embedded mode (default): a loopback Loro-replica node.
    let port = pick_free_loopback_port();
    let bind = format!("127.0.0.1:{port}");

    let mut child = match spawn_server(&bind) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "tesela-desktop: failed to spawn {} ({e}). Build it first: `cargo build -p tesela-server`.",
                tesela_server_bin().display()
            );
            std::process::exit(1);
        }
    };

    if let Err(reason) = wait_for_port(&mut child, port, Duration::from_secs(20)) {
        // The server never came up. wait_for_port distinguishes two flavors:
        //   - timed out binding — the single-writer lock is likely held by
        //     another tesela-server already running on this mosaic.
        //   - the child already exited (flock conflict, bad binary, panic,
        //     missing mosaic path) — try_wait() inside the loop surfaced
        //     this so we don't burn the full 20s on a dead process.
        // Reap + exit clearly in both cases; don't open a window onto a
        // dead port (a blank "connection refused" page).
        let _ = child.kill();
        let _ = child.wait();
        eprintln!(
            "tesela-desktop: the embedded tesela-server didn't start on {bind}: {reason}. \
             If another instance is running on this mosaic, close it first. Exiting."
        );
        std::process::exit(1);
    }

    let url = format!("http://127.0.0.1:{port}/g");
    run_app(url, Some(child));
}

/// Build + run the Tauri window pointed at `url`. `child` is the embedded server
/// to reap on exit; `None` in remote mode (the external hub owns its lifecycle —
/// nothing to reap, no lock taken).
fn run_app(url: String, child: Option<Child>) {
    tauri::Builder::default()
        .manage(ServerChild(Mutex::new(child)))
        // Cmd+R reloads the webview — the one-keystroke recovery if the page
        // ever goes blank (a crashed WebKit content process, a transient
        // asset-load hiccup). `reload()` is native (runs in the app process),
        // so it works even when the page's own JS is dead.
        .on_menu_event(|app, event| match event.id().as_ref() {
            "reload" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.reload();
                }
            }
            // Settings (⌘,): tell the UI to open its own settings overlay. The
            // SPA's GraphiteShell listens for this event (same surface ⌘K / the
            // gear / leader `,` open).
            "settings" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ =
                        w.eval("document.dispatchEvent(new CustomEvent('tesela:open-settings'))");
                }
            }
            _ => {}
        })
        .setup(move |app| {
            // Default app menu + Settings (⌘,) and a View ▸ Reload (Cmd+R) item.
            // The menu is owned by the (always-alive) app process, so the
            // accelerators fire even when the webview content process has died.
            let settings =
                MenuItem::with_id(app, "settings", "Settings…", true, Some("CmdOrCtrl+,"))?;
            let reload = MenuItem::with_id(app, "reload", "Reload", true, Some("CmdOrCtrl+R"))?;
            let view = Submenu::with_items(app, "View", true, &[&settings, &reload])?;
            let menu = Menu::default(app.handle())?;
            menu.append(&view)?;
            app.set_menu(menu)?;

            WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::External(url.parse().expect("valid server url")),
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
            // Dock-icon re-open (macOS `applicationShouldHandleReopen`) normally
            // only focuses an existing window. If no window is visible, reload +
            // show as a blank-screen recovery path.
            if let RunEvent::Reopen {
                has_visible_windows,
                ..
            } = event
            {
                if let Some(w) = app.get_webview_window("main") {
                    if has_visible_windows {
                        let _ = w.set_focus();
                    } else {
                        let _ = w.reload();
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
            }
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
                        // The server's graceful shutdown runs VACUUM INTO +
                        // a validated backup (auto-on-quit, default on), which
                        // can exceed 5s on a real mosaic. Give it 30s before
                        // the SIGKILL backstop so the backup isn't killed
                        // mid-flight.
                        eprintln!(
                            "tesela-desktop: sending SIGTERM to embedded server; \
                             waiting up to 30s for graceful shutdown \
                             (auto-backup running if enabled)..."
                        );
                        let mut exited = false;
                        for _ in 0..300 {
                            if matches!(child.try_wait(), Ok(Some(_))) {
                                exited = true;
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        if !exited {
                            eprintln!(
                                "tesela-desktop: server did not exit within 30s; sending SIGKILL."
                            );
                            let _ = child.kill();
                        }
                        let _ = child.wait();
                    }
                }
            }
        });
}
