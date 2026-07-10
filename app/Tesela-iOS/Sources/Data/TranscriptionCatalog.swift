import Foundation

/// A transcription model the user can download to their device. Two
/// families today: Whisper (via ggerganov/whisper.cpp) and Parakeet
/// (NVIDIA NeMo). Each entry includes the download URL and an
/// approximate on-disk size for the manage-models UI.
struct TranscriptionModel: Identifiable, Codable, Hashable {
    let id: String
    let family: ModelFamily
    let displayName: String
    let shortDescription: String
    let sizeBytes: Int64
    /// HuggingFace `.bin` URL for Whisper models (downloaded by
    /// `TranscriptionStore`). `nil` for Parakeet — FluidAudio downloads
    /// and caches those itself; see `parakeetVersion`.
    let downloadURL: URL?
    /// Suggested use-cases shown as small chips in the list.
    let suggestedFor: [String]
    /// True if the model is intended to run on-device (vs. server-only).
    let onDevice: Bool
    /// True if Tesela can actually run inference for this model.
    let inferenceSupported: Bool
    /// For Parakeet models, the FluidAudio `AsrModels.Version` token
    /// (`v2` / `v3` / `tdtCtc110m`). `nil` for Whisper.
    var parakeetVersion: String? = nil
}

enum ModelFamily: String, Codable, Hashable {
    case whisper
    case parakeet
    /// Parakeet Unified 0.6B via FluidAudio's `StreamingUnifiedAsrManager`
    /// — the only family that streams live partials (see `TranscriptionEngine`
    /// protocol's streaming methods and `ParakeetUnifiedTier`). A distinct
    /// case from `.parakeet` because it has its own download shape (a
    /// per-tier encoder, not a single fixed model set) and its own engine.
    case parakeetUnified

    var displayName: String {
        switch self {
        case .whisper:         return "Whisper"
        case .parakeet:        return "Parakeet"
        case .parakeetUnified: return "Parakeet Unified"
        }
    }
}

/// The Parakeet Unified streaming latency tiers FluidAudio ships as
/// distinct CoreML encoder downloads — the `[left, chunk, right]` attention
/// context is baked into the encoder at conversion time (FluidAudio
/// `UnifiedConfig`/`StreamingModelVariant.parakeetUnified*`, v0.15.5), so
/// switching tiers means downloading a different ~565 MB encoder rather
/// than flipping a runtime parameter. Raw values match FluidAudio's own
/// `StreamingModelVariant` display convention and are what's persisted at
/// `voice.streamingTier`.
enum ParakeetUnifiedTier: String, CaseIterable, Identifiable, Codable {
    case tier320ms = "320ms"
    case tier640ms = "640ms"
    case tier1120ms = "1120ms"
    case tier2080ms = "2080ms"

    var id: String { rawValue }

    /// Shipped default pending a real-device memory profile (tesela-v5t.3
    /// re-spec note 2): 640ms int8 is the target for live-partial latency,
    /// but its peak RSS on older/lower-RAM devices hasn't been measured
    /// yet. If that profile shows it's too tight, flip this to
    /// `.tier1120ms` (the documented fallback) rather than re-deriving the
    /// choice from scratch.
    static let `default` = ParakeetUnifiedTier.tier640ms

    var displayName: String {
        switch self {
        case .tier320ms:  return "320ms · lowest latency"
        case .tier640ms:  return "640ms · balanced (default)"
        case .tier1120ms: return "1.12s · better accuracy"
        case .tier2080ms: return "2.08s · best accuracy"
        }
    }

    /// `[left, chunk, right]` encoder frames (80 ms each) — mirrors
    /// FluidAudio's `StreamingModelVariant.unifiedConfig` exactly
    /// (ParakeetModelVariant.swift, FluidAudio v0.15.5).
    var contextFrames: (left: Int, chunk: Int, right: Int) {
        switch self {
        case .tier320ms:  return (70, 2, 2)
        case .tier640ms:  return (70, 7, 1)
        case .tier1120ms: return (70, 7, 7)
        case .tier2080ms: return (70, 13, 13)
        }
    }

    /// FluidAudio's encoder filename suffix for this tier (e.g. `"70_7_1"`)
    /// — keys both the on-disk cache lookup and the `ModelHub.download`
    /// request (`ModelNames.ParakeetUnified.streamingEncoderFile`).
    var contextSuffix: String {
        "\(contextFrames.left)_\(contextFrames.chunk)_\(contextFrames.right)"
    }

    /// The active tier from `voice.streamingTier` (written by
    /// `VoiceSettingsView`'s picker), falling back to `.default` when
    /// unset or holding a stale/invalid raw value.
    static var active: ParakeetUnifiedTier {
        guard let raw = UserDefaults.standard.string(forKey: "voice.streamingTier"),
              let tier = ParakeetUnifiedTier(rawValue: raw)
        else { return .default }
        return tier
    }
}

/// Curated, hardcoded catalog. Could move to a remote JSON file later
/// so it can be updated without an app release.
enum TranscriptionCatalog {
    static let all: [TranscriptionModel] = [
        // ── Whisper.cpp GGML models ──────────────────────────────────
        TranscriptionModel(
            id: "whisper-tiny",
            family: .whisper,
            displayName: "Whisper · tiny",
            shortDescription: "Smallest, fastest. Acceptable for short, clear speech.",
            sizeBytes: 39 * 1024 * 1024,
            downloadURL: URL(string: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin")!,
            suggestedFor: ["fast capture"],
            onDevice: true,
            inferenceSupported: true
        ),
        TranscriptionModel(
            id: "whisper-base",
            family: .whisper,
            displayName: "Whisper · base",
            shortDescription: "Balanced. Good default for everyday voice notes.",
            sizeBytes: 142 * 1024 * 1024,
            downloadURL: URL(string: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin")!,
            suggestedFor: ["default"],
            onDevice: true,
            inferenceSupported: true
        ),
        TranscriptionModel(
            id: "whisper-small",
            family: .whisper,
            displayName: "Whisper · small",
            shortDescription: "Noticeably better accuracy. Slower on iPhone.",
            sizeBytes: 466 * 1024 * 1024,
            downloadURL: URL(string: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin")!,
            suggestedFor: ["accuracy"],
            onDevice: true,
            inferenceSupported: true
        ),
        TranscriptionModel(
            id: "whisper-medium",
            family: .whisper,
            displayName: "Whisper · medium",
            shortDescription: "Strong accuracy. Heavy for on-device.",
            sizeBytes: 1_500_000_000,
            downloadURL: URL(string: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin")!,
            suggestedFor: ["accuracy"],
            onDevice: true,
            inferenceSupported: true
        ),
        TranscriptionModel(
            id: "whisper-large-v3-turbo",
            family: .whisper,
            displayName: "Whisper · large v3 turbo",
            shortDescription: "Apple's recommended large variant. Fast for its size.",
            sizeBytes: 1_700_000_000,
            downloadURL: URL(string: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin")!,
            suggestedFor: ["best on-device"],
            onDevice: true,
            inferenceSupported: true
        ),
        TranscriptionModel(
            id: "whisper-large-v3",
            family: .whisper,
            displayName: "Whisper · large v3",
            shortDescription: "Best accuracy. Slow on phones; great on Mac.",
            sizeBytes: 3_100_000_000,
            downloadURL: URL(string: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin")!,
            suggestedFor: ["best accuracy"],
            onDevice: true,
            inferenceSupported: true
        ),

        // ── Parakeet (NVIDIA, on-device via the FluidAudio package) ──
        // FluidAudio downloads and caches the Parakeet CoreML model set
        // itself (`AsrModels.downloadAndLoad`), so these entries carry
        // no `downloadURL` — `parakeetVersion` is the FluidAudio
        // `AsrModels.Version` token instead. `sizeBytes` is a UI
        // estimate only.
        TranscriptionModel(
            id: "parakeet-tdt-0.6b-v2",
            family: .parakeet,
            displayName: "Parakeet · TDT 0.6B (v2)",
            shortDescription: "English-only. Highest English accuracy.",
            sizeBytes: 450_000_000,
            downloadURL: nil,
            suggestedFor: ["english", "low latency"],
            onDevice: true,
            inferenceSupported: true,
            parakeetVersion: "v2"
        ),
        TranscriptionModel(
            id: "parakeet-tdt-0.6b-v3",
            family: .parakeet,
            displayName: "Parakeet · TDT 0.6B (v3)",
            shortDescription: "Multilingual — 25 European languages.",
            sizeBytes: 450_000_000,
            downloadURL: nil,
            suggestedFor: ["multilingual", "streaming"],
            onDevice: true,
            inferenceSupported: true,
            parakeetVersion: "v3"
        ),
        TranscriptionModel(
            id: "parakeet-tdt-ctc-110m",
            family: .parakeet,
            displayName: "Parakeet · TDT-CTC 110M",
            shortDescription: "Smaller, faster hybrid. Lowest latency on iPhone.",
            sizeBytes: 250_000_000,
            downloadURL: nil,
            suggestedFor: ["fast", "low latency"],
            onDevice: true,
            inferenceSupported: true,
            parakeetVersion: "tdtCtc110m"
        ),

        // ── Parakeet Unified (streaming, on-device via FluidAudio's
        //    StreamingUnifiedAsrManager) ─────────────────────────────
        // ONE catalog entry: which of the 4 latency-tier encoders is
        // actually downloaded/active is a runtime choice
        // (`ParakeetUnifiedTier.active`, set from Voice settings), not a
        // separate catalog row per tier. `sizeBytes` is the download for
        // ONE tier (encoder + shared decoder/joint/vocab) — verified via
        // HEAD requests against the HF repo 2026-07-09: int8 streaming
        // encoder weight ≈590 MB (consistent across all 4 tiers) + shared
        // decoder/jointDecision/vocab/metadata ≈18 MB ≈ 608 MB total.
        // Switching tiers after the first download only adds the new
        // tier's ~590 MB encoder — decoder/joint/vocab are shared.
        TranscriptionModel(
            id: "parakeet-unified-en-0.6b",
            family: .parakeetUnified,
            displayName: "Parakeet Unified 0.6B (streaming)",
            shortDescription: "Live partials as you speak. English only.",
            sizeBytes: 608_000_000,
            downloadURL: nil,
            suggestedFor: ["live dictation", "streaming", "english"],
            onDevice: true,
            inferenceSupported: true
        ),
    ]

    static func find(_ id: String) -> TranscriptionModel? {
        all.first(where: { $0.id == id })
    }

    /// Group by family for the Settings UI.
    static var grouped: [(family: ModelFamily, models: [TranscriptionModel])] {
        let byFamily = Dictionary(grouping: all, by: \.family)
        let order: [ModelFamily] = [.whisper, .parakeet, .parakeetUnified]
        return order.compactMap { f in
            guard let m = byFamily[f] else { return nil }
            return (family: f, models: m)
        }
    }
}

/// Human-readable size string (MB or GB).
extension Int64 {
    var humanReadableModelSize: String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useMB, .useGB]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: self)
    }
}
