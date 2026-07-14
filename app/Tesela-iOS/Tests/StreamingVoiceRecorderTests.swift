import XCTest
@testable import Tesela

private actor ServerVoiceUploadRecorder {
    private var requestURLs: [URL] = []

    func record(_ url: URL) {
        requestURLs.append(url)
    }

    func urls() -> [URL] {
        requestURLs
    }
}

private final class ServerVoiceUploadURLProtocol: URLProtocol {
    static var recorder = ServerVoiceUploadRecorder()

    override class func canInit(with request: URLRequest) -> Bool {
        request.url?.path == "/transcription/transcribe"
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        Task { [weak self] in
            guard let self, let url = request.url else { return }
            await Self.recorder.record(url)
            let data = Data(#"{"text":"stale profile transcript","model_id":"test","duration_ms":1}"#.utf8)
            let response = HTTPURLResponse(
                url: url,
                statusCode: 200,
                httpVersion: "HTTP/1.1",
                headerFields: ["Content-Type": "application/json"]
            )!
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: data)
            client?.urlProtocolDidFinishLoading(self)
        }
    }

    override func stopLoading() {}
}

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
        var delayFinish = false
        var onFinishStarted: (() -> Void)?

        private(set) var startStreamingCallCount = 0
        private(set) var appendedChunks: [[Float]] = []
        private(set) var finishStreamingCallCount = 0
        private(set) var cancelStreamingCallCount = 0

        private var partialCallback: (@MainActor (String) -> Void)?
        private var finishContinuation: CheckedContinuation<Void, Never>?

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
            onFinishStarted?()
            if delayFinish {
                await withCheckedContinuation { continuation in
                    finishContinuation = continuation
                }
            }
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

        func completeFinish() {
            finishContinuation?.resume()
            finishContinuation = nil
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

    func testServerEngineCreatedUnderProfileARefusesToUploadThroughProfileB() async throws {
        ServerVoiceUploadURLProtocol.recorder = ServerVoiceUploadRecorder()
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [ServerVoiceUploadURLProtocol.self]
        let mosaic = MockMosaicService(
            session: URLSession(configuration: configuration)
        )
        mosaic.attach(backend: .http(URL(string: "https://profile-a.test")!))
        let engine = ServerTranscriptionEngine(mosaic: mosaic)

        mosaic.attach(backend: .http(URL(string: "https://profile-b.test")!))

        do {
            _ = try await engine.transcribe(samples: Array(repeating: 0, count: 4_800))
            XCTFail("an engine created under profile A must reject stop-time upload under B")
        } catch is CancellationError {
            // Expected: the engine's creation-time backend lease is stale.
        } catch {
            XCTFail("unexpected error: \(error)")
        }
        let uploadedURLs = await ServerVoiceUploadURLProtocol.recorder.urls()
        XCTAssertTrue(uploadedURLs.isEmpty, "stale A audio must never upload to profile B")
    }

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
        XCTAssertEqual(recorder.lastTranscript?.text, "the final committed text")
        XCTAssertEqual(recorder.lastTranscript?.scope, .testing)
        XCTAssertNil(recorder.livePartial)
        XCTAssertFalse(recorder.streamingSessionActive)
    }

    func testProfileSwitchInvalidationDropsLateFinalTranscript() async {
        let recorder = StreamingVoiceRecorder()
        let profileA = FakeStreamingTranscriptionEngine()
        profileA.finalTranscript = "profile A transcript"
        profileA.delayFinish = true
        let finishStarted = expectation(description: "profile A finish started")
        profileA.onFinishStarted = { finishStarted.fulfill() }
        _ = await recorder.beginStreamingSession(
            using: profileA,
            scope: VoiceCaptureScope(profileIdentity: "A", backendGeneration: 1)
        )

        let oldFinish = Task { @MainActor in
            await recorder.finishStreamingSession()
        }
        await fulfillment(of: [finishStarted], timeout: 2)

        recorder.invalidateForProfileSwitch()
        profileA.completeFinish()
        await oldFinish.value

        XCTAssertNil(recorder.lastTranscript)
        XCTAssertNil(recorder.livePartial)
        XCTAssertFalse(recorder.transcribingChunk)
    }

    func testTranscriptCannotBeConsumedByDifferentProfileBeforeGenerationChanges() {
        let profileA = VoiceCaptureScope(profileIdentity: "A", backendGeneration: 7)
        let selectedProfileB = VoiceCaptureScope(profileIdentity: "B", backendGeneration: 7)
        let transcript = VoiceTranscript(text: "belongs to A", scope: profileA)

        XCTAssertNil(transcript.text(ifCurrent: selectedProfileB))
        XCTAssertEqual(transcript.text(ifCurrent: profileA), "belongs to A")
    }

    func testStartingNewSessionDropsLateFinalTranscriptFromPreviousSession() async {
        let recorder = StreamingVoiceRecorder()
        let profileA = FakeStreamingTranscriptionEngine()
        profileA.finalTranscript = "profile A transcript"
        profileA.delayFinish = true
        let finishStarted = expectation(description: "profile A finish started")
        profileA.onFinishStarted = { finishStarted.fulfill() }
        _ = await recorder.beginStreamingSession(using: profileA)

        let oldFinish = Task { @MainActor in
            await recorder.finishStreamingSession()
        }
        await fulfillment(of: [finishStarted], timeout: 2)

        let profileB = FakeStreamingTranscriptionEngine()
        _ = await recorder.beginStreamingSession(using: profileB)
        profileA.completeFinish()
        await oldFinish.value

        XCTAssertNil(
            recorder.lastTranscript,
            "a late profile A finalizer must not publish into profile B's recorder state"
        )
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
