import Foundation
import AVFoundation
import SwiftWhisper
import FluidAudio

/// Abstraction over the two transcription paths Tesela ships:
///   • `LocalTranscriptionEngine` — runs whisper.cpp on-device via
///     SwiftWhisper. Works offline; requires the active model file
///     to be downloaded.
///   • `ServerTranscriptionEngine` — uploads the audio to the
///     tesela-server's /transcription/transcribe endpoint.
///
/// `CaptureSheet` reads the active engine choice from
/// `@AppStorage("voice.useOnDevice")` and picks the right impl.
protocol TranscriptionEngine: AnyObject {
    /// Transcribe a WAV file at `url`. Returns the joined transcript.
    func transcribe(audio url: URL) async throws -> String

    /// Transcribe an in-memory PCM sample buffer (16 kHz mono floats).
    /// Used by the streaming path which can avoid the WAV round-trip.
    func transcribe(samples: [Float]) async throws -> String

    /// Human-readable label for the active engine — shown in the
    /// composer's helper row.
    var displayLabel: String { get }

    /// True when this engine can stream partial transcripts as audio
    /// arrives (today: only `LocalTranscriptionEngine` on the Parakeet
    /// Unified model). Non-streaming engines fall back to the whole-clip
    /// `transcribe(samples:)` path — `StreamingVoiceRecorder` checks this
    /// before attempting a streaming session and never calls the
    /// streaming methods below when it's false.
    var supportsStreaming: Bool { get }

    /// Begin a streaming session. `onPartial` fires on the MAIN ACTOR with
    /// the full COMMITTED transcript so far, every time it advances.
    /// FluidAudio's unified streaming manager emits one monotonic
    /// committed string — never a tentative tail — so there is no
    /// separate "tentative" callback to wire (unlike the web dictation
    /// popover's committed+tentative split; see tesela-v5t.3 notes).
    func startStreaming(onPartial: @escaping @MainActor (String) -> Void) async throws

    /// Feed a chunk of 16 kHz mono float32 samples to the active
    /// streaming session.
    func appendStreaming(samples: [Float]) async throws

    /// Flush remaining audio and return the final transcript, ending the
    /// streaming session.
    func finishStreaming() async throws -> String

    /// Abort the streaming session without producing a final transcript
    /// — e.g. the recording was cancelled, or an AVAudioSession
    /// interruption ended it early.
    func cancelStreaming() async
}

/// Default (non-streaming) behavior — `ServerTranscriptionEngine` and the
/// whisper.cpp path through `LocalTranscriptionEngine` don't override any
/// of this; they rely entirely on `transcribe(samples:)`.
extension TranscriptionEngine {
    var supportsStreaming: Bool { false }

    func startStreaming(onPartial: @escaping @MainActor (String) -> Void) async throws {
        throw TranscriptionEngineError.streamingNotSupported
    }

    func appendStreaming(samples: [Float]) async throws {}

    func finishStreaming() async throws -> String { "" }

    func cancelStreaming() async {}
}

enum TranscriptionEngineError: Error, LocalizedError {
    case streamingNotSupported

    var errorDescription: String? {
        switch self {
        case .streamingNotSupported:
            return "This engine doesn't support live streaming."
        }
    }
}

/// Map a catalog `parakeetVersion` token to FluidAudio's model version.
func fluidAudioVersion(_ token: String?) -> AsrModelVersion? {
    switch token {
    case "v2":         return .v2
    case "v3":         return .v3
    case "tdtCtc110m": return .tdtCtc110m
    default:           return nil
    }
}

// MARK: - Local engine (whisper.cpp via SwiftWhisper)

@MainActor
final class LocalTranscriptionEngine: TranscriptionEngine {
    /// Resolves the active model id from `TranscriptionStore` and
    /// the file URL it maps to.
    private let store: TranscriptionStore
    /// Cached SwiftWhisper context, keyed by the model id it was
    /// initialized with. `static` so it survives across engine
    /// instances — `CaptureBar` rebuilds a fresh `LocalTranscriptionEngine`
    /// for every recording, and re-loading hundreds of MB of model on
    /// each one is what made voice capture feel broken. Safe as shared
    /// mutable state: the class is `@MainActor`, so every access is
    /// serialized on the main actor.
    private static var cached: (id: String, whisper: Whisper)? = nil
    /// Cached FluidAudio manager for the active Parakeet model — also
    /// `static`, same reasoning.
    private static var cachedParakeet: (id: String, manager: AsrManager)? = nil
    /// Cached FluidAudio unified-streaming manager, keyed by
    /// `"<modelId>-<tier>"` so switching the latency tier mid-app-lifetime
    /// rebuilds it (each tier is a distinct encoder — `ParakeetUnifiedTier`).
    /// `static`, same reasoning as `cachedParakeet`.
    private static var cachedUnified: (key: String, manager: StreamingUnifiedAsrManager)? = nil

    init(store: TranscriptionStore) {
        self.store = store
    }

    var displayLabel: String {
        let modelId = store.activeModelId
        if modelId.isEmpty {
            return "on-device · no model"
        }
        let name = TranscriptionCatalog.find(modelId)?.displayName ?? modelId
        return "on-device · \(name)"
    }

    func transcribe(audio url: URL) async throws -> String {
        let samples = try Self.readWavSamples(at: url)
        return try await transcribe(samples: samples)
    }

    func transcribe(samples: [Float]) async throws -> String {
        guard !samples.isEmpty else { return "" }
        let id = store.activeModelId
        let family = TranscriptionCatalog.find(id)?.family
        voiceDiag("engine: \(samples.count) samples, model=\(id)")
        // Parakeet models run through FluidAudio; everything else is
        // whisper.cpp. Both consume the same 16 kHz mono float samples.
        if family == .parakeet {
            let manager = try await loadParakeet()
            // Size the decoder state to the *loaded* model — v2/v3 use 2
            // LSTM layers, tdtCtc110m uses 1. The bare `TdtDecoderState()`
            // hardcodes 2, which throws a CoreML "MultiArray shape"
            // mismatch when the model disagrees. This mirrors FluidAudio's
            // own callers (`SlidingWindowAsrManager`, `ChunkProcessor`),
            // which always build the state from `manager.decoderLayerCount`.
            let decoderLayers = await manager.decoderLayerCount
            var decoderState = TdtDecoderState.make(decoderLayers: decoderLayers)
            voiceDiag("Parakeet inference begin (decoderLayers=\(decoderLayers))")
            let result = try await manager.transcribe(samples, decoderState: &decoderState)
            voiceDiag("Parakeet inference done — \(result.text.count) chars")
            return result.text.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        if family == .parakeetUnified {
            // Whole-clip fallback for the streaming model: drive the same
            // `StreamingUnifiedAsrManager` as a one-shot append + finish
            // rather than adding a second (offline `UnifiedAsrManager`)
            // model type just for this path.
            let manager = try await loadUnifiedStreaming()
            try await manager.reset()
            guard let buffer = Self.makeStreamingBuffer(from: samples) else { return "" }
            try await manager.appendAudio(buffer)
            let text = try await manager.finish()
            voiceDiag("Parakeet Unified whole-clip inference done — \(text.count) chars")
            return text.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        let whisper = try await loadWhisper()
        let segments = try await whisper.transcribe(audioFrames: samples)
        return segments
            .map { $0.text }
            .joined()
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    // MARK: - Streaming (Parakeet Unified)

    var supportsStreaming: Bool {
        TranscriptionCatalog.find(store.activeModelId)?.family == .parakeetUnified
    }

    func startStreaming(onPartial: @escaping @MainActor (String) -> Void) async throws {
        let manager = try await loadUnifiedStreaming()
        try await manager.reset()
        await manager.setPartialTranscriptCallback { text in
            Task { @MainActor in onPartial(text) }
        }
    }

    func appendStreaming(samples: [Float]) async throws {
        guard let manager = Self.cachedUnified?.manager,
              let buffer = Self.makeStreamingBuffer(from: samples)
        else { return }
        try await manager.appendAudio(buffer)
        try await manager.processBufferedAudio()
    }

    func finishStreaming() async throws -> String {
        guard let manager = Self.cachedUnified?.manager else { return "" }
        return try await manager.finish()
    }

    func cancelStreaming() async {
        guard let manager = Self.cachedUnified?.manager else { return }
        try? await manager.reset()
    }

    /// Resolve (downloading if needed) and cache the FluidAudio
    /// `StreamingUnifiedAsrManager` for the active tier
    /// (`ParakeetUnifiedTier.active`). Idempotent once the tier's encoder
    /// is cached on disk, same shape as `loadParakeet()`.
    private func loadUnifiedStreaming() async throws -> StreamingUnifiedAsrManager {
        let id = store.activeModelId
        let tier = ParakeetUnifiedTier.active
        let key = "\(id)-\(tier.rawValue)"
        if let cached = Self.cachedUnified, cached.key == key {
            return cached.manager
        }
        let frames = tier.contextFrames
        let config = UnifiedConfig(leftFrames: frames.left, chunkFrames: frames.chunk, rightFrames: frames.right)
        let manager = StreamingUnifiedAsrManager(config: config, encoderPrecision: .int8)
        voiceDiag("loadUnifiedStreaming: downloading/loading tier \(tier.rawValue)")
        try await manager.loadModels(to: TranscriptionStore.parakeetUnifiedCacheURL())
        voiceDiag("loadUnifiedStreaming: ready")
        Self.cachedUnified = (key, manager)
        return manager
    }

    /// Wrap 16 kHz mono float32 samples in an `AVAudioPCMBuffer` —
    /// `StreamingUnifiedAsrManager.appendAudio` takes a buffer, not a raw
    /// array (its internal `AudioConverter` fast-paths a buffer already in
    /// this format, so no resampling happens).
    private static func makeStreamingBuffer(from samples: [Float]) -> AVAudioPCMBuffer? {
        guard !samples.isEmpty else { return nil }
        guard let format = AVAudioFormat(
            commonFormat: .pcmFormatFloat32, sampleRate: 16_000, channels: 1, interleaved: false
        ), let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(samples.count)) else {
            return nil
        }
        buffer.frameLength = AVAudioFrameCount(samples.count)
        guard let ptr = buffer.floatChannelData?.pointee else { return nil }
        samples.withUnsafeBufferPointer { src in
            guard let base = src.baseAddress else { return }
            ptr.update(from: base, count: samples.count)
        }
        return buffer
    }

    // MARK: - Helpers

    private func loadWhisper() async throws -> Whisper {
        let id = store.activeModelId
        guard !id.isEmpty else {
            throw NSError(
                domain: "Tesela.Transcription",
                code: -1,
                userInfo: [NSLocalizedDescriptionKey: "No active model picked. Open Settings → Voice → Manage models."]
            )
        }
        if let cached = Self.cached, cached.id == id {
            return cached.whisper
        }
        let url = store.localURL(for: id)
        guard FileManager.default.fileExists(atPath: url.path) else {
            throw NSError(
                domain: "Tesela.Transcription",
                code: -1,
                userInfo: [NSLocalizedDescriptionKey: "Model \(id) isn't downloaded yet."]
            )
        }
        // SwiftWhisper.Whisper(fromFileURL:) — synchronous, model load.
        // Offload to a background queue to avoid blocking the main
        // actor while several hundred MB of GGML are mmapped.
        let whisper: Whisper = try await Task.detached(priority: .userInitiated) {
            try Whisper(fromFileURL: url)
        }.value
        Self.cached = (id, whisper)
        return whisper
    }

    /// Resolve (downloading if needed) and cache the FluidAudio
    /// `AsrManager` for the active Parakeet model. `downloadAndLoad`
    /// is idempotent — instant once the model set is cached on disk.
    private func loadParakeet() async throws -> AsrManager {
        let id = store.activeModelId
        guard let model = TranscriptionCatalog.find(id),
              let version = fluidAudioVersion(model.parakeetVersion)
        else {
            throw NSError(
                domain: "Tesela.Transcription",
                code: -1,
                userInfo: [NSLocalizedDescriptionKey: "No active Parakeet model. Open Settings → Voice → Manage models."]
            )
        }
        if let cachedParakeet = Self.cachedParakeet, cachedParakeet.id == id {
            return cachedParakeet.manager
        }
        voiceDiag("loadParakeet: downloadAndLoad begin")
        let models = try await AsrModels.downloadAndLoad(
            to: TranscriptionStore.parakeetCacheURL(versionToken: model.parakeetVersion ?? ""),
            version: version
        )
        voiceDiag("loadParakeet: downloadAndLoad done, loading models")
        let manager = AsrManager(config: .default)
        try await manager.loadModels(models)
        voiceDiag("loadParakeet: AsrManager ready")
        Self.cachedParakeet = (id, manager)
        return manager
    }

    /// Read a 16-bit PCM WAV at `url` and return 16 kHz mono floats.
    /// Uses AVAudioFile so any sample rate / channel layout AVFoundation
    /// can decode works (M4A, AAC, etc. — though VoiceRecorder always
    /// writes 16k mono WAV).
    private static func readWavSamples(at url: URL) throws -> [Float] {
        let file = try AVAudioFile(forReading: url)
        guard let pcmFormat = AVAudioFormat(
            commonFormat: .pcmFormatFloat32,
            sampleRate: 16_000,
            channels: 1,
            interleaved: false
        ) else {
            throw NSError(
                domain: "Tesela.Transcription",
                code: -2,
                userInfo: [NSLocalizedDescriptionKey: "Couldn't create target PCM format"]
            )
        }
        // Convert to 16 kHz mono float32 via AVAudioConverter.
        guard let converter = AVAudioConverter(from: file.processingFormat, to: pcmFormat) else {
            throw NSError(
                domain: "Tesela.Transcription",
                code: -3,
                userInfo: [NSLocalizedDescriptionKey: "Couldn't build audio converter"]
            )
        }
        let frameCount = AVAudioFrameCount(file.length)
        guard let inputBuffer = AVAudioPCMBuffer(pcmFormat: file.processingFormat, frameCapacity: frameCount) else {
            throw NSError(
                domain: "Tesela.Transcription",
                code: -4,
                userInfo: [NSLocalizedDescriptionKey: "Couldn't allocate input buffer"]
            )
        }
        try file.read(into: inputBuffer)
        // Estimate output capacity: ratio of sample rates.
        let ratio = pcmFormat.sampleRate / file.processingFormat.sampleRate
        let outFrames = AVAudioFrameCount(Double(frameCount) * ratio) + 1024
        guard let outputBuffer = AVAudioPCMBuffer(pcmFormat: pcmFormat, frameCapacity: outFrames) else {
            throw NSError(
                domain: "Tesela.Transcription",
                code: -5,
                userInfo: [NSLocalizedDescriptionKey: "Couldn't allocate output buffer"]
            )
        }
        var error: NSError?
        var supplied = false
        converter.convert(to: outputBuffer, error: &error) { _, statusOut in
            if supplied {
                statusOut.pointee = .endOfStream
                return nil
            }
            supplied = true
            statusOut.pointee = .haveData
            return inputBuffer
        }
        if let error { throw error }
        guard let ptr = outputBuffer.floatChannelData?.pointee else {
            throw NSError(
                domain: "Tesela.Transcription",
                code: -6,
                userInfo: [NSLocalizedDescriptionKey: "Output buffer has no float channel data"]
            )
        }
        let n = Int(outputBuffer.frameLength)
        return Array(UnsafeBufferPointer(start: ptr, count: n))
    }
}

// MARK: - Server engine (HTTP upload)

@MainActor
final class ServerTranscriptionEngine: TranscriptionEngine {
    private let mosaic: MockMosaicService
    private let backendGeneration: UInt64

    init(mosaic: MockMosaicService) {
        self.mosaic = mosaic
        backendGeneration = mosaic.backendGenerationLease
    }

    var displayLabel: String {
        "server · /transcription/transcribe"
    }

    func transcribe(audio url: URL) async throws -> String {
        try await mosaic.transcribe(audio: url, expectedGeneration: backendGeneration)
    }

    func transcribe(samples: [Float]) async throws -> String {
        // Server path expects a WAV file. Encode the buffer to a
        // temp WAV and reuse the upload path.
        let url = try Self.writeWav(samples: samples)
        defer { try? FileManager.default.removeItem(at: url) }
        return try await mosaic.transcribe(audio: url, expectedGeneration: backendGeneration)
    }

    private static func writeWav(samples: [Float]) throws -> URL {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("voice-stream", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let url = dir.appendingPathComponent("chunk-\(UUID().uuidString.prefix(8)).wav")
        let sampleRate: Double = 16_000
        let bytesPerSample = 2
        let dataSize = UInt32(samples.count * bytesPerSample)
        var header = Data()
        header.append("RIFF".data(using: .ascii)!)
        header.append(UInt32(36 + dataSize).littleEndianBytes)
        header.append("WAVEfmt ".data(using: .ascii)!)
        header.append(UInt32(16).littleEndianBytes)
        header.append(UInt16(1).littleEndianBytes) // PCM
        header.append(UInt16(1).littleEndianBytes) // mono
        header.append(UInt32(sampleRate).littleEndianBytes)
        header.append(UInt32(sampleRate * Double(bytesPerSample)).littleEndianBytes)
        header.append(UInt16(bytesPerSample).littleEndianBytes)
        header.append(UInt16(16).littleEndianBytes) // bits/sample
        header.append("data".data(using: .ascii)!)
        header.append(dataSize.littleEndianBytes)
        var body = Data(capacity: Int(dataSize))
        for f in samples {
            let s = Int16(max(-1, min(1, f)) * Float(Int16.max))
            body.append(s.littleEndianBytes)
        }
        try (header + body).write(to: url)
        return url
    }
}

private extension UInt32 {
    var littleEndianBytes: Data {
        var v = littleEndian
        return Data(bytes: &v, count: MemoryLayout<UInt32>.size)
    }
}
private extension UInt16 {
    var littleEndianBytes: Data {
        var v = littleEndian
        return Data(bytes: &v, count: MemoryLayout<UInt16>.size)
    }
}
private extension Int16 {
    var littleEndianBytes: Data {
        var v = littleEndian
        return Data(bytes: &v, count: MemoryLayout<Int16>.size)
    }
}
