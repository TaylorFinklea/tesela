import XCTest
@testable import Tesela

/// iOS consumer of the shared inline-span rendering conformance fixture
/// (`crates/tesela-core/tests/fixtures/inline-span-conformance.json`,
/// tesela-pfix.6).
///
/// The fixture pins the flat, ordered inline-span list a block's
/// single-line prose renders into. There is no Rust consumer — Rust never
/// renders UI — so this fixture has exactly two engines: this file (the
/// REAL production `BlockText.parseInlineSpans`) and the web mirror
/// (`parseInlineSpans` in block-parser.ts, inline-span-conformance.test.mjs).
/// See the fixture's `_contract` header for the full scope/precedence rules.
/// `href` is compared to nothing here — it's not part of the shared
/// contract (display-text-only); only `kind` + `text` are asserted.
final class InlineSpanConformanceTests: XCTestCase {

    private struct Fixture: Decodable {
        let cases: [Case]
    }

    private struct Case: Decodable {
        let name: String
        let text: String
        let expected: [ExpectedSpan]
    }

    private struct ExpectedSpan: Decodable, Equatable {
        let kind: String
        let text: String
    }

    /// The canonical fixture lives in the Rust crate. The simulator test
    /// host shares the Mac's filesystem, so resolve it relative to this
    /// source file (`#filePath`) — no copied resource to drift (mirror of
    /// `QueryConformanceTests.fixtureURL`).
    private static let fixtureURL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent() // → Tests/
        .deletingLastPathComponent() // → Tesela-iOS/
        .deletingLastPathComponent() // → app/
        .deletingLastPathComponent() // → repo root
        .appendingPathComponent("crates/tesela-core/tests/fixtures/inline-span-conformance.json")

    private func loadFixture() throws -> Fixture {
        let data = try Data(contentsOf: Self.fixtureURL)
        return try JSONDecoder().decode(Fixture.self, from: data)
    }

    /// `BlockText.parseInlineSpans` must match the fixture for every case:
    /// same ordered (kind, text) pairs. `href` is deliberately excluded from
    /// the comparison — it's not part of the shared contract.
    func testParseInlineSpansMatchesFixture() throws {
        let fixture = try loadFixture()
        var failures: [String] = []
        for c in fixture.cases {
            let got = BlockText.parseInlineSpans(c.text).map { ExpectedSpan(kind: $0.kind.rawValue, text: $0.text) }
            if got != c.expected {
                failures.append(
                    "  \(c.name) — text \(String(reflecting: c.text)):\n"
                        + "    expected \(c.expected)\n"
                        + "    got      \(got)"
                )
            }
        }
        XCTAssertTrue(
            failures.isEmpty,
            "\(failures.count) of \(fixture.cases.count) conformance case(s) diverged:\n" + failures.joined(separator: "\n")
        )
    }

    /// Case names are unique (they're the cross-language assertion ids).
    func testCaseNamesAreUnique() throws {
        let fixture = try loadFixture()
        var seen = Set<String>()
        for c in fixture.cases {
            XCTAssertTrue(seen.insert(c.name).inserted, "duplicate case name: \(c.name)")
        }
    }

    /// The fixture covers every required span kind (plus the pre-existing
    /// wikilink/bold/italic baseline).
    func testFixtureCoversRequiredSurface() throws {
        let fixture = try loadFixture()
        XCTAssertGreaterThanOrEqual(fixture.cases.count, 20, "fixture has \(fixture.cases.count) cases; expected 20+")
        var kinds = Set<String>()
        for c in fixture.cases {
            for span in c.expected { kinds.insert(span.kind) }
        }
        for required in ["plain", "bold", "italic", "code", "strike", "link", "wikilink"] {
            XCTAssertTrue(kinds.contains(required), "fixture must cover the \"\(required)\" span kind")
        }
    }
}
