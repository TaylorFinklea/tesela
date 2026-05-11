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

fn internal<E: std::fmt::Display>(e: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e))
}
fn internal_io(e: std::io::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e))
}
fn server_error<E: std::fmt::Display>(e: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e))
}
