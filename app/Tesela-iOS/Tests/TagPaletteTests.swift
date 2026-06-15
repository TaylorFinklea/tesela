import XCTest
@testable import Tesela

/// Parity guard for `TagPalette` — the iOS port of web `tag-color.ts`. The
/// FNV-1a hash + palette + named-override resolution must agree with the web
/// so a tag is the same color on every surface. Expected indices were computed
/// by running the web `paletteIndex` (the canonical implementation) directly.
final class TagPaletteTests: XCTestCase {

    /// Indices captured from the web `paletteIndex` for these exact names.
    /// If the Swift hash drifts from the web, this breaks loudly.
    func testHashIndicesMatchWeb() {
        let expected: [(String, Int)] = [
            ("task", 6),
            ("l5test", 2),
            ("nature/birds", 3),
            ("project", 8),
            ("testpoints", 2),
            ("event", 7),
            ("note", 9),
            ("person", 8),
            ("query", 3),
        ]
        for (name, idx) in expected {
            XCTAssertEqual(TagPalette.index(for: name), idx, "index drift for \(name)")
        }
    }

    /// Casing must not change the color (web lowercases before hashing).
    func testHashIsCaseInsensitive() {
        XCTAssertEqual(TagPalette.index(for: "Task"), TagPalette.index(for: "task"))
        XCTAssertEqual(TagPalette.index(for: "TestPoints"), TagPalette.index(for: "testpoints"))
    }

    /// `color::` overrides: 6-digit hex, 3-digit hex (expanded), named hue,
    /// and case-insensitive names; unrecognized → nil (falls back to hash).
    func testResolveOverride() {
        XCTAssertEqual(TagPalette.resolveOverride("#E8697F"), 0xE8697F)
        XCTAssertEqual(TagPalette.resolveOverride("#abc"), 0xAABBCC)
        XCTAssertEqual(TagPalette.resolveOverride("coral"), 0xFF6B5A)
        XCTAssertEqual(TagPalette.resolveOverride("CORAL"), 0xFF6B5A)
        XCTAssertEqual(TagPalette.resolveOverride("  teal  "), 0x62B8CE)
        XCTAssertNil(TagPalette.resolveOverride("notacolor"))
        XCTAssertNil(TagPalette.resolveOverride(""))
    }
}
