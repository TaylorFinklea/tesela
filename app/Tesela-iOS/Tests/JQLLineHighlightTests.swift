import XCTest
@testable import Tesela

/// Pure-logic tests for the `query::` line JQL syntax highlighter
/// (tesela-vp9.6) — a thin adapter over `LocalQueryEngine.tokenizeDsl`
/// (tesela-vp9.4) + `QueryAuthoring.buildPreviewSpans` (tesela-vp9.5).
final class JQLLineHighlightTests: XCTestCase {

    // MARK: - detectSpans: a single query:: line

    func testDetectSpansForSimpleQueryLine() {
        let text = "query:: status = todo"
        let ns = text as NSString
        let rendered = JQLLineHighlight.detectSpans(in: text)
            .map { (ns.substring(with: $0.range), $0.kind) }
        XCTAssertEqual(rendered.map(\.0), ["status", "=", "todo"])
        XCTAssertEqual(rendered.map(\.1), [.key, .operatorKind, .value])
    }

    func testDetectSpansUTF16CorrectForMultibyteQuotedValue() {
        // "café" contains a 2-UTF8-byte / 1-UTF16-unit character — a
        // byte-offset-as-UTF16 bug would misalign every span after it.
        let text = "query:: title LIKE \"café\" AND status = \"done\""
        let ns = text as NSString
        let rendered = JQLLineHighlight.detectSpans(in: text)
            .map { (ns.substring(with: $0.range), $0.kind) }
        XCTAssertEqual(rendered.map(\.0), [
            "title", "LIKE", "\"café\"", "AND", "status", "=", "\"done\"",
        ])
        XCTAssertEqual(rendered.map(\.1), [
            .key, .operatorKind, .string, .operatorKind, .key, .operatorKind, .string,
        ])
    }

    func testDetectSpansToleratesLeadingIndentAndCase() {
        let text = "  QUERY:: type = Task"
        let ns = text as NSString
        let rendered = JQLLineHighlight.detectSpans(in: text).map { ns.substring(with: $0.range) }
        XCTAssertEqual(rendered, ["type", "=", "Task"])
    }

    // MARK: - Non-query lines yield nothing

    func testDetectSpansEmptyForPlainProseLine() {
        XCTAssertEqual(JQLLineHighlight.detectSpans(in: "Call Alice tomorrow p2"), [])
    }

    func testQueryLineRangesEmptyForPlainProseLine() {
        XCTAssertTrue(JQLLineHighlight.queryLineRanges(in: "Call Alice tomorrow p2").isEmpty)
    }

    // MARK: - Mixed multi-line block text: only the query:: line lights up

    func testDetectSpansOnlyHighlightsTheQueryLine() {
        let text = "Some prose p2\nquery:: status = todo\nAnother line tomorrow"
        let ns = text as NSString
        let spans = JQLLineHighlight.detectSpans(in: text)
        XCTAssertEqual(spans.map { ns.substring(with: $0.range) }, ["status", "=", "todo"])
        let line2Start = ns.range(of: "query::").location
        let line2End = ns.range(of: "\nAnother").location
        for span in spans {
            XCTAssertGreaterThanOrEqual(span.range.location, line2Start)
            XCTAssertLessThanOrEqual(span.range.location + span.range.length, line2End)
        }
    }

    func testQueryLineRangesIdentifiesOnlyTheQueryLine() {
        let text = "Some prose p2\nquery:: status = todo\nAnother line tomorrow"
        let ranges = JQLLineHighlight.queryLineRanges(in: text)
        XCTAssertEqual(ranges.count, 1)
        let ns = text as NSString
        XCTAssertEqual(ns.substring(with: ranges[0]), "query:: status = todo")
    }
}
