import XCTest
@testable import Tesela

/// Tests for `LocalQueryEngine` — the `.relay` local mirrors of the
/// server's `/agenda` (`agenda_blocks`, sqlite.rs) and `/search/query`
/// (`block_matches`, query.rs) semantics. Blocks are produced by the
/// real service parser (`testableParseBlocks`) so the fixtures exercise
/// the exact same parse → filter path production uses.
@MainActor
final class LocalQueryEngineTests: XCTestCase {

    private let today = "2026-06-10"

    private func blocks(_ body: String, noteId: String) -> [Block] {
        MockMosaicService().testableParseBlocks(from: body, noteId: noteId)
    }

    /// The canonical first-run Inbox DSL — same string `defaultInboxDsl()`
    /// derives from the chip registry and the web seeds into the saved
    /// Query note: untriaged, no headings, no daily pages, no system pages.
    private var defaultDsl: LocalQueryEngine.SimpleDsl {
        LocalQueryEngine.parseSimpleDsl(defaultInboxDsl())
    }

    // MARK: - Default DSL shape

    func testDefaultInboxDslIsTheUntriagedQuery() {
        XCTAssertEqual(
            defaultInboxDsl(),
            "kind:block -has:status -is:heading -on:daily-page -on:system-pages"
        )
        let parsed = defaultDsl
        XCTAssertEqual(parsed.kind, .block)
        XCTAssertEqual(parsed.clauses.count, 4)
        XCTAssertTrue(parsed.clauses.allSatisfy { $0.negated })
    }

    // MARK: - Inbox filtering (mirrors block_matches)

    func testUntriagedProseBlockLandsInInbox() {
        let bs = blocks("- Call the plumber about the leak", noteId: "house-projects")
        let items = LocalQueryEngine.queryItems(
            blocks: bs, noteId: "house-projects", noteTitle: "House projects",
            pageNoteType: nil, dsl: defaultDsl
        )
        XCTAssertEqual(items.count, 1)
        XCTAssertEqual(items[0].text, "Call the plumber about the leak")
        XCTAssertEqual(items[0].block_id, "house-projects:0")
        XCTAssertEqual(items[0].kind, .block)
        XCTAssertEqual(items[0].parent_breadcrumb, ["House projects"])
    }

    /// `-has:status` — a block that HAS any `status::` is triaged and
    /// stays out of the untriaged Inbox (todo and done alike).
    func testStatusBlocksAreExcludedFromUntriagedInbox() {
        let body = """
        - Ship the relay fix
          status:: todo
        - Old chore
          status:: done
        - Untriaged thought
        """
        let bs = blocks(body, noteId: "work")
        let items = LocalQueryEngine.queryItems(
            blocks: bs, noteId: "work", noteTitle: "Work",
            pageNoteType: nil, dsl: defaultDsl
        )
        XCTAssertEqual(items.map(\.text), ["Untriaged thought"])
    }

    /// `-is:heading` — markdown headings are dividers, not triage items.
    func testHeadingBlocksAreExcluded() {
        let body = """
        - ## Reference section
        - actual content under it
        """
        let bs = blocks(body, noteId: "notes-page")
        let items = LocalQueryEngine.queryItems(
            blocks: bs, noteId: "notes-page", noteTitle: "Notes",
            pageNoteType: nil, dsl: defaultDsl
        )
        XCTAssertEqual(items.map(\.text), ["actual content under it"])
    }

    /// `-on:daily-page` — blocks on `YYYY-MM-DD` notes are journal
    /// captures, not triage items.
    func testDailyPageBlocksAreExcluded() {
        let bs = blocks("- jotted this in the daily", noteId: "2026-06-10")
        let items = LocalQueryEngine.queryItems(
            blocks: bs, noteId: "2026-06-10", noteTitle: "2026-06-10",
            pageNoteType: nil, dsl: defaultDsl
        )
        XCTAssertTrue(items.isEmpty)
    }

    /// `-on:system-pages` — blocks on Tag/Property/Query/Template pages
    /// stay out (e.g. the saved Inbox filter's own `query::` note).
    func testSystemPageBlocksAreExcluded() {
        let bs = blocks("- query:: kind:block", noteId: "inbox")
        let items = LocalQueryEngine.queryItems(
            blocks: bs, noteId: "inbox", noteTitle: "Inbox",
            pageNoteType: "Query", dsl: defaultDsl
        )
        XCTAssertTrue(items.isEmpty)
    }

    /// `has:scheduled` (the dates chip) — positive presence filter.
    func testHasScheduledChipFiltersToDatedBlocks() {
        let body = """
        - dated thing
          scheduled:: 2026-06-12
        - undated thing
        """
        let bs = blocks(body, noteId: "plans")
        let dsl = LocalQueryEngine.parseSimpleDsl("kind:block has:scheduled")
        let items = LocalQueryEngine.queryItems(
            blocks: bs, noteId: "plans", noteTitle: "Plans",
            pageNoteType: nil, dsl: dsl
        )
        XCTAssertEqual(items.map(\.text), ["dated thing"])
    }

    /// `tag-in:` matches own AND inherited tags (the ancestor chain),
    /// mirroring the Rust matcher's inherited_tags handling.
    func testTagInMatchesInheritedTags() {
        let body = """
        - parent #project
          - child task under it
        """
        let bs = blocks(body, noteId: "p1")
        let dsl = LocalQueryEngine.parseSimpleDsl("kind:block tag-in:project,errand")
        let items = LocalQueryEngine.queryItems(
            blocks: bs, noteId: "p1", noteTitle: "P1",
            pageNoteType: nil, dsl: dsl
        )
        XCTAssertEqual(items.count, 2, "parent (own tag) + child (inherited) should both match")
    }

    /// Bare `key:value` falls through to property equality; `-key:value`
    /// matches when the property is missing ("missing != value").
    func testPropertyEqualityFallback() {
        let body = """
        - doing thing
          status:: doing
        - todo thing
          status:: todo
        - plain thing
        """
        let bs = blocks(body, noteId: "n")
        let eq = LocalQueryEngine.parseSimpleDsl("kind:block status:doing")
        XCTAssertEqual(
            LocalQueryEngine.queryItems(blocks: bs, noteId: "n", noteTitle: "N", pageNoteType: nil, dsl: eq)
                .map(\.text),
            ["doing thing"]
        )
        let ne = LocalQueryEngine.parseSimpleDsl("kind:block -status:doing")
        XCTAssertEqual(
            LocalQueryEngine.queryItems(blocks: bs, noteId: "n", noteTitle: "N", pageNoteType: nil, dsl: ne)
                .map(\.text),
            ["todo thing", "plain thing"]
        )
    }

    /// `kind:page` queries aren't served locally — empty, never wrong.
    func testPageKindQueriesParseAsPage() {
        XCTAssertEqual(LocalQueryEngine.parseSimpleDsl("kind:page tag:project").kind, .page)
    }

    // MARK: - Agenda (mirrors agenda_blocks)

    func testScheduledTodayTaskLandsInTodayBucket() {
        let body = """
        - file the expense report
          scheduled:: 2026-06-10
          status:: todo
        """
        let bs = blocks(body, noteId: "work")
        let rows = LocalQueryEngine.agendaRows(
            blocks: bs, from: "2026-03-12", to: "2026-08-09",
            includeDone: false, today: today
        )
        XCTAssertEqual(rows.count, 1)
        let row = rows[0]
        XCTAssertEqual(row.occurrence_date, "2026-06-10")
        XCTAssertFalse(row.overdue, "scheduled today is not overdue → lands in the Today group")
        XCTAssertTrue(row.is_anchor)
        XCTAssertEqual(row.kind, .task)
        XCTAssertEqual(row.field, .scheduled)
        XCTAssertEqual(row.status, "todo")
        XCTAssertEqual(row.block_id, "work:0")
        XCTAssertEqual(row.text, "file the expense report")
    }

    /// `include_done: false` drops `status:: done` rows; true keeps them.
    func testDoneRowsExcludedUnlessIncludeDone() {
        let body = """
        - already finished
          scheduled:: 2026-06-10
          status:: done
        """
        let bs = blocks(body, noteId: "work")
        let hidden = LocalQueryEngine.agendaRows(
            blocks: bs, from: "2026-06-01", to: "2026-06-30",
            includeDone: false, today: today
        )
        XCTAssertTrue(hidden.isEmpty)
        let shown = LocalQueryEngine.agendaRows(
            blocks: bs, from: "2026-06-01", to: "2026-06-30",
            includeDone: true, today: today
        )
        XCTAssertEqual(shown.count, 1)
        XCTAssertEqual(shown[0].status, "done")
    }

    /// Anchor prefers `scheduled`; `deadline` only when scheduled absent.
    func testDeadlineOnlyBlockAnchorsOnDeadline() {
        let body = """
        - taxes due
          deadline:: 2026-06-15
          status:: todo
        """
        let bs = blocks(body, noteId: "life")
        let rows = LocalQueryEngine.agendaRows(
            blocks: bs, from: "2026-06-01", to: "2026-06-30",
            includeDone: false, today: today
        )
        XCTAssertEqual(rows.count, 1)
        XCTAssertEqual(rows[0].field, .deadline)
        XCTAssertEqual(rows[0].occurrence_date, "2026-06-15")
    }

    func testPastScheduledIsOverdue() {
        let body = """
        - missed it
          scheduled:: 2026-06-01
          status:: todo
        """
        let bs = blocks(body, noteId: "work")
        let rows = LocalQueryEngine.agendaRows(
            blocks: bs, from: "2026-05-01", to: "2026-06-30",
            includeDone: false, today: today
        )
        XCTAssertEqual(rows.count, 1)
        XCTAssertTrue(rows[0].overdue)
    }

    /// Non-task prose without dated properties contributes nothing.
    func testUndatedProseProducesNoAgendaRows() {
        let bs = blocks("- just a thought\n- another note", noteId: "n")
        let rows = LocalQueryEngine.agendaRows(
            blocks: bs, from: "2020-01-01", to: "2030-01-01",
            includeDone: true, today: today
        )
        XCTAssertTrue(rows.isEmpty)
    }

    /// A dated block with neither `status::` nor `tags:: Task` is an
    /// event, mirroring the server's task/event split.
    func testDatedBlockWithoutStatusIsEvent() {
        let body = """
        - team offsite
          scheduled:: 2026-06-20
        """
        let bs = blocks(body, noteId: "cal")
        let rows = LocalQueryEngine.agendaRows(
            blocks: bs, from: "2026-06-01", to: "2026-06-30",
            includeDone: false, today: today
        )
        XCTAssertEqual(rows.count, 1)
        XCTAssertEqual(rows[0].kind, .event)
        XCTAssertNil(rows[0].status)
    }

    /// Mirrors the server's `post_agenda_returns_rows_in_window` test:
    /// weekly anchor 2026-05-22 in window 2026-05-22..2026-06-12 emits
    /// the anchor + 3 projections (05-29, 06-05, 06-12).
    func testWeeklyRecurrenceProjectsForward() {
        let body = """
        - weekly review
          scheduled:: 2026-05-22
          recurring:: weekly
          tags:: Task
          status:: todo
        """
        let bs = blocks(body, noteId: "agenda-weekly")
        let rows = LocalQueryEngine.agendaRows(
            blocks: bs, from: "2026-05-22", to: "2026-06-12",
            includeDone: false, today: today
        )
        XCTAssertEqual(rows.count, 4, "anchor + 3 weekly projections, got \(rows.map(\.occurrence_date))")
        XCTAssertEqual(rows[0].occurrence_date, "2026-05-22")
        XCTAssertTrue(rows[0].is_anchor)
        XCTAssertEqual(
            rows.map(\.occurrence_date),
            ["2026-05-22", "2026-05-29", "2026-06-05", "2026-06-12"]
        )
        XCTAssertTrue(rows.dropFirst().allSatisfy { !$0.is_anchor })
    }

    /// `scheduled:: 2026-06-10 09:30` carries the HH:MM through;
    /// wiki-wrapped `[[2026-06-10]]` still parses.
    func testDatedValueTimeAndWikiWrapForms() {
        let body = """
        - standup
          scheduled:: 2026-06-10 09:30
          status:: todo
        - legacy form
          scheduled:: [[2026-06-11]]
          status:: todo
        """
        let bs = blocks(body, noteId: "cal")
        var rows = LocalQueryEngine.agendaRows(
            blocks: bs, from: "2026-06-01", to: "2026-06-30",
            includeDone: false, today: today
        )
        LocalQueryEngine.sortAgendaRows(&rows)
        XCTAssertEqual(rows.count, 2)
        XCTAssertEqual(rows[0].occurrence_time, "09:30")
        XCTAssertEqual(rows[1].occurrence_date, "2026-06-11")
        XCTAssertNil(rows[1].occurrence_time)
    }

    /// Canonical sort: (occurrence_date, occurrence_time, block_id) with
    /// nil time before any time, mirroring Rust Option ordering.
    func testAgendaSortOrder() {
        let body = """
        - later that day
          scheduled:: 2026-06-10 15:00
          status:: todo
        - all-day same date
          scheduled:: 2026-06-10
          status:: todo
        - earlier date
          scheduled:: 2026-06-09
          status:: todo
        """
        let bs = blocks(body, noteId: "s")
        var rows = LocalQueryEngine.agendaRows(
            blocks: bs, from: "2026-06-01", to: "2026-06-30",
            includeDone: false, today: today
        )
        LocalQueryEngine.sortAgendaRows(&rows)
        XCTAssertEqual(rows.map(\.text), ["earlier date", "all-day same date", "later that day"])
    }

    // MARK: - Recurrence vocabulary (mirrors recurrence.rs)

    func testRecurrenceParseVocabulary() {
        XCTAssertEqual(
            LocalQueryEngine.parseRecurrence("every 2 weeks"),
            .init(freq: .weekly, interval: 2, byWeekday: [], end: nil)
        )
        XCTAssertEqual(
            LocalQueryEngine.parseRecurrence("Weekdays"),
            .init(freq: .weekly, interval: 1, byWeekday: [1, 2, 3, 4, 5], end: nil)
        )
        XCTAssertEqual(
            LocalQueryEngine.parseRecurrence("every mon, wed, fri"),
            .init(freq: .weekly, interval: 1, byWeekday: [1, 3, 5], end: nil)
        )
        XCTAssertEqual(
            LocalQueryEngine.parseRecurrence("daily until 2026-07-01"),
            .init(freq: .daily, interval: 1, byWeekday: [], end: .until("2026-07-01"))
        )
        XCTAssertEqual(
            LocalQueryEngine.parseRecurrence("weekly count 3"),
            .init(freq: .weekly, interval: 1, byWeekday: [], end: .count(3))
        )
        XCTAssertNil(LocalQueryEngine.parseRecurrence("whenever I feel like it"))
    }

    func testRecurrenceAdvanceRespectsEnds() {
        let counted = LocalQueryEngine.Recurrence(
            freq: .weekly, interval: 1, byWeekday: [], end: .count(2)
        )
        // 2 total occurrences: completing the 1st yields a 2nd…
        XCTAssertEqual(LocalQueryEngine.advance(counted, current: "2026-06-01", doneSoFar: 0), "2026-06-08")
        // …completing the 2nd exhausts the series.
        XCTAssertNil(LocalQueryEngine.advance(counted, current: "2026-06-08", doneSoFar: 1))

        let until = LocalQueryEngine.Recurrence(
            freq: .daily, interval: 1, byWeekday: [], end: .until("2026-06-02")
        )
        XCTAssertEqual(LocalQueryEngine.advance(until, current: "2026-06-01", doneSoFar: 0), "2026-06-02")
        XCTAssertNil(LocalQueryEngine.advance(until, current: "2026-06-02", doneSoFar: 1))
    }

    func testMonthlyAdvanceClampsDayOfMonth() {
        let monthly = LocalQueryEngine.Recurrence.simple(.monthly, 1)
        XCTAssertEqual(LocalQueryEngine.nextAfter(monthly, anchor: "2026-01-31"), "2026-02-28")
    }

    /// BYDAY stepping: from a Friday, `weekdays` advances to Monday.
    func testByWeekdayAdvanceSkipsWeekend() {
        let weekdays = LocalQueryEngine.Recurrence(
            freq: .weekly, interval: 1, byWeekday: [1, 2, 3, 4, 5], end: nil
        )
        // 2026-06-05 is a Friday → next weekday occurrence is Monday 06-08.
        XCTAssertEqual(LocalQueryEngine.nextAfter(weekdays, anchor: "2026-06-05"), "2026-06-08")
    }

    // MARK: - Predicate helpers

    func testIsDailyNoteId() {
        XCTAssertTrue(LocalQueryEngine.isDailyNoteId("2026-06-10"))
        XCTAssertFalse(LocalQueryEngine.isDailyNoteId("house-projects"))
        XCTAssertFalse(LocalQueryEngine.isDailyNoteId("2026-6-10"))
    }

    func testIsHeadingText() {
        XCTAssertTrue(LocalQueryEngine.isHeadingText("## Section"))
        XCTAssertTrue(LocalQueryEngine.isHeadingText("###### Deep"))
        XCTAssertFalse(LocalQueryEngine.isHeadingText("#hashtag not heading"))
        XCTAssertFalse(LocalQueryEngine.isHeadingText("####### seven"))
        XCTAssertFalse(LocalQueryEngine.isHeadingText("plain text"))
        XCTAssertFalse(LocalQueryEngine.isHeadingText("###"))
    }

    func testExtractIsoDateForms() {
        XCTAssertEqual(LocalQueryEngine.extractIsoDate("2026-06-10"), "2026-06-10")
        XCTAssertEqual(LocalQueryEngine.extractIsoDate("[[2026-06-10]]"), "2026-06-10")
        XCTAssertEqual(LocalQueryEngine.extractIsoDate("2026-06-10 09:30"), "2026-06-10")
        XCTAssertNil(LocalQueryEngine.extractIsoDate("next tuesday"))
    }
}
