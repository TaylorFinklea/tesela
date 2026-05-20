import Foundation
import AVFoundation
import Combine

/// Live transcription coordinator. Replaces `AVAudioRecorder` (which
/// writes to a file and only hands it off on stop) with
/// `AVAudioEngine` so we can tap the input PCM in real time, send
/// 5-second chunks through the active TranscriptionEngine, and stream
/// transcripts into the composer as the user speaks.
///
/// Owners pass an engine + a callback. The recorder calls back on
/// the MainActor whenever a chunk has finished transcribing.
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
    @Published private(set) var meterLevel: Float = 0
    @Published private(set) var transcribingChunk: Bool = false

    var onChunk: ((String) -> Void)? = nil
    var onError: ((String) -> Void)? = nil

    private let engine = AVAudioEngine()
    private var converter: AVAudioConverter?
    private var startedAt: Date?
    private var elapsedTimer: Timer?

    /// Samples buffered since the last transcription tick. Each tick
    /// drains this buffer and dispatches it through the active
    /// `TranscriptionEngine`.
    private var pendingSamples: [Float] = []
    private var chunkTimer: Timer?
    private var transcriber: TranscriptionEngine?
    /// True while a `transcribeNextChunk` drain loop is running. The 5s
    /// chunk timer checks this and skips rather than starting a second,
    /// concurrent transcription — critical for Parakeet, whose first-call
    /// CoreML model load is far slower than the chunk interval.
    private var isTranscribing = false

    /// 5-second windows are short enough for sub-second latency on
    /// whisper-tiny while giving the model real context to work with.
    private let chunkInterval: TimeInterval = 5

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
        guard await ensurePermission() else {
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
            return true
        } catch {
            state = .failed(error.localizedDescription)
            return false
        }
    }

    /// Stops recording immediately. Audio still buffered is transcribed
    /// best-effort in the background — stopping must never wait on
    /// transcription, which for Parakeet includes a CoreML model load
    /// that can take tens of seconds.
    func stop() {
        guard case .recording = state else { return }
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        try? AVAudioSession.sharedInstance().setActive(false, options: [.notifyOthersOnDeactivation])
        stopTimers()
        state = .idle
        // Flush the tail. If a drain loop is already running it will pick
        // up these samples itself; otherwise this kicks one off.
        Task { @MainActor [weak self] in await self?.transcribeNextChunk() }
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
        // Compute a rough RMS for the meter from the new chunk.
        var sum: Float = 0
        for i in 0..<n {
            let s = ptr[i]
            sum += s * s
        }
        let rms = sqrt(sum / Float(max(1, n)))
        let lvl = min(1, rms * 6) // boost
        let chunk = Array(UnsafeBufferPointer(start: ptr, count: n))
        Task { @MainActor in
            self.meterLevel = 0.6 * self.meterLevel + 0.4 * lvl
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

    // MARK: - Chunk dispatch

    private func startTimers() {
        elapsedTimer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            Task { @MainActor [weak self] in self?.tickElapsed() }
        }
        chunkTimer = Timer.scheduledTimer(withTimeInterval: chunkInterval, repeats: true) { [weak self] _ in
            Task { @MainActor [weak self] in await self?.transcribeNextChunk() }
        }
    }

    private func stopTimers() {
        elapsedTimer?.invalidate()
        elapsedTimer = nil
        chunkTimer?.invalidate()
        chunkTimer = nil
    }

    private func tickElapsed() {
        guard case .recording = state, let startedAt else { return }
        state = .recording(elapsed: Date().timeIntervalSince(startedAt))
    }

    /// Drain `pendingSamples` through the transcriber, delivering each
    /// result via `onChunk`. Re-entrant callers (the 5s chunk timer, the
    /// stop flush) are coalesced: `isTranscribing` ensures only one drain
    /// loop runs at a time, and it keeps going until the buffer is empty.
    ///
    /// Without this coalescing a slow transcriber lets the timer spawn
    /// many concurrent transcriptions — each Parakeet call loads a fresh
    /// hundreds-of-MB CoreML model set, and a few in parallel get the app
    /// jetsam-killed before the first result ever lands.
    private func transcribeNextChunk() async {
        guard !isTranscribing, !pendingSamples.isEmpty, let transcriber else { return }
        isTranscribing = true
        transcribingChunk = true
        defer {
            isTranscribing = false
            transcribingChunk = false
        }
        while !pendingSamples.isEmpty {
            let chunk = pendingSamples
            pendingSamples.removeAll(keepingCapacity: true)
            do {
                let text = try await transcriber.transcribe(samples: chunk)
                let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
                if !trimmed.isEmpty {
                    onChunk?(trimmed)
                }
            } catch {
                onError?(error.localizedDescription)
                return
            }
        }
    }
}
