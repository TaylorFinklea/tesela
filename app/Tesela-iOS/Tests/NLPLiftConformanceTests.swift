import XCTest
@testable import Tesela

/// iOS consumer of the shared inline-NLP-lift conformance fixture
/// (`crates/tesela-core/tests/fixtures/nlp-lift-conformance.json`,
/// `tesela-pfix.3`).
///
/// The fixture's `expected` values are PINNED LITERALS generated from the
/// real web `detectTaskTokens` (see the module docs in
/// `nlp_lift_conformance.rs`). This file asserts the REAL iOS
/// `InlineNLP.detectLifts` (`EditorAutocomplete.swift`) against those same
/// values. `detectLifts` no longer reimplements the detection logic in
/// Swift — it delegates to the shared Rust `tesela_core::nlp_lift`
/// implementation via the `detectNlpLifts` FFI call (`tesela-ug7`), so this
/// test runs THROUGH the FFI boundary, not against a Swift mirror. The
/// fixture's shared `registry` (select `priority` + date `deadline`) is
/// synthesized here into a `PropertyRegistry` via a single `Fixture` Tag
/// page, mirroring `PropertyRegistryTests`' `RegistryNote` builder pattern.
final class NLPLiftConformanceTests: XCTestCase {

    // MARK: - Fixture decoding

    private struct FixtureFile: Decodable {
        let registry: RegistrySpec
        let anchorDate: String
        let cases: [Case]

        enum CodingKeys: String, CodingKey {
            case registry, cases
            case anchorDate = "anchor_date"
        }
    }

    private struct RegistrySpec: Decodable {
        let defaultDateProperty: String
        let properties: [PropertySpec]

        enum CodingKeys: String, CodingKey {
            case properties
            case defaultDateProperty = "default_date_property"
        }
    }

    private struct PropertySpec: Decodable {
        let key: String
        let valueType: String
        let choices: [String]
        let triggers: [String]

        enum CodingKeys: String, CodingKey {
            case key, choices, triggers
            case valueType = "value_type"
        }
    }

    private struct Case: Decodable {
        let name: String
        let text: String
        let expected: Expected
    }

    private struct Expected: Decodable {
        let stripped: String
        let props: [ExpectedProp]
    }

    private struct ExpectedProp: Decodable {
        let key: String
        let value: String
    }

    /// The canonical fixture lives in the Rust crate. The simulator test
    /// host shares the Mac's filesystem, so resolve it relative to this
    /// source file (`#filePath`) — no copied resource to drift (mirror of
    /// `RecurrenceConformanceTests.fixtureURL`).
    private static let fixtureURL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent() // → Tests/
        .deletingLastPathComponent() // → Tesela-iOS/
        .deletingLastPathComponent() // → app/
        .deletingLastPathComponent() // → repo root
        .appendingPathComponent("crates/tesela-core/tests/fixtures/nlp-lift-conformance.json")

    private func loadFixture() throws -> FixtureFile {
        let data = try Data(contentsOf: Self.fixtureURL)
        return try JSONDecoder().decode(FixtureFile.self, from: data)
    }

    /// Build a `PropertyRegistry` carrying ONE "Fixture" tag whose
    /// `tag_properties` + Property pages mirror the fixture's shared
    /// `registry` spec exactly (same keys, value types, choices, triggers
    /// the web `SPEC` object encodes).
    private func buildRegistry(_ spec: RegistrySpec) -> PropertyRegistry {
        var notes: [RegistryNote] = [
            RegistryNote(title: "Root Tag", noteType: "Tag", custom: ["tag_properties": [String]()]),
        ]
        for p in spec.properties {
            notes.append(RegistryNote(title: p.key, noteType: "Property", custom: [
                "value_type": p.valueType,
                "choices": p.choices,
                "nl_triggers": p.triggers,
            ]))
        }
        notes.append(RegistryNote(title: "Fixture", noteType: "Tag", custom: [
            "extends": "Root Tag",
            "tag_properties": spec.properties.map { $0.key },
        ]))
        return PropertyRegistry.build(from: notes)
    }

    /// Parse a fixture `anchor_date` ("YYYY-MM-DD") into a local-midnight
    /// `Date`, mirroring `RecurrenceConformanceTests.anchorDate`.
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

    /// `InlineNLP.detectLifts` must match the fixture's `expected` for
    /// every case: same stripped text, same props (order-independent — the
    /// fixture pins insertion order but Set-equality is what actually
    /// matters cross-language).
    func testDetectLiftsMatchesFixture() throws {
        let fixture = try loadFixture()
        let registry = buildRegistry(fixture.registry)
        let today = anchorDate(fixture.anchorDate)
        var failures: [String] = []
        for c in fixture.cases {
            let (stripped, props) = InlineNLP.detectLifts(in: c.text, tags: ["Fixture"], registry: registry, today: today)
            let gotSet = Set(props.map { "\($0.key)=\($0.value)" })
            let wantSet = Set(c.expected.props.map { "\($0.key)=\($0.value)" })
            if stripped != c.expected.stripped || gotSet != wantSet {
                failures.append(
                    "  \(c.name) — text \"\(c.text)\":\n"
                        + "    expected stripped=\"\(c.expected.stripped)\" props=\(wantSet.sorted())\n"
                        + "    got      stripped=\"\(stripped)\" props=\(gotSet.sorted())"
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

    /// The fixture covers today-noon, the URL-embedded no-lift guard, and
    /// the trailing-position rule (positive + negative cases).
    func testFixtureCoversRequiredSurface() throws {
        let fixture = try loadFixture()
        XCTAssertGreaterThanOrEqual(fixture.cases.count, 10, "fixture has \(fixture.cases.count) cases; expected 10+")

        func byName(_ n: String) -> Case {
            guard let c = fixture.cases.first(where: { $0.name == n }) else {
                XCTFail("fixture must include a \"\(n)\" case")
                fatalError()
            }
            return c
        }

        let todayNoon = byName("bare_trailing_today_noon")
        XCTAssertEqual(todayNoon.expected.props.map { "\($0.key)=\($0.value)" }, ["deadline=2026-05-22 12:00"])

        let url = byName("url_embedded_priority_no_lift")
        XCTAssertTrue(url.expected.props.isEmpty)
        XCTAssertEqual(url.expected.stripped, url.text, "URL survives the (non-)strip intact")

        let trailing = byName("bare_trailing_lift")
        XCTAssertFalse(trailing.expected.props.isEmpty, "trailing bare date must lift")

        let midProse = byName("bare_midprose_not_lifted")
        XCTAssertTrue(midProse.expected.props.isEmpty)
        XCTAssertEqual(midProse.expected.stripped, midProse.text)
    }
}
