import Foundation
import AVFoundation
import Combine
import os

/// Shared logger for the on-device voice + transcription path.
let voiceLog = Logger(subsystem: "app.tesela.ios", category: "voice")

/// Emit a voice-path diagnostic to the unified log. Stream it with:
///   `log stream --predicate 'subsystem == "app.tesela.ios"'`
func voiceDiag(_ message: String) {
    voiceLog.notice("\(message, privacy: .public)")
}

/// A rolling window of recent microphone RMS levels (0…1) that drives
/// the capture bar's live waveform. Kept as its own object so only the
/// small waveform view re-renders at the audio-buffer rate — observing
/// the level from the whole capture bar caused a re-render storm that
/// previously ate the stop button's taps.
@MainActor
final class AudioLevelMonitor: ObservableObject {
    /// Oldest-to-newest, fixed length so the waveform is stable-width.
    @Published private(set) var levels: [Float]

    private let windowSize = 32

    init() {
        levels = Array(repeating: 0, count: windowSize)
    }

    func push(_ level: Float) {
        var next = levels
        next.removeFirst()
        next.append(min(1, max(0, level)))
        levels = next
    }

    func reset() {
        levels = Array(repeating: 0, count: windowSize)
    }
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

    /// The most recent finished transcript. The capture bar observes
    /// this (`@ObservedObject` + `.onChange`) and appends it to the
    /// composer. A *published* property rather than a callback closure:
    /// a closure stored here would capture the SwiftUI view struct and,
    /// firing seconds later, mutate a stale snapshot whose `@State` no
    /// longer reaches the live view. The consumer sets it back to `nil`.
    @Published var lastTranscript: String? = nil

    /// Live microphone level history — drives the capture bar's waveform
    /// while recording. Its own object so the waveform re-renders
    /// without dragging the whole capture bar with it.
    let levelMonitor = AudioLevelMonitor()

    /// Set when transcription throws, so the capture bar can show a
    /// clean error rather than silently producing nothing. Cleared at
    /// the next `start()`.
    @Published var transcriptionError: String? = nil

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
            transcriptionError = nil
            levelMonitor.reset()
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

    /// Clear a surfaced error so the capture bar returns to the text
    /// field. Called when the user taps the error chip.
    func dismissError() {
        transcriptionError = nil
        switch state {
        case .failed, .denied: state = .idle
        default: break
        }
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
                // `.noDataNow`, NOT `.endOfStream`: this one converter
                // is reused for every tap buffer. `.endOfStream` would
                // permanently finish it — only the first buffer would
                // ever convert and every later one would yield nothing
                // (the bug behind "recording too short — 1600 samples").
                status.pointee = .noDataNow
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
        // RMS of this buffer drives the live waveform. Boosted so
        // ordinary speech fills the meter without clipping.
        var sumSquares: Float = 0
        for sample in chunk { sumSquares += sample * sample }
        let rms = n > 0 ? (sumSquares / Float(n)).squareRoot() : 0
        let level = min(1, rms * 6)
        Task { @MainActor in
            self.pendingSamples.append(contentsOf: chunk)
            self.levelMonitor.push(level)
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
                lastTranscript = trimmed
            } else {
                voiceDiag("transcription produced no text")
            }
        } catch {
            voiceDiag("transcription FAILED: \(error.localizedDescription)")
            transcriptionError = error.localizedDescription
        }
    }
}
