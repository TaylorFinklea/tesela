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
    let downloadURL: URL
    /// Suggested use-cases shown as small chips in the list.
    let suggestedFor: [String]
    /// True if the model is intended to run on-device (vs. server-only).
    let onDevice: Bool
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
            onDevice: true
        ),
        TranscriptionModel(
            id: "whisper-base",
            family: .whisper,
            displayName: "Whisper · base",
            shortDescription: "Balanced. Good default for everyday voice notes.",
            sizeBytes: 142 * 1024 * 1024,
            downloadURL: URL(string: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin")!,
            suggestedFor: ["default"],
            onDevice: true
        ),
        TranscriptionModel(
            id: "whisper-small",
            family: .whisper,
            displayName: "Whisper · small",
            shortDescription: "Noticeably better accuracy. Slower on iPhone.",
            sizeBytes: 466 * 1024 * 1024,
            downloadURL: URL(string: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin")!,
            suggestedFor: ["accuracy"],
            onDevice: true
        ),
        TranscriptionModel(
            id: "whisper-medium",
            family: .whisper,
            displayName: "Whisper · medium",
            shortDescription: "Strong accuracy. Heavy for on-device.",
            sizeBytes: 1_500_000_000,
            downloadURL: URL(string: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin")!,
            suggestedFor: ["accuracy"],
            onDevice: true
        ),
        TranscriptionModel(
            id: "whisper-large-v3-turbo",
            family: .whisper,
            displayName: "Whisper · large v3 turbo",
            shortDescription: "Apple's recommended large variant. Fast for its size.",
            sizeBytes: 1_700_000_000,
            downloadURL: URL(string: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin")!,
            suggestedFor: ["best on-device"],
            onDevice: true
        ),
        TranscriptionModel(
            id: "whisper-large-v3",
            family: .whisper,
            displayName: "Whisper · large v3",
            shortDescription: "Best accuracy. Slow on phones; great on Mac.",
            sizeBytes: 3_100_000_000,
            downloadURL: URL(string: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin")!,
            suggestedFor: ["best accuracy"],
            onDevice: true
        ),

        // ── Parakeet (NVIDIA NeMo) ──────────────────────────────────
        // Parakeet ships as a .nemo bundle. On iOS, integration goes
        // through CoreML conversion; for now the entry exists so the
        // manage-models UI can show it. Actual model loading lands
        // with the voice-capture integration.
        TranscriptionModel(
            id: "parakeet-tdt-0.6b",
            family: .parakeet,
            displayName: "Parakeet · TDT 0.6B",
            shortDescription: "NVIDIA NeMo · fast streaming transcription.",
            sizeBytes: 620_000_000,
            downloadURL: URL(string: "https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2/resolve/main/parakeet-tdt-0.6b-v2.nemo")!,
            suggestedFor: ["streaming", "low latency"],
            onDevice: true
        ),
        TranscriptionModel(
            id: "parakeet-tdt-1.1b",
            family: .parakeet,
            displayName: "Parakeet · TDT 1.1B",
            shortDescription: "Larger Parakeet. Higher accuracy than 0.6B.",
            sizeBytes: 1_100_000_000,
            downloadURL: URL(string: "https://huggingface.co/nvidia/parakeet-tdt-1.1b/resolve/main/parakeet-tdt-1.1b.nemo")!,
            suggestedFor: ["accuracy"],
            onDevice: true
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
