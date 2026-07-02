import XCTest
@testable import Tesela

/// iOS consumer of the shared recurrence-DSL conformance fixture
/// (`crates/tesela-core/tests/fixtures/recurrence-conformance.json`).
///
/// The fixture is GENERATED from the Rust side
/// (`crates/tesela-core/tests/recurrence_conformance.rs`) — Rust is the
/// source of truth for `valid`. `canonical_display` is sourced from the
/// web consumer's `formatRecurrence` (no Rust formatter exists); this
/// file asserts the REAL `parseRecurrenceInput` / `RecurrenceFormat.human`
/// from `DateParser.swift` / `RecurrenceFormat.swift` against it — not a
/// reimplementation.
final class RecurrenceConformanceTests: XCTestCase {

    private struct Fixture: Decodable {
        let _contract: [String]
        let cases: [Case]
        let _client_extraction_contract: [String]
        let client_extraction_cases: [ExtractionCase]
    }

    private struct Case: Decodable {
        let name: String
        let input: String
        let valid: Bool
        let canonicalDisplay: String

        enum CodingKeys: String, CodingKey {
            case name, input, valid
            case canonicalDisplay = "canonical_display"
        }
    }

    /// Client-only mixed "<date phrase> <recurrence phrase>" EXTRACTION
    /// cases — asserted through the REAL `DateParser.parse`, not
    /// standalone `parseRecurrenceInput`. Rust has no extraction concept
    /// (see recurrence_conformance.rs module docs), so `expected` is a
    /// pinned literal rather than parser-derived.
    private struct ExtractionCase: Decodable {
        let name: String
        let input: String
        let anchorDate: String
        let expected: ExpectedExtraction?

        enum CodingKeys: String, CodingKey {
            case name, input, expected
            case anchorDate = "anchor_date"
        }
    }

    private struct ExpectedExtraction: Decodable {
        let date: String
        let time: String?
        let recurrence: String?
        let field: String?
    }

    /// The canonical fixture lives in the Rust crate. The simulator
    /// test host shares the Mac's filesystem, so resolve it relative
    /// to this source file (`#filePath`) — no copied resource to drift.
    private static let fixtureURL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent() // → Tests/
        .deletingLastPathComponent() // → Tesela-iOS/
        .deletingLastPathComponent() // → app/
        .deletingLastPathComponent() // → repo root
        .appendingPathComponent("crates/tesela-core/tests/fixtures/recurrence-conformance.json")

    private func loadFixture() throws -> Fixture {
        let data = try Data(contentsOf: Self.fixtureURL)
        return try JSONDecoder().decode(Fixture.self, from: data)
    }

    /// `parseRecurrenceInput` validity must match the Rust fixture's `valid`.
    func testParseRecurrenceInputValidityMatchesFixture() throws {
        let fixture = try loadFixture()
        var failures: [String] = []
        for c in fixture.cases {
            let got = parseRecurrenceInput(c.input) != nil
            if got != c.valid {
                failures.append("  \(c.name) — input \"\(c.input)\": expected valid=\(c.valid), got \(got)")
            }
        }
        XCTAssertTrue(
            failures.isEmpty,
            "\(failures.count) conformance case(s) diverged on validity:\n" + failures.joined(separator: "\n")
        )
    }

    /// `RecurrenceFormat.human` output must match the fixture's `canonical_display`.
    func testRecurrenceFormatMatchesFixture() throws {
        let fixture = try loadFixture()
        var failures: [String] = []
        for c in fixture.cases {
            let got = RecurrenceFormat.human(c.input)
            if got != c.canonicalDisplay {
                failures.append("  \(c.name) — input \"\(c.input)\": expected \"\(c.canonicalDisplay)\", got \"\(got)\"")
            }
        }
        XCTAssertTrue(
            failures.isEmpty,
            "\(failures.count) conformance case(s) diverged on display:\n" + failures.joined(separator: "\n")
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

    /// The fixture covers the 2026-06-20 grammar (biweekly-class cadences).
    func testFixtureCoversRequiredSurface() throws {
        let fixture = try loadFixture()
        XCTAssertGreaterThanOrEqual(
            fixture.cases.count, 30,
            "fixture has \(fixture.cases.count) cases; expected 30+ for full-grammar coverage"
        )
        for name in ["biweekly", "fortnightly", "quarterly"] {
            let c = fixture.cases.first { $0.name == name }
            XCTAssertNotNil(c, "fixture must include a \"\(name)\" case")
            XCTAssertEqual(c?.valid, true, "\"\(name)\" must be valid")
        }
    }

    /// Parse a fixture `anchor_date` ("YYYY-MM-DD") into a local-midnight
    /// `Date`, mirroring how the web tests build `new Date(y, m - 1, d)`.
    private func anchorDate(_ isoDate: String) -> Date {
        let parts = isoDate.split(separator: "-").compactMap { Int($0) }
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone.current
        var components = DateComponents()
        components.year = parts[0]
        components.month = parts[1]
        components.day = parts[2]
        return calendar.date(from: components)!
    }

    /// `DateParser.parse` extraction must match the fixture's
    /// `client_extraction_cases` — the mixed "<date phrase> <recurrence
    /// phrase>" path, not standalone `parseRecurrenceInput`.
    func testDateParserExtractionMatchesFixture() throws {
        let fixture = try loadFixture()
        var failures: [String] = []
        for c in fixture.client_extraction_cases {
            let got = DateParser.parse(c.input, today: anchorDate(c.anchorDate))
            guard let want = c.expected else {
                if got != nil {
                    failures.append("  \(c.name) — input \"\(c.input)\": expected nil, got \(String(describing: got))")
                }
                continue
            }
            let wantDesc = "{date: \(want.date), time: \(String(describing: want.time)), recurrence: \(String(describing: want.recurrence)), field: \(String(describing: want.field))}"
            guard let got = got,
                  got.date == want.date,
                  got.time == want.time,
                  got.recurrence == want.recurrence,
                  got.field?.rawValue == want.field
            else {
                failures.append("  \(c.name) — input \"\(c.input)\": expected \(wantDesc), got \(String(describing: got))")
                continue
            }
        }
        XCTAssertTrue(
            failures.isEmpty,
            "\(failures.count) client extraction case(s) diverged:\n" + failures.joined(separator: "\n")
        )
    }

    /// `client_extraction_cases` covers trailing extraction for the new grammar.
    func testClientExtractionCoversNewGrammar() throws {
        let fixture = try loadFixture()
        let inputs = fixture.client_extraction_cases.map { $0.input }
        for needle in ["biweekly", "fortnightly", "quarterly", "every other", "every weekday"] {
            XCTAssertTrue(
                inputs.contains { $0.contains(needle) },
                "client_extraction_cases must exercise trailing \"\(needle)\" extraction"
            )
        }
    }
}
