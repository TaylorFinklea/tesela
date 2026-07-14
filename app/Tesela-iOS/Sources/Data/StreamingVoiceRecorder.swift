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

/// An AVAudioSession event the recorder needs to react to mid-recording.
/// Modeled as plain data (not the raw `Notification`) so the decision of
/// what to DO about it — `voiceSessionAction(for:isRecording:)` — is pure
/// and testable without a real `AVAudioSession`.
enum AudioSessionEvent: Equatable {
    /// Another app (a phone call, Siri, another recorder) took the audio
    /// session away from us.
    case interruptionBegan
    /// The interruption ended; `shouldResume` mirrors
    /// `AVAudioSession.InterruptionOptions.shouldResume`.
    case interruptionEnded(shouldResume: Bool)
    /// The input route lost a device (e.g. Bluetooth mic disconnected) —
    /// the engine's tap is now attached to a route that no longer exists.
    case routeChangeOldDeviceUnavailable
    /// A new input device became available (e.g. headset plugged in).
    /// Informational only — the existing route keeps working.
    case routeChangeNewDeviceAvailable
    /// The session's category changed out from under us (another
    /// component reconfigured shared `AVAudioSession` state).
    case routeChangeCategoryChange
    /// Any other route-change reason (override, mode change, …) —
    /// treated as informational.
    case routeChangeOther
}

/// What the recorder should do in response to an `AudioSessionEvent`.
enum AudioSessionAction: Equatable {
    /// End the recording cleanly — stop the tap, deactivate the session,
    /// and finalize whatever was captured (the streaming partial, or the
    /// whole-clip buffer) rather than silently dropping it.
    case stopAndFinalize
    /// Nothing to do.
    case ignore
}

/// Pure decision function for `AudioSessionEvent` handling — no
/// `AVAudioSession`, no I/O, so it's directly unit-testable. A phone
/// call or Siri taking the mic (or the input route disappearing under
/// us) must end the capture session cleanly rather than let the tap
/// keep writing into a torn-down engine. `interruptionEnded` does NOT
/// auto-resume — matching `VoiceRecorder`'s existing stop-on-background
/// posture, the user re-taps the mic to start a fresh session rather
/// than risk recording over a context they've moved on from.
func voiceSessionAction(for event: AudioSessionEvent, isRecording: Bool) -> AudioSessionAction {
    guard isRecording else { return .ignore }
    switch event {
    case .interruptionBegan, .routeChangeOldDeviceUnavailable, .routeChangeCategoryChange:
        return .stopAndFinalize
    case .interruptionEnded, .routeChangeNewDeviceAvailable, .routeChangeOther:
        return .ignore
    }
}

/// Identity captured at the instant a voice session starts. Both parts are
/// required: the profile token changes as soon as the user selects another
/// profile, while the backend generation can remain on the old attachment
/// until an in-flight operation drains.
struct VoiceCaptureScope: Equatable, Sendable {
    let profileIdentity: String
    let backendGeneration: UInt64

    static let testing = VoiceCaptureScope(profileIdentity: "test", backendGeneration: 0)
}

/// A finished transcript carries the session identity it belongs to so a
/// stable shell can reject it after the user has switched profiles.
struct VoiceTranscript: Equatable, Sendable {
    let text: String
    let scope: VoiceCaptureScope

    func text(ifCurrent currentScope: VoiceCaptureScope) -> String? {
        scope == currentScope ? text : nil
    }
}

/// Voice capture coordinator. Uses `AVAudioEngine` to tap the mic input.
/// Two paths depending on the active `TranscriptionEngine`:
///   • Streaming (`supportsStreaming == true`, e.g. Parakeet Unified):
///     each buffer feeds `appendStreaming` live, and `livePartial` is
///     updated as the engine commits more text; `stop()` calls
///     `finishStreaming()`.
///   • Whole-clip (whisper, server, Parakeet TDT): samples accumulate in
///     `pendingSamples` and are transcribed in one pass on `stop()`.
///
/// Whole-clip existed first because FluidAudio's non-streaming Parakeet
/// `transcribe` rejects clips shorter than 300 ms and chunks long audio
/// internally, so hand-rolled windowing only ever produced short tail
/// chunks it refused — streaming engines don't have that constraint.
/// Owners pass an engine + callbacks; `onChunk` fires once, on the
/// MainActor, with the final transcript.
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
    /// longer reaches the live view. The consumer clears it through
    /// `clearLastTranscript()`.
    @Published private(set) var lastTranscript: VoiceTranscript? = nil

    /// The live COMMITTED transcript for a streaming session — grows as
    /// the engine advances it. There is deliberately no separate
    /// "tentative tail" property: FluidAudio's unified streaming manager
    /// emits one monotonic committed string (see `TranscriptionEngine`
    /// protocol docs), so this is the whole live-preview surface. `nil`
    /// outside a streaming session (whole-clip engines never touch it).
    @Published var livePartial: String? = nil

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
    /// and transcribed in one pass on `stop()`. Unused for a streaming
    /// session — those samples go straight to `appendStreaming` instead.
    private var pendingSamples: [Float] = []
    private var transcriber: TranscriptionEngine?
    /// Monotonic session identity. Async callbacks and finalizers may mutate
    /// UI state only while their captured epoch is still current.
    private var sessionEpoch: UInt64 = 0
    private var sessionScope: VoiceCaptureScope?
    private var transcribingSessionEpoch: UInt64?
    private var finalizationTask: Task<Void, Never>?
    /// True once `beginStreamingSession` has successfully started a
    /// streaming session on `transcriber` — routes `handleInputBuffer`
    /// and `stop()` through the streaming path instead of the whole-clip
    /// one.
    private(set) var streamingSessionActive = false

    private var interruptionObserver: NSObjectProtocol?
    private var routeChangeObserver: NSObjectProtocol?

    /// FluidAudio's Parakeet `transcribe` rejects anything shorter than
    /// 300 ms — 4800 samples at 16 kHz. A clip that short is noise
    /// anyway, so it's skipped rather than sent (and rejected). Only
    /// applies to the whole-clip path.
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

    init() {
        setUpAudioSessionObservers()
    }

    deinit {
        if let interruptionObserver { NotificationCenter.default.removeObserver(interruptionObserver) }
        if let routeChangeObserver { NotificationCenter.default.removeObserver(routeChangeObserver) }
    }

    // MARK: - Public API

    /// `preferStreaming` mirrors the `voice.streaming` setting
    /// (`VoiceSettingsView`) — when false, always use the whole-clip path
    /// even for an engine that supports streaming.
    @discardableResult
    func start(
        using transcriber: TranscriptionEngine,
        scope: VoiceCaptureScope,
        preferStreaming: Bool = true
    ) async -> Bool {
        voiceDiag("start requested")
        let epoch = beginNewSession(scope: scope)
        guard await ensurePermission() else {
            guard isCurrentSession(epoch) else { return false }
            voiceDiag("start: microphone permission DENIED")
            state = .denied
            return false
        }
        guard isCurrentSession(epoch) else { return false }
        do {
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.playAndRecord, mode: .measurement, options: [.defaultToSpeaker])
            try session.setActive(true, options: [.notifyOthersOnDeactivation])

            transcriptionError = nil
            levelMonitor.reset()
            pendingSamples.removeAll(keepingCapacity: true)
            livePartial = nil

            if preferStreaming {
                _ = await beginStreamingSession(using: transcriber, epoch: epoch)
            } else {
                self.transcriber = transcriber
                streamingSessionActive = false
            }
            guard isCurrentSession(epoch) else {
                try? session.setActive(false, options: [.notifyOthersOnDeactivation])
                return false
            }

            // Set up the input tap. We convert each buffer into 16 kHz
            // mono float32 and feed it to whichever path is active.
            let input = engine.inputNode
            let inputFormat = input.outputFormat(forBus: 0)
            converter = AVAudioConverter(from: inputFormat, to: targetFormat)
            guard converter != nil else {
                state = .failed("Audio format converter unavailable")
                return false
            }
            input.installTap(onBus: 0, bufferSize: 4_096, format: inputFormat) { [weak self] buffer, _ in
                self?.handleInputBuffer(buffer, expectedEpoch: epoch)
            }
            engine.prepare()
            try engine.start()

            startedAt = Date()
            state = .recording(elapsed: 0)
            startTimers()
            voiceDiag("recording started (streaming=\(streamingSessionActive))")
            return true
        } catch {
            guard isCurrentSession(epoch) else { return false }
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
        voiceDiag("stopped — finalizing")
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        try? AVAudioSession.sharedInstance().setActive(false, options: [.notifyOthersOnDeactivation])
        stopTimers()
        state = .idle
        let epoch = sessionEpoch
        if streamingSessionActive {
            finalizationTask = Task { @MainActor [weak self] in
                await self?.finishStreamingSession(expectedEpoch: epoch)
            }
        } else {
            finalizationTask = Task { @MainActor [weak self] in
                await self?.transcribeAll(expectedEpoch: epoch)
            }
        }
    }

    func cancel() {
        stopAudioCaptureIfNeeded()
        invalidateCurrentSession()
    }

    /// Called synchronously from the app shell when its profile/backend token
    /// changes. Invalidating before activation begins closes the interval in
    /// which the selected profile is B but the shared backend is still A.
    func invalidateForProfileSwitch() {
        stopAudioCaptureIfNeeded()
        invalidateCurrentSession()
    }

    func clearLastTranscript() {
        lastTranscript = nil
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

    @discardableResult
    private func beginNewSession(scope: VoiceCaptureScope) -> UInt64 {
        let streamingTranscriber = streamingSessionActive ? transcriber : nil
        finalizationTask?.cancel()
        finalizationTask = nil
        sessionEpoch &+= 1
        sessionScope = scope
        transcriber = nil
        streamingSessionActive = false
        transcribingSessionEpoch = nil
        transcribingChunk = false
        pendingSamples.removeAll(keepingCapacity: true)
        livePartial = nil
        lastTranscript = nil
        transcriptionError = nil
        if let streamingTranscriber {
            Task { await streamingTranscriber.cancelStreaming() }
        }
        return sessionEpoch
    }

    private func invalidateCurrentSession() {
        let streamingTranscriber = streamingSessionActive ? transcriber : nil
        finalizationTask?.cancel()
        finalizationTask = nil
        sessionEpoch &+= 1
        sessionScope = nil
        transcriber = nil
        streamingSessionActive = false
        transcribingSessionEpoch = nil
        transcribingChunk = false
        pendingSamples.removeAll(keepingCapacity: true)
        livePartial = nil
        lastTranscript = nil
        transcriptionError = nil
        state = .idle
        if let streamingTranscriber {
            Task { await streamingTranscriber.cancelStreaming() }
        }
    }

    private func isCurrentSession(_ epoch: UInt64) -> Bool {
        epoch == sessionEpoch && sessionScope != nil
    }

    private func stopAudioCaptureIfNeeded() {
        if isCurrentlyRecording {
            engine.inputNode.removeTap(onBus: 0)
            engine.stop()
        }
        try? AVAudioSession.sharedInstance().setActive(
            false,
            options: [.notifyOthersOnDeactivation]
        )
        stopTimers()
        state = .idle
    }

    // MARK: - Streaming session (extracted from `start`/`stop`/`handleInputBuffer`
    // so the wiring is testable against a fake `TranscriptionEngine` without
    // spinning up a real `AVAudioEngine`.)

    /// Attempt to begin a streaming session on `transcriber`. Always sets
    /// `self.transcriber` (used by both paths); sets `streamingSessionActive`
    /// and wires `onPartial` into `livePartial` only on success. On failure
    /// (engine doesn't support streaming, or `startStreaming` throws) the
    /// caller transparently falls back to the whole-clip path.
    @discardableResult
    func beginStreamingSession(
        using transcriber: TranscriptionEngine,
        scope: VoiceCaptureScope = .testing
    ) async -> Bool {
        let epoch = beginNewSession(scope: scope)
        return await beginStreamingSession(using: transcriber, epoch: epoch)
    }

    private func beginStreamingSession(
        using transcriber: TranscriptionEngine,
        epoch: UInt64
    ) async -> Bool {
        guard isCurrentSession(epoch) else { return false }
        self.transcriber = transcriber
        guard transcriber.supportsStreaming else {
            streamingSessionActive = false
            return false
        }
        do {
            try await transcriber.startStreaming { [weak self] partial in
                guard let self, self.isCurrentSession(epoch) else { return }
                self.livePartial = partial
            }
            guard isCurrentSession(epoch) else {
                await transcriber.cancelStreaming()
                return false
            }
            streamingSessionActive = true
            return true
        } catch {
            guard isCurrentSession(epoch) else { return false }
            voiceDiag("startStreaming failed, falling back to whole-clip: \(error.localizedDescription)")
            streamingSessionActive = false
            return false
        }
    }

    /// Feed one converted buffer's worth of samples to the active
    /// streaming session. No-op when no session is active.
    func feedStreaming(_ chunk: [Float]) async {
        await feedStreaming(chunk, expectedEpoch: sessionEpoch)
    }

    private func feedStreaming(_ chunk: [Float], expectedEpoch: UInt64) async {
        guard isCurrentSession(expectedEpoch), streamingSessionActive, let transcriber else { return }
        do {
            try await transcriber.appendStreaming(samples: chunk)
        } catch {
            voiceDiag("appendStreaming failed: \(error.localizedDescription)")
        }
    }

    /// Flush and finalize the active streaming session, landing the
    /// result in `lastTranscript` exactly like the whole-clip path.
    func finishStreamingSession() async {
        await finishStreamingSession(expectedEpoch: sessionEpoch)
    }

    private func finishStreamingSession(expectedEpoch: UInt64) async {
        guard isCurrentSession(expectedEpoch),
              streamingSessionActive,
              let transcriber,
              let scope = sessionScope
        else { return }
        streamingSessionActive = false
        transcribingSessionEpoch = expectedEpoch
        transcribingChunk = true
        defer {
            if transcribingSessionEpoch == expectedEpoch {
                transcribingSessionEpoch = nil
                transcribingChunk = false
            }
        }
        do {
            let text = try await transcriber.finishStreaming()
            guard isCurrentSession(expectedEpoch) else { return }
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            voiceDiag("finishStreaming done — \(trimmed.count) chars")
            livePartial = nil
            if !trimmed.isEmpty {
                lastTranscript = VoiceTranscript(text: trimmed, scope: scope)
            } else {
                voiceDiag("finishStreaming produced no text")
            }
        } catch {
            guard isCurrentSession(expectedEpoch) else { return }
            voiceDiag("finishStreaming FAILED: \(error.localizedDescription)")
            transcriptionError = error.localizedDescription
        }
    }

    // MARK: - Audio buffer plumbing

    private func handleInputBuffer(_ buffer: AVAudioPCMBuffer, expectedEpoch: UInt64) {
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
            guard self.isCurrentSession(expectedEpoch) else { return }
            self.levelMonitor.push(level)
            if self.streamingSessionActive {
                await self.feedStreaming(chunk, expectedEpoch: expectedEpoch)
            } else {
                self.pendingSamples.append(contentsOf: chunk)
            }
        }
    }

    // MARK: - AVAudioSession interruption + route-change handling

    /// A phone call, Siri, or another app taking the microphone mid-
    /// recording must end the session cleanly (see `voiceSessionAction`)
    /// rather than silently leave the tap writing into a torn-down
    /// engine. Installed once for the recorder's lifetime; each handler
    /// no-ops when not currently recording.
    private func setUpAudioSessionObservers() {
        let nc = NotificationCenter.default
        interruptionObserver = nc.addObserver(
            forName: AVAudioSession.interruptionNotification, object: nil, queue: .main
        ) { [weak self] note in
            MainActor.assumeIsolated { self?.handleInterruption(note) }
        }
        routeChangeObserver = nc.addObserver(
            forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main
        ) { [weak self] note in
            MainActor.assumeIsolated { self?.handleRouteChange(note) }
        }
    }

    private func handleInterruption(_ note: Notification) {
        guard let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
              let type = AVAudioSession.InterruptionType(rawValue: raw)
        else { return }
        let event: AudioSessionEvent
        switch type {
        case .began:
            event = .interruptionBegan
        case .ended:
            let raw = note.userInfo?[AVAudioSessionInterruptionOptionKey] as? UInt ?? 0
            let shouldResume = AVAudioSession.InterruptionOptions(rawValue: raw).contains(.shouldResume)
            event = .interruptionEnded(shouldResume: shouldResume)
        @unknown default:
            return
        }
        apply(voiceSessionAction(for: event, isRecording: isCurrentlyRecording))
    }

    private func handleRouteChange(_ note: Notification) {
        guard let raw = note.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt,
              let reason = AVAudioSession.RouteChangeReason(rawValue: raw)
        else { return }
        let event: AudioSessionEvent
        switch reason {
        case .oldDeviceUnavailable: event = .routeChangeOldDeviceUnavailable
        case .newDeviceAvailable:   event = .routeChangeNewDeviceAvailable
        case .categoryChange:       event = .routeChangeCategoryChange
        default:                    event = .routeChangeOther
        }
        apply(voiceSessionAction(for: event, isRecording: isCurrentlyRecording))
    }

    private var isCurrentlyRecording: Bool {
        if case .recording = state { return true }
        return false
    }

    private func apply(_ action: AudioSessionAction) {
        switch action {
        case .stopAndFinalize:
            voiceDiag("audio session event — ending capture cleanly")
            stop()
        case .ignore:
            break
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
    private func transcribeAll(expectedEpoch: UInt64) async {
        guard isCurrentSession(expectedEpoch),
              transcribingSessionEpoch == nil,
              let transcriber,
              let scope = sessionScope
        else { return }
        let samples = pendingSamples
        pendingSamples.removeAll(keepingCapacity: true)
        guard samples.count >= minimumSamples else {
            voiceDiag("recording too short — \(samples.count) samples (<300ms), skipped")
            return
        }
        transcribingSessionEpoch = expectedEpoch
        transcribingChunk = true
        defer {
            if transcribingSessionEpoch == expectedEpoch {
                transcribingSessionEpoch = nil
                transcribingChunk = false
            }
        }
        voiceDiag("transcribing \(samples.count) samples")
        do {
            let text = try await transcriber.transcribe(samples: samples)
            guard isCurrentSession(expectedEpoch) else { return }
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            voiceDiag("transcription done — \(trimmed.count) chars")
            if !trimmed.isEmpty {
                lastTranscript = VoiceTranscript(text: trimmed, scope: scope)
            } else {
                voiceDiag("transcription produced no text")
            }
        } catch {
            guard isCurrentSession(expectedEpoch) else { return }
            voiceDiag("transcription FAILED: \(error.localizedDescription)")
            transcriptionError = error.localizedDescription
        }
    }
}
