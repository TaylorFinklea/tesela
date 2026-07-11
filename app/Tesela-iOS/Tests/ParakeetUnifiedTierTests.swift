import XCTest
@testable import Tesela

/// tesela-v5t.3: the Parakeet Unified streaming latency tiers FluidAudio
/// ships as distinct CoreML encoder downloads. Pure logic — no FluidAudio
/// types involved — so it's tested directly: the raw-value round trip
/// used for `voice.streamingTier` persistence, the `[left,chunk,right]`
/// context-frame mapping (pinned against FluidAudio v0.15.5's
/// `StreamingModelVariant.unifiedConfig`), and `ParakeetUnifiedTier.active`'s
/// UserDefaults resolution + fallback.
final class ParakeetUnifiedTierTests: XCTestCase {

    private let key = "voice.streamingTier"

    override func tearDown() {
        UserDefaults.standard.removeObject(forKey: key)
        super.tearDown()
    }

    // MARK: - contextFrames / contextSuffix (pinned against FluidAudio v0.15.5)

    func testContextFramesMatchFluidAudioV0155() {
        XCTAssertTrue(ParakeetUnifiedTier.tier320ms.contextFrames == (70, 2, 2))
        XCTAssertTrue(ParakeetUnifiedTier.tier640ms.contextFrames == (70, 7, 1))
        XCTAssertTrue(ParakeetUnifiedTier.tier1120ms.contextFrames == (70, 7, 7))
        XCTAssertTrue(ParakeetUnifiedTier.tier2080ms.contextFrames == (70, 13, 13))
    }

    func testContextSuffixMatchesFluidAudioFilenameConvention() {
        XCTAssertEqual(ParakeetUnifiedTier.tier320ms.contextSuffix, "70_2_2")
        XCTAssertEqual(ParakeetUnifiedTier.tier640ms.contextSuffix, "70_7_1")
        XCTAssertEqual(ParakeetUnifiedTier.tier1120ms.contextSuffix, "70_7_7")
        XCTAssertEqual(ParakeetUnifiedTier.tier2080ms.contextSuffix, "70_13_13")
    }

    func testEncoderCachePathIncludesFluidAudioRepositoryDirectory() {
        let base = URL(fileURLWithPath: "/tmp/TranscriptionModels/parakeet-unified", isDirectory: true)

        let encoder = TranscriptionStore.parakeetUnifiedEncoderURL(
            baseDirectory: base,
            tier: .tier640ms
        )

        XCTAssertEqual(
            encoder.path,
            "/tmp/TranscriptionModels/parakeet-unified/parakeet-unified-en-0.6b/"
                + "parakeet_unified_encoder_streaming_70_7_1_int8.mlmodelc"
        )
    }

    func testDownloadProgressExpandsFluidAudioDownloadPhaseToFullRange() {
        XCTAssertEqual(TranscriptionStore.parakeetUnifiedDownloadFraction(0), 0)
        XCTAssertEqual(TranscriptionStore.parakeetUnifiedDownloadFraction(0.25), 0.5)
        XCTAssertEqual(TranscriptionStore.parakeetUnifiedDownloadFraction(0.5), 1)
        XCTAssertEqual(TranscriptionStore.parakeetUnifiedDownloadFraction(1), 1)
    }

    // MARK: - Raw value round-trip (voice.streamingTier persistence shape)

    func testRawValueRoundTrip() {
        for tier in ParakeetUnifiedTier.allCases {
            XCTAssertEqual(ParakeetUnifiedTier(rawValue: tier.rawValue), tier)
        }
    }

    func testDefaultIs640ms() {
        XCTAssertEqual(ParakeetUnifiedTier.default, .tier640ms)
    }

    // MARK: - `.active` resolution

    func testActiveFallsBackToDefaultWhenUnset() {
        UserDefaults.standard.removeObject(forKey: key)
        XCTAssertEqual(ParakeetUnifiedTier.active, ParakeetUnifiedTier.default)
    }

    func testActiveFallsBackToDefaultWhenInvalid() {
        UserDefaults.standard.set("not-a-real-tier", forKey: key)
        XCTAssertEqual(ParakeetUnifiedTier.active, ParakeetUnifiedTier.default)
    }

    func testActiveReflectsStoredValue() {
        UserDefaults.standard.set(ParakeetUnifiedTier.tier320ms.rawValue, forKey: key)
        XCTAssertEqual(ParakeetUnifiedTier.active, .tier320ms)

        UserDefaults.standard.set(ParakeetUnifiedTier.tier2080ms.rawValue, forKey: key)
        XCTAssertEqual(ParakeetUnifiedTier.active, .tier2080ms)
    }
}
