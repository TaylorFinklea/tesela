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

    var displayName: String {
        switch self {
        case .whisper:  return "Whisper"
        case .parakeet: return "Parakeet"
        }
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
    ]

    static func find(_ id: String) -> TranscriptionModel? {
        all.first(where: { $0.id == id })
    }

    /// Group by family for the Settings UI.
    static var grouped: [(family: ModelFamily, models: [TranscriptionModel])] {
        let byFamily = Dictionary(grouping: all, by: \.family)
        let order: [ModelFamily] = [.whisper, .parakeet]
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
