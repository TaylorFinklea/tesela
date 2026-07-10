import XCTest
@testable import Tesela

/// tesela-v5t.3: the streaming shape added to `TranscriptionEngine` (catalog
/// entry for Parakeet Unified + the protocol's default non-streaming
/// behavior). `LocalTranscriptionEngine`'s actual FluidAudio-backed
/// streaming implementation isn't covered here — it needs a loaded CoreML
/// model and isn't reachable without a real device/simulator download; the
/// wiring between an engine's streaming callbacks and
/// `StreamingVoiceRecorder` is covered separately in
/// `StreamingVoiceRecorderTests` via a fake engine.
@MainActor
final class TranscriptionEngineStreamingTests: XCTestCase {

    // MARK: - Catalog entry

    func testCatalogHasParakeetUnifiedEntry() {
        let model = TranscriptionCatalog.find("parakeet-unified-en-0.6b")
        XCTAssertNotNil(model)
        XCTAssertEqual(model?.family, .parakeetUnified)
        XCTAssertTrue(model?.inferenceSupported ?? false)
        XCTAssertTrue(model?.onDevice ?? false)
        XCTAssertNil(model?.downloadURL, "FluidAudio owns the download, like the other Parakeet entries")
    }

    func testGroupedIncludesParakeetUnifiedFamily() {
        let families = TranscriptionCatalog.grouped.map(\.family)
        XCTAssertTrue(families.contains(.parakeetUnified))
    }

    func testParakeetUnifiedModelFamilyDisplayName() {
        XCTAssertEqual(ModelFamily.parakeetUnified.displayName, "Parakeet Unified")
    }

    // MARK: - Protocol default (non-streaming) behavior

    private final class MinimalWholeClipEngine: TranscriptionEngine {
        var displayLabel: String { "minimal" }
        func transcribe(audio url: URL) async throws -> String { "" }
        func transcribe(samples: [Float]) async throws -> String { "" }
        // Deliberately does NOT override any streaming requirement —
        // relies entirely on the protocol extension's defaults.
    }

    func testDefaultSupportsStreamingIsFalse() {
        let engine = MinimalWholeClipEngine()
        XCTAssertFalse(engine.supportsStreaming)
    }

    func testDefaultStartStreamingThrows() async {
        let engine = MinimalWholeClipEngine()
        do {
            try await engine.startStreaming { _ in }
            XCTFail("expected TranscriptionEngineError.streamingNotSupported")
        } catch is TranscriptionEngineError {
            // expected
        } catch {
            XCTFail("unexpected error type: \(error)")
        }
    }

    func testDefaultFinishStreamingReturnsEmptyString() async throws {
        let engine = MinimalWholeClipEngine()
        let result = try await engine.finishStreaming()
        XCTAssertEqual(result, "")
    }

    func testDefaultAppendAndCancelStreamingAreNoOps() async throws {
        let engine = MinimalWholeClipEngine()
        // Should not throw and should not crash — nothing to assert
        // beyond "these are safely callable defaults".
        try await engine.appendStreaming(samples: [0.1, 0.2])
        await engine.cancelStreaming()
    }
}
