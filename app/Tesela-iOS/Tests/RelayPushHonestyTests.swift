import XCTest
@testable import Tesela

/// Locks the "last-successful-push age" honesty fix (tesela-4mc): Settings
/// → Sync must only advance its push-age clock on a tick that actually
/// delivered ops, never on an empty poll or a failed batch — otherwise the
/// displayed age silently lies about whether iPhone's edits are reaching
/// the relay.
@MainActor
final class RelayPushHonestyTests: XCTestCase {

    func testEmptyPollIsNotASuccessfulPush() {
        XCTAssertFalse(RelayTicker.isSuccessfulPush(opsSent: 0, batchesFailed: 0))
    }

    func testFailedBatchIsNotASuccessfulPushEvenWithOpsSent() {
        // opsSent can be non-zero on a partially-failed tick (some batches
        // landed, one didn't) — that's still not "successful" for display.
        XCTAssertFalse(RelayTicker.isSuccessfulPush(opsSent: 3, batchesFailed: 1))
    }

    func testOpsDeliveredWithNoFailuresIsASuccessfulPush() {
        XCTAssertTrue(RelayTicker.isSuccessfulPush(opsSent: 1, batchesFailed: 0))
    }
}
