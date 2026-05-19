import Foundation
import AVFoundation
import Combine

/// Voice capture coordinator. Wraps `AVAudioRecorder` configured for
/// 16 kHz mono 16-bit PCM WAV — the format the server's
/// `/transcription/transcribe` endpoint expects.
///
/// Lifecycle:
///   1. `start()` requests mic permission, configures the audio
///      session, and begins recording to a temp file. Publishes
///      `meterLevel` for live waveform UI (10 Hz).
///   2. `stop()` finalizes the file and returns its URL.
///   3. Caller uploads the file (via `transcribe(...)` on the mosaic
///      service), then deletes it.
@MainActor
final class VoiceRecorder: NSObject, ObservableObject {
    enum State: Equatable {
        case idle
        case requestingPermission
        case denied
        case recording(elapsed: TimeInterval)
        case stopping
        case failed(String)
    }

    @Published private(set) var state: State = .idle
    @Published private(set) var meterLevel: Float = 0   // 0…1, smoothed

    private var recorder: AVAudioRecorder?
    private var meterTimer: Timer?
    private var elapsedTimer: Timer?
    private var startedAt: Date?
    private(set) var fileURL: URL?

    // MARK: - Public API

    /// Start recording. Returns true if the recorder actually fired
    /// (mic permission granted + audio session ready), false on
    /// permission denial or session errors.
    @discardableResult
    func start() async -> Bool {
        guard await ensurePermission() else {
            state = .denied
            return false
        }
        do {
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.playAndRecord, mode: .measurement, options: [.defaultToSpeaker])
            try session.setActive(true, options: [.notifyOthersOnDeactivation])
            let url = makeTempURL()
            let settings: [String: Any] = [
                AVFormatIDKey: Int(kAudioFormatLinearPCM),
                AVSampleRateKey: 16_000,
                AVNumberOfChannelsKey: 1,
                AVLinearPCMBitDepthKey: 16,
                AVLinearPCMIsBigEndianKey: false,
                AVLinearPCMIsFloatKey: false,
                AVEncoderAudioQualityKey: AVAudioQuality.high.rawValue,
            ]
            let recorder = try AVAudioRecorder(url: url, settings: settings)
            recorder.delegate = self
            recorder.isMeteringEnabled = true
            guard recorder.record() else {
                state = .failed("Couldn't start the recorder")
                return false
            }
            self.recorder = recorder
            self.fileURL = url
            self.startedAt = Date()
            startTimers()
            state = .recording(elapsed: 0)
            return true
        } catch {
            state = .failed(error.localizedDescription)
            return false
        }
    }

    /// Stop and return the file URL on disk.
    @discardableResult
    func stop() -> URL? {
        guard case .recording = state, let recorder else {
            return fileURL
        }
        state = .stopping
        recorder.stop()
        stopTimers()
        try? AVAudioSession.sharedInstance().setActive(false, options: [.notifyOthersOnDeactivation])
        let url = recorder.url
        self.recorder = nil
        state = .idle
        return url
    }

    /// Cancel — drop the recording without keeping the file.
    func cancel() {
        recorder?.stop()
        stopTimers()
        try? AVAudioSession.sharedInstance().setActive(false, options: [.notifyOthersOnDeactivation])
        if let url = recorder?.url {
            try? FileManager.default.removeItem(at: url)
        }
        recorder = nil
        fileURL = nil
        state = .idle
    }

    /// Wipe the file backing the last recording.
    func discardFile() {
        if let url = fileURL {
            try? FileManager.default.removeItem(at: url)
        }
        fileURL = nil
    }

    // MARK: - Helpers

    private func ensurePermission() async -> Bool {
        let session = AVAudioSession.sharedInstance()
        switch session.recordPermission {
        case .granted:
            return true
        case .denied:
            return false
        case .undetermined:
            state = .requestingPermission
            return await withCheckedContinuation { cont in
                session.requestRecordPermission { granted in
                    cont.resume(returning: granted)
                }
            }
        @unknown default:
            return false
        }
    }

    private func makeTempURL() -> URL {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("voice-recordings", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let name = "rec-\(Int(Date().timeIntervalSince1970)).wav"
        return dir.appendingPathComponent(name)
    }

    private func startTimers() {
        meterTimer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.tickMeter()
            }
        }
        elapsedTimer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.tickElapsed()
            }
        }
    }

    private func stopTimers() {
        meterTimer?.invalidate()
        meterTimer = nil
        elapsedTimer?.invalidate()
        elapsedTimer = nil
    }

    private func tickMeter() {
        guard let recorder, recorder.isRecording else { return }
        recorder.updateMeters()
        let db = recorder.averagePower(forChannel: 0)
        // Map -60..0 dB → 0..1
        let normalized = max(0, min(1, (db + 60) / 60))
        // Light smoothing for a calmer waveform.
        meterLevel = 0.6 * meterLevel + 0.4 * normalized
    }

    private func tickElapsed() {
        guard case .recording = state, let startedAt else { return }
        state = .recording(elapsed: Date().timeIntervalSince(startedAt))
    }
}

extension VoiceRecorder: AVAudioRecorderDelegate {
    nonisolated func audioRecorderDidFinishRecording(_ recorder: AVAudioRecorder, successfully flag: Bool) {
        if !flag {
            Task { @MainActor in
                self.state = .failed("Recording ended unexpectedly")
            }
        }
    }

    nonisolated func audioRecorderEncodeErrorDidOccur(_ recorder: AVAudioRecorder, error: Error?) {
        Task { @MainActor in
            self.state = .failed(error?.localizedDescription ?? "Encoding error")
        }
    }
}
