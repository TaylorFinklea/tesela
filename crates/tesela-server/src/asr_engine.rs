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

/// True while a live streaming session holds the engine lease. Batch
/// requests fail fast instead of loading a second copy of a 700 MB
/// model next to the leased one (Handy's engine-lease posture).
///
/// Read/written ONLY while holding the [`cache`] mutex, so the
/// check-then-act (fail-fast vs. take-the-engine) is atomic against
/// batch requests. It stays an atomic because [`StreamLease`]'s Drop
/// must clear it without re-taking the lock on an unwinding path where
/// the lock may be held.
static STREAM_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// RAII guard for the streaming lease. Clearing the flag on Drop makes
/// the lease panic-safe: a Rust panic anywhere in the session body
/// (feed/finalize error mapped to a panic, a poisoned lock, etc.)
/// still releases the lease as the stack unwinds — without it, one
/// panic would wedge STREAM_ACTIVE=true forever and brick BOTH live
/// dictation and batch transcription for the rest of the process.
///
/// (A transcribe.cpp `abort()` — e.g. a GGML_ASSERT — kills the whole
/// process, so there is no lease left to leak in that case.)
struct StreamLease;

impl StreamLease {
    /// Acquire the exclusive streaming lease under the cache lock, or
    /// return None if a session already holds it. Caller must hold the
    /// cache mutex so the flag flip is atomic with the engine take.
    fn acquire_locked() -> Option<StreamLease> {
        if STREAM_ACTIVE.swap(true, std::sync::atomic::Ordering::AcqRel) {
            None
        } else {
            Some(StreamLease)
        }
    }
}

impl Drop for StreamLease {
    fn drop(&mut self) {
        STREAM_ACTIVE.store(false, std::sync::atomic::Ordering::Release);
    }
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
    // Checked UNDER the cache lock (not before it) so a streaming
    // session that acquired the lease can't slip in between the check
    // and the load and leave two models resident.
    if STREAM_ACTIVE.load(std::sync::atomic::Ordering::Acquire) {
        return Err(anyhow!(
            "a live dictation session is in progress — retry when it ends"
        ));
    }
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

// ── Streaming sessions (dictation P2, tesela-v5t.2) ──────────────────

/// Commands the WS task sends into the streaming worker.
pub enum StreamCmd {
    /// One chunk of 16 kHz mono f32 PCM.
    Audio(Vec<f32>),
    /// Finalize: flush the stream and emit the final transcript.
    Stop,
}

/// Events the streaming worker sends back to the WS task. Serialized
/// verbatim as the wire protocol's text frames.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Model is loaded and the session accepts audio. `streaming:false`
    /// means the active model has no native streaming mode — audio is
    /// accumulated and transcribed once on stop (no partials).
    Ready {
        model_id: String,
        streaming: bool,
    },
    /// The committed/tentative split moved. Emitted only on change.
    Partial {
        committed: String,
        tentative: String,
        revision: i32,
    },
    /// The session finished; `text` is the full transcript.
    Final {
        text: String,
        model_id: String,
        duration_ms: u64,
    },
    Error {
        message: String,
    },
}

/// Hard cap on buffered session audio: 15 minutes at 16 kHz. Guards the
/// batch-fallback accumulator (and any runaway client) from unbounded
/// memory.
const MAX_SESSION_SAMPLES: usize = 16_000 * 60 * 15;

/// Run one full streaming dictation session on the current thread.
/// Blocking — call from `spawn_blocking`. Owns the engine lease for the
/// session's lifetime: taken from (or loaded into) the single-slot
/// cache up front, returned on every exit path. All outcomes — ready,
/// partials, final, errors — flow through `events`; the function itself
/// only errs internally.
pub fn stream_session_blocking(
    kind: EngineKind,
    id: &str,
    path: &Path,
    mut commands: tokio::sync::mpsc::Receiver<StreamCmd>,
    events: tokio::sync::mpsc::UnboundedSender<StreamEvent>,
) {
    // Acquire the lease and take the engine ATOMICALLY under the cache
    // lock: a batch request that locks the cache after this point sees
    // STREAM_ACTIVE=true and fails fast, so no second model is loaded
    // beside this session's. `_lease`'s Drop clears the flag on every
    // path below, including a panic mid-session.
    let (_lease, engine) = {
        let mut slot = cache().lock().unwrap();
        let Some(lease) = StreamLease::acquire_locked() else {
            let _ = events.send(StreamEvent::Error {
                message: "another dictation session is already live".into(),
            });
            return;
        };
        let engine = match slot.take() {
            Some(loaded) if loaded.id == id => Ok(loaded),
            other => {
                // Wrong (or no) cached model: drop it first, then load.
                drop(other);
                LoadedEngine::load(kind, id, path)
            }
        };
        (lease, engine)
    };
    let mut engine = match engine {
        Ok(e) => e,
        Err(e) => {
            let _ = events.send(StreamEvent::Error {
                message: format!("model load: {e}"),
            });
            return; // _lease Drop clears STREAM_ACTIVE
        }
    };

    run_session(&mut engine, id, &mut commands, &events);

    // Return the engine so the next batch request (or session) reuses
    // it without a reload. The lease flag is cleared by `_lease`'s Drop
    // AFTER the engine is back in the slot — order matters so a batch
    // request that observes the flag cleared also finds the engine.
    *cache().lock().unwrap() = Some(engine);
}

/// The session body, engine already leased. Separated so every exit
/// path in here still runs the lease-return in the caller.
fn run_session(
    engine: &mut LoadedEngine,
    id: &str,
    commands: &mut tokio::sync::mpsc::Receiver<StreamCmd>,
    events: &tokio::sync::mpsc::UnboundedSender<StreamEvent>,
) {
    let started = std::time::Instant::now();
    if engine_supports_streaming(engine) {
        run_native_streaming(engine, id, commands, events, started);
    } else {
        run_batch_fallback(engine, id, commands, events, started);
    }
}

fn engine_supports_streaming(engine: &LoadedEngine) -> bool {
    match &engine.backend {
        #[cfg(feature = "transcribecpp")]
        Backend::TranscribeCpp(model) => model.capabilities().supports_streaming,
        #[cfg(feature = "whisper-fallback")]
        Backend::WhisperRs(_) => false,
        Backend::Unavailable => false,
    }
}

/// Native transcribe.cpp streaming: feed → committed/tentative partials
/// on change → finalize.
#[cfg(feature = "transcribecpp")]
fn run_native_streaming(
    engine: &mut LoadedEngine,
    id: &str,
    commands: &mut tokio::sync::mpsc::Receiver<StreamCmd>,
    events: &tokio::sync::mpsc::UnboundedSender<StreamEvent>,
    started: std::time::Instant,
) {
    let Backend::TranscribeCpp(model) = &mut engine.backend else {
        unreachable!("engine_supports_streaming gated on TranscribeCpp");
    };
    let mut session = match model.session() {
        Ok(s) => s,
        Err(e) => {
            let _ = events.send(StreamEvent::Error {
                message: format!("transcribe.cpp session: {e}"),
            });
            return;
        }
    };
    let opts = transcribe_cpp::StreamOptions {
        commit_policy: transcribe_cpp::CommitPolicy::Auto,
        ..Default::default()
    };
    let mut stream = match session.stream(&transcribe_cpp::RunOptions::default(), &opts) {
        Ok(s) => s,
        Err(e) => {
            let _ = events.send(StreamEvent::Error {
                message: format!("transcribe.cpp stream begin: {e}"),
            });
            return;
        }
    };
    let _ = events.send(StreamEvent::Ready {
        model_id: id.to_string(),
        streaming: true,
    });

    let mut fed: usize = 0;
    while let Some(cmd) = commands.blocking_recv() {
        match cmd {
            StreamCmd::Audio(chunk) => {
                fed += chunk.len();
                if fed > MAX_SESSION_SAMPLES {
                    let _ = events.send(StreamEvent::Error {
                        message: "session exceeded the 15-minute audio cap".into(),
                    });
                    return;
                }
                match stream.feed(&chunk) {
                    Ok(update) => {
                        if update.committed_changed || update.tentative_changed {
                            let text = stream.text();
                            let _ = events.send(StreamEvent::Partial {
                                committed: text.committed,
                                tentative: text.tentative,
                                revision: update.revision,
                            });
                        }
                    }
                    Err(e) => {
                        let _ = events.send(StreamEvent::Error {
                            message: format!("transcribe.cpp feed: {e}"),
                        });
                        return;
                    }
                }
            }
            StreamCmd::Stop => {
                match stream.finalize() {
                    Ok(_update) => {
                        let text = stream.text();
                        let _ = events.send(StreamEvent::Final {
                            text: text.full.trim().to_string(),
                            model_id: id.to_string(),
                            duration_ms: started.elapsed().as_millis() as u64,
                        });
                    }
                    Err(e) => {
                        let _ = events.send(StreamEvent::Error {
                            message: format!("transcribe.cpp finalize: {e}"),
                        });
                    }
                }
                return;
            }
        }
    }
    // Channel closed without Stop: the client vanished — cancel quietly.
}

#[cfg(not(feature = "transcribecpp"))]
fn run_native_streaming(
    _engine: &mut LoadedEngine,
    _id: &str,
    _commands: &mut tokio::sync::mpsc::Receiver<StreamCmd>,
    events: &tokio::sync::mpsc::UnboundedSender<StreamEvent>,
    _started: std::time::Instant,
) {
    let _ = events.send(StreamEvent::Error {
        message: "streaming needs the `transcribecpp` build".into(),
    });
}

/// Non-streaming model (whisper under the fallback build, or a batch
/// GGUF family): accumulate the session's audio, run one batch pass on
/// stop. No partials — the client sees `streaming:false` in Ready and
/// shows a spinner instead.
fn run_batch_fallback(
    engine: &mut LoadedEngine,
    id: &str,
    commands: &mut tokio::sync::mpsc::Receiver<StreamCmd>,
    events: &tokio::sync::mpsc::UnboundedSender<StreamEvent>,
    started: std::time::Instant,
) {
    let _ = events.send(StreamEvent::Ready {
        model_id: id.to_string(),
        streaming: false,
    });
    let mut samples: Vec<f32> = Vec::new();
    while let Some(cmd) = commands.blocking_recv() {
        match cmd {
            StreamCmd::Audio(chunk) => {
                if samples.len() + chunk.len() > MAX_SESSION_SAMPLES {
                    let _ = events.send(StreamEvent::Error {
                        message: "session exceeded the 15-minute audio cap".into(),
                    });
                    return;
                }
                samples.extend_from_slice(&chunk);
            }
            StreamCmd::Stop => {
                match engine.transcribe(&samples) {
                    Ok(text) => {
                        let _ = events.send(StreamEvent::Final {
                            text,
                            model_id: id.to_string(),
                            duration_ms: started.elapsed().as_millis() as u64,
                        });
                    }
                    Err(e) => {
                        let _ = events.send(StreamEvent::Error {
                            message: format!("transcription: {e}"),
                        });
                    }
                }
                return;
            }
        }
    }
    // Channel closed without Stop: cancel quietly.
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

    // These serialize on the process-global STREAM_ACTIVE flag, so they
    // share one #[test] to avoid cross-test interference.
    #[test]
    fn stream_lease_is_exclusive_and_panic_safe() {
        use std::sync::atomic::Ordering;

        assert!(!STREAM_ACTIVE.load(Ordering::Acquire), "flag starts clear");

        // Held → exclusive.
        {
            let lease = StreamLease::acquire_locked();
            assert!(lease.is_some());
            assert!(STREAM_ACTIVE.load(Ordering::Acquire));
            assert!(
                StreamLease::acquire_locked().is_none(),
                "a second lease is refused while one is held"
            );
        }
        // Dropped → cleared, re-acquirable.
        assert!(
            !STREAM_ACTIVE.load(Ordering::Acquire),
            "Drop clears the flag"
        );
        assert!(StreamLease::acquire_locked().is_some());
        assert!(!STREAM_ACTIVE.load(Ordering::Acquire));

        // Panic while holding the lease still clears it as the stack
        // unwinds — the whole point of the RAII guard (a leak here would
        // brick all transcription for the process).
        let panicked = std::panic::catch_unwind(|| {
            let _lease = StreamLease::acquire_locked().expect("acquire");
            assert!(STREAM_ACTIVE.load(Ordering::Acquire));
            panic!("boom mid-session");
        });
        assert!(panicked.is_err(), "the closure panicked");
        assert!(
            !STREAM_ACTIVE.load(Ordering::Acquire),
            "STREAM_ACTIVE cleared despite the panic"
        );
    }
}
