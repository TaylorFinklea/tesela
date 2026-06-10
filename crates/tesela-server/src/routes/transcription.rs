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

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::{
    extract::{Multipart, Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

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
    pub inference_supported: bool,
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
    pub inference_supported: bool,
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
            inference_supported: true,
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
            inference_supported: true,
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
            inference_supported: true,
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
            inference_supported: true,
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
            inference_supported: true,
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
            inference_supported: true,
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
            inference_supported: false,
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
            inference_supported: false,
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
        let state = if on_disk_bytes.is_some() {
            "downloaded"
        } else {
            "available"
        };
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
            inference_supported: entry.inference_supported,
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
        inference_supported: entry.inference_supported,
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
    let entry = catalog()
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| AppError::NotFound(format!("Unknown model: {id}")))?;
    if !entry.inference_supported {
        return Err(AppError::Validation(format!(
            "Inference for {id} isn't supported on this Tesela build yet"
        )));
    }
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
pub async fn get_active(State(s): State<Arc<AppState>>) -> AppResult<Json<ActiveModelResponse>> {
    Ok(Json(ActiveModelResponse {
        active: read_active(&s).await,
    }))
}

// ── Phase 26 — actual transcription inference ─────────────────────────

/// In-process cache of the loaded Whisper model. Loading from disk
/// takes a few hundred ms even for small models, so we keep the
/// context around between requests. Re-loaded when the active model
/// changes.
static MODEL_CACHE: OnceLock<Mutex<Option<LoadedModel>>> = OnceLock::new();

struct LoadedModel {
    id: String,
    ctx: WhisperContext,
}

fn model_cache() -> &'static Mutex<Option<LoadedModel>> {
    MODEL_CACHE.get_or_init(|| Mutex::new(None))
}

#[derive(Serialize)]
pub struct TranscribeResponse {
    pub text: String,
    pub model_id: String,
    pub duration_ms: u64,
}

/// POST /transcription/transcribe — multipart upload with one file
/// field named "audio". Decodes 16-bit WAV mono/stereo at any sample
/// rate (resamples to 16kHz mono), runs Whisper inference using the
/// active model, returns the text.
///
/// The active model file lives at `<mosaic>/.tesela/models/<id>.bin`.
/// If no active model is set, returns 400.
pub async fn transcribe(
    State(s): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> AppResult<Json<TranscribeResponse>> {
    let active_id = read_active(&s)
        .await
        .ok_or_else(|| AppError::Validation("No active transcription model".into()))?;
    let model_path = model_path(&s, &active_id);
    if fs::metadata(&model_path).await.is_err() {
        return Err(AppError::Validation(format!(
            "Active model {active_id} not on disk"
        )));
    }

    // Pull the audio file out of the multipart body.
    let mut audio_bytes: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("multipart read: {e}")))?
    {
        if field.name() == Some("audio") {
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("read audio field: {e}")))?;
            audio_bytes = Some(bytes.to_vec());
            break;
        }
    }
    let audio = audio_bytes
        .ok_or_else(|| AppError::Validation("missing 'audio' multipart field".into()))?;

    let started = std::time::Instant::now();
    let samples = decode_audio_to_16k_mono(&audio)?;

    // Run inference on a blocking thread so we don't stall the
    // tokio runtime — Whisper inference is CPU-heavy and the Metal
    // accelerator still drives a synchronous Rust API.
    let model_id_owned = active_id.clone();
    let model_path_owned = model_path.clone();
    let text = tokio::task::spawn_blocking(move || -> Result<String, anyhow::Error> {
        let mut cache = model_cache().lock().unwrap();
        let needs_load = match cache.as_ref() {
            Some(loaded) => loaded.id != model_id_owned,
            None => true,
        };
        if needs_load {
            let path_str = model_path_owned.to_string_lossy().to_string();
            let ctx =
                WhisperContext::new_with_params(&path_str, WhisperContextParameters::default())
                    .map_err(|e| anyhow::anyhow!("load whisper model: {e}"))?;
            *cache = Some(LoadedModel {
                id: model_id_owned.clone(),
                ctx,
            });
        }
        let ctx = &cache.as_ref().unwrap().ctx;
        let mut state = ctx
            .create_state()
            .map_err(|e| anyhow::anyhow!("whisper state: {e}"))?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_language(Some("en"));
        state
            .full(params, &samples)
            .map_err(|e| anyhow::anyhow!("whisper inference: {e}"))?;
        let n = state
            .full_n_segments()
            .map_err(|e| anyhow::anyhow!("segment count: {e}"))?;
        let mut out = String::new();
        for i in 0..n {
            if let Ok(seg) = state.full_get_segment_text(i) {
                out.push_str(&seg);
            }
        }
        Ok(out.trim().to_string())
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("blocking task: {e}")))?
    .map_err(AppError::Internal)?;

    Ok(Json(TranscribeResponse {
        text,
        model_id: active_id,
        duration_ms: started.elapsed().as_millis() as u64,
    }))
}

/// Decode a WAV byte buffer to mono 16kHz f32 samples.
/// Falls back with a Validation error if the format isn't a
/// recognized 16-bit PCM WAV.
fn decode_audio_to_16k_mono(bytes: &[u8]) -> Result<Vec<f32>, AppError> {
    use std::io::Cursor;
    let cursor = Cursor::new(bytes);
    let mut reader = hound::WavReader::new(cursor)
        .map_err(|e| AppError::Validation(format!("not a WAV file: {e}")))?;
    let spec = reader.spec();
    if spec.sample_format != hound::SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err(AppError::Validation("expected 16-bit PCM WAV".into()));
    }
    // Read samples interleaved by channel, then collapse to mono.
    let mut interleaved: Vec<i16> = Vec::with_capacity(reader.len() as usize);
    for s in reader.samples::<i16>() {
        let s = s.map_err(|e| AppError::Validation(format!("read sample: {e}")))?;
        interleaved.push(s);
    }
    let mono: Vec<f32> = if spec.channels == 1 {
        interleaved
            .iter()
            .map(|s| *s as f32 / i16::MAX as f32)
            .collect()
    } else {
        // Average each channel-group into one sample.
        let ch = spec.channels as usize;
        interleaved
            .chunks_exact(ch)
            .map(|frame| {
                let sum: i32 = frame.iter().map(|s| *s as i32).sum();
                (sum as f32 / ch as f32) / i16::MAX as f32
            })
            .collect()
    };
    // Resample to 16kHz if needed via simple linear interpolation.
    let target_rate = 16_000u32;
    if spec.sample_rate == target_rate {
        return Ok(mono);
    }
    let ratio = spec.sample_rate as f64 / target_rate as f64;
    let out_len = (mono.len() as f64 / ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let lo = pos.floor() as usize;
        let hi = (lo + 1).min(mono.len() - 1);
        let frac = (pos - lo as f64) as f32;
        let s = mono[lo] * (1.0 - frac) + mono[hi] * frac;
        out.push(s);
    }
    Ok(out)
}
