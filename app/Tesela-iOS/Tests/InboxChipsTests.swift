import XCTest
@testable import Tesela

/// Port of `web/tests/unit/inbox-chips.test.mjs` — the chip registry +
/// `chipsFromDsl` / `dslFromChips` round-trip must behave identically
/// on iOS so a saved Inbox filter authored on web parses to the same
/// chip state on iOS (and vice-versa).
@MainActor
final class InboxChipsTests: XCTestCase {

    // MARK: - chipsFromDsl

    func testChipsFromDsl_parsesDefaultInboxDsl() {
        let dsl = "kind:block -has:status -is:heading -on:daily-page -on:system-pages"
        let state = chipsFromDsl(dsl)
        XCTAssertEqual(state.active["untriaged"], true)
        XCTAssertEqual(state.active["notHeading"], true)
        XCTAssertEqual(state.active["notDailyPage"], true)
        XCTAssertEqual(state.active["notSystemPages"], true)
        XCTAssertEqual(state.active["hasScheduled"], false)
        XCTAssertTrue(state.unknownClauses.isEmpty)
    }

    func testChipsFromDsl_kindBlockIsImplicit() {
        let state = chipsFromDsl("kind:block")
        XCTAssertTrue(state.unknownClauses.isEmpty)
    }

    func testChipsFromDsl_preservesUnknownClausesVerbatim() {
        let state = chipsFromDsl("kind:block -has:status text:urgent priority:>=3")
        XCTAssertEqual(state.active["untriaged"], true)
        XCTAssertEqual(state.unknownClauses.sorted(), ["priority:>=3", "text:urgent"])
    }

    func testChipsFromDsl_tagInExtractsActiveTypes() {
        let state = chipsFromDsl("kind:block tag-in:Task,Issue,Domain")
        XCTAssertEqual(state.activeTypes, ["Task", "Issue", "Domain"])
        XCTAssertTrue(state.unknownClauses.isEmpty)
    }

    func testChipsFromDsl_pageAndBlockExclusionsExtractToHiddenLists() {
        let state = chipsFromDsl("kind:block -page:projects -block:abc:5")
        XCTAssertEqual(state.hiddenPages, ["projects"])
        XCTAssertEqual(state.hiddenBlocks, ["abc:5"])
        XCTAssertTrue(state.unknownClauses.isEmpty)
    }

    // MARK: - dslFromChips

    func testDslFromChips_emitsKindBlockAndActiveChipClauses() {
        var state = ChipState.empty()
        state.active["untriaged"] = true
        state.active["notHeading"] = true
        let dsl = dslFromChips(state)
        XCTAssertTrue(dsl.contains("kind:block"))
        XCTAssertTrue(dsl.contains("-has:status"))
        XCTAssertTrue(dsl.contains("-is:heading"))
    }

    func testDslFromChips_emitsTagInForActiveTypes() {
        var state = ChipState.empty()
        state.activeTypes = ["Task", "Issue"]
        XCTAssertTrue(dslFromChips(state).contains("tag-in:Task,Issue"))
    }

    func testDslFromChips_emitsExclusionsForHiddenPagesAndBlocks() {
        var state = ChipState.empty()
        state.hiddenPages = ["projects", "scratch"]
        state.hiddenBlocks = ["abc:5"]
        let dsl = dslFromChips(state)
        XCTAssertTrue(dsl.contains("-page:projects"))
        XCTAssertTrue(dsl.contains("-page:scratch"))
        XCTAssertTrue(dsl.contains("-block:abc:5"))
    }

    func testDslFromChips_preservesUnknownClauses() {
        var state = ChipState.empty()
        state.unknownClauses = ["text:urgent", "priority:>=3"]
        let dsl = dslFromChips(state)
        XCTAssertTrue(dsl.contains("text:urgent"))
        XCTAssertTrue(dsl.contains("priority:>=3"))
    }

    // MARK: - Round-trip

    func testRoundTrip_defaultDslPreserved() {
        let dsl = "kind:block -has:status -is:heading -on:daily-page -on:system-pages"
        let state = chipsFromDsl(dsl)
        let back = dslFromChips(state)
        // Order of clauses is registry-driven, so compare token sets.
        let aTokens = Set(dsl.split(whereSeparator: { $0.isWhitespace }).map(String.init))
        let bTokens = Set(back.split(whereSeparator: { $0.isWhitespace }).map(String.init))
        XCTAssertEqual(aTokens, bTokens)
    }

    func testRoundTrip_preservesNewJqlClausesAsUnknown() {
        // The new JQL grammar (`status != done`, `type IN (…)`,
        // `scheduled IS NULL`, `BETWEEN`, `LIKE`, `ORDER BY`) doesn't
        // have chip shapes today; the round-trip must preserve those
        // clauses verbatim as unknownClauses so chip-only edits don't
        // drop them.
        //
        // Whitespace tokenization splits these into multiple tokens,
        // which all end up as unknownClauses tokens. The user's raw
        // edit survives because dslFromChips writes them back as
        // whitespace-joined parts.
        let dsl = "kind:block status != done"
        let state = chipsFromDsl(dsl)
        XCTAssertEqual(state.unknownClauses.sorted(), ["!=", "done", "status"])
        let back = dslFromChips(state)
        XCTAssertTrue(back.contains("status"))
        XCTAssertTrue(back.contains("!="))
        XCTAssertTrue(back.contains("done"))
    }

    // MARK: - defaultInboxDsl

    func testDefaultInboxDsl_matchesWeb() {
        // Verifies parity with `web/src/lib/ambients/inbox/chips.ts ::
        // defaultInboxDsl()` — every chip with `defaultOn: true`
        // contributes its clauses; the implicit `kind:block` baseline
        // is always present.
        let dsl = defaultInboxDsl()
        XCTAssertTrue(dsl.contains("kind:block"))
        XCTAssertTrue(dsl.contains("-has:status"))
        XCTAssertTrue(dsl.contains("-is:heading"))
        XCTAssertTrue(dsl.contains("-on:daily-page"))
        XCTAssertTrue(dsl.contains("-on:system-pages"))
        // off-by-default chips must NOT appear
        XCTAssertFalse(dsl.contains("has:scheduled"))
        XCTAssertFalse(dsl.contains("has:deadline"))
        XCTAssertFalse(dsl.contains("-has:tag"))
    }
}
