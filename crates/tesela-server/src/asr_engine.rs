//! Speech-to-text engine dispatch for `/transcription/transcribe`.
//!
//! One engine per build, selected by cargo feature:
//!
//! - `transcribecpp` (default) → transcribe.cpp runs EVERY family:
//!   Whisper GGML/GGUF (architecture auto-detected from the file) plus
//!   NeMo-family GGUF (Canary, Parakeet) — dictation P1, tesela-v5t.1.
//! - `whisper-fallback` → the Phase 26 whisper-rs path, whisper-only,
//!   kept as an emergency build while transcribe.cpp (v0.1.x) proves
//!   out: `--no-default-features --features whisper-fallback`.
//!
//! The two are MUTUALLY EXCLUSIVE: whisper-rs and transcribe-cpp each
//! statically vendor ggml, and two ggml copies interleave at link time
//! — whisper.cpp's backend registry then walks transcribe.cpp's Metal
//! backend and aborts (GGML_ASSERT(index == 0), ggml-metal.m:3889).
//! This is why Handy v0.9.0 deleted whisper-rs when it adopted
//! transcribe.cpp.
//!
//! Every engine consumes the same input contract: 16 kHz mono f32 PCM
//! in [-1, 1] — exactly what `decode_audio_to_16k_mono` produces
//! server-side and what the iOS `TranscriptionEngine` protocol feeds
//! on-device.

#[cfg(all(feature = "transcribecpp", feature = "whisper-fallback"))]
compile_error!(
    "features `transcribecpp` and `whisper-fallback` are mutually exclusive: \
     both statically vendor ggml and the merged symbols abort at model load \
     (build the fallback with --no-default-features --features whisper-fallback)"
);

use anyhow::{anyhow, Result};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

#[cfg(feature = "whisper-fallback")]
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Which inference backend a catalog family maps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    /// Whisper GGML — runs on transcribe.cpp (auto-detected) in the
    /// default build, or whisper-rs under `whisper-fallback`.
    Whisper,
    /// NeMo-family GGUF (Canary, Parakeet) — transcribe.cpp only.
    Gguf,
}

/// Map a catalog `family` string to the backend that runs it.
pub fn kind_for_family(family: &str) -> Option<EngineKind> {
    match family {
        "whisper" => Some(EngineKind::Whisper),
        "canary" | "parakeet" => Some(EngineKind::Gguf),
        _ => None,
    }
}

/// True when this build can actually run models of `kind`. The catalog
/// surfaces this as `inference_supported`, and `activate` gates on it.
pub fn kind_supported(kind: EngineKind) -> bool {
    match kind {
        EngineKind::Whisper => {
            cfg!(any(feature = "transcribecpp", feature = "whisper-fallback"))
        }
        EngineKind::Gguf => cfg!(feature = "transcribecpp"),
    }
}

/// One loaded model, ready to transcribe. Loading takes hundreds of ms
/// to seconds (disk read + weight upload), so the process keeps the
/// single-slot cache below and reloads only when the active model
/// changes.
pub struct LoadedEngine {
    id: String,
    backend: Backend,
}

enum Backend {
    #[cfg(feature = "transcribecpp")]
    TranscribeCpp(transcribe_cpp::Model),
    #[cfg(feature = "whisper-fallback")]
    WhisperRs(WhisperContext),
    /// Keeps the enum non-empty (and the match arms honest) in a build
    /// with neither engine feature.
    #[allow(dead_code)]
    Unavailable,
}

impl LoadedEngine {
    /// Blocking (disk + weight upload) — call from `spawn_blocking`.
    fn load(kind: EngineKind, id: &str, path: &Path) -> Result<Self> {
        if !kind_supported(kind) {
            return Err(anyhow!(
                "no engine compiled for this model family in this build"
            ));
        }
        let backend = load_backend(kind, path)?;
        Ok(Self {
            id: id.to_string(),
            backend,
        })
    }

    /// Blocking inference over 16 kHz mono f32 PCM.
    fn transcribe(&mut self, samples: &[f32]) -> Result<String> {
        match &mut self.backend {
            #[cfg(feature = "transcribecpp")]
            Backend::TranscribeCpp(model) => {
                let mut session = model
                    .session()
                    .map_err(|e| anyhow!("transcribe.cpp session: {e}"))?;
                let result = session
                    .run(samples, &transcribe_cpp::RunOptions::default())
                    .map_err(|e| anyhow!("transcribe.cpp inference: {e}"))?;
                Ok(result.text.trim().to_string())
            }
            #[cfg(feature = "whisper-fallback")]
            Backend::WhisperRs(ctx) => {
                let mut state = ctx
                    .create_state()
                    .map_err(|e| anyhow!("whisper state: {e}"))?;
                let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
                params.set_print_special(false);
                params.set_print_progress(false);
                params.set_print_realtime(false);
                params.set_print_timestamps(false);
                params.set_language(Some("en"));
                state
                    .full(params, samples)
                    .map_err(|e| anyhow!("whisper inference: {e}"))?;
                let n = state
                    .full_n_segments()
                    .map_err(|e| anyhow!("segment count: {e}"))?;
                let mut out = String::new();
                for i in 0..n {
                    if let Ok(seg) = state.full_get_segment_text(i) {
                        out.push_str(&seg);
                    }
                }
                Ok(out.trim().to_string())
            }
            Backend::Unavailable => Err(anyhow!(
                "no engine compiled for this model family in this build"
            )),
        }
    }
}

#[cfg(feature = "transcribecpp")]
fn load_backend(_kind: EngineKind, path: &Path) -> Result<Backend> {
    // transcribe.cpp auto-detects the architecture from the file, so
    // Whisper GGML .bin and NeMo GGUF go through the same loader.
    let model = transcribe_cpp::Model::load(path.to_string_lossy().as_ref())
        .map_err(|e| anyhow!("transcribe.cpp model load: {e}"))?;
    Ok(Backend::TranscribeCpp(model))
}

#[cfg(all(not(feature = "transcribecpp"), feature = "whisper-fallback"))]
fn load_backend(kind: EngineKind, path: &Path) -> Result<Backend> {
    match kind {
        EngineKind::Whisper => {
            let path_str = path.to_string_lossy().to_string();
            let ctx =
                WhisperContext::new_with_params(&path_str, WhisperContextParameters::default())
                    .map_err(|e| anyhow!("load whisper model: {e}"))?;
            Ok(Backend::WhisperRs(ctx))
        }
        EngineKind::Gguf => Err(anyhow!(
            "GGUF (Canary/Parakeet) models need the `transcribecpp` build"
        )),
    }
}

#[cfg(all(not(feature = "transcribecpp"), not(feature = "whisper-fallback")))]
fn load_backend(_kind: EngineKind, _path: &Path) -> Result<Backend> {
    Ok(Backend::Unavailable)
}

/// Single-slot in-process model cache: one loaded model at a time, all
/// transcriptions serialized behind the mutex (same posture as the
/// Phase 26 whisper cache; per-session engines arrive with the P2
/// streaming work).
static ENGINE_CACHE: OnceLock<Mutex<Option<LoadedEngine>>> = OnceLock::new();

fn cache() -> &'static Mutex<Option<LoadedEngine>> {
    ENGINE_CACHE.get_or_init(|| Mutex::new(None))
}

/// Ensure `id` is the cached model (loading it if needed) and run one
/// transcription. Blocking — call from `spawn_blocking`.
pub fn transcribe_blocking(
    kind: EngineKind,
    id: &str,
    path: &Path,
    samples: &[f32],
) -> Result<String> {
    let mut slot = cache().lock().unwrap();
    let needs_load = match slot.as_ref() {
        Some(loaded) => loaded.id != id,
        None => true,
    };
    if needs_load {
        // Drop the previous model before loading the next so peak
        // memory holds one model, not two.
        *slot = None;
        *slot = Some(LoadedEngine::load(kind, id, path)?);
    }
    slot.as_mut().unwrap().transcribe(samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_routing_covers_catalog_families() {
        assert_eq!(kind_for_family("whisper"), Some(EngineKind::Whisper));
        assert_eq!(kind_for_family("canary"), Some(EngineKind::Gguf));
        assert_eq!(kind_for_family("parakeet"), Some(EngineKind::Gguf));
        assert_eq!(kind_for_family("nemo"), None);
    }

    #[test]
    fn engine_support_tracks_build_features() {
        assert_eq!(
            kind_supported(EngineKind::Whisper),
            cfg!(any(feature = "transcribecpp", feature = "whisper-fallback"))
        );
        assert_eq!(
            kind_supported(EngineKind::Gguf),
            cfg!(feature = "transcribecpp")
        );
    }
}
