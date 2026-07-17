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

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::menu::{Menu, MenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_updater::UpdaterExt;

use tesela_core::config::Config;
use tesela_server::{serve, ServeConfig};

/// Owns the in-process server's shutdown trigger + join handle so the Exit
/// handler can stop it gracefully and wait for the drain/backup to finish.
struct EmbedHandle {
    shutdown: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    join: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

/// Workspace root = parent of this crate's dir (`src-tauri/`).
#[cfg(debug_assertions)]
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
    desktop_config_value("TESELA_DESKTOP_REMOTE_URL", |c| c.remote_url.clone())
}

/// If the embedded server should join the relay directly, return its base URL.
/// An explicit desktop override wins; otherwise use the selected mosaic's
/// persisted pairing config. The embedded server owns that mosaic's flock, so
/// it is necessarily the sole writer while the app is running.
fn resolve_embed_relay_url(mosaic: Option<&std::path::Path>) -> Option<String> {
    resolve_embed_relay_url_from(
        desktop_config_value("TESELA_EMBED_RELAY_URL", |c| c.relay_url.clone()),
        mosaic,
    )
}

fn resolve_embed_relay_url_from(
    explicit: Option<String>,
    mosaic: Option<&std::path::Path>,
) -> Option<String> {
    explicit
        .filter(|url| !url.trim().is_empty())
        .or_else(|| mosaic.and_then(mosaic_relay_url))
}

fn mosaic_relay_url(mosaic: &std::path::Path) -> Option<String> {
    let config = Config::load(&mosaic.join(".tesela").join("config.toml")).ok()?;
    config
        .sync
        .relay
        .map(|relay| relay.url)
        .filter(|url| !url.trim().is_empty())
}

/// Flat keys read from `desktop.toml`. Mirrors what `resolve_remote_url` /
/// `resolve_embed_relay_url` look for; unknown keys are ignored by serde
/// default (no `deny_unknown_fields`).
#[derive(Debug, Default, serde::Deserialize)]
struct DesktopConfig {
    remote_url: Option<String>,
    relay_url: Option<String>,
}

/// Parse `desktop.toml` (if present) via serde. Errors (missing file, bad
/// TOML) resolve to an empty config rather than failing the app.
fn desktop_config() -> DesktopConfig {
    (|| -> Option<DesktopConfig> {
        let home = std::env::var_os("HOME")?;
        let cfg = PathBuf::from(home).join("Library/Application Support/tesela/desktop.toml");
        let text = std::fs::read_to_string(&cfg).ok()?;
        toml::from_str(&text).ok()
    })()
    .unwrap_or_default()
}

/// Read a config value, with an `env_var` override (env wins — terminal
/// launches; the file works for Finder/Dock launches that don't inherit
/// shell env).
fn desktop_config_value(
    env_var: &str,
    from_file: impl FnOnce(&DesktopConfig) -> Option<String>,
) -> Option<String> {
    if let Ok(v) = std::env::var(env_var) {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return Some(v);
        }
    }
    from_file(&desktop_config()).filter(|v| !v.trim().is_empty())
}

/// The built static `/g` bundle the embedded server serves. Installed apps use
/// their packaged `Resources/web` tree, making the UI independent of the source
/// checkout that produced the executable. `TESELA_STATIC_DIR` remains an
/// explicit override, and unbundled Cargo development falls back to web/build.
fn static_dir(resource_dir: &Path) -> std::io::Result<PathBuf> {
    let explicit = std::env::var_os("TESELA_STATIC_DIR").map(PathBuf::from);
    #[cfg(debug_assertions)]
    {
        let development = workspace_root().join("web").join("build");
        resolve_static_dir(explicit, resource_dir, Some(&development))
    }
    #[cfg(not(debug_assertions))]
    {
        resolve_static_dir(explicit, resource_dir, None)
    }
}

fn resolve_static_dir(
    explicit: Option<PathBuf>,
    resource_dir: &Path,
    development_dir: Option<&Path>,
) -> std::io::Result<PathBuf> {
    if let Some(explicit) = explicit {
        return require_static_dir(explicit);
    }

    let bundled = resource_dir.join("web");
    if bundled.join("index.html").is_file() {
        return Ok(bundled);
    }
    if let Some(development_dir) = development_dir {
        if development_dir.join("index.html").is_file() {
            return Ok(development_dir.to_path_buf());
        }
    }

    let development_expectation = development_dir
        .map(|development_dir| format!(" or {}", development_dir.join("index.html").display()));
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!(
            "Tesela web UI is missing: expected {}{}",
            bundled.join("index.html").display(),
            development_expectation.as_deref().unwrap_or_default()
        ),
    ))
}

fn require_static_dir(path: PathBuf) -> std::io::Result<PathBuf> {
    if path.join("index.html").is_file() {
        return Ok(path);
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!(
            "Tesela web UI is missing: expected {}",
            path.join("index.html").display()
        ),
    ))
}

/// Set the process env the in-process `serve` reads, mirroring what the old
/// `spawn_server` set on the child — minus the parent-death watchdog vars (no
/// child to orphan) and plus `TESELA_EMBEDDED` (lets the server disable the
/// `/server/restart` re-exec, which would relaunch THIS binary, not a server).
fn set_embed_env(config: &ServeConfig, static_dir: &Path) {
    // Ephemeral loopback port; the real one comes back via `on_bound`.
    std::env::set_var("TESELA_SERVER_BIND", "127.0.0.1:0");
    std::env::set_var("TESELA_STATIC_DIR", static_dir);
    // Loopback node — never mDNS-advertise, never LAN-peer-write.
    std::env::set_var("TESELA_DISABLE_MDNS", "1");
    std::env::set_var("TESELA_DISABLE_PEER_SYNC", "1");
    // In-process: a UI-triggered server restart can't re-exec a child.
    std::env::set_var("TESELA_EMBEDDED", "1");
    // Pairing persists the relay in the mosaic config. This is the same
    // already-resolved config carried into serve(), so URL and mosaic cannot
    // diverge if cwd/default configuration changes during startup.
    match resolve_embed_relay_url(Some(config.mosaic.as_path())) {
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
    Embedded(ServeConfig),
}

fn main() {
    let mode = match resolve_remote_url() {
        Some(remote) => Mode::Remote(format!("{}/g", remote.trim_end_matches('/'))),
        None => {
            let config = ServeConfig::resolve(resolve_mosaic())
                .expect("resolve mosaic for embedded Tesela desktop");
            Mode::Embedded(config)
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
        // Auto-update: `AppHandle::restart()` (tauri core, no plugin needed)
        // is used after an update installs — see `check_and_install_update`.
        .plugin(tauri_plugin_updater::Builder::new().build())
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
                Mode::Embedded(config) => {
                    let resource_dir = app.path().resource_dir()?;
                    let static_dir = static_dir(&resource_dir)?;
                    set_embed_env(&config, &static_dir);
                    start_embedded_server(app, &setup_handle, config)?
                }
            };
            build_main_window(app, &url)?;
            build_tray(app)?;
            // Silent startup check: auto-downloads + installs a newer signed
            // release if the manifest reports one, then restarts into it.
            // User-triggered checks (View > Check for Updates…) reuse the
            // same path with `user_initiated: true` for eval feedback.
            let update_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                check_and_install_update(update_handle, false).await;
            });
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
                    let _ = handle.block_on(async {
                        tokio::time::timeout(Duration::from_secs(30), join).await
                    });
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
    config: ServeConfig,
) -> Result<String, Box<dyn std::error::Error>> {
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
            return Err(
                "the embedded tesela-server failed to start (is another instance \
                        already open on this mosaic?)"
                    .into(),
            );
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
        "check-updates" => {
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                check_and_install_update(handle, true).await;
            });
        }
        _ => {}
    }
}

/// Dispatch a `CustomEvent` to the main window's `document`, mirroring the
/// existing `tesela:open-settings` pattern (the webview loads an external
/// `/g` URL, not a Tauri-aware frontend, so `eval` is the only bridge). Best-
/// effort UI hook for the web app to surface update state; `detail` is
/// JSON-serialized so arbitrary text (e.g. error messages) round-trips safely.
fn notify_webview(app: &AppHandle, event: &str, detail: serde_json::Value) {
    if let Some(w) = app.get_webview_window("main") {
        let detail_json = serde_json::to_string(&detail).unwrap_or_else(|_| "{}".to_string());
        let script = format!(
            "document.dispatchEvent(new CustomEvent({event:?}, {{ detail: {detail_json} }}))"
        );
        let _ = w.eval(&script);
    }
}

/// Check the updater manifest and, if a newer signed release is available,
/// download + install it and restart into it. `user_initiated` gates the
/// "checking" / "up to date" eval events (the silent startup check should not
/// spam the webview when there's nothing to report); failures and an
/// available/installed update are always logged to stderr either way.
async fn check_and_install_update(app: AppHandle, user_initiated: bool) {
    if user_initiated {
        notify_webview(&app, "tesela:update-checking", serde_json::json!({}));
    }
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            eprintln!("tesela-desktop: updater unavailable: {e}");
            if user_initiated {
                notify_webview(
                    &app,
                    "tesela:update-error",
                    serde_json::json!({ "message": e.to_string() }),
                );
            }
            return;
        }
    };
    match updater.check().await {
        Ok(Some(update)) => {
            eprintln!(
                "tesela-desktop: update {} available; downloading…",
                update.version
            );
            notify_webview(
                &app,
                "tesela:update-available",
                serde_json::json!({ "version": update.version }),
            );
            let downloaded = Arc::new(AtomicU64::new(0));
            let downloaded_at_finish = downloaded.clone();
            let result = update
                .download_and_install(
                    move |chunk_len, _content_len| {
                        downloaded.fetch_add(chunk_len as u64, Ordering::Relaxed);
                    },
                    move || {
                        eprintln!(
                            "tesela-desktop: update download finished ({} bytes)",
                            downloaded_at_finish.load(Ordering::Relaxed)
                        );
                    },
                )
                .await;
            match result {
                Ok(()) => {
                    eprintln!("tesela-desktop: update installed; restarting");
                    notify_webview(
                        &app,
                        "tesela:update-installed",
                        serde_json::json!({ "version": update.version }),
                    );
                    app.restart();
                }
                Err(e) => {
                    eprintln!("tesela-desktop: update install failed: {e}");
                    notify_webview(
                        &app,
                        "tesela:update-error",
                        serde_json::json!({ "message": e.to_string() }),
                    );
                }
            }
        }
        Ok(None) => {
            eprintln!("tesela-desktop: no update available");
            if user_initiated {
                notify_webview(&app, "tesela:update-none", serde_json::json!({}));
            }
        }
        Err(e) => {
            eprintln!("tesela-desktop: update check failed: {e}");
            if user_initiated {
                notify_webview(
                    &app,
                    "tesela:update-error",
                    serde_json::json!({ "message": e.to_string() }),
                );
            }
        }
    }
}

fn desktop_initialization_script() -> &'static str {
    "window.__TESELA_API_BASE__ = ''; window.__TESELA_PLATFORM__ = 'desktop';"
}

fn build_main_window(app: &mut tauri::App, url: &str) -> tauri::Result<()> {
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, Some("CmdOrCtrl+,"))?;
    let reload = MenuItem::with_id(app, "reload", "Reload", true, Some("CmdOrCtrl+R"))?;
    let check_updates = MenuItem::with_id(
        app,
        "check-updates",
        "Check for Updates…",
        true,
        None::<&str>,
    )?;
    let view = Submenu::with_items(app, "View", true, &[&settings, &reload, &check_updates])?;
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
    // Same-origin API plus an explicit runtime identity for platform-specific
    // release history and seen-state storage.
    .initialization_script(desktop_initialization_script())
    .disable_drag_drop_handler()
    .build()?;
    Ok(())
}

fn build_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "tray-show", "Show Tesela", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "tray-hide", "Hide", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "tray-quit", "Quit", true, None::<&str>)?;
    let tray_menu = Menu::with_items(app, &[&show, &hide, &quit])?;
    TrayIconBuilder::new()
        .icon(
            app.default_window_icon()
                .cloned()
                .expect("default window icon"),
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_static_bundle(root: &std::path::Path) -> PathBuf {
        let static_dir = root.join("web");
        std::fs::create_dir_all(&static_dir).unwrap();
        std::fs::write(static_dir.join("index.html"), "<!doctype html>").unwrap();
        static_dir
    }

    #[test]
    fn resolve_static_dir_prefers_the_bundled_web_ui() {
        let tmp = tempfile::tempdir().unwrap();
        let resources = tmp.path().join("Resources");
        let bundled = create_static_bundle(&resources);
        let development = create_static_bundle(&tmp.path().join("source"));

        assert_eq!(
            resolve_static_dir(None, &resources, Some(&development)).unwrap(),
            bundled
        );
    }

    #[test]
    fn resolve_static_dir_uses_the_source_tree_for_unbundled_development() {
        let tmp = tempfile::tempdir().unwrap();
        let resources = tmp.path().join("missing-resources");
        let development = create_static_bundle(&tmp.path().join("source"));

        assert_eq!(
            resolve_static_dir(None, &resources, Some(&development)).unwrap(),
            development
        );
    }

    #[test]
    fn resolve_static_dir_rejects_an_explicit_directory_without_an_index() {
        let tmp = tempfile::tempdir().unwrap();
        let explicit = tmp.path().join("explicit");
        std::fs::create_dir_all(&explicit).unwrap();
        let bundled = create_static_bundle(&tmp.path().join("Resources"));

        let error = resolve_static_dir(
            Some(explicit.clone()),
            &bundled,
            Some(&tmp.path().join("source")),
        )
        .unwrap_err();

        assert!(
            error.to_string().contains(&explicit.display().to_string()),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn resolve_static_dir_fails_when_no_web_ui_is_available() {
        let tmp = tempfile::tempdir().unwrap();
        let resources = tmp.path().join("missing-resources");
        let development = tmp.path().join("missing-source");

        let error = resolve_static_dir(None, &resources, Some(&development)).unwrap_err();

        assert!(
            error.to_string().contains("index.html"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn resolve_static_dir_disables_source_fallback_for_packaged_releases() {
        let tmp = tempfile::tempdir().unwrap();
        let resources = tmp.path().join("missing-resources");
        let development = create_static_bundle(&tmp.path().join("source"));

        let error = resolve_static_dir(None, &resources, None).unwrap_err();

        assert!(
            !error
                .to_string()
                .contains(&development.display().to_string()),
            "packaged releases must not consult the build checkout: {error}"
        );
    }

    #[test]
    fn paired_mosaic_relay_enables_embedded_relay_without_desktop_override() {
        let tmp = tempfile::tempdir().unwrap();
        let tesela_dir = tmp.path().join(".tesela");
        std::fs::create_dir_all(&tesela_dir).unwrap();
        std::fs::write(
            tesela_dir.join("config.toml"),
            "[sync.relay]\nurl = \"https://relay.example.test\"\n",
        )
        .unwrap();

        assert_eq!(
            resolve_embed_relay_url_from(None, Some(tmp.path())).as_deref(),
            Some("https://relay.example.test")
        );
    }

    #[test]
    fn explicit_desktop_relay_override_wins_over_mosaic_pairing_config() {
        let tmp = tempfile::tempdir().unwrap();
        let tesela_dir = tmp.path().join(".tesela");
        std::fs::create_dir_all(&tesela_dir).unwrap();
        std::fs::write(
            tesela_dir.join("config.toml"),
            "[sync.relay]\nurl = \"https://paired.example.test\"\n",
        )
        .unwrap();

        assert_eq!(
            resolve_embed_relay_url_from(
                Some("https://override.example.test".to_string()),
                Some(tmp.path()),
            )
            .as_deref(),
            Some("https://override.example.test")
        );
    }

    #[test]
    fn embedded_mode_carries_the_already_resolved_serve_config() {
        let tmp = tempfile::tempdir().unwrap();
        let expected = tmp.path().to_path_buf();
        let mode = Mode::Embedded(ServeConfig {
            mosaic: expected.clone(),
        });

        match mode {
            Mode::Embedded(config) => assert_eq!(config.mosaic, expected),
            Mode::Remote(_) => panic!("expected embedded mode"),
        }
    }

    #[test]
    fn main_window_disables_native_drag_drop_interception() {
        let source = include_str!("main.rs");
        let window_builder = source
            .split_once("fn build_main_window")
            .expect("main window builder exists")
            .1
            .split_once("fn build_tray")
            .expect("tray builder follows main window builder")
            .0;

        assert!(
            window_builder.contains(".disable_drag_drop_handler()"),
            "Tauri's native file-drop handler consumes HTML drag events on macOS"
        );
    }

    #[test]
    fn main_window_identifies_the_desktop_release_notes_platform() {
        let script = desktop_initialization_script();

        assert!(script.contains("window.__TESELA_API_BASE__ = '';"));
        assert!(script.contains("window.__TESELA_PLATFORM__ = 'desktop';"));
    }
}
