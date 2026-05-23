import XCTest
@testable import Tesela

/// Tests for MockMosaicService internals that are testable without a live
/// server — specifically the `parseBlocks(from:noteId:)` line-number
/// tracking introduced for the `recur-bump` block_id fix.
@MainActor
final class MockMosaicServiceTests: XCTestCase {

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
}
