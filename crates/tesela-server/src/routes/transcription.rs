//! Transcription model management — Settings → Voice → Manage models
//! on both iOS and the web client. The desktop is the source of truth
//! for the catalog; iOS may mirror its own downloads but the server
//! also serves a copy stored under `<mosaic_root>/.tesela/models/`.
//!
//! Endpoints:
//!   GET    /transcription/models                    → catalog + on-disk state
//!   POST   /transcription/models/{id}/download      → start a download
//!   DELETE /transcription/models/{id}               → remove the local file
//!   POST   /transcription/models/{id}/activate      → set as the active model
//!   GET    /transcription/active                    → current active model id

use crate::state::AppState;
use crate::error::{AppError, AppResult};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCatalogEntry {
    pub id: String,
    pub family: String,
    pub display_name: String,
    pub short_description: String,
    pub size_bytes: u64,
    pub download_url: String,
    pub suggested_for: Vec<String>,
    pub on_device: bool,
}

#[derive(Debug, Serialize)]
pub struct ModelStatus {
    pub id: String,
    pub family: String,
    pub display_name: String,
    pub short_description: String,
    pub size_bytes: u64,
    pub download_url: String,
    pub suggested_for: Vec<String>,
    pub on_device: bool,
    /// "available" | "downloading" | "downloaded" | "failed"
    pub state: String,
    pub on_disk_bytes: Option<u64>,
    pub active: bool,
}

/// Same catalog the iOS app ships with. Keeping this hand-mirrored
/// is fine for now; if it drifts we can centralize via tesela-core.
fn catalog() -> Vec<ModelCatalogEntry> {
    vec![
        ModelCatalogEntry {
            id: "whisper-tiny".into(),
            family: "whisper".into(),
            display_name: "Whisper · tiny".into(),
            short_description: "Smallest, fastest. Acceptable for short, clear speech.".into(),
            size_bytes: 39 * 1024 * 1024,
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin".into(),
            suggested_for: vec!["fast capture".into()],
            on_device: true,
        },
        ModelCatalogEntry {
            id: "whisper-base".into(),
            family: "whisper".into(),
            display_name: "Whisper · base".into(),
            short_description: "Balanced. Good default for everyday voice notes.".into(),
            size_bytes: 142 * 1024 * 1024,
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin".into(),
            suggested_for: vec!["default".into()],
            on_device: true,
        },
        ModelCatalogEntry {
            id: "whisper-small".into(),
            family: "whisper".into(),
            display_name: "Whisper · small".into(),
            short_description: "Noticeably better accuracy. Slower on phones.".into(),
            size_bytes: 466 * 1024 * 1024,
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin".into(),
            suggested_for: vec!["accuracy".into()],
            on_device: true,
        },
        ModelCatalogEntry {
            id: "whisper-medium".into(),
            family: "whisper".into(),
            display_name: "Whisper · medium".into(),
            short_description: "Strong accuracy. Heavy for on-device.".into(),
            size_bytes: 1_500_000_000,
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin".into(),
            suggested_for: vec!["accuracy".into()],
            on_device: true,
        },
        ModelCatalogEntry {
            id: "whisper-large-v3-turbo".into(),
            family: "whisper".into(),
            display_name: "Whisper · large v3 turbo".into(),
            short_description: "Best large variant for on-device. Fast for its size.".into(),
            size_bytes: 1_700_000_000,
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin".into(),
            suggested_for: vec!["best on-device".into()],
            on_device: true,
        },
        ModelCatalogEntry {
            id: "whisper-large-v3".into(),
            family: "whisper".into(),
            display_name: "Whisper · large v3".into(),
            short_description: "Best accuracy. Slow on phones; great on Mac.".into(),
            size_bytes: 3_100_000_000,
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin".into(),
            suggested_for: vec!["best accuracy".into()],
            on_device: true,
        },
        ModelCatalogEntry {
            id: "parakeet-tdt-0.6b".into(),
            family: "parakeet".into(),
            display_name: "Parakeet · TDT 0.6B".into(),
            short_description: "NVIDIA NeMo · fast streaming transcription.".into(),
            size_bytes: 620_000_000,
            download_url: "https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2/resolve/main/parakeet-tdt-0.6b-v2.nemo".into(),
            suggested_for: vec!["streaming".into(), "low latency".into()],
            on_device: true,
        },
        ModelCatalogEntry {
            id: "parakeet-tdt-1.1b".into(),
            family: "parakeet".into(),
            display_name: "Parakeet · TDT 1.1B".into(),
            short_description: "Larger Parakeet. Higher accuracy than 0.6B.".into(),
            size_bytes: 1_100_000_000,
            download_url: "https://huggingface.co/nvidia/parakeet-tdt-1.1b/resolve/main/parakeet-tdt-1.1b.nemo".into(),
            suggested_for: vec!["accuracy".into()],
            on_device: true,
        },
    ]
}

fn models_dir(state: &AppState) -> PathBuf {
    state.mosaic_root.join(".tesela").join("models")
}

fn active_marker(state: &AppState) -> PathBuf {
    models_dir(state).join("ACTIVE")
}

fn model_path(state: &AppState, id: &str) -> PathBuf {
    let safe = id.replace(['/', '.'], "-");
    models_dir(state).join(format!("{safe}.bin"))
}

async fn read_active(state: &AppState) -> Option<String> {
    fs::read_to_string(active_marker(state))
        .await
        .ok()
        .map(|s| s.trim().to_string())
}

async fn write_active(state: &AppState, id: &str) -> std::io::Result<()> {
    fs::create_dir_all(models_dir(state)).await?;
    fs::write(active_marker(state), id).await
}

/// GET /transcription/models — full catalog enriched with the local
/// status for each entry.
pub async fn list_models(State(s): State<Arc<AppState>>) -> AppResult<Json<Vec<ModelStatus>>> {
    let active = read_active(&s).await;
    let mut out: Vec<ModelStatus> = Vec::new();
    for entry in catalog() {
        let path = model_path(&s, &entry.id);
        let on_disk_bytes = match fs::metadata(&path).await {
            Ok(m) => Some(m.len()),
            Err(_) => None,
        };
        let state = if on_disk_bytes.is_some() { "downloaded" } else { "available" };
        out.push(ModelStatus {
            id: entry.id.clone(),
            family: entry.family,
            display_name: entry.display_name,
            short_description: entry.short_description,
            size_bytes: entry.size_bytes,
            download_url: entry.download_url,
            suggested_for: entry.suggested_for,
            on_device: entry.on_device,
            state: state.into(),
            on_disk_bytes,
            active: active.as_deref() == Some(entry.id.as_str()),
        });
    }
    Ok(Json(out))
}

/// POST /transcription/models/{id}/download — synchronously fetch the
/// model and write it under `.tesela/models/`. Returns the final
/// on-disk size on success. Synchronous because the web UI polls
/// `GET /transcription/models` for progress separately; this keeps
/// the endpoint simple and idempotent.
pub async fn download_model(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<ModelStatus>> {
    let entry = catalog()
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| AppError::NotFound(format!("Unknown model: {id}")))?;
    fs::create_dir_all(models_dir(&s))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("create models dir: {e}")))?;
    let dest = model_path(&s, &entry.id);

    // Stream the download to disk so we don't load the whole model
    // into memory.
    let resp = reqwest::get(&entry.download_url)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("download: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "download failed with HTTP {}",
            resp.status()
        )));
    }
    let mut stream = resp.bytes_stream();
    let mut file = fs::File::create(&dest)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("create file: {e}")))?;
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::Internal(anyhow::anyhow!("read chunk: {e}")))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("write chunk: {e}")))?;
    }
    file.flush()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("flush: {e}")))?;

    let size = fs::metadata(&dest)
        .await
        .map(|m| m.len())
        .unwrap_or(entry.size_bytes);
    let active = read_active(&s).await;
    Ok(Json(ModelStatus {
        id: entry.id.clone(),
        family: entry.family,
        display_name: entry.display_name,
        short_description: entry.short_description,
        size_bytes: entry.size_bytes,
        download_url: entry.download_url,
        suggested_for: entry.suggested_for,
        on_device: entry.on_device,
        state: "downloaded".into(),
        on_disk_bytes: Some(size),
        active: active.as_deref() == Some(entry.id.as_str()),
    }))
}

/// DELETE /transcription/models/{id} — remove the on-disk file. If
/// this was the active model, clear the marker too.
pub async fn delete_model(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<serde_json::Value>> {
    let path = model_path(&s, &id);
    let _ = fs::remove_file(&path).await;
    if read_active(&s).await.as_deref() == Some(id.as_str()) {
        let _ = fs::remove_file(active_marker(&s)).await;
    }
    Ok(Json(serde_json::json!({ "ok": true, "id": id })))
}

/// POST /transcription/models/{id}/activate — set the active marker.
/// Refuses if the model isn't on disk yet so the client can't end up
/// pointing at a missing file.
pub async fn activate_model(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<serde_json::Value>> {
    let path = model_path(&s, &id);
    if fs::metadata(&path).await.is_err() {
        return Err(AppError::Validation(format!(
            "Model {id} isn't downloaded yet"
        )));
    }
    write_active(&s, &id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("write active marker: {e}")))?;
    Ok(Json(serde_json::json!({ "ok": true, "active": id })))
}

#[derive(Serialize)]
pub struct ActiveModelResponse {
    pub active: Option<String>,
}

/// GET /transcription/active — current active model id, if any.
pub async fn get_active(
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<ActiveModelResponse>> {
    Ok(Json(ActiveModelResponse {
        active: read_active(&s).await,
    }))
}
