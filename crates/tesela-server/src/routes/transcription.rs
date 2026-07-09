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

use crate::asr_engine;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::{
    extract::{Multipart, Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
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
    pub inference_supported: bool,
    /// On-disk file name under `.tesela/models/`. Whisper entries keep
    /// the legacy `<id>.bin` scheme so existing installs keep working;
    /// GGUF entries use the upstream artifact name (traceable quant).
    pub file_name: String,
    /// Expected SHA-256 of the download (hex). Verified before the
    /// file is moved into place; `None` skips verification.
    pub sha256: Option<String>,
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

/// Server-side catalog (web + desktop). iOS keeps its own mirrored
/// catalog in TranscriptionCatalog.swift — the two have already
/// diverged deliberately (iOS runs Parakeet via FluidAudio CoreML; the
/// server runs GGUF via transcribe.cpp).
///
/// Sizes are exact bytes and sha256 the LFS oids from the HuggingFace
/// tree API (fetched 2026-07-08); update both together when bumping a
/// model file.
fn catalog() -> Vec<ModelCatalogEntry> {
    let whisper_supported = asr_engine::kind_supported(asr_engine::EngineKind::Whisper);
    let gguf_supported = asr_engine::kind_supported(asr_engine::EngineKind::Gguf);
    vec![
        ModelCatalogEntry {
            id: "whisper-tiny".into(),
            family: "whisper".into(),
            display_name: "Whisper · tiny".into(),
            short_description: "Smallest, fastest. Acceptable for short, clear speech.".into(),
            size_bytes: 77_691_713,
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin".into(),
            suggested_for: vec!["fast capture".into()],
            on_device: true,
            inference_supported: whisper_supported,
            file_name: "whisper-tiny.bin".into(),
            sha256: Some("be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21".into()),
        },
        ModelCatalogEntry {
            id: "whisper-base".into(),
            family: "whisper".into(),
            display_name: "Whisper · base".into(),
            short_description: "Balanced. Good default for everyday voice notes.".into(),
            size_bytes: 147_951_465,
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin".into(),
            suggested_for: vec!["default".into()],
            on_device: true,
            inference_supported: whisper_supported,
            file_name: "whisper-base.bin".into(),
            sha256: Some("60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe".into()),
        },
        ModelCatalogEntry {
            id: "whisper-small".into(),
            family: "whisper".into(),
            display_name: "Whisper · small".into(),
            short_description: "Noticeably better accuracy. Slower on phones.".into(),
            size_bytes: 487_601_967,
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin".into(),
            suggested_for: vec!["accuracy".into()],
            on_device: true,
            inference_supported: whisper_supported,
            file_name: "whisper-small.bin".into(),
            sha256: Some("1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b".into()),
        },
        ModelCatalogEntry {
            id: "whisper-medium".into(),
            family: "whisper".into(),
            display_name: "Whisper · medium".into(),
            short_description: "Strong accuracy. Heavy for on-device.".into(),
            size_bytes: 1_533_763_059,
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin".into(),
            suggested_for: vec!["accuracy".into()],
            on_device: true,
            inference_supported: whisper_supported,
            file_name: "whisper-medium.bin".into(),
            sha256: Some("6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208".into()),
        },
        ModelCatalogEntry {
            id: "whisper-large-v3-turbo".into(),
            family: "whisper".into(),
            display_name: "Whisper · large v3 turbo".into(),
            short_description: "Best large variant for on-device. Fast for its size.".into(),
            size_bytes: 1_624_555_275,
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin".into(),
            suggested_for: vec!["best on-device".into()],
            on_device: true,
            inference_supported: whisper_supported,
            file_name: "whisper-large-v3-turbo.bin".into(),
            sha256: Some("1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69".into()),
        },
        ModelCatalogEntry {
            id: "whisper-large-v3".into(),
            family: "whisper".into(),
            display_name: "Whisper · large v3".into(),
            short_description: "Best accuracy. Slow on phones; great on Mac.".into(),
            size_bytes: 3_095_033_483,
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin".into(),
            suggested_for: vec!["best accuracy".into()],
            on_device: true,
            inference_supported: whisper_supported,
            file_name: "whisper-large-v3.bin".into(),
            sha256: Some("64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2".into()),
        },
        // ── NeMo-family GGUF (transcribe.cpp) ────────────────────────
        // WER-verified quants from the handy-computer HF org (each
        // published GGUF is numerically validated against the NeMo
        // reference — see the transcribe.cpp README).
        ModelCatalogEntry {
            id: "canary-180m-flash".into(),
            family: "canary".into(),
            display_name: "Canary · 180M flash".into(),
            short_description: "NVIDIA · tiny but sharp. English/German/French/Spanish with punctuation.".into(),
            size_bytes: 218_447_552,
            download_url: "https://huggingface.co/handy-computer/canary-180m-flash-gguf/resolve/main/canary-180m-flash-Q8_0.gguf".into(),
            suggested_for: vec!["small + multilingual".into()],
            on_device: true,
            inference_supported: gguf_supported,
            file_name: "canary-180m-flash-Q8_0.gguf".into(),
            sha256: Some("e13c7f5d0952b056a027cfffec13e3a3a134d1608babed24f983568f141e297c".into()),
        },
        ModelCatalogEntry {
            id: "parakeet-unified-en-0.6b".into(),
            family: "parakeet".into(),
            display_name: "Parakeet · Unified 0.6B".into(),
            short_description: "NVIDIA · best accuracy, English only. Streaming-capable (live partials land with the streaming spine).".into(),
            size_bytes: 731_357_568,
            download_url: "https://huggingface.co/handy-computer/parakeet-unified-en-0.6b-gguf/resolve/main/parakeet-unified-en-0.6b-Q8_0.gguf".into(),
            suggested_for: vec!["best accuracy".into()],
            on_device: true,
            inference_supported: gguf_supported,
            file_name: "parakeet-unified-en-0.6b-Q8_0.gguf".into(),
            sha256: Some("4b50b6dd862bf6e346929aaf4f5eaacec003bfa3f56462d6c874b41ef2f38795".into()),
        },
    ]
}

fn models_dir(state: &AppState) -> PathBuf {
    state.mosaic_root.join(".tesela").join("models")
}

fn active_marker(state: &AppState) -> PathBuf {
    models_dir(state).join("ACTIVE")
}

/// On-disk file name for a model id. Catalog entries carry theirs
/// explicitly; unknown ids (e.g. entries removed from the catalog but
/// still on disk) fall back to the legacy sanitized-`.bin` scheme so
/// `delete` can still clean them up.
fn model_file_name(id: &str) -> String {
    match catalog().into_iter().find(|e| e.id == id) {
        Some(e) => e.file_name,
        None => format!("{}.bin", id.replace(['/', '.'], "-")),
    }
}

fn model_path(state: &AppState, id: &str) -> PathBuf {
    models_dir(state).join(model_file_name(id))
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
///
/// The download lands in a `<file>.part` staging file, is SHA-256
/// verified against the catalog, and only then renamed into place —
/// so a killed or corrupted download can never read as "downloaded"
/// (the status check looks at the final path only).
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
    let part = models_dir(&s).join(format!("{}.part", entry.file_name));

    // Stream the download to the staging file, hashing as we go, so we
    // never hold the whole model in memory.
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
    let mut file = fs::File::create(&part)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("create file: {e}")))?;
    let mut hasher = Sha256::new();
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::Internal(anyhow::anyhow!("read chunk: {e}")))?;
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("write chunk: {e}")))?;
    }
    file.flush()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("flush: {e}")))?;
    drop(file);

    if let Some(expected) = &entry.sha256 {
        let got = hex::encode(hasher.finalize());
        if &got != expected {
            let _ = fs::remove_file(&part).await;
            return Err(AppError::Internal(anyhow::anyhow!(
                "checksum mismatch for {}: expected {expected}, got {got} — download discarded",
                entry.id
            )));
        }
    }
    fs::rename(&part, &dest)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("move verified download into place: {e}")))?;

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
// The engines themselves (whisper-rs + transcribe.cpp GGUF) and the
// single-slot model cache live in `crate::asr_engine`; this route only
// decodes audio and dispatches on the active model's catalog family.

#[derive(Serialize)]
pub struct TranscribeResponse {
    pub text: String,
    pub model_id: String,
    pub duration_ms: u64,
}

/// POST /transcription/transcribe — multipart upload with one file
/// field named "audio". Decodes 16-bit WAV mono/stereo at any sample
/// rate (resamples to 16kHz mono), runs inference with the engine the
/// active model's family maps to (whisper-rs or transcribe.cpp), and
/// returns the text.
///
/// The active model file lives under `<mosaic>/.tesela/models/`.
/// If no active model is set, returns 400.
pub async fn transcribe(
    State(s): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> AppResult<Json<TranscribeResponse>> {
    let active_id = read_active(&s)
        .await
        .ok_or_else(|| AppError::Validation("No active transcription model".into()))?;
    let entry = catalog()
        .into_iter()
        .find(|e| e.id == active_id)
        .ok_or_else(|| {
            AppError::Validation(format!("Active model {active_id} is not in the catalog"))
        })?;
    let kind = asr_engine::kind_for_family(&entry.family)
        .filter(|k| asr_engine::kind_supported(*k))
        .ok_or_else(|| {
            AppError::Validation(format!(
                "Inference for {active_id} isn't supported on this Tesela build"
            ))
        })?;
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

    // Run inference on a blocking thread so we don't stall the tokio
    // runtime — both engines drive synchronous, CPU/GPU-heavy APIs.
    let model_id_owned = active_id.clone();
    let model_path_owned = model_path.clone();
    let text = tokio::task::spawn_blocking(move || {
        asr_engine::transcribe_blocking(kind, &model_id_owned, &model_path_owned, &samples)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn wav_bytes(spec: hound::WavSpec, samples: &[i16]) -> Vec<u8> {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
            for s in samples {
                writer.write_sample(*s).unwrap();
            }
            writer.finalize().unwrap();
        }
        cursor.into_inner()
    }

    #[test]
    fn catalog_entries_are_well_formed() {
        let cat = catalog();
        let mut ids = std::collections::HashSet::new();
        for e in &cat {
            assert!(ids.insert(e.id.clone()), "duplicate catalog id {}", e.id);
            assert!(!e.file_name.is_empty(), "{}: empty file_name", e.id);
            assert!(
                !e.file_name.contains('/') && !e.file_name.contains(".."),
                "{}: file_name must be a bare name",
                e.id
            );
            assert!(
                asr_engine::kind_for_family(&e.family).is_some(),
                "{}: family `{}` has no engine mapping",
                e.id,
                e.family
            );
            let sha = e.sha256.as_ref().expect("all shipped entries carry a sha256");
            assert_eq!(sha.len(), 64, "{}: sha256 must be 64 hex chars", e.id);
            assert!(
                sha.chars().all(|c| c.is_ascii_hexdigit()),
                "{}: sha256 must be hex",
                e.id
            );
        }
    }

    #[test]
    fn whisper_entries_keep_legacy_file_names() {
        // Existing installs have `<id>.bin` on disk (the old sanitized
        // scheme); renaming would orphan every downloaded model.
        for e in catalog().iter().filter(|e| e.family == "whisper") {
            assert_eq!(e.file_name, format!("{}.bin", e.id));
        }
    }

    #[test]
    fn gguf_entries_track_the_transcribecpp_feature() {
        let expect = cfg!(feature = "transcribecpp");
        let gguf: Vec<_> = catalog()
            .into_iter()
            .filter(|e| e.family == "canary" || e.family == "parakeet")
            .collect();
        assert_eq!(gguf.len(), 2, "expected canary + parakeet-unified entries");
        for e in gguf {
            assert_eq!(
                e.inference_supported, expect,
                "{}: inference_supported should mirror the feature",
                e.id
            );
            assert!(e.file_name.ends_with(".gguf"), "{}: expected a GGUF artifact", e.id);
            let url_tail = e.download_url.rsplit('/').next().unwrap();
            assert_eq!(
                url_tail, e.file_name,
                "{}: GGUF file_name should mirror the upstream artifact name",
                e.id
            );
        }
    }

    #[test]
    fn model_file_name_falls_back_to_legacy_scheme_for_unknown_ids() {
        // Removed catalog entries (the old .nemo placeholders) must
        // still resolve so `delete` can clean their files up.
        assert_eq!(
            model_file_name("parakeet-tdt-0.6b"),
            "parakeet-tdt-0-6b.bin"
        );
        assert_eq!(
            model_file_name("canary-180m-flash"),
            "canary-180m-flash-Q8_0.gguf"
        );
    }

    #[test]
    fn decode_upsamples_8k_mono_to_16k() {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let samples: Vec<i16> = (0..800).map(|i| (i % 100) as i16 * 100).collect();
        let out = decode_audio_to_16k_mono(&wav_bytes(spec, &samples)).unwrap();
        assert_eq!(out.len(), 1_600);
        assert!(out.iter().all(|s| (-1.0..=1.0).contains(s)));
    }

    #[test]
    fn decode_collapses_stereo_to_mono() {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        // L = 1000, R = -1000 → averaged mono ≈ 0.
        let samples: Vec<i16> = (0..640).map(|i| if i % 2 == 0 { 1000 } else { -1000 }).collect();
        let out = decode_audio_to_16k_mono(&wav_bytes(spec, &samples)).unwrap();
        assert_eq!(out.len(), 320);
        assert!(out.iter().all(|s| s.abs() < 1e-3));
    }

    #[test]
    fn decode_rejects_non_wav_and_non_16bit() {
        let err = decode_audio_to_16k_mono(b"definitely not audio").unwrap_err();
        assert!(matches!(err, AppError::Validation(ref m) if m.contains("not a WAV file")));

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
            for _ in 0..64 {
                writer.write_sample(0.5f32).unwrap();
            }
            writer.finalize().unwrap();
        }
        let err = decode_audio_to_16k_mono(&cursor.into_inner()).unwrap_err();
        assert!(matches!(err, AppError::Validation(ref m) if m.contains("expected 16-bit PCM WAV")));
    }
}
