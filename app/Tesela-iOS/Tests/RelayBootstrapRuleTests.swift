import XCTest
@testable import Tesela

/// Locks the snapshot-bootstrap cursor rule (audit A4, commit ddd8def)
/// and the date→daily-slug derivation the `.relay` write path falls back
/// to before the first materialization (commit 182210d).
@MainActor
final class RelayBootstrapRuleTests: XCTestCase {

    // MARK: Bootstrap gate — when does the snapshot bootstrap run?

    func testBootstrapRunsWhenWatermarkIsPastCursor() {
        // Fresh identity (cursor 0) with any compaction → bootstrap.
        XCTAssertTrue(RelayTicker.shouldRunSnapshotBootstrap(compactionSeq: 1, inboundCursorSeq: 0))
        XCTAssertTrue(RelayTicker.shouldRunSnapshotBootstrap(compactionSeq: 500, inboundCursorSeq: 42))
    }

    func testBootstrapSkippedWhenCursorCoversWatermark() {
        // Cursor at/past the GC watermark → the tail poll covers everything.
        XCTAssertFalse(RelayTicker.shouldRunSnapshotBootstrap(compactionSeq: 42, inboundCursorSeq: 42))
        XCTAssertFalse(RelayTicker.shouldRunSnapshotBootstrap(compactionSeq: 42, inboundCursorSeq: 100))
        // No compaction yet (watermark 0) → never bootstrap.
        XCTAssertFalse(RelayTicker.shouldRunSnapshotBootstrap(compactionSeq: 0, inboundCursorSeq: 0))
    }

    // MARK: Cursor jump — all-or-nothing imports

    func testCursorJumpsOnlyWhenEveryImportSucceeded() {
        XCTAssertTrue(RelayTicker.shouldJumpBootstrapCursor(failedImports: 0))
    }

    func testCursorHoldsOnAnyFailedImport() {
        // Jumping past a failed import would skip that note PERMANENTLY:
        // the covered ops are GC'd and the watermark guard makes every
        // future bootstrap a no-op. One failure → hold and retry.
        XCTAssertFalse(RelayTicker.shouldJumpBootstrapCursor(failedImports: 1))
        XCTAssertFalse(RelayTicker.shouldJumpBootstrapCursor(failedImports: 17))
    }

    // MARK: Daily slug derivation (date → yyyy-MM-dd)

    private func date(_ y: Int, _ m: Int, _ d: Int) -> Date {
        var c = DateComponents()
        c.year = y; c.month = m; c.day = d; c.hour = 12
        return Calendar.current.date(from: c)!
    }

    func testDailySlugIsZeroPaddedIsoDate() {
        XCTAssertEqual(MockMosaicService.dailySlug(for: date(2026, 6, 9)), "2026-06-09")
        XCTAssertEqual(MockMosaicService.dailySlug(for: date(2026, 1, 5)), "2026-01-05")
        XCTAssertEqual(MockMosaicService.dailySlug(for: date(2026, 12, 31)), "2026-12-31")
    }

    func testTodayDailySlugDerivesFromTodayDate() {
        // The `.relay` splice fallback + the explicit first-connect
        // bootstrap both key off this slug; it must equal today's date
        // formatted yyyy-MM-dd.
        let mosaic = MockMosaicService()
        XCTAssertEqual(mosaic.todayDailySlug, MockMosaicService.dailySlug(for: mosaic.todayDate))
    }
}
