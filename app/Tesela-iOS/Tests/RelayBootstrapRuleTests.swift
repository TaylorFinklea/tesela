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

    // MARK: Mid-run bootstrap gate — stranded behind compaction

    func testMidRunBootstrapWhenBehindWatermarkEvenWithEmptyPoll() {
        // The bug: poll returns applied == 0 (the ops we need were GC'd, so
        // the tail is empty) yet the relay's watermark is past our cursor.
        // An empty poll must NOT be read as "caught up" — bootstrap.
        XCTAssertTrue(
            RelayTicker.shouldBootstrapMidRun(applied: 0, compactionSeq: 1001, cursor: 1)
        )
        // Behind is behind regardless of how many envelopes applied this tick.
        XCTAssertTrue(
            RelayTicker.shouldBootstrapMidRun(applied: 5, compactionSeq: 500, cursor: 42)
        )
    }

    func testMidRunBootstrapSkippedWhenCaughtUp() {
        // Cursor at/past the watermark → the tail poll covers everything,
        // no mid-run bootstrap (applied doesn't change the answer).
        XCTAssertFalse(
            RelayTicker.shouldBootstrapMidRun(applied: 0, compactionSeq: 42, cursor: 42)
        )
        XCTAssertFalse(
            RelayTicker.shouldBootstrapMidRun(applied: 3, compactionSeq: 42, cursor: 100)
        )
        // No compaction yet (watermark 0, e.g. an older relay surfaces 0) →
        // never bootstrap.
        XCTAssertFalse(
            RelayTicker.shouldBootstrapMidRun(applied: 0, compactionSeq: 0, cursor: 0)
        )
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

    // MARK: Builtin-views seed gate (adversarial review, 2026-06-10)

    func testViewsSeedRunsImmediatelyWithoutPairing() {
        // No pairing → no group to receive a registry from; the device
        // seeds its Inbox right away (mirrors a relay-less server).
        XCTAssertTrue(
            RelayTicker.shouldSeedBuiltinViews(hasPairing: false, bootstrapCompleted: false)
        )
        XCTAssertTrue(
            RelayTicker.shouldSeedBuiltinViews(hasPairing: false, bootstrapCompleted: true)
        )
    }

    func testViewsSeedDefersUntilBootstrapWhenPaired() {
        // A paired fresh install must let the snapshot bootstrap land the
        // group's registry (possibly a user-edited Inbox) BEFORE seeding —
        // the engine seed then no-ops instead of authoring a default entry.
        XCTAssertFalse(
            RelayTicker.shouldSeedBuiltinViews(hasPairing: true, bootstrapCompleted: false)
        )
        XCTAssertTrue(
            RelayTicker.shouldSeedBuiltinViews(hasPairing: true, bootstrapCompleted: true)
        )
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
