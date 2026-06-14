// Tesela desktop — a native (Tauri) window around the `/g` web UI. The
// `tesela-server` now runs IN-PROCESS on our own tokio runtime via
// `tesela_server::serve` (L4 Phase B), instead of being spawned as a child
// binary: no sidecar to locate, no port TOCTOU, no child to reap. The server
// binds LOOPBACK ONLY and the webview loads `http://127.0.0.1:<port>/g` —
// same-origin, no CORS, the UI's host-derived WS URL works unchanged.
//
// Design rule (sync model): the embedded server is a LOOPBACK Loro-replica
// node, NOT a hub. It binds 127.0.0.1 only and mDNS is disabled — other devices
// never point at it; cross-device sync flows through the spine (relay/LAN), the
// same transport as iOS. Do not let this bind 0.0.0.0 / become a hub.
//
// REMOTE MODE: when a remote URL is configured (env `TESELA_DESKTOP_REMOTE_URL`
// or `remote_url` in `~/Library/Application Support/tesela/desktop.toml`), the
// desktop does NOT run a server — it just wraps that external server's `/g`.
//
// Lifecycle: `serve` is spawned inside Tauri `.setup()` (AFTER the single-
// instance plugin, so only the primary instance ever takes the flock); it holds
// the single-writer flock for the whole app lifetime and releases it when the
// shutdown future resolves. On `RunEvent::Exit` we fire that future and block on
// `serve` returning, so the graceful drain + auto-backup completes before exit
// (same guarantee the old 30s SIGTERM grace gave the child).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::menu::{Menu, MenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use tesela_server::{serve, ServeConfig};

/// Owns the in-process server's shutdown trigger + join handle so the Exit
/// handler can stop it gracefully and wait for the drain/backup to finish.
struct EmbedHandle {
    shutdown: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    join: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

/// Workspace root = parent of this crate's dir (`src-tauri/`).
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
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
/// hub or the relay) instead of running its own loopback node, return its base
/// URL. Source: `TESELA_DESKTOP_REMOTE_URL` env (wins, for terminal launches),
/// else a `remote_url = "..."` line in `desktop.toml`. `None` → embed (default).
fn resolve_remote_url() -> Option<String> {
    desktop_config_value("TESELA_DESKTOP_REMOTE_URL", "remote_url")
}

/// If the EMBEDDED server should JOIN the relay (the spine) directly — instead
/// of the default loopback-only Loro-replica — return the relay base URL.
/// Source: `TESELA_EMBED_RELAY_URL` env, else `relay_url = "..."` in
/// desktop.toml. ⚠ Only opt in when this embed is the SOLE writer for the mosaic
/// — never alongside a standalone server, or two relay participants share one
/// device_id. `None` → embed stays loopback-only (default).
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

/// Set the process env the in-process `serve` reads, mirroring what the old
/// `spawn_server` set on the child — minus the parent-death watchdog vars (no
/// child to orphan) and plus `TESELA_EMBEDDED` (lets the server disable the
/// `/server/restart` re-exec, which would relaunch THIS binary, not a server).
fn set_embed_env() {
    // Ephemeral loopback port; the real one comes back via `on_bound`.
    std::env::set_var("TESELA_SERVER_BIND", "127.0.0.1:0");
    std::env::set_var("TESELA_STATIC_DIR", static_dir());
    // Loopback node — never mDNS-advertise, never LAN-peer-write.
    std::env::set_var("TESELA_DISABLE_MDNS", "1");
    std::env::set_var("TESELA_DISABLE_PEER_SYNC", "1");
    // In-process: a UI-triggered server restart can't re-exec a child.
    std::env::set_var("TESELA_EMBEDDED", "1");
    // Relay participation (see resolve_embed_relay_url). DEFAULT loopback-only.
    match resolve_embed_relay_url() {
        Some(url) => {
            std::env::remove_var("TESELA_DISABLE_RELAY");
            std::env::set_var("TESELA_RELAY_URL", url);
        }
        None => {
            std::env::set_var("TESELA_DISABLE_RELAY", "1");
        }
    }
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "info,loro=warn");
    }
}

/// Run mode, resolved once in `main`.
enum Mode {
    /// Wrap an external hub/relay's `/g` — no server, no flock.
    Remote(String),
    /// Default: a loopback Loro-replica node served in-process.
    Embedded,
}

fn main() {
    let mode = match resolve_remote_url() {
        Some(remote) => Mode::Remote(format!("{}/g", remote.trim_end_matches('/'))),
        None => {
            set_embed_env();
            Mode::Embedded
        }
    };
    run(mode);
}

/// Single Tauri entry point for both modes (so `generate_context!` is expanded
/// exactly once). Embedded mode spawns `serve` inside `.setup()` — AFTER the
/// single-instance plugin, so only the primary instance ever takes the flock.
fn run(mode: Mode) {
    // A dedicated multi-thread runtime owns the server + its daemons for the
    // app's lifetime. Leaked so its `Handle` is `'static` for the Exit closure;
    // the OS reclaims it on process exit. Remote mode leaves it idle.
    let runtime = Box::leak(Box::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime for the embedded server"),
    ));
    let handle = runtime.handle().clone();
    let setup_handle = handle.clone();

    let app = tauri::Builder::default()
        // Single-instance FIRST: a 2nd launch focuses the existing window and
        // exits before `.setup()` — so only the primary ever reaches `serve`
        // and takes the single-writer flock (#202). MUST be the first plugin.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .on_menu_event(menu_event)
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(move |app| {
            let url = match mode {
                Mode::Remote(url) => url,
                Mode::Embedded => start_embedded_server(app, &setup_handle)?,
            };
            build_main_window(app, &url)?;
            build_tray(app)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tesela-desktop");

    app.run(move |app, event| {
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
        // Graceful shutdown of the in-process server (no-op in remote mode,
        // where no `EmbedHandle` was managed): fire the shutdown future and
        // block until `serve` returns (drain + auto-backup), so a real mosaic's
        // VACUUM-INTO backup isn't cut off — the in-process equivalent of the
        // old 30s SIGTERM grace given the child.
        if let RunEvent::Exit = event {
            if let Some(state) = app.try_state::<EmbedHandle>() {
                if let Some(tx) = state.shutdown.lock().unwrap().take() {
                    let _ = tx.send(());
                }
                if let Some(join) = state.join.lock().unwrap().take() {
                    eprintln!(
                        "tesela-desktop: stopping embedded server; waiting up to 30s for \
                         graceful shutdown (auto-backup running if enabled)..."
                    );
                    let _ = handle
                        .block_on(async { tokio::time::timeout(Duration::from_secs(30), join).await });
                }
            }
        }
    });
}

/// Spawn `serve` on `handle`, wait for it to bind, manage the shutdown handle,
/// and return the `http://127.0.0.1:<port>/g` URL. Errors (flock conflict, boot
/// failure, timeout) bubble out of `.setup()` so Tauri reports them and exits.
fn start_embedded_server(
    app: &mut tauri::App,
    handle: &tokio::runtime::Handle,
) -> Result<String, Box<dyn std::error::Error>> {
    let config = ServeConfig::resolve(resolve_mosaic()).map_err(|e| format!("resolve mosaic: {e}"))?;
    let (bound_tx, bound_rx) = std::sync::mpsc::channel::<std::net::SocketAddr>();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let join = handle.spawn(async move {
        if let Err(e) = serve(
            config,
            async move {
                let _ = shutdown_rx.await;
            },
            move |addr| {
                let _ = bound_tx.send(addr);
            },
        )
        .await
        {
            eprintln!("tesela-desktop: embedded server exited with error: {e}");
        }
    });

    // Wait for the bind. Bail fast if `serve` returned before binding — a flock
    // conflict (another writer on this mosaic) or a boot error — rather than
    // burning the whole timeout on a dead task.
    let deadline = Instant::now() + Duration::from_secs(20);
    let addr = loop {
        if let Ok(addr) = bound_rx.try_recv() {
            break addr;
        }
        if join.is_finished() {
            return Err("the embedded tesela-server failed to start (is another instance \
                        already open on this mosaic?)"
                .into());
        }
        if Instant::now() >= deadline {
            return Err("the embedded tesela-server did not bind within 20s".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    app.manage(EmbedHandle {
        shutdown: Mutex::new(Some(shutdown_tx)),
        join: Mutex::new(Some(join)),
    });
    Ok(format!("http://{addr}/g"))
}

fn menu_event(app: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        "reload" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.reload();
            }
        }
        "settings" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.eval("document.dispatchEvent(new CustomEvent('tesela:open-settings'))");
            }
        }
        _ => {}
    }
}

fn build_main_window(app: &mut tauri::App, url: &str) -> tauri::Result<()> {
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, Some("CmdOrCtrl+,"))?;
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
    // Tells the UI to use same-origin (server serves API + UI on one origin).
    .initialization_script("window.__TESELA_API_BASE__ = '';")
    .build()?;
    Ok(())
}

fn build_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "tray-show", "Show Tesela", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "tray-hide", "Hide", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "tray-quit", "Quit", true, None::<&str>)?;
    let tray_menu = Menu::with_items(app, &[&show, &hide, &quit])?;
    TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().expect("default window icon"))
        .menu(&tray_menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "tray-show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "tray-hide" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            "tray-quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}
