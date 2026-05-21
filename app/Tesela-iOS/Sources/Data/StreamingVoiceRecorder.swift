import Foundation
import AVFoundation
import Combine
import os

/// Shared logger for the on-device voice + transcription path.
let voiceLog = Logger(subsystem: "app.tesela.ios", category: "voice")

/// Mirrors the most recent voice-path diagnostic into the UI so a
/// failure or stall is visible in the capture bar without a log tool.
@MainActor
final class VoiceDiagnostics: ObservableObject {
    static let shared = VoiceDiagnostics()
    @Published var lastLine: String = ""
    private init() {}
}

/// Emit a voice-path diagnostic: unified log + the on-screen mirror.
@MainActor
func voiceDiag(_ message: String) {
    voiceLog.notice("\(message, privacy: .public)")
    VoiceDiagnostics.shared.lastLine = message
}

/// Voice capture coordinator. Uses `AVAudioEngine` to tap the mic
/// input, accumulate the whole utterance as 16 kHz mono float
/// samples, and — on `stop()` — transcribe it in a single pass.
///
/// Transcribe-on-stop rather than live 5-second chunks: FluidAudio's
/// Parakeet `transcribe` rejects clips shorter than 300 ms and chunks
/// long audio internally, so hand-rolled windowing only ever produced
/// short tail chunks it refused. Owners pass an engine + callbacks;
/// `onChunk` fires once, on the MainActor, with the final transcript.
@MainActor
final class StreamingVoiceRecorder: ObservableObject {
    enum State: Equatable {
        case idle
        case requestingPermission
        case denied
        case recording(elapsed: TimeInterval)
        case failed(String)
    }

    @Published private(set) var state: State = .idle
    @Published private(set) var transcribingChunk: Bool = false

    var onChunk: ((String) -> Void)? = nil
    var onError: ((String) -> Void)? = nil

    private let engine = AVAudioEngine()
    private var converter: AVAudioConverter?
    private var startedAt: Date?
    private var elapsedTimer: Timer?

    /// The whole recording, accumulated as 16 kHz mono float samples
    /// and transcribed in one pass on `stop()`.
    private var pendingSamples: [Float] = []
    private var transcriber: TranscriptionEngine?
    /// Guards against a second transcription starting while one runs.
    private var isTranscribing = false

    /// FluidAudio's Parakeet `transcribe` rejects anything shorter than
    /// 300 ms — 4800 samples at 16 kHz. A clip that short is noise
    /// anyway, so it's skipped rather than sent (and rejected).
    private let minimumSamples = 4_800

    /// Whisper expects 16 kHz mono float32 samples.
    private let targetFormat: AVAudioFormat = {
        AVAudioFormat(
            commonFormat: .pcmFormatFloat32,
            sampleRate: 16_000,
            channels: 1,
            interleaved: false
        )!
    }()

    // MARK: - Public API

    @discardableResult
    func start(using transcriber: TranscriptionEngine) async -> Bool {
        voiceDiag("start requested")
        guard await ensurePermission() else {
            voiceDiag("start: microphone permission DENIED")
            state = .denied
            return false
        }
        do {
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.playAndRecord, mode: .measurement, options: [.defaultToSpeaker])
            try session.setActive(true, options: [.notifyOthersOnDeactivation])

            self.transcriber = transcriber
            pendingSamples.removeAll(keepingCapacity: true)

            // Set up the input tap. We convert each buffer into 16 kHz
            // mono float32 and append to `pendingSamples`.
            let input = engine.inputNode
            let inputFormat = input.outputFormat(forBus: 0)
            converter = AVAudioConverter(from: inputFormat, to: targetFormat)
            guard converter != nil else {
                state = .failed("Audio format converter unavailable")
                return false
            }
            input.installTap(onBus: 0, bufferSize: 4_096, format: inputFormat) { [weak self] buffer, _ in
                self?.handleInputBuffer(buffer)
            }
            engine.prepare()
            try engine.start()

            startedAt = Date()
            state = .recording(elapsed: 0)
            startTimers()
            voiceDiag("recording started")
            return true
        } catch {
            voiceDiag("start FAILED: \(error.localizedDescription)")
            state = .failed(error.localizedDescription)
            return false
        }
    }

    /// Stops recording immediately and kicks off transcription of the
    /// whole recording in the background — stopping must never wait on
    /// transcription, which for Parakeet includes a CoreML model load
    /// that can take tens of seconds.
    func stop() {
        guard case .recording = state else {
            voiceDiag("stop ignored — not recording")
            return
        }
        voiceDiag("stopped — transcribing recording")
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        try? AVAudioSession.sharedInstance().setActive(false, options: [.notifyOthersOnDeactivation])
        stopTimers()
        state = .idle
        Task { @MainActor [weak self] in await self?.transcribeAll() }
    }

    func cancel() {
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        try? AVAudioSession.sharedInstance().setActive(false, options: [.notifyOthersOnDeactivation])
        stopTimers()
        pendingSamples.removeAll()
        state = .idle
    }

    // MARK: - Audio buffer plumbing

    private func handleInputBuffer(_ buffer: AVAudioPCMBuffer) {
        guard let converter else { return }
        // Estimate output capacity from sample-rate ratio.
        let ratio = targetFormat.sampleRate / buffer.format.sampleRate
        let outFrames = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 256
        guard let outBuffer = AVAudioPCMBuffer(pcmFormat: targetFormat, frameCapacity: outFrames) else {
            return
        }
        var error: NSError?
        var supplied = false
        converter.convert(to: outBuffer, error: &error) { _, status in
            if supplied {
                status.pointee = .endOfStream
                return nil
            }
            supplied = true
            status.pointee = .haveData
            return buffer
        }
        guard error == nil,
              let ptr = outBuffer.floatChannelData?.pointee
        else { return }
        let n = Int(outBuffer.frameLength)
        let chunk = Array(UnsafeBufferPointer(start: ptr, count: n))
        Task { @MainActor in
            self.pendingSamples.append(contentsOf: chunk)
        }
    }

    // MARK: - Permission

    private func ensurePermission() async -> Bool {
        let session = AVAudioSession.sharedInstance()
        switch session.recordPermission {
        case .granted: return true
        case .denied:  return false
        case .undetermined:
            state = .requestingPermission
            return await withCheckedContinuation { cont in
                session.requestRecordPermission { granted in
                    cont.resume(returning: granted)
                }
            }
        @unknown default: return false
        }
    }

    // MARK: - Timers

    private func startTimers() {
        elapsedTimer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            Task { @MainActor [weak self] in self?.tickElapsed() }
        }
    }

    private func stopTimers() {
        elapsedTimer?.invalidate()
        elapsedTimer = nil
    }

    private func tickElapsed() {
        guard case .recording = state, let startedAt else { return }
        state = .recording(elapsed: Date().timeIntervalSince(startedAt))
    }

    /// Transcribe the entire recording in a single pass. FluidAudio's
    /// Parakeet model chunks long audio internally, so there is no need
    /// to pre-window — and pre-windowing only produced sub-300 ms tail
    /// chunks it rejected with "Invalid audio data".
    private func transcribeAll() async {
        guard !isTranscribing, let transcriber else { return }
        let samples = pendingSamples
        pendingSamples.removeAll(keepingCapacity: true)
        guard samples.count >= minimumSamples else {
            voiceDiag("recording too short — \(samples.count) samples (<300ms), skipped")
            return
        }
        isTranscribing = true
        transcribingChunk = true
        defer {
            isTranscribing = false
            transcribingChunk = false
        }
        voiceDiag("transcribing \(samples.count) samples")
        do {
            let text = try await transcriber.transcribe(samples: samples)
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            voiceDiag("transcription done — \(trimmed.count) chars")
            if !trimmed.isEmpty {
                onChunk?(trimmed)
            } else {
                voiceDiag("transcription produced no text")
            }
        } catch {
            voiceDiag("transcription FAILED: \(error.localizedDescription)")
            onError?(error.localizedDescription)
        }
    }
}
