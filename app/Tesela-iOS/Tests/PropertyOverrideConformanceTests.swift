import XCTest
@testable import Tesela

/// iOS consumer of the shared property-override-resolution conformance
/// fixture (`crates/tesela-core/tests/fixtures/property-override-conformance.json`).
///
/// The fixture is the ONE source of truth for per-type property-override
/// merge semantics across the three implementations — Rust
/// (`db/sqlite.rs` `build_overrides`/`apply_override`, consumed by
/// `crates/tesela-core/tests/property_override_conformance.rs`, the source
/// of truth), the web TS consumer (mirroring `buildOverrides`/
/// `applyOverride`), and this file, running the REAL
/// `PropertyRegistry.buildOverrides`/`applyOverride`.
///
/// Adapter contract (mirrors the `_contract` header in the fixture and the
/// Rust/TS consumers): `rows` feed `buildOverrides` directly; `property` is
/// looked up lowercased against the built override map; `definedInRegistry`/
/// `base` construct the starting `PropertyDef` exactly as
/// `resolvedDefs(forTag:)`'s per-property `def`/`stub` branches do; `expect`
/// is compared against the def `applyOverride` returns.
final class PropertyOverrideConformanceTests: XCTestCase {

    private struct FixtureCase {
        let name: String
        let rows: [(overrides: [String: Any], hidden: [String: [String]])]
        let property: String
        let definedInRegistry: Bool
        let base: Base?
        let expect: Expect
    }

    private struct Base {
        let valueType: String
        let choices: [String]
        let def: String?
        let hideByDefault: Bool
    }

    private struct Expect {
        let choices: [String]
        let def: String?
        let show: String
    }

    /// The canonical fixture lives in the Rust crate. The simulator test
    /// host shares the Mac's filesystem, so resolve it relative to this
    /// source file (`#filePath`) — no copied resource to drift.
    private static let fixtureURL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent() // → Tests/
        .deletingLastPathComponent() // → Tesela-iOS/
        .deletingLastPathComponent() // → app/
        .deletingLastPathComponent() // → repo root
        .appendingPathComponent("crates/tesela-core/tests/fixtures/property-override-conformance.json")

    private func loadCases() throws -> [FixtureCase] {
        let data = try Data(contentsOf: Self.fixtureURL)
        let obj = try JSONSerialization.jsonObject(with: data) as! [String: Any]
        let rawCases = obj["cases"] as! [[String: Any]]
        return rawCases.map { c in
            let rawRows = c["rows"] as! [[String: Any]]
            let rows: [(overrides: [String: Any], hidden: [String: [String]])] = rawRows.map { r in
                let overrides = (r["overrides"] as? [String: Any]) ?? [:]
                let hiddenRaw = (r["hidden"] as? [String: Any]) ?? [:]
                var hidden: [String: [String]] = [:]
                for (k, v) in hiddenRaw {
                    hidden[k] = (v as? [Any])?.compactMap { $0 as? String } ?? []
                }
                return (overrides: overrides, hidden: hidden)
            }
            var base: Base?
            if let b = c["base"] as? [String: Any] {
                base = Base(
                    valueType: b["valueType"] as! String,
                    choices: (b["choices"] as? [Any])?.compactMap { $0 as? String } ?? [],
                    def: b["default"] as? String,
                    hideByDefault: (b["hideByDefault"] as? Bool) ?? false
                )
            }
            let e = c["expect"] as! [String: Any]
            let expect = Expect(
                choices: (e["choices"] as? [Any])?.compactMap { $0 as? String } ?? [],
                def: e["default"] as? String,
                show: e["show"] as! String
            )
            return FixtureCase(
                name: c["name"] as! String,
                rows: rows,
                property: c["property"] as! String,
                definedInRegistry: (c["definedInRegistry"] as? Bool) ?? false,
                base: base,
                expect: expect
            )
        }
    }

    /// Mirror of `resolvedDefs(forTag:)`'s per-property `def`/`stub`
    /// branches: a defined registry property starts from its global config;
    /// an undefined one starts from the §3.5c text stub.
    private func startingDef(_ c: FixtureCase) -> PropertyDef {
        if c.definedInRegistry, let b = c.base {
            return PropertyDef(
                name: c.property,
                valueType: PropertyType.parse(b.valueType),
                choices: b.choices,
                def: b.def,
                show: nil,
                hideByDefault: b.hideByDefault,
                hideEmpty: true,
                chipIcon: nil,
                chipLabelMode: nil,
                chipShortLabel: nil,
                chipValueFormat: nil,
                chordKey: nil,
                valueChordKeys: [:],
                choiceColors: [:],
                nlTriggers: []
            )
        }
        return PropertyDef(
            name: c.property,
            valueType: .text,
            choices: [],
            def: nil,
            show: nil,
            hideByDefault: false,
            hideEmpty: true,
            chipIcon: nil,
            chipLabelMode: nil,
            chipShortLabel: nil,
            chipValueFormat: nil,
            chordKey: nil,
            valueChordKeys: [:],
            choiceColors: [:],
            nlTriggers: []
        )
    }

    func testAllConformanceCasesResolveThroughTheRealMerge() throws {
        let cases = try loadCases()
        for c in cases {
            let overrides = PropertyRegistry.buildOverrides(rows: c.rows)
            let over = overrides[c.property.lowercased()]
            let def = startingDef(c)
            let hideByDefault = c.base?.hideByDefault ?? false
            let resolved = PropertyRegistry.applyOverride(def, over, hideByDefault: hideByDefault)
            XCTAssertEqual(resolved.choices, c.expect.choices, "case \(c.name): choices")
            XCTAssertEqual(resolved.def, c.expect.def, "case \(c.name): default")
            XCTAssertEqual(resolved.show?.rawValue, c.expect.show, "case \(c.name): show")
        }
    }

    func testCaseNamesAreUnique() throws {
        let cases = try loadCases()
        var seen = Set<String>()
        for c in cases {
            XCTAssertTrue(seen.insert(c.name).inserted, "duplicate case name: \(c.name)")
        }
    }

    func testFixtureCoversRequiredSurface() throws {
        let cases = try loadCases()
        XCTAssertGreaterThanOrEqual(cases.count, 10, "fixture has \(cases.count) cases; expected at least 10")
        let multiRowCases = cases.filter { $0.rows.count > 1 }.count
        XCTAssertGreaterThanOrEqual(multiRowCases, 2, "expected at least 2 cases exercising a multi-row (extends-chain) walk")
        let stubCases = cases.filter { !$0.definedInRegistry }.count
        XCTAssertGreaterThanOrEqual(stubCases, 1, "expected at least 1 case exercising the §3.5c text-stub branch")
    }
}
