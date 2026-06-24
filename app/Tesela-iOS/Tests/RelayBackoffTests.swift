import XCTest
@testable import Tesela

/// Pins the relay tick backoff so a run of transient relay/network
/// failures can never park the sync loop for more than a minute.
///
/// Regression guard for the 2026-06-24 liveness bug: the sleep was
/// `tickIntervalSeconds * (1 << min(consecutiveErrors, 12))` =
/// `2 * 2^12 ≈ 8192s ≈ 2.3h`, so a handful of failed ticks stranded
/// iOS edits for hours (looked like data loss). All the surrounding
/// comments claimed a ~60s cap; the math never delivered it. See
/// decisions.md 2026-06-24.
@MainActor
final class RelayBackoffTests: XCTestCase {

    func testBaseCadenceWithNoErrors() {
        XCTAssertEqual(RelayTicker.backoffSleepSeconds(consecutiveErrors: 0), 2)
    }

    func testBackoffDoublesEarly() {
        XCTAssertEqual(RelayTicker.backoffSleepSeconds(consecutiveErrors: 1), 4)
        XCTAssertEqual(RelayTicker.backoffSleepSeconds(consecutiveErrors: 2), 8)
        XCTAssertEqual(RelayTicker.backoffSleepSeconds(consecutiveErrors: 3), 16)
    }

    func testBackoffIsHardCappedAtOneMinute() {
        // The bug: errors=12 yielded 8192s (~2.3h). The cap is on the
        // resulting SECONDS, not the shift exponent.
        XCTAssertLessThanOrEqual(RelayTicker.backoffSleepSeconds(consecutiveErrors: 12), 60)
    }

    func testBackoffNeverExceedsCapForAnyErrorCount() {
        for n: UInt32 in [5, 6, 12, 50, 1000, UInt32.max] {
            XCTAssertLessThanOrEqual(
                RelayTicker.backoffSleepSeconds(consecutiveErrors: n), 60,
                "errors=\(n) must stay capped — never park the loop for hours")
        }
    }

    func testBackoffIsMonotonicUpToCap() {
        var prev: UInt64 = 0
        for n: UInt32 in 0...10 {
            let s = RelayTicker.backoffSleepSeconds(consecutiveErrors: n)
            XCTAssertGreaterThanOrEqual(s, prev, "backoff must not decrease as errors climb")
            prev = s
        }
    }
}
