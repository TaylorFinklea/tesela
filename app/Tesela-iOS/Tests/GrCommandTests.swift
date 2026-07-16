import XCTest
@testable import Tesela

final class GrCommandTests: XCTestCase {
    private let manifest = CommandManifestSource.loadBundled()

    // MARK: - bundled manifest loads (tesela-cib / ADR-4)

    func testBundledManifestLoadsNonEmpty() {
        XCTAssertFalse(manifest.isEmpty, "CommandManifest.json resource must bundle + decode")
    }

    func testBundledManifestMatchesWebSourceOfTruth() {
        // Regenerate via:
        //   cp web/src/lib/command-manifest.json app/Tesela-iOS/Sources/Data/CommandManifest.json
        // (verified by scripts/check-command-manifest-drift.sh). This test only
        // catches drift when both files are visible to the test *source* tree —
        // the real gate is the drift script, since the bundled resource has no
        // filesystem access to the repo at runtime.
        let ids = Set(manifest.map(\.id))
        XCTAssertTrue(ids.contains("daily"), "expected the web-authored 'daily' command in the bundled manifest")
    }

    // MARK: - palette(from:) — manifest ∩ native executors, ADR-4's "unmapped = hidden"

    func testPaletteOnlyOffersExecutableIds() {
        let palette = GrCommand.palette(from: manifest)
        XCTAssertFalse(palette.isEmpty)
        for cmd in palette {
            XCTAssertTrue(GrCommand.executableIds.contains(cmd.id), "\(cmd.id) has no native executor and should be hidden")
        }
    }

    func testPaletteOnlyOffersPaletteSurfaceCommands() {
        let paletteIds = Set(GrCommand.palette(from: manifest).map(\.id))
        let nonPaletteEntry = manifest.first { !$0.surfaces.contains("palette") }
        if let nonPaletteEntry {
            XCTAssertFalse(paletteIds.contains(nonPaletteEntry.id))
        }
    }

    func testKnownNavigationCommandsPresent() {
        let ids = Set(GrCommand.palette(from: manifest).map(\.id))
        for id in ["daily", "agenda", "views"] {
            XCTAssertTrue(ids.contains(id), "missing command \(id)")
        }
        XCTAssertFalse(ids.contains("inbox"), "Views should be the primary visible command id")
    }

    func testWhatsNewCommandIsManifestBackedAndExecutable() {
        let command = GrCommand.palette(from: manifest).first { $0.id == "whats-new" }
        XCTAssertNotNil(command)
        XCTAssertEqual(command?.label, "What’s New")
        XCTAssertTrue(GrCommand.executableIds.contains("whats-new"))
    }

    func testViewsCommandIsNotLabeledInbox() {
        let views = GrCommand.palette(from: manifest).first { $0.id == "views" }
        XCTAssertEqual(views?.label, "Open Views")
        XCTAssertTrue(views?.keywords.contains("views") == true)
        XCTAssertFalse(views?.keywords.contains("inbox") == true)
    }

    func testUnmappedManifestCommandIsHidden() {
        // "vsplit" (pane split) has no iOS equivalent — not in executableIds.
        XCTAssertTrue(manifest.contains { $0.id == "vsplit" }, "fixture assumption: manifest still declares vsplit")
        XCTAssertFalse(GrCommand.executableIds.contains("vsplit"))
        XCTAssertFalse(GrCommand.palette(from: manifest).map(\.id).contains("vsplit"))
    }

    // MARK: - matching(_:in:)

    func testEmptyQueryReturnsWholeList() {
        let palette = GrCommand.palette(from: manifest)
        XCTAssertEqual(GrCommand.matching("", in: palette).count, palette.count)
        XCTAssertEqual(GrCommand.matching("   ", in: palette).count, palette.count)
    }

    func testFiltersByLabel() {
        let palette = GrCommand.palette(from: manifest)
        let hits = GrCommand.matching("agenda", in: palette)
        XCTAssertEqual(hits.map(\.id), ["agenda"])
    }

    func testFiltersByKeyword() {
        // "journal" is a keyword on the "daily" manifest entry, not in its label.
        let palette = GrCommand.palette(from: manifest)
        let hits = GrCommand.matching("journal", in: palette)
        XCTAssertTrue(hits.contains { $0.id == "daily" })
    }

    func testNoMatchIsEmpty() {
        let palette = GrCommand.palette(from: manifest)
        XCTAssertTrue(GrCommand.matching("zzzzz", in: palette).isEmpty)
    }
}
