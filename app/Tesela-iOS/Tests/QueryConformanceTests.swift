import XCTest
@testable import Tesela

/// iOS consumer of the shared query-DSL conformance fixture
/// (`crates/tesela-core/tests/fixtures/query-conformance.json`).
///
/// The fixture is the ONE source of truth for DSL matching semantics
/// across the three implementations — Rust
/// (`crates/tesela-core/tests/query_conformance.rs`, the source of
/// truth), the web TS consumer, and this file. Every case runs through
/// the REAL parser + matcher (`parseSimpleDsl` → `clauseMatches` via
/// `blockMatches`), not a reimplementation. Zero skips.
///
/// Adapter contract (mirrors the `_contract` header in the fixture and
/// the Rust `to_parsed_block`): the language-neutral fixture block maps
/// onto `LocalQueryEngine.BlockContext` as
///   text        → `block.text` (and `block.rawText` = "- {text}")
///   tags        → `ownTags` (own resolved tags; inherited chain empty)
///   properties  → `properties`
///   isHeading   → asserted consistent with `isHeadingText(text)`
///   onDailyPage → `noteId` = "2026-06-10" (canonical daily id) when
///                 true, "fixture-note" otherwise
///   noteType    → `pageNoteType`
final class QueryConformanceTests: XCTestCase {

    private struct Fixture: Decodable {
        let _contract: [String]
        let cases: [Case]
    }

    private struct Case: Decodable {
        let name: String
        let dsl: String
        let block: FixtureBlock
        let expect: Bool
    }

    private struct FixtureBlock: Decodable {
        let text: String
        let tags: [String]
        let properties: [String: String]
        let isHeading: Bool
        let onDailyPage: Bool
        let noteType: String?
    }

    /// The canonical Inbox view DSL (`INBOX_VIEW_DSL` in query.rs) —
    /// the seeded built-in view; the fixture gates exactly this string.
    private let inboxViewDsl = "status:backlog,todo -has:scheduled -has:deadline"

    /// The canonical fixture lives in the Rust crate. The simulator
    /// test host shares the Mac's filesystem, so resolve it relative
    /// to this source file (`#filePath`) — no copied resource to drift.
    private static let fixtureURL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent() // → Tests/
        .deletingLastPathComponent() // → Tesela-iOS/
        .deletingLastPathComponent() // → app/
        .deletingLastPathComponent() // → repo root
        .appendingPathComponent("crates/tesela-core/tests/fixtures/query-conformance.json")

    private func loadFixture() throws -> Fixture {
        let data = try Data(contentsOf: Self.fixtureURL)
        return try JSONDecoder().decode(Fixture.self, from: data)
    }

    /// Mirror of the Rust consumer's `to_parsed_block` adapter.
    private func context(for b: FixtureBlock) -> LocalQueryEngine.BlockContext {
        let noteId = b.onDailyPage ? "2026-06-10" : "fixture-note"
        let block = Block(
            id: "\(noteId):1",
            kind: .note,
            text: b.text,
            rawText: "- \(b.text)",
            lineNumber: 1,
            noteId: noteId
        )
        return LocalQueryEngine.BlockContext(
            block: block,
            blockId: "\(noteId):1",
            ownTags: b.tags,
            inheritedTags: [],
            properties: b.properties,
            noteId: noteId,
            pageNoteType: b.noteType
        )
    }

    /// Every fixture case must match through the real parser + matcher.
    func testAllConformanceCasesPassThroughRealMatcher() throws {
        let fixture = try loadFixture()
        var failures: [String] = []
        for c in fixture.cases {
            let dsl = LocalQueryEngine.parseSimpleDsl(c.dsl)
            let got = LocalQueryEngine.blockMatches(dsl, ctx: context(for: c.block))
            if got != c.expect {
                failures.append("  \(c.name) — dsl \"\(c.dsl)\": expected \(c.expect), got \(got)")
            }
        }
        XCTAssertTrue(
            failures.isEmpty,
            "\(failures.count) conformance case(s) diverged from the fixture:\n"
                + failures.joined(separator: "\n")
        )
    }

    /// The fixture's `isHeading` flag must agree with what the engine
    /// derives from `text` — mirrors the Rust consumer's consistency
    /// test so a stale flag can't let this consumer pass while
    /// disagreeing with Rust.
    func testIsHeadingFlagsAreConsistentWithText() throws {
        let fixture = try loadFixture()
        let q = LocalQueryEngine.parseSimpleDsl("is:heading")
        for c in fixture.cases {
            XCTAssertEqual(
                LocalQueryEngine.blockMatches(q, ctx: context(for: c.block)),
                c.block.isHeading,
                "case \(c.name): isHeading flag disagrees with text \"\(c.block.text)\""
            )
        }
    }

    /// Case names are unique (they're the cross-language assertion ids).
    func testCaseNamesAreUnique() throws {
        let fixture = try loadFixture()
        var seen = Set<String>()
        for c in fixture.cases {
            XCTAssertTrue(seen.insert(c.name).inserted, "duplicate case name: \(c.name)")
        }
    }

    /// The fixture meets the spec's breadth bar and pins the canonical
    /// Inbox DSL verbatim — the string that makes the built-in Inbox
    /// view work in `.relay` mode.
    func testFixtureCoversRequiredSurface() throws {
        let fixture = try loadFixture()
        XCTAssertGreaterThanOrEqual(
            fixture.cases.count, 40,
            "fixture has \(fixture.cases.count) cases; the spec requires 40+"
        )
        let inboxCases = fixture.cases.filter { $0.dsl == inboxViewDsl }.count
        XCTAssertGreaterThanOrEqual(
            inboxCases, 5,
            "expected a full Inbox-default matrix (>=5 cases using the Inbox DSL verbatim), found \(inboxCases)"
        )
    }
}
