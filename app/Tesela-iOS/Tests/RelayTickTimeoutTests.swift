import XCTest
@testable import Tesela

/// Regression coverage for tesela-96y: the iPad in-memory sync wedge — the
/// relay poll loop stopped applying inbound changes entirely mid-session,
/// even a manual pull-to-refresh showed nothing new, and only force-quitting
/// the app restored sync.
///
/// Root cause (see `RelayTicker.tickOnce`'s doc comment for the full
/// evidence trail): the tick loop had NO bound on how long a single tick's
/// engine work could take, and no protection against a `wake()`-triggered
/// loop restart racing an in-flight tick — either can leave the loop
/// permanently stuck or its shared `@Published`/cursor state silently
/// corrupted by a stale result winning a race, with `isRunning` still
/// reading true and no error surfaced. Both hazards get WORSE specifically
/// under heavy sync (bigger batches ⇒ slower ticks ⇒ wider windows), which
/// matches the reported trigger.
///
/// These tests exercise the two extracted, FFI-independent primitives the
/// fix is built on — `raceAgainstTimeout` (a slow tick gets abandoned
/// promptly instead of blocking forever) and `shouldCommitTick` (a
/// superseded tick's result is discarded, never applied over fresher
/// state) — without needing a live relay/engine.
@MainActor
final class RelayTickTimeoutTests: XCTestCase {

    func testRaceReturnsTrueWhenWorkFinishesBeforeTimeout() async {
        let finishedInTime = await RelayTicker.raceAgainstTimeout(seconds: 2) {
            try? await Task.sleep(nanoseconds: 20_000_000) // 20ms — well under the 2s ceiling
        }
        XCTAssertTrue(finishedInTime, "fast work must win the race cleanly")
    }

    /// The core regression: a `work` closure that never returns (mirrors a
    /// wedged engine-apply call with no timeout of its own) must NOT block
    /// the caller forever — `raceAgainstTimeout` must return `false`
    /// promptly, at (approximately) the requested ceiling, not after
    /// `work` eventually finishes (it never does, here).
    func testRaceAbandonsWorkThatNeverFinishes() async {
        let start = Date()
        let finishedInTime = await RelayTicker.raceAgainstTimeout(seconds: 1) {
            // Simulates the exact hazard: an unbounded await with no
            // internal timeout of its own (a stuck FFI/engine call).
            try? await Task.sleep(nanoseconds: 3_600_000_000_000) // 1 hour
        }
        let elapsed = Date().timeIntervalSince(start)
        XCTAssertFalse(finishedInTime, "a work item that never finishes must be treated as timed out")
        XCTAssertLessThan(
            elapsed, 2.5,
            "the caller must be released at ~the timeout ceiling, not block until `work` eventually finishes"
        )
    }

    func testRaceAbandonsSlowWorkEvenThoughItWouldEventuallyFinish() async {
        let start = Date()
        let finishedInTime = await RelayTicker.raceAgainstTimeout(seconds: 1) {
            try? await Task.sleep(nanoseconds: 3_000_000_000) // 3s — would finish, just not in time
        }
        let elapsed = Date().timeIntervalSince(start)
        XCTAssertFalse(finishedInTime)
        XCTAssertLessThan(elapsed, 2.0, "the timeout must win the race, not wait out the slow work")
    }

    // MARK: - shouldCommitTick (pure)

    func testShouldCommitTickWhenStillCurrentGeneration() {
        XCTAssertTrue(RelayTicker.shouldCommitTick(issuedGeneration: 5, currentGeneration: 5))
    }

    func testShouldNotCommitTickWhenSupersededByNewerGeneration() {
        // A `wake()`-triggered loop restart (or this tick's own timeout
        // handler) bumped the generation counter past the one this tick
        // was issued under — its result must be discarded.
        XCTAssertFalse(RelayTicker.shouldCommitTick(issuedGeneration: 5, currentGeneration: 6))
    }

    func testShouldNotCommitTickForAnOlderGenerationEvenIfMuchOlder() {
        XCTAssertFalse(RelayTicker.shouldCommitTick(issuedGeneration: 1, currentGeneration: 42))
    }
}
