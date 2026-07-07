import XCTest
@testable import Tesela

/// Unit tests for the authoring-only diagnostics pass (tesela-vp9.4) —
/// the iOS mirror of web's `parseQueryWithDiagnostics`
/// (`web/tests/unit/query-diagnostics.test.mjs`, tesela-vp9.1).
///
/// `LocalQueryEngine.parseSimpleDslWithDiagnostics` wraps the SAME
/// recursive-descent parser `parseSimpleDsl` uses (`DslParser`, threaded
/// through the shared `parseSimpleDslInternal`) and records
/// `{start, end, got, hint}` spans wherever the parser silently drops a
/// token or leaves an operator dangling while re-syncing. Diagnostics are
/// additive UI metadata only — they must never change what
/// `parseSimpleDsl` itself returns; that invariant is asserted directly
/// below, on top of the shared 182-case conformance fixture
/// (`QueryConformanceTests.swift`) which pins `parseSimpleDsl`'s
/// well-formed-input behavior separately.
final class QueryDiagnosticsTests: XCTestCase {

    private typealias BoolExpr = LocalQueryEngine.SimpleDsl.BoolExpr

    /// Raw UTF-8 source slice at `[start, end)` — mirrors what
    /// `QueryDiagnostic.start`/`.end` index into.
    private func slice(_ dsl: String, _ start: Int, _ end: Int) -> String {
        let bytes = Array(dsl.utf8)
        return String(decoding: bytes[start..<end], as: UTF8.self)
    }

    private func atom(_ pred: LocalQueryEngine.SimpleDsl.Predicate) -> BoolExpr { .atom(pred) }

    // ────────────────────────────────────────────────────────────────
    // Clean JQL → zero diagnostics
    // ────────────────────────────────────────────────────────────────

    private let cleanQueries: [String] = [
        "points > 5",
        "points >= 5 AND points <= 10",
        "status != done",
        "tag = urgent",
        "tag IN (urgent, blocked)",
        "tag NOT IN (urgent, blocked)",
        "text LIKE \"%foo%\"",
        "text NOT LIKE \"%foo%\"",
        "points BETWEEN 1 AND 10",
        "deadline IS NULL",
        "deadline IS NOT NULL",
        "(status:todo OR status:doing) AND priority:high",
        "status:todo AND priority:high",
        "status:todo OR priority:high",
        "ORDER BY points DESC, created ASC",
        "status:todo ORDER BY points DESC, created ASC",
        "status:backlog,todo -has:scheduled -has:deadline",
        "tag-in:a,b,c",
        "kind:page status:todo",
        "-status:done",
        "text:\"hello world\"",
        "",
        "   ",
    ]

    func testCleanJqlProducesZeroDiagnostics() {
        for q in cleanQueries {
            let (_, diagnostics) = LocalQueryEngine.parseSimpleDslWithDiagnostics(q)
            XCTAssertTrue(
                diagnostics.isEmpty,
                "expected no diagnostics for \(String(reflecting: q)), got \(diagnostics)"
            )
        }
    }

    // ────────────────────────────────────────────────────────────────
    // Malformed inputs → diagnostics with correct spans
    // ────────────────────────────────────────────────────────────────

    func testDanglingAndRecordsDiagnosticSpanningTheAndToken() {
        let dsl = "status:todo AND"
        let (parsed, diagnostics) = LocalQueryEngine.parseSimpleDslWithDiagnostics(dsl)
        XCTAssertEqual(diagnostics.count, 1)
        let d = diagnostics[0]
        XCTAssertEqual(slice(dsl, d.start, d.end), "AND")
        XCTAssertEqual(d.got, "AND")
        XCTAssertTrue(d.hint.contains("AND"))
        // The dangling AND is dropped — the predicate before it still parses.
        XCTAssertEqual(parsed, LocalQueryEngine.parseSimpleDsl(dsl))
        XCTAssertEqual(parsed.expr, atom(.cmp(key: "status", op: .eq, value: "todo")))
    }

    func testDanglingOrRecordsDiagnosticSpanningTheOrToken() {
        let dsl = "status:todo OR"
        let (parsed, diagnostics) = LocalQueryEngine.parseSimpleDslWithDiagnostics(dsl)
        XCTAssertEqual(diagnostics.count, 1)
        let d = diagnostics[0]
        XCTAssertEqual(slice(dsl, d.start, d.end), "OR")
        XCTAssertEqual(d.got, "OR")
        XCTAssertEqual(parsed, LocalQueryEngine.parseSimpleDsl(dsl))
    }

    func testUnclosedParenRecordsDiagnosticSpanningFromParenToEndOfInput() {
        let dsl = "(status:todo"
        let (parsed, diagnostics) = LocalQueryEngine.parseSimpleDslWithDiagnostics(dsl)
        XCTAssertEqual(diagnostics.count, 1)
        let d = diagnostics[0]
        XCTAssertEqual(d.start, 0)
        XCTAssertEqual(d.end, dsl.utf8.count)
        XCTAssertEqual(d.got, dsl)
        XCTAssertTrue(d.hint.lowercased().contains("unclosed"))
        // Content inside the unclosed paren still parses as if closed.
        XCTAssertEqual(parsed, LocalQueryEngine.parseSimpleDsl(dsl))
        XCTAssertEqual(parsed.expr, atom(.cmp(key: "status", op: .eq, value: "todo")))
    }

    func testUnclosedQuoteRecordsDiagnosticSpanningTheQuotedToken() {
        let dsl = "text:\"foo"
        let (parsed, diagnostics) = LocalQueryEngine.parseSimpleDslWithDiagnostics(dsl)
        XCTAssertEqual(diagnostics.count, 1)
        let d = diagnostics[0]
        XCTAssertEqual(d.start, 5) // index of the opening '"'
        XCTAssertEqual(d.end, dsl.utf8.count)
        XCTAssertEqual(d.got, "\"foo")
        XCTAssertTrue(d.hint.lowercased().contains("unclosed"))
        XCTAssertEqual(parsed, LocalQueryEngine.parseSimpleDsl(dsl))
        // The unterminated quote still yields its content as the value.
        XCTAssertEqual(parsed.expr, atom(.cmp(key: "text", op: .eq, value: "foo")))
    }

    func testBareUnknownWordBetweenPredicatesRecordsDiagnosticAndIsDropped() {
        let dsl = "status:todo blah status:done"
        let (parsed, diagnostics) = LocalQueryEngine.parseSimpleDslWithDiagnostics(dsl)
        XCTAssertEqual(diagnostics.count, 1)
        let d = diagnostics[0]
        XCTAssertEqual(slice(dsl, d.start, d.end), "blah")
        XCTAssertEqual(d.got, "blah")
        XCTAssertTrue(d.hint.lowercased().contains("unknown word"))
        XCTAssertEqual(parsed, LocalQueryEngine.parseSimpleDsl(dsl))
        // "blah" dropped; both status predicates survive as an implicit AND.
        XCTAssertEqual(
            parsed.expr,
            .and([
                atom(.cmp(key: "status", op: .eq, value: "todo")),
                atom(.cmp(key: "status", op: .eq, value: "done")),
            ])
        )
    }

    func testInfixOperatorWithNoOperandRecordsDiagnostic() {
        let dsl = "points >"
        let (parsed, diagnostics) = LocalQueryEngine.parseSimpleDslWithDiagnostics(dsl)
        XCTAssertEqual(diagnostics.count, 1)
        let d = diagnostics[0]
        XCTAssertEqual(slice(dsl, d.start, d.end), ">")
        XCTAssertEqual(d.got, ">")
        XCTAssertTrue(d.hint.lowercased().contains("no operand"))
        XCTAssertEqual(parsed, LocalQueryEngine.parseSimpleDsl(dsl))
        // Predicate still produced, with an empty value (matches parseSimpleDsl).
        XCTAssertEqual(parsed.expr, atom(.cmp(key: "points", op: .gt, value: "")))
    }

    func testColonPredicateWithNoValueRecordsDiagnosticAndDrops() {
        // "AND" is itself a word token so it's slurped as the value if used
        // after a bare colon — use a punctuation boundary instead to force
        // a genuinely missing value.
        let dsl = "status:)"
        let (parsed, diagnostics) = LocalQueryEngine.parseSimpleDslWithDiagnostics(dsl)
        XCTAssertGreaterThanOrEqual(diagnostics.count, 1)
        let colonDiag = diagnostics.first { $0.hint.contains("has no value") }
        XCTAssertNotNil(colonDiag, "expected a \"has no value\" diagnostic, got \(diagnostics)")
        if let colonDiag {
            XCTAssertEqual(slice(dsl, colonDiag.start, colonDiag.end), "status:")
        }
        XCTAssertEqual(parsed, LocalQueryEngine.parseSimpleDsl(dsl))
    }

    func testLikeWithNoOperandRecordsDiagnostic() {
        let dsl = "text LIKE"
        let (parsed, diagnostics) = LocalQueryEngine.parseSimpleDslWithDiagnostics(dsl)
        XCTAssertEqual(diagnostics.count, 1)
        let d = diagnostics[0]
        XCTAssertEqual(slice(dsl, d.start, d.end), "LIKE")
        XCTAssertTrue(d.hint.lowercased().contains("no operand"))
        XCTAssertEqual(parsed, LocalQueryEngine.parseSimpleDsl(dsl))
    }

    func testDanglingNotRecordsDiagnostic() {
        let dsl = "NOT"
        let (parsed, diagnostics) = LocalQueryEngine.parseSimpleDslWithDiagnostics(dsl)
        XCTAssertEqual(diagnostics.count, 1)
        XCTAssertEqual(slice(dsl, diagnostics[0].start, diagnostics[0].end), "NOT")
        XCTAssertEqual(parsed, LocalQueryEngine.parseSimpleDsl(dsl))
    }

    func testDanglingMinusRecordsDiagnostic() {
        let dsl = "status:todo -"
        let (parsed, diagnostics) = LocalQueryEngine.parseSimpleDslWithDiagnostics(dsl)
        XCTAssertEqual(diagnostics.count, 1)
        let d = diagnostics[0]
        XCTAssertEqual(slice(dsl, d.start, d.end), "-")
        XCTAssertEqual(parsed, LocalQueryEngine.parseSimpleDsl(dsl))
    }

    func testStrayTrailingTokenAfterWellFormedExpressionRecordsDiagnostic() {
        let dsl = "status:todo)"
        let (parsed, diagnostics) = LocalQueryEngine.parseSimpleDslWithDiagnostics(dsl)
        XCTAssertEqual(diagnostics.count, 1)
        let d = diagnostics[0]
        XCTAssertEqual(slice(dsl, d.start, d.end), ")")
        XCTAssertTrue(d.hint.lowercased().contains("trailing"))
        XCTAssertEqual(parsed, LocalQueryEngine.parseSimpleDsl(dsl))
    }

    func testMalformedTokenAtPredicatePositionRecordsDiagnostic() {
        let dsl = "()"
        let (parsed, diagnostics) = LocalQueryEngine.parseSimpleDslWithDiagnostics(dsl)
        XCTAssertGreaterThanOrEqual(diagnostics.count, 1)
        XCTAssertEqual(parsed, LocalQueryEngine.parseSimpleDsl(dsl))
    }

    // ────────────────────────────────────────────────────────────────
    // Invariant: parseSimpleDsl(s) === parseSimpleDslWithDiagnostics(s).parsed
    // for a broad corpus of malformed strings.
    // ────────────────────────────────────────────────────────────────

    private let malformedCorpus: [String] = [
        "status:todo AND",
        "status:todo OR",
        "(status:todo",
        "((status:todo)",
        "status:todo)",
        "status:todo))",
        "text:\"foo",
        "text:\"",
        "status:todo blah status:done",
        "blah",
        "points >",
        "points >=",
        "status:",
        "status:)",
        "status:,",
        "text LIKE",
        "text NOT LIKE",
        "NOT",
        "-",
        "status:todo -",
        "()",
        "(",
        ")",
        ",",
        ":",
        "AND",
        "OR",
        "AND OR",
        "status:todo AND OR priority:high",
        "points BETWEEN 5",
        "points BETWEEN",
        "kind:",
        "ORDER BY",
        "status:todo ORDER BY",
        "tag IN",
        "tag IN (",
        "tag NOT",
        "\"unterminated",
        "status:todo AND blah OR (priority:high",
        "   status:todo   ",
    ]

    func testInvariantParseSimpleDslMatchesParseSimpleDslWithDiagnosticsParsed() {
        for dsl in malformedCorpus {
            let plain = LocalQueryEngine.parseSimpleDsl(dsl)
            let (parsed, _) = LocalQueryEngine.parseSimpleDslWithDiagnostics(dsl)
            XCTAssertEqual(parsed, plain, "mismatch for \(String(reflecting: dsl))")
        }
    }
}
