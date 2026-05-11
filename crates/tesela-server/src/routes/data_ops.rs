//! Backup / Restore / Export / Import HTTP endpoints — driven by the
//! web Settings UI.
//!
//! Backup operations are run in-process via the `tesela-backup` crate
//! (we already have the live SQLite handle, so `VACUUM INTO` can be
//! issued without spinning up a subprocess). Export and imports shell
//! out to the installed `tesela` CLI binary — they're heavy + rare and
//! avoid duplicating the ~2000-line importer modules.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use tesela_core::config::Config;
use tesela_core::db::SqliteIndex;

use crate::state::AppState;

/// Manifest shape returned to the UI. Keeps just the fields the user
/// cares about — full Manifest has lots of internals the table doesn't
/// need to render.
#[derive(Debug, Serialize)]
pub struct BackupSummary {
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub destination_kind: String,
    pub encryption_kind: String,
    pub file_count: usize,
    pub validated: Option<bool>,
    pub validated_at: Option<String>,
}

pub async fn list_backups(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<BackupSummary>>, (StatusCode, String)> {
    let root = state.mosaic_root.join(".tesela").join("backups");
    let backups = tokio::task::spawn_blocking(move || tesela_backup::list(&root))
        .await
        .map_err(internal)?
        .map_err(server_error)?;

    let summaries: Vec<BackupSummary> = backups
        .into_iter()
        .map(|(path, manifest)| BackupSummary {
            name: path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string(),
            path: path.to_string_lossy().into_owned(),
            created_at: manifest.created_at.to_rfc3339(),
            destination_kind: match manifest.destination {
                tesela_backup::manifest::ManifestDestination::Local { .. } => "local".into(),
                tesela_backup::manifest::ManifestDestination::External { .. } => "external".into(),
                tesela_backup::manifest::ManifestDestination::Git { .. } => "git".into(),
            },
            encryption_kind: match manifest.encryption {
                tesela_backup::ManifestEncryption::None => "none".into(),
                tesela_backup::ManifestEncryption::Age { .. } => "age".into(),
            },
            file_count: manifest.files.len(),
            validated: manifest.validated.as_ref().map(|v| v.ok),
            validated_at: manifest
                .validated
                .as_ref()
                .map(|v| v.checked_at.to_rfc3339()),
        })
        .collect();

    Ok(Json(summaries))
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct RunBackupRequest {
    /// "local" | "external" | "git"
    pub destination: String,
    /// Only meaningful when destination == "external"
    pub external_path: Option<String>,
    /// Only meaningful when destination == "git"
    pub git_remote: Option<String>,
    pub git_branch: Option<String>,
    /// Force encryption (otherwise auto-on for non-local)
    pub encrypt: bool,
    pub no_validate: bool,
    pub no_prune: bool,
}

#[derive(Debug, Serialize)]
pub struct RunBackupResponse {
    pub path: String,
    pub file_count: usize,
    pub validated: bool,
    pub validation_note: Option<String>,
}

pub async fn run_backup(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RunBackupRequest>,
) -> Result<Json<RunBackupResponse>, (StatusCode, String)> {
    // Pre-stage SQLite snapshot (we hold the live index handle so
    // can't easily delegate this to the blocking task).
    let snapshot = tempfile::Builder::new()
        .prefix("tesela-vacuum-")
        .suffix(".db")
        .tempfile()
        .map_err(internal_io)?;
    let snap_path = snapshot.path().to_path_buf();
    state
        .index
        .vacuum_into(&snap_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("vacuum: {}", e)))?;

    let mosaic = state.mosaic_root.clone();
    let outcome = tokio::task::spawn_blocking(move || -> anyhow::Result<tesela_backup::BackupOutcome> {
        let destination = match req.destination.as_str() {
            "external" => {
                let path = req
                    .external_path
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("external_path required for external destination"))?;
                tesela_backup::Destination::External {
                    path: PathBuf::from(path),
                }
            }
            "git" => {
                let remote = req
                    .git_remote
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("git_remote required for git destination"))?;
                let branch = req
                    .git_branch
                    .clone()
                    .unwrap_or_else(|| "main".to_string());
                let mirror = mosaic.join(".tesela").join("backups").join(".git-mirror");
                tesela_backup::Destination::Git {
                    remote: remote.clone(),
                    branch,
                    local_mirror: mirror,
                }
            }
            _ => tesela_backup::Destination::Local,
        };

        let should_encrypt = req.encrypt || !matches!(destination, tesela_backup::Destination::Local);
        let encryption = if should_encrypt {
            match tesela_backup::encrypt::load_identity_for_mosaic(&mosaic)
                .map_err(|e| anyhow::anyhow!("keychain: {}", e))?
            {
                Some(id) => tesela_backup::ManifestEncryption::Age {
                    recipient: id.to_public().to_string(),
                },
                None => {
                    return Err(anyhow::anyhow!(
                        "No age identity in Keychain for this mosaic. Click \"Generate encryption keypair\" first."
                    ));
                }
            }
        } else {
            tesela_backup::ManifestEncryption::None
        };

        let outcome = tesela_backup::backup(
            &mosaic,
            tesela_backup::BackupOptions {
                destination,
                validate: !req.no_validate,
                extra_files: vec![(".tesela/tesela.db".to_string(), snap_path)],
                retention: if req.no_prune {
                    None
                } else {
                    Some(tesela_backup::GfsPolicy::default())
                },
                encryption,
            },
        )
        .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(outcome)
    })
    .await
    .map_err(internal)?
    .map_err(server_error)?;

    drop(snapshot);
    let validated = outcome
        .manifest
        .validated
        .as_ref()
        .map(|v| v.ok)
        .unwrap_or(false);
    let validation_note = outcome
        .manifest
        .validated
        .as_ref()
        .and_then(|v| v.note.clone());
    Ok(Json(RunBackupResponse {
        path: outcome.path.to_string_lossy().into_owned(),
        file_count: outcome.manifest.files.len(),
        validated,
        validation_note,
    }))
}

pub async fn verify_backup(
    State(state): State<Arc<AppState>>,
    AxumPath(name): AxumPath<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let path = state
        .mosaic_root
        .join(".tesela")
        .join("backups")
        .join(&name);
    let status = tokio::task::spawn_blocking(move || tesela_backup::verify(&path))
        .await
        .map_err(internal)?
        .map_err(server_error)?;
    Ok(Json(serde_json::json!({
        "ok": status.ok,
        "elapsed_ms": status.elapsed_ms,
        "checked_at": status.checked_at.to_rfc3339(),
        "note": status.note,
    })))
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct RestoreRequest {
    pub in_place: bool,
    pub allow_newer: bool,
}

pub async fn restore_backup(
    State(state): State<Arc<AppState>>,
    AxumPath(name): AxumPath<String>,
    Json(req): Json<RestoreRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let backup_path = state
        .mosaic_root
        .join(".tesela")
        .join("backups")
        .join(&name);
    let current_mosaic = state.mosaic_root.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        tesela_backup::restore(
            &backup_path,
            &current_mosaic,
            tesela_backup::RestoreOptions {
                in_place: req.in_place,
                target_override: None,
                allow_newer: req.allow_newer,
            },
        )
    })
    .await
    .map_err(internal)?
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("{}", e)))?;

    Ok(Json(serde_json::json!({
        "target": outcome.target.to_string_lossy(),
        "renamed_previous": outcome
            .renamed_previous
            .map(|p| p.to_string_lossy().into_owned()),
        "file_count": outcome.manifest.files.len(),
    })))
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct PruneRequest {
    pub dry_run: bool,
}

pub async fn prune_backups(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PruneRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let root = state.mosaic_root.join(".tesela").join("backups");
    let outcome = tokio::task::spawn_blocking(move || {
        tesela_backup::prune_gfs(&root, tesela_backup::GfsPolicy::default(), req.dry_run)
    })
    .await
    .map_err(internal)?
    .map_err(server_error)?;
    Ok(Json(serde_json::json!({
        "kept": outcome.kept.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
        "removed": outcome.removed.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
        "dry_run": req.dry_run,
    })))
}

pub async fn keygen(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mosaic = state.mosaic_root.clone();
    let recipient = tokio::task::spawn_blocking(move || {
        tesela_backup::encrypt::keygen_for_mosaic(&mosaic)
    })
    .await
    .map_err(internal)?
    .map_err(server_error)?;
    Ok(Json(serde_json::json!({ "recipient": recipient })))
}

/// Inspect whether an age identity already exists for this mosaic (so
/// the UI can show "Generate" vs "Rotate" + display the recipient).
pub async fn key_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mosaic = state.mosaic_root.clone();
    let info = tokio::task::spawn_blocking(move || {
        tesela_backup::encrypt::load_identity_for_mosaic(&mosaic)
    })
    .await
    .map_err(internal)?
    .map_err(server_error)?;
    Ok(Json(serde_json::json!({
        "exists": info.is_some(),
        "recipient": info.map(|id| id.to_public().to_string()),
    })))
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct BackupConfigDto {
    pub auto_on_quit: bool,
    pub external_path: Option<String>,
    pub git_remote: Option<String>,
    pub git_branch: Option<String>,
}

pub async fn get_backup_config(
    State(state): State<Arc<AppState>>,
) -> Result<Json<BackupConfigDto>, (StatusCode, String)> {
    let path = state.mosaic_root.join(".tesela").join("config.toml");
    let cfg = if path.exists() {
        Config::load(&path).map_err(server_error)?
    } else {
        Config::default()
    };
    Ok(Json(BackupConfigDto {
        auto_on_quit: cfg.backup.auto_on_quit,
        external_path: cfg.backup.external_path.map(|p| p.to_string_lossy().into_owned()),
        git_remote: cfg.backup.git_remote,
        git_branch: cfg.backup.git_branch,
    }))
}

pub async fn put_backup_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BackupConfigDto>,
) -> Result<Json<BackupConfigDto>, (StatusCode, String)> {
    let path = state.mosaic_root.join(".tesela").join("config.toml");
    let mut cfg = if path.exists() {
        Config::load(&path).map_err(server_error)?
    } else {
        Config::default()
    };
    cfg.backup.auto_on_quit = req.auto_on_quit;
    cfg.backup.external_path = req
        .external_path
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from);
    cfg.backup.git_remote = req
        .git_remote
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .cloned();
    cfg.backup.git_branch = req
        .git_branch
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .cloned();
    cfg.save(&path).map_err(server_error)?;
    Ok(Json(req))
}

#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    pub out_path: String,
    pub mode: String,
    #[serde(default)]
    pub include_attachments: bool,
}

#[derive(Debug, Serialize)]
pub struct ExportResponse {
    pub note_count: usize,
    pub attachment_count: usize,
    pub stripped_property_count: usize,
    pub out_path: String,
}

pub async fn run_export(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExportRequest>,
) -> Result<Json<ExportResponse>, (StatusCode, String)> {
    use tesela_core::export::markdown::{export_mosaic, ExportOptions, MarkdownMode};
    let mode = match req.mode.as_str() {
        "full" => MarkdownMode::Full,
        "portable" => MarkdownMode::Portable,
        other => return Err((StatusCode::BAD_REQUEST, format!("unknown mode: {}", other))),
    };
    let out_path = PathBuf::from(&req.out_path);
    let mosaic = state.mosaic_root.clone();
    let out_for_resp = out_path.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        export_mosaic(
            &mosaic,
            &out_path,
            &ExportOptions {
                mode,
                include_attachments: req.include_attachments,
            },
        )
    })
    .await
    .map_err(internal)?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e)))?;
    Ok(Json(ExportResponse {
        note_count: outcome.note_count,
        attachment_count: outcome.attachment_count,
        stripped_property_count: outcome.stripped_property_count,
        out_path: out_for_resp.to_string_lossy().into_owned(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    pub source: String,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Serialize)]
pub struct ImportResponse {
    pub kind: String,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Shell out to the installed `tesela` CLI for imports. Avoids
/// duplicating ~2000 lines of importer logic into both crates. The
/// CLI's stdout already prints a structured summary; we just relay it.
async fn run_import_cli(
    state: &AppState,
    subcommand: &str,
    source: &str,
    dry_run: bool,
) -> Result<ImportResponse, (StatusCode, String)> {
    let mosaic_str = state.mosaic_root.to_string_lossy().into_owned();
    let source_owned = source.to_string();
    let subcommand_owned = subcommand.to_string();
    let output = tokio::task::spawn_blocking(move || {
        let mut cmd = std::process::Command::new("tesela");
        cmd.arg("--mosaic")
            .arg(&mosaic_str)
            .arg(&subcommand_owned)
            .arg("--source")
            .arg(&source_owned);
        if dry_run {
            cmd.arg("--dry-run");
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()
    })
    .await
    .map_err(internal)?
    .map_err(internal_io)?;
    Ok(ImportResponse {
        kind: subcommand.trim_start_matches("import-").to_string(),
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub async fn import_obsidian(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ImportRequest>,
) -> Result<Json<ImportResponse>, (StatusCode, String)> {
    run_import_cli(&state, "import-obsidian", &req.source, req.dry_run)
        .await
        .map(Json)
}

pub async fn import_logseq(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ImportRequest>,
) -> Result<Json<ImportResponse>, (StatusCode, String)> {
    run_import_cli(&state, "import-logseq", &req.source, req.dry_run)
        .await
        .map(Json)
}

pub async fn import_org(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ImportRequest>,
) -> Result<Json<ImportResponse>, (StatusCode, String)> {
    run_import_cli(&state, "import-org", &req.source, req.dry_run)
        .await
        .map(Json)
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct PickFolderRequest {
    /// Optional prompt label shown in the dialog title.
    pub prompt: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PickFolderResponse {
    /// `null` when the user canceled the dialog.
    pub path: Option<String>,
}

/// Open the OS's native "Choose Folder" dialog and return the picked
/// absolute path. Returns `path: null` when the user cancels.
///
/// - macOS: AppleScript `choose folder` via `osascript`.
/// - Linux: `zenity --file-selection --directory`, falling back to
///   `kdialog --getexistingdirectory`. If neither is on PATH, returns
///   a 501-ish error so the UI can fall back to manual entry.
/// - Windows: PowerShell `System.Windows.Forms.FolderBrowserDialog`.
pub async fn pick_folder(
    Json(req): Json<PickFolderRequest>,
) -> Result<Json<PickFolderResponse>, (StatusCode, String)> {
    let prompt = req
        .prompt
        .clone()
        .unwrap_or_else(|| "Pick a folder".to_string());
    let outcome = tokio::task::spawn_blocking(move || run_native_picker(&prompt))
        .await
        .map_err(internal)?;
    match outcome {
        PickerOutcome::Picked(path) => Ok(Json(PickFolderResponse { path: Some(path) })),
        PickerOutcome::Canceled => Ok(Json(PickFolderResponse { path: None })),
        PickerOutcome::Unsupported(msg) => Err((StatusCode::NOT_IMPLEMENTED, msg)),
        PickerOutcome::Failed(msg) => Err((StatusCode::INTERNAL_SERVER_ERROR, msg)),
    }
}

#[derive(Debug)]
#[allow(dead_code)] // Unsupported is only constructed on Linux/Windows builds.
enum PickerOutcome {
    Picked(String),
    Canceled,
    Unsupported(String),
    Failed(String),
}

#[cfg(target_os = "macos")]
fn run_native_picker(prompt: &str) -> PickerOutcome {
    // System Events activate brings the dialog forward so it pops on
    // top of the user's browser, not buried behind it.
    let escaped = prompt.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        "tell application \"System Events\" to activate\n\
         POSIX path of (choose folder with prompt \"{}\")",
        escaped
    );
    let output = match std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
    {
        Ok(o) => o,
        Err(e) => return PickerOutcome::Failed(format!("osascript: {}", e)),
    };
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout)
            .trim_end_matches('\n')
            .trim_end_matches('/')
            .to_string();
        PickerOutcome::Picked(path)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("User canceled") || stderr.contains("(-128)") {
            PickerOutcome::Canceled
        } else {
            PickerOutcome::Failed(format!("osascript: {}", stderr.trim()))
        }
    }
}

#[cfg(target_os = "linux")]
fn run_native_picker(prompt: &str) -> PickerOutcome {
    use std::process::Command;
    // Try zenity first (GNOME, most common); fall back to kdialog
    // (KDE). On cancel, both exit non-zero with empty stdout — we
    // distinguish "no tool available" from "user canceled" by
    // checking which command was actually missing.
    if which_exists("zenity") {
        match Command::new("zenity")
            .args(["--file-selection", "--directory", "--title"])
            .arg(prompt)
            .output()
        {
            Ok(o) if o.status.success() => {
                let path = String::from_utf8_lossy(&o.stdout)
                    .trim_end_matches('\n')
                    .to_string();
                return PickerOutcome::Picked(path);
            }
            Ok(_) => return PickerOutcome::Canceled,
            Err(e) => return PickerOutcome::Failed(format!("zenity: {}", e)),
        }
    }
    if which_exists("kdialog") {
        match Command::new("kdialog")
            .args(["--getexistingdirectory", ".", "--title"])
            .arg(prompt)
            .output()
        {
            Ok(o) if o.status.success() => {
                let path = String::from_utf8_lossy(&o.stdout)
                    .trim_end_matches('\n')
                    .to_string();
                return PickerOutcome::Picked(path);
            }
            Ok(_) => return PickerOutcome::Canceled,
            Err(e) => return PickerOutcome::Failed(format!("kdialog: {}", e)),
        }
    }
    PickerOutcome::Unsupported(
        "Install `zenity` (GNOME) or `kdialog` (KDE) to enable the folder picker, or paste the path manually."
            .to_string(),
    )
}

#[cfg(target_os = "windows")]
fn run_native_picker(prompt: &str) -> PickerOutcome {
    // PowerShell needs the STA threading model for WinForms; the -Sta
    // flag covers that. The dialog returns DialogResult.OK on pick;
    // anything else (cancel, close) leaves SelectedPath unset and we
    // emit an empty line, which we treat as cancellation.
    let escaped = prompt.replace('"', "`\"");
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms | Out-Null; \
         $d = New-Object System.Windows.Forms.FolderBrowserDialog; \
         $d.Description = \"{}\"; \
         $d.ShowNewFolderButton = $true; \
         if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{ \
           [Console]::Out.WriteLine($d.SelectedPath) \
         }}",
        escaped
    );
    let output = match std::process::Command::new("powershell")
        .args(["-NoProfile", "-Sta", "-Command"])
        .arg(&script)
        .output()
    {
        Ok(o) => o,
        Err(e) => return PickerOutcome::Failed(format!("powershell: {}", e)),
    };
    if !output.status.success() {
        return PickerOutcome::Failed(format!(
            "powershell: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let path = stdout.trim().to_string();
    if path.is_empty() {
        PickerOutcome::Canceled
    } else {
        PickerOutcome::Picked(path)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn run_native_picker(_prompt: &str) -> PickerOutcome {
    PickerOutcome::Unsupported(
        "Folder picker is not implemented on this platform; paste the path manually.".to_string(),
    )
}

#[cfg(target_os = "linux")]
fn which_exists(name: &str) -> bool {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {}", name))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ──────────────────────────────────────────────────────────────────────
// Mosaic management (Phase 13 follow-up)
//
// Lets the user create a fresh mosaic from the UI — blank or seeded by
// an import from Obsidian / Logseq / Org — and switch the running
// server to it. The switch path writes the new default to
// ~/.config/tesela/config.toml, sends SIGTERM to the running server
// (so the graceful shutdown + auto-backup hook fires), and (if no
// LaunchAgent is managing the process) spawns a detached re-exec of
// the server binary that waits ~2s for the port to free before
// rebinding.
// ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct CurrentMosaicResponse {
    pub path: String,
    pub config_path: String,
    pub config_default_mosaic: Option<String>,
    /// Parent directory under which new mosaics should be created by
    /// default (`~/Library/Application Support/tesela/` on macOS,
    /// `~/.local/share/tesela/` on Linux, `%APPDATA%/tesela/` on
    /// Windows). The UI uses this to pre-fill new-mosaic paths.
    pub suggested_root: String,
}

pub async fn get_current_mosaic(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CurrentMosaicResponse>, (StatusCode, String)> {
    let config_path = Config::default_path();
    let config_default = if config_path.exists() {
        Config::load(&config_path)
            .map_err(server_error)?
            .general
            .default_mosaic
            .map(|p| p.to_string_lossy().into_owned())
    } else {
        None
    };
    Ok(Json(CurrentMosaicResponse {
        path: state.mosaic_root.to_string_lossy().into_owned(),
        config_path: config_path.to_string_lossy().into_owned(),
        config_default_mosaic: config_default,
        suggested_root: Config::mosaic_root_dir().to_string_lossy().into_owned(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct CreateMosaicRequest {
    /// Absolute path where the mosaic will be initialized. Mutually
    /// exclusive with `name` — if both are given, `path` wins.
    pub path: Option<String>,
    /// Just a mosaic name; the server places it under the standard
    /// mosaic root directory (`<data_dir>/tesela/<name>`). Slashes,
    /// `..`, and other path separators are rejected.
    pub name: Option<String>,
    /// Optional import to run after init. `kind`: obsidian | logseq | org.
    pub import: Option<ImportSpec>,
}

#[derive(Debug, Deserialize)]
pub struct ImportSpec {
    pub kind: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct CreateMosaicResponse {
    pub path: String,
    pub import_stdout: Option<String>,
    pub import_stderr: Option<String>,
    pub import_success: Option<bool>,
}

pub async fn create_mosaic(
    Json(req): Json<CreateMosaicRequest>,
) -> Result<Json<CreateMosaicResponse>, (StatusCode, String)> {
    let path = if let Some(p) = req.path.as_ref().filter(|s| !s.trim().is_empty()) {
        PathBuf::from(p)
    } else if let Some(name) = req.name.as_ref().filter(|s| !s.trim().is_empty()) {
        if name.contains('/')
            || name.contains('\\')
            || name.contains("..")
            || name.starts_with('.')
        {
            return Err((
                StatusCode::BAD_REQUEST,
                "Mosaic name can't contain slashes, `..`, or start with `.`".to_string(),
            ));
        }
        Config::mosaic_root_dir().join(name.trim())
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            "Either `path` or `name` is required".to_string(),
        ));
    };
    if path.join(".tesela").exists() {
        return Err((
            StatusCode::CONFLICT,
            format!("a mosaic already exists at {} (`.tesela/` dir present)", path.display()),
        ));
    }

    // Init layout mirrors crates/tesela-cli/src/main.rs::cmd_init.
    let init_path = path.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let tesela_dir = init_path.join(".tesela");
        std::fs::create_dir_all(&tesela_dir)?;
        std::fs::create_dir_all(init_path.join("notes"))?;
        std::fs::create_dir_all(init_path.join("attachments"))?;
        Config::default().save(&tesela_dir.join("config.toml"))?;
        Ok(())
    })
    .await
    .map_err(internal)?
    .map_err(server_error)?;

    // Initializing SQLite needs a tokio runtime (sqlx), so do it in
    // the async context.
    let db_path = path.join(".tesela").join("tesela.db");
    SqliteIndex::open(&db_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("init sqlite: {}", e)))?;

    let mut import_stdout = None;
    let mut import_stderr = None;
    let mut import_success = None;

    if let Some(spec) = req.import {
        let subcommand = match spec.kind.as_str() {
            "obsidian" => "import-obsidian",
            "logseq" => "import-logseq",
            "org" => "import-org",
            other => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!("unknown import kind: {}", other),
                ));
            }
        };
        let mosaic_str = path.to_string_lossy().into_owned();
        let source_owned = spec.source.clone();
        let subcommand_owned = subcommand.to_string();
        let output = tokio::task::spawn_blocking(move || {
            std::process::Command::new("tesela")
                .arg("--mosaic")
                .arg(&mosaic_str)
                .arg(&subcommand_owned)
                .arg("--source")
                .arg(&source_owned)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
        })
        .await
        .map_err(internal)?
        .map_err(internal_io)?;
        import_success = Some(output.status.success());
        import_stdout = Some(String::from_utf8_lossy(&output.stdout).into_owned());
        import_stderr = Some(String::from_utf8_lossy(&output.stderr).into_owned());
    }

    Ok(Json(CreateMosaicResponse {
        path: path.to_string_lossy().into_owned(),
        import_stdout,
        import_stderr,
        import_success,
    }))
}

#[derive(Debug, Serialize)]
pub struct DiscoveredMosaic {
    pub name: String,
    pub path: String,
    pub is_current: bool,
    /// Best-effort count of `.md` files directly under `notes/`.
    pub note_count: usize,
    /// ISO timestamp of the most-recent file mtime under `notes/`,
    /// or null when the dir is empty / unreadable.
    pub last_modified: Option<String>,
}

/// Scan the standard mosaic root for any subdirectory containing a
/// `.tesela/` marker. Always includes the current mosaic, even when
/// it lives outside the standard root (e.g. cwd-walk found a dev
/// mosaic in a git checkout).
pub async fn list_discovered_mosaics(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DiscoveredMosaic>>, (StatusCode, String)> {
    let root = Config::mosaic_root_dir();
    let current = state.mosaic_root.clone();
    let mosaics = tokio::task::spawn_blocking(move || -> std::io::Result<Vec<DiscoveredMosaic>> {
        let mut out: Vec<DiscoveredMosaic> = Vec::new();
        if root.exists() {
            for entry in std::fs::read_dir(&root)? {
                let entry = entry?;
                let p = entry.path();
                if p.is_dir() && p.join(".tesela").exists() {
                    out.push(summarize_mosaic(&p, &current));
                }
            }
        }
        // Include the current mosaic if it isn't already in the list.
        if !out.iter().any(|m| std::path::Path::new(&m.path) == current.as_path()) {
            out.push(summarize_mosaic(&current, &current));
        }
        // Sort: current first, then alpha by name.
        out.sort_by(|a, b| {
            b.is_current
                .cmp(&a.is_current)
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(out)
    })
    .await
    .map_err(internal)?
    .map_err(internal_io)?;
    Ok(Json(mosaics))
}

fn summarize_mosaic(path: &std::path::Path, current: &std::path::Path) -> DiscoveredMosaic {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();
    let notes_dir = path.join("notes");
    let mut note_count = 0usize;
    let mut last_mtime: Option<std::time::SystemTime> = None;
    if let Ok(entries) = std::fs::read_dir(&notes_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("md") {
                note_count += 1;
                if let Ok(meta) = entry.metadata() {
                    if let Ok(m) = meta.modified() {
                        last_mtime = Some(match last_mtime {
                            Some(prev) if prev > m => prev,
                            _ => m,
                        });
                    }
                }
            }
        }
    }
    let last_modified = last_mtime.and_then(|t| {
        chrono::DateTime::<chrono::Local>::from(t)
            .to_rfc3339()
            .into()
    });
    DiscoveredMosaic {
        name,
        path: path.to_string_lossy().into_owned(),
        is_current: path == current,
        note_count,
        last_modified,
    }
}

#[derive(Debug, Deserialize)]
pub struct SwitchMosaicRequest {
    pub path: String,
}

/// Persist the new mosaic as the default in `~/.config/tesela/config.toml`.
/// Doesn't restart anything — call `/server/restart` afterwards to swap.
pub async fn switch_mosaic(
    Json(req): Json<SwitchMosaicRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let target = PathBuf::from(&req.path);
    if !target.join(".tesela").exists() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("{} is not a mosaic (no `.tesela/` dir)", target.display()),
        ));
    }
    let config_path = Config::default_path();
    let mut cfg = if config_path.exists() {
        Config::load(&config_path).map_err(server_error)?
    } else {
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).map_err(internal_io)?;
        }
        Config::default()
    };
    cfg.general.default_mosaic = Some(target.clone());
    cfg.save(&config_path).map_err(server_error)?;
    Ok(Json(serde_json::json!({
        "config_path": config_path.to_string_lossy(),
        "default_mosaic": target.to_string_lossy(),
    })))
}

/// Gracefully shut down (clean-shutdown auto-backup runs), and if no
/// process supervisor is configured, spawn a detached re-exec of
/// ourselves that waits 2 seconds for the port to free before
/// rebinding.
///
/// Passes the freshly-written `default_mosaic` (if any) as
/// `TESELA_DEFAULT_MOSAIC` so the respawned server wins over its
/// cwd-walk — otherwise re-execing from the same working directory
/// finds the old mosaic and the Switch silently no-ops.
pub async fn restart_server() -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    #[cfg(unix)]
    {
        // Read the freshly-written config so we can pin the respawn
        // to the configured default mosaic.
        let pinned_mosaic = {
            let cfg_path = Config::default_path();
            if cfg_path.exists() {
                Config::load(&cfg_path)
                    .ok()
                    .and_then(|c| c.general.default_mosaic)
                    .map(|p| p.to_string_lossy().into_owned())
            } else {
                None
            }
        };
        let respawn_used = maybe_respawn_detached(pinned_mosaic.as_deref())
            .map_err(server_error)?;
        // Schedule SIGTERM after the HTTP response goes out.
        tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            unsafe {
                libc::kill(std::process::id() as i32, libc::SIGTERM);
            }
        });
        Ok(Json(serde_json::json!({
            "respawn_used": respawn_used,
            "pinned_mosaic": pinned_mosaic,
        })))
    }
    #[cfg(not(unix))]
    {
        Err((
            StatusCode::NOT_IMPLEMENTED,
            "server restart is currently Unix-only; stop and relaunch the server manually"
                .to_string(),
        ))
    }
}

#[cfg(unix)]
fn maybe_respawn_detached(pinned_mosaic: Option<&str>) -> anyhow::Result<bool> {
    // If launchd is managing us via the LaunchAgent (`tesela install`),
    // it'll restart automatically — don't double-spawn.
    if launchd_managing_us() {
        return Ok(false);
    }
    let exe = std::env::current_exe()?;
    let exe_str = exe.to_string_lossy().into_owned();
    // sh -c so we can `sleep` before re-exec. The intermediate sh
    // becomes the parent of the new server, then exits via exec.
    // `nohup` + redirected stdio detaches us from the terminal too.
    //
    // The env-var assignment in front of `exec` is the load-bearing
    // bit for the Switch flow — without it the respawned server's
    // cwd-walk would find the *old* mosaic again before checking the
    // config default we just wrote.
    let prefix = match pinned_mosaic {
        Some(m) => format!("TESELA_DEFAULT_MOSAIC={} ", shell_escape(m)),
        None => String::new(),
    };
    std::process::Command::new("nohup")
        .args(["sh", "-c"])
        .arg(format!(
            "sleep 2 && {}exec {}",
            prefix,
            shell_escape(&exe_str)
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(true)
}

#[cfg(unix)]
fn launchd_managing_us() -> bool {
    std::process::Command::new("launchctl")
        .args(["list", "com.tesela.server"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(unix)]
fn shell_escape(s: &str) -> String {
    // Single-quote and escape any embedded single quotes.
    let escaped = s.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

fn internal<E: std::fmt::Display>(e: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e))
}
fn internal_io(e: std::io::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e))
}
fn server_error<E: std::fmt::Display>(e: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e))
}
