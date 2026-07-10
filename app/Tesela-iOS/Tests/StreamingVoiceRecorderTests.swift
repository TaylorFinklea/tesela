import XCTest
@testable import Tesela

/// tesela-v5t.3: `StreamingVoiceRecorder`'s streaming seam and its
/// AVAudioSession interruption/route-change state machine. Zero dictation
/// tests existed before this bead.
///
/// The streaming-session tests exercise `beginStreamingSession` /
/// `feedStreaming` / `finishStreamingSession` directly against a fake
/// `TranscriptionEngine` — these were extracted specifically so they're
/// reachable WITHOUT spinning up a real `AVAudioEngine` tap (which needs
/// microphone permission and isn't reliably driveable headlessly). They
/// pin the exact wiring `start()`/`handleInputBuffer()`/`stop()` rely on:
/// a fake engine's `onPartial` callback lands in `livePartial`, buffers
/// route to `appendStreaming` only while a session is active, and
/// `finishStreaming()`'s result lands in `lastTranscript`.
@MainActor
final class StreamingVoiceRecorderTests: XCTestCase {

    // MARK: - Fakes

    /// Records every call so tests can assert on call counts/arguments,
    /// and exposes `emitPartial` to simulate FluidAudio's committed-
    /// transcript callback firing mid-session.
    @MainActor
    private final class FakeStreamingTranscriptionEngine: TranscriptionEngine {
        var displayLabel: String { "fake-streaming" }
        var supportsStreaming: Bool = true
        var startStreamingShouldThrow = false
        var finalTranscript = "final transcript"

        private(set) var startStreamingCallCount = 0
        private(set) var appendedChunks: [[Float]] = []
        private(set) var finishStreamingCallCount = 0
        private(set) var cancelStreamingCallCount = 0

        private var partialCallback: (@MainActor (String) -> Void)?

        func transcribe(audio url: URL) async throws -> String { "" }
        func transcribe(samples: [Float]) async throws -> String { "" }

        func startStreaming(onPartial: @escaping @MainActor (String) -> Void) async throws {
            if startStreamingShouldThrow {
                throw TranscriptionEngineError.streamingNotSupported
            }
            startStreamingCallCount += 1
            partialCallback = onPartial
        }

        func appendStreaming(samples: [Float]) async throws {
            appendedChunks.append(samples)
        }

        func finishStreaming() async throws -> String {
            finishStreamingCallCount += 1
            return finalTranscript
        }

        func cancelStreaming() async {
            cancelStreamingCallCount += 1
        }

        /// Simulate FluidAudio's `setPartialTranscriptCallback` firing
        /// with the next committed transcript.
        func emitPartial(_ text: String) {
            partialCallback?(text)
        }
    }

    /// A whole-clip-only engine (whisper/server shape) — relies entirely
    /// on the `TranscriptionEngine` protocol extension's defaults.
    private final class FakeWholeClipTranscriptionEngine: TranscriptionEngine {
        var displayLabel: String { "fake-whole-clip" }
        func transcribe(audio url: URL) async throws -> String { "" }
        func transcribe(samples: [Float]) async throws -> String { "" }
    }

    // MARK: - beginStreamingSession / partial-callback plumbing

    func testBeginStreamingSessionWiresPartialsToLivePartial() async {
        let recorder = StreamingVoiceRecorder()
        let fake = FakeStreamingTranscriptionEngine()

        let began = await recorder.beginStreamingSession(using: fake)

        XCTAssertTrue(began)
        XCTAssertTrue(recorder.streamingSessionActive)
        XCTAssertEqual(fake.startStreamingCallCount, 1)
        XCTAssertNil(recorder.livePartial)

        fake.emitPartial("hello")
        XCTAssertEqual(recorder.livePartial, "hello")

        fake.emitPartial("hello world")
        XCTAssertEqual(recorder.livePartial, "hello world")
    }

    func testBeginStreamingSessionReturnsFalseForNonStreamingEngine() async {
        let recorder = StreamingVoiceRecorder()
        let fake = FakeWholeClipTranscriptionEngine()

        let began = await recorder.beginStreamingSession(using: fake)

        XCTAssertFalse(began)
        XCTAssertFalse(recorder.streamingSessionActive)
    }

    func testBeginStreamingSessionFallsBackWhenStartStreamingThrows() async {
        let recorder = StreamingVoiceRecorder()
        let fake = FakeStreamingTranscriptionEngine()
        fake.startStreamingShouldThrow = true

        let began = await recorder.beginStreamingSession(using: fake)

        XCTAssertFalse(began)
        XCTAssertFalse(recorder.streamingSessionActive)
    }

    // MARK: - feedStreaming

    func testFeedStreamingForwardsSamplesWhenSessionActive() async {
        let recorder = StreamingVoiceRecorder()
        let fake = FakeStreamingTranscriptionEngine()
        _ = await recorder.beginStreamingSession(using: fake)

        await recorder.feedStreaming([0.1, 0.2, 0.3])
        await recorder.feedStreaming([0.4])

        XCTAssertEqual(fake.appendedChunks.count, 2)
        XCTAssertEqual(fake.appendedChunks[0], [0.1, 0.2, 0.3])
        XCTAssertEqual(fake.appendedChunks[1], [0.4])
    }

    func testFeedStreamingIsNoOpWithoutAnActiveSession() async {
        let recorder = StreamingVoiceRecorder()
        // No `beginStreamingSession` call — `streamingSessionActive` is
        // false and there's no transcriber wired up.
        await recorder.feedStreaming([0.1, 0.2])
        // Nothing to assert on directly (no fake attached); the test's
        // job is to prove this doesn't crash/throw when nothing is active.
    }

    // MARK: - finishStreamingSession

    func testFinishStreamingSessionSetsLastTranscriptAndClearsLivePartial() async {
        let recorder = StreamingVoiceRecorder()
        let fake = FakeStreamingTranscriptionEngine()
        fake.finalTranscript = "the final committed text"
        _ = await recorder.beginStreamingSession(using: fake)
        fake.emitPartial("the fin")
        XCTAssertEqual(recorder.livePartial, "the fin")

        await recorder.finishStreamingSession()

        XCTAssertEqual(fake.finishStreamingCallCount, 1)
        XCTAssertEqual(recorder.lastTranscript, "the final committed text")
        XCTAssertNil(recorder.livePartial)
        XCTAssertFalse(recorder.streamingSessionActive)
    }

    func testFinishStreamingSessionWithEmptyTranscriptDoesNotSetLastTranscript() async {
        let recorder = StreamingVoiceRecorder()
        let fake = FakeStreamingTranscriptionEngine()
        fake.finalTranscript = "   "
        _ = await recorder.beginStreamingSession(using: fake)

        await recorder.finishStreamingSession()

        XCTAssertNil(recorder.lastTranscript)
    }

    func testFinishStreamingSessionIsNoOpWithoutAnActiveSession() async {
        let recorder = StreamingVoiceRecorder()
        await recorder.finishStreamingSession()
        XCTAssertNil(recorder.lastTranscript)
    }

    // MARK: - AVAudioSession interruption / route-change state machine

    func testVoiceSessionActionIgnoresEverythingWhenNotRecording() {
        let events: [AudioSessionEvent] = [
            .interruptionBegan,
            .interruptionEnded(shouldResume: true),
            .interruptionEnded(shouldResume: false),
            .routeChangeOldDeviceUnavailable,
            .routeChangeNewDeviceAvailable,
            .routeChangeCategoryChange,
            .routeChangeOther,
        ]
        for event in events {
            XCTAssertEqual(
                voiceSessionAction(for: event, isRecording: false), .ignore,
                "expected .ignore for \(event) while not recording"
            )
        }
    }

    func testVoiceSessionActionStopsAndFinalizesOnInterruptionBegan() {
        XCTAssertEqual(voiceSessionAction(for: .interruptionBegan, isRecording: true), .stopAndFinalize)
    }

    func testVoiceSessionActionDoesNotAutoResumeOnInterruptionEnded() {
        // Neither `shouldResume` value auto-resumes — the user re-taps
        // the mic for a fresh session (see `voiceSessionAction` doc).
        XCTAssertEqual(
            voiceSessionAction(for: .interruptionEnded(shouldResume: true), isRecording: true), .ignore
        )
        XCTAssertEqual(
            voiceSessionAction(for: .interruptionEnded(shouldResume: false), isRecording: true), .ignore
        )
    }

    func testVoiceSessionActionStopsOnOldDeviceUnavailable() {
        XCTAssertEqual(
            voiceSessionAction(for: .routeChangeOldDeviceUnavailable, isRecording: true), .stopAndFinalize
        )
    }

    func testVoiceSessionActionStopsOnCategoryChange() {
        XCTAssertEqual(
            voiceSessionAction(for: .routeChangeCategoryChange, isRecording: true), .stopAndFinalize
        )
    }

    func testVoiceSessionActionIgnoresNewDeviceAvailableAndOtherReasons() {
        XCTAssertEqual(
            voiceSessionAction(for: .routeChangeNewDeviceAvailable, isRecording: true), .ignore
        )
        XCTAssertEqual(
            voiceSessionAction(for: .routeChangeOther, isRecording: true), .ignore
        )
    }
}
