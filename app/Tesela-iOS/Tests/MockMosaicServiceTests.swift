import XCTest
@testable import Tesela

/// Tests for MockMosaicService internals that are testable without a live
/// server — specifically the `parseBlocks(from:noteId:)` line-number
/// tracking introduced for the `recur-bump` block_id fix.
@MainActor
final class MockMosaicServiceTests: XCTestCase {

    // MARK: - Block folding

    func testBlockFoldVisibleBlocksHideCollapsedDescendantsOnly() {
        let blocks = [
            Block(id: "root", kind: .note, text: "root", indent: 0),
            Block(id: "child-a", kind: .note, text: "child a", indent: 1),
            Block(id: "grandchild", kind: .note, text: "grandchild", indent: 2),
            Block(id: "child-b", kind: .note, text: "child b", indent: 1),
            Block(id: "sibling", kind: .note, text: "sibling", indent: 0),
        ]

        let visible = BlockFold.visibleBlocks(in: blocks, collapsed: ["root"])

        XCTAssertEqual(visible.map(\.id), ["root", "sibling"])
    }

    func testBlockFoldNestedCollapseResumesAtMatchingIndent() {
        let blocks = [
            Block(id: "root", kind: .note, text: "root", indent: 0),
            Block(id: "child-a", kind: .note, text: "child a", indent: 1),
            Block(id: "grandchild", kind: .note, text: "grandchild", indent: 2),
            Block(id: "child-b", kind: .note, text: "child b", indent: 1),
            Block(id: "sibling", kind: .note, text: "sibling", indent: 0),
        ]

        let visible = BlockFold.visibleBlocks(in: blocks, collapsed: ["child-a"])

        XCTAssertEqual(visible.map(\.id), ["root", "child-a", "child-b", "sibling"])
    }

    func testBlockFoldHasChildrenUsesUnderlyingBlockOrder() {
        let blocks = [
            Block(id: "root", kind: .note, text: "root", indent: 0),
            Block(id: "child", kind: .note, text: "child", indent: 1),
            Block(id: "sibling", kind: .note, text: "sibling", indent: 0),
        ]

        XCTAssertTrue(BlockFold.hasChildren(block: blocks[0], in: blocks))
        XCTAssertFalse(BlockFold.hasChildren(block: blocks[1], in: blocks))
        XCTAssertFalse(BlockFold.hasChildren(block: blocks[2], in: blocks))
    }

    // MARK: - Past daily editing

    func testEditPastDailyBlockUpdatesFeedEntryAndLoadedPage() async {
        let service = MockMosaicService()
        service.testableSetPastDailies([
            DailyEntry(
                id: "2026-06-08",
                blocks: [
                    Block(id: "old-root", kind: .note, text: "old root"),
                    Block(id: "old-child", kind: .note, text: "old child", indent: 1),
                ]
            )
        ])

        service.editPastDailyBlock(dayId: "2026-06-08", blockId: "old-child", text: "edited child #done")
        await service.testableWaitForPendingTasks()

        XCTAssertEqual(service.pastDailies[0].blocks[1].text, "edited child")
        XCTAssertEqual(service.pastDailies[0].blocks[1].rawText, "edited child")
        XCTAssertEqual(service.pastDailies[0].blocks[1].tags, ["#done"])
        XCTAssertEqual(service.loadedPageBlocks["2026-06-08"]?[1].text, "edited child")
        XCTAssertEqual(service.loadedPageBlocks["2026-06-08"]?[1].tags, ["#done"])
    }

    func testRelayRefreshUsesCurrentCalendarDayAfterServiceStayedAliveAcrossMidnight() async throws {
        let previousDay = "2099-06-11"
        let currentDay = "2099-06-12"
        try resetLocalDailyFixtures([previousDay, currentDay])
        defer { try? resetLocalDailyFixtures([previousDay, currentDay]) }
        try writeLocalDaily(id: previousDay, body: "- old launch-day block")
        try writeLocalDaily(id: currentDay, body: "- current calendar-day block")

        var now = date(2099, 6, 11)
        let service = MockMosaicService(now: { now })

        await service.refresh(from: .relay)
        XCTAssertEqual(service.todayDailySlug, previousDay)
        XCTAssertEqual(service.todayBlocks.map(\.text), ["old launch-day block"])

        now = date(2099, 6, 12)
        await service.refresh(from: .relay)

        XCTAssertEqual(service.todayDailySlug, currentDay)
        XCTAssertEqual(service.todayBlocks.map(\.text), ["current calendar-day block"])
        XCTAssertEqual(service.yesterdayBlocks.map(\.text), ["old launch-day block"])
    }

    func testRelayRefreshClearsStaleTodayBlocksWhenNewDayHasNoLocalDailyYet() async throws {
        let previousDay = "2099-06-13"
        let currentDay = "2099-06-14"
        try resetLocalDailyFixtures([previousDay, currentDay])
        defer { try? resetLocalDailyFixtures([previousDay, currentDay]) }
        try writeLocalDaily(id: previousDay, body: "- previous day block")

        var now = date(2099, 6, 13)
        let service = MockMosaicService(now: { now })

        await service.refresh(from: .relay)
        XCTAssertEqual(service.todayBlocks.map(\.text), ["previous day block"])

        now = date(2099, 6, 14)
        await service.refresh(from: .relay)

        XCTAssertEqual(service.todayDailySlug, currentDay)
        XCTAssertTrue(service.todayBlocks.isEmpty)
        XCTAssertEqual(service.yesterdayBlocks.map(\.text), ["previous day block"])
    }

    // MARK: - parseBlocks line-number tracking

    /// Verify that each Block's `lineNumber` records the 0-based index of
    /// the `- ` bullet line in the body string. This is the line number
    /// the server's `POST /blocks/recur-bump` route expects as the second
    /// component of `<noteId>:<line>`.
    func testParseBlocksLineNumbers() throws {
        // Build a three-block body. The blocks are at lines 0, 2, and 4;
        // lines 1 and 3 are property sub-lines.
        let body = """
        - First block
          status:: todo
        - Second block
          tags:: Task
        - Third block
        """

        let service = MockMosaicService()
        // Call through the internal helper via a fresh refresh so we can
        // inspect todayBlocks — or reach it directly via the internal
        // `@testable` access.
        let blocks = service.testableParseBlocks(from: body, noteId: "test-note")

        XCTAssertEqual(blocks.count, 3, "Expected 3 blocks")

        XCTAssertEqual(blocks[0].lineNumber, 0, "First block should be at line 0")
        XCTAssertEqual(blocks[0].noteId, "test-note")
        XCTAssertEqual(blocks[0].text, "First block")

        XCTAssertEqual(blocks[1].lineNumber, 2, "Second block should be at line 2")
        XCTAssertEqual(blocks[1].noteId, "test-note")
        XCTAssertEqual(blocks[1].text, "Second block")

        XCTAssertEqual(blocks[2].lineNumber, 4, "Third block should be at line 4")
        XCTAssertEqual(blocks[2].noteId, "test-note")
        XCTAssertEqual(blocks[2].text, "Third block")
    }

    /// Blocks that start at line 0 with no sub-lines should have lineNumber 0.
    func testParseBlocksSingleBlock() {
        let body = "- Only block"
        let service = MockMosaicService()
        let blocks = service.testableParseBlocks(from: body, noteId: "note-abc")
        XCTAssertEqual(blocks.count, 1)
        XCTAssertEqual(blocks[0].lineNumber, 0)
        XCTAssertEqual(blocks[0].noteId, "note-abc")
    }

    /// Empty body should yield no blocks.
    func testParseBlocksEmptyBody() {
        let service = MockMosaicService()
        let blocks = service.testableParseBlocks(from: "", noteId: "empty-note")
        XCTAssertTrue(blocks.isEmpty)
    }

    /// A block whose body spans multiple lines (continuation lines under
    /// the bullet) should keep every line in `rawText` so the iOS
    /// outliner renders all of them, matching the web client. `text`
    /// remains the first line only for previews/grep.
    ///
    /// Regression test: previously, continuation lines were silently
    /// dropped during parse, which both truncated display on iOS and
    /// caused permanent data loss on writeback (the dropped lines never
    /// made it back to disk after any subsequent edit).
    func testParseBlocksKeepsContinuationLinesInRawText() {
        let body = """
        - best for stable preferences and environment facts
          A realistic "value loop" for you
          1. You ask Hermes to do some annoying multi-step task.
          2. Hermes completes it.
        - next block
        """

        let service = MockMosaicService()
        let blocks = service.testableParseBlocks(from: body, noteId: "multi")

        XCTAssertEqual(blocks.count, 2)
        XCTAssertEqual(blocks[0].text, "best for stable preferences and environment facts")
        XCTAssertEqual(
            blocks[0].rawText,
            """
            best for stable preferences and environment facts
            A realistic "value loop" for you
            1. You ask Hermes to do some annoying multi-step task.
            2. Hermes completes it.
            """
        )
        XCTAssertEqual(blocks[1].text, "next block")
        XCTAssertEqual(blocks[1].rawText, "next block")
    }

    /// Round-trip a multi-line block through parse → render and verify
    /// every continuation line survives. The pre-fix writeback emitted
    /// only `- block.text`, so a multi-line block would collapse to one
    /// line on the first edit anywhere on the daily.
    ///
    /// We include canonical-UUID bid comments so the renderer's
    /// "emit bid suffix" branch produces deterministic output equal
    /// to the input.
    func testRenderBodyPreservesContinuationLines() {
        let body = """
        - first block <!-- bid:4BF3B0E3-BF14-4514-B47A-E8F763066756 -->
          continuation alpha
          continuation beta
        - second block <!-- bid:F4864AC3-2CF0-407B-8895-34548623E794 -->
        """

        let service = MockMosaicService()
        let blocks = service.testableParseBlocks(from: body, noteId: "round")
        let rendered = service.testableRenderBody(from: blocks)

        XCTAssertEqual(rendered, body)
    }

    /// `tags:: Issue` is the canonical block-tag form emitted by the
    /// desktop/web engine. iOS should surface it as a visible tag chip,
    /// keep it out of generic properties, and write it back as `tags::`
    /// rather than converting it to an inline `#Issue`.
    func testParseBlocksPromotesTagsPropertyToTagsAndPreservesCanonicalRender() {
        let body = """
        - Figure out finances <!-- bid:6AE83FC1-9EE9-4626-9EFE-58E0D83E7176 -->
          tags:: Issue
          IssueStatus::
        """

        let service = MockMosaicService()
        let blocks = service.testableParseBlocks(from: body, noteId: "2026-06-11")

        XCTAssertEqual(blocks.count, 1)
        XCTAssertEqual(blocks[0].text, "Figure out finances")
        XCTAssertEqual(blocks[0].tags, ["#Issue"])
        XCTAssertFalse(blocks[0].properties.contains { $0.key.lowercased() == "tags" })
        XCTAssertEqual(
            blocks[0].properties.first(where: { $0.key == "IssueStatus" })?.value,
            ""
        )
        XCTAssertEqual(service.testableRenderBody(from: blocks), body)
    }

    /// Blocks separated only by blank lines should get correct line numbers.
    func testParseBlocksWithBlankLines() {
        let body = """
        - Alpha

        - Beta

        - Gamma
        """
        let service = MockMosaicService()
        let blocks = service.testableParseBlocks(from: body, noteId: "gaps-note")
        XCTAssertEqual(blocks.count, 3)
        XCTAssertEqual(blocks[0].lineNumber, 0)
        XCTAssertEqual(blocks[1].lineNumber, 2)
        XCTAssertEqual(blocks[2].lineNumber, 4)
    }

    private func date(_ y: Int, _ m: Int, _ d: Int) -> Date {
        var components = DateComponents()
        components.year = y
        components.month = m
        components.day = d
        components.hour = 12
        components.minute = 0
        components.second = 0
        return Calendar.current.date(from: components)!
    }

    private func writeLocalDaily(id: String, body: String) throws {
        let notesDir = localFixtureNotesDir()
        try FileManager.default.createDirectory(at: notesDir, withIntermediateDirectories: true)
        let content = """
        ---
        title: \(id)
        tags: [daily]
        created: \(id)T00:00:00Z
        ---

        \(body)
        """
        try content.write(to: notesDir.appendingPathComponent("\(id).md"), atomically: true, encoding: .utf8)
    }

    private func resetLocalDailyFixtures(_ ids: [String]) throws {
        let notesDir = localFixtureNotesDir()
        for id in ids {
            let path = notesDir.appendingPathComponent("\(id).md")
            if FileManager.default.fileExists(atPath: path.path) {
                try FileManager.default.removeItem(at: path)
            }
        }
    }

    private func localFixtureNotesDir() -> URL {
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("sync-ios-mosaic")
            .appendingPathComponent("notes")
    }

    // MARK: - Placeholder-authoring gate (2026-06-10 product test)

    /// A daily writeback consisting ONLY of bare empty blocks for a daily
    /// that has never been materialized locally is the editable-row
    /// placeholder state, not user content — authoring it as the note's
    /// first synced state put a stray empty bullet ABOVE the peer's real
    /// content after the fresh-day union (iOS: [empty, dude, empty];
    /// desktop: [dude, empty]).
    func testPlaceholderOnlyFreshDailyIsSuppressed() {
        let placeholder = Block(id: "5E0A4E27-0A57-4D6B-9F6F-1B1F58A2D001", kind: .note, text: "")
        XCTAssertTrue(MockMosaicService.isBarePlaceholder(placeholder))
        XCTAssertTrue(
            MockMosaicService.shouldSuppressPlaceholderAuthoring(
                blocks: [placeholder],
                dailyFileExists: false
            )
        )
        // Vacuously-bare empty list on a fresh daily: nothing to author.
        XCTAssertTrue(
            MockMosaicService.shouldSuppressPlaceholderAuthoring(
                blocks: [],
                dailyFileExists: false
            )
        )
    }

    /// Once a local file exists, an all-bare state is a REAL edit (the
    /// user deleted the last contentful block) and must flow — the gate
    /// only applies to a never-persisted daily.
    func testAllBareStateOnExistingDailyStillPushes() {
        let placeholder = Block(id: "5E0A4E27-0A57-4D6B-9F6F-1B1F58A2D002", kind: .note, text: "")
        XCTAssertFalse(
            MockMosaicService.shouldSuppressPlaceholderAuthoring(
                blocks: [placeholder],
                dailyFileExists: true
            )
        )
    }

    /// Any real content — text, tags, properties, or task state — defeats
    /// the gate even on a fresh daily.
    func testContentfulFreshDailyIsAuthored() {
        let typed = Block(id: "5E0A4E27-0A57-4D6B-9F6F-1B1F58A2D003", kind: .note, text: "hello")
        let placeholder = Block(id: "5E0A4E27-0A57-4D6B-9F6F-1B1F58A2D004", kind: .note, text: "")
        XCTAssertFalse(MockMosaicService.isBarePlaceholder(typed))
        XCTAssertFalse(
            MockMosaicService.shouldSuppressPlaceholderAuthoring(
                blocks: [typed, placeholder],
                dailyFileExists: false
            )
        )
        // A bare-text TASK block is still task state (checkbox) — content.
        let task = Block(id: "5E0A4E27-0A57-4D6B-9F6F-1B1F58A2D005", kind: .task, text: "")
        XCTAssertFalse(MockMosaicService.isBarePlaceholder(task))
        // Tags / properties count as content too.
        let tagged = Block(
            id: "5E0A4E27-0A57-4D6B-9F6F-1B1F58A2D006",
            kind: .note,
            text: "",
            tags: ["#inbox"]
        )
        XCTAssertFalse(MockMosaicService.isBarePlaceholder(tagged))
    }

    // MARK: - Capture insertion (append at the bottom, 2026-06-10)

    /// A daily capture appends AFTER the last contentful block — never at
    /// the top (web parity).
    func testCaptureInsertIndexAppendsAfterContent() {
        let a = Block(id: "a", kind: .note, text: "first")
        let b = Block(id: "b", kind: .task, text: "second")
        XCTAssertEqual(MockMosaicService.captureInsertIndex(in: []), 0)
        XCTAssertEqual(MockMosaicService.captureInsertIndex(in: [a]), 1)
        XCTAssertEqual(MockMosaicService.captureInsertIndex(in: [a, b]), 2)
    }

    /// A trailing run of bare "Add block" placeholders stays the visual
    /// tail of the day: the capture slots in just before it.
    func testCaptureInsertIndexSkipsTrailingPlaceholders() {
        let content = Block(id: "a", kind: .note, text: "real")
        let empty1 = Block(id: "b", kind: .note, text: "")
        let empty2 = Block(id: "c", kind: .note, text: "")
        XCTAssertEqual(
            MockMosaicService.captureInsertIndex(in: [content, empty1]), 1
        )
        XCTAssertEqual(
            MockMosaicService.captureInsertIndex(in: [content, empty1, empty2]), 1
        )
        // A placeholder ABOVE content does not move the index — only the
        // trailing run matters.
        XCTAssertEqual(
            MockMosaicService.captureInsertIndex(in: [empty1, content]), 2
        )
        // All-placeholder day: capture lands first.
        XCTAssertEqual(
            MockMosaicService.captureInsertIndex(in: [empty1, empty2]), 0
        )
    }

    /// End-to-end through `capture(_:target:)` on the in-memory model:
    /// the captured block is appended after the seed's last block, not
    /// inserted at index 0.
    func testCaptureAppendsAtBottomOfToday() {
        let service = MockMosaicService()
        let countBefore = service.todayBlocks.count
        service.capture("captured at the bottom", target: .today)
        XCTAssertEqual(service.todayBlocks.count, countBefore + 1)
        XCTAssertEqual(service.todayBlocks.last?.text, "captured at the bottom")
        XCTAssertNotEqual(service.todayBlocks.first?.text, "captured at the bottom")
        // Inbox captures append too (with the #inbox tag).
        service.capture("inbox capture", target: .inbox)
        XCTAssertEqual(service.todayBlocks.last?.text, "inbox capture")
        XCTAssertEqual(service.todayBlocks.last?.tags, ["#inbox"])
    }

    // MARK: - Task toggle write path (2026-06-10 revert fix)

    func testTaskStatusValue() {
        XCTAssertEqual(MockMosaicService.taskStatusValue(done: true), "done")
        XCTAssertEqual(MockMosaicService.taskStatusValue(done: false), "todo")
    }

    /// The toggle flips `done` AND mirrors it into the block's `status`
    /// property — the typed `status::` write is what persists (engine
    /// container op in `.relay` / set-property POST in `.http`); the
    /// in-memory property keeps any later whole-note writeback
    /// consistent with the flip.
    func testToggleTaskFlipsDoneAndStatusProperty() {
        let service = MockMosaicService()
        guard let task = service.todayBlocks.first(where: { $0.kind == .task && !$0.done }) else {
            return XCTFail("seed should contain an open task")
        }
        service.toggleTask(id: task.id)
        let toggled = service.todayBlocks.first(where: { $0.id == task.id })
        XCTAssertEqual(toggled?.done, true)
        XCTAssertEqual(
            toggled?.properties.first(where: { $0.key.lowercased() == "status" })?.value,
            "done"
        )
        // Toggle back: status mirrors to todo.
        service.toggleTask(id: task.id)
        let untoggled = service.todayBlocks.first(where: { $0.id == task.id })
        XCTAssertEqual(untoggled?.done, false)
        XCTAssertEqual(
            untoggled?.properties.first(where: { $0.key.lowercased() == "status" })?.value,
            "todo"
        )
    }

    /// Toggling a non-task block is a no-op (the checkbox only exists on
    /// task rows).
    func testToggleTaskIgnoresNotes() {
        let service = MockMosaicService()
        guard let note = service.todayBlocks.first(where: { $0.kind == .note }) else {
            return XCTFail("seed should contain a note block")
        }
        service.toggleTask(id: note.id)
        let after = service.todayBlocks.first(where: { $0.id == note.id })
        XCTAssertEqual(after?.done, false)
        XCTAssertTrue(after?.properties.isEmpty ?? false)
    }

    // MARK: - Daily feed pagination helpers (2026-06-10)

    func testIsDailySlug() {
        XCTAssertTrue(MockMosaicService.isDailySlug("2026-06-09"))
        XCTAssertTrue(MockMosaicService.isDailySlug("1999-01-31"))
        XCTAssertFalse(MockMosaicService.isDailySlug("tag-system"))
        XCTAssertFalse(MockMosaicService.isDailySlug("2026-6-9"))
        XCTAssertFalse(MockMosaicService.isDailySlug("2026-06-09-extra"))
        XCTAssertFalse(MockMosaicService.isDailySlug(""))
    }
}
