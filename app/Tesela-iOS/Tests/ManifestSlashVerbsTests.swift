import XCTest
@testable import Tesela

/// tesela-cmdd.5 — iOS's slash base-verb set traces to the shared command
/// manifest (`web/src/lib/command-manifest.json`, tesela-cmdd.2's ONE
/// checked-in extraction point), the same way web's `BlockEditor
/// .getSlashTree` derives its verb leaves from `commandRegistry
/// .availableOn('slash', …)`.
///
/// The simulator test host shares the Mac's filesystem, so the fixture is
/// resolved relative to this source file (`#filePath`) — no copied resource
/// to drift (mirrors `PropertyOverrideConformanceTests`/
/// `RecurrenceConformanceTests`).
final class ManifestSlashVerbsTests: XCTestCase {

    private struct ManifestEntry {
        let id: String
        let category: String
        let surfaces: [String]
    }

    private static let fixtureURL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent() // → Tests/
        .deletingLastPathComponent() // → Tesela-iOS/
        .deletingLastPathComponent() // → app/
        .deletingLastPathComponent() // → repo root
        .appendingPathComponent("web/src/lib/command-manifest.json")

    /// Manifest entries that are `editor` category and `slash`-surfaced but
    /// are NOT top-level slash verbs on web either — `editor.property` is
    /// invoked from the `/p` submenu leaf (no `slashKey`), `editor.widget`
    /// is leader-only. Excluding them here mirrors exactly what
    /// `commandRegistry.availableOn('slash', …).filter(cmd => cmd.slashKey)`
    /// yields on web (`web/src/lib/components/BlockEditor.svelte`).
    private static let notTopLevelSlashVerbs: Set<String> = ["editor.property", "editor.widget"]

    private func loadManifest() throws -> [ManifestEntry] {
        let data = try Data(contentsOf: Self.fixtureURL)
        let raw = try JSONSerialization.jsonObject(with: data) as! [[String: Any]]
        return raw.map { entry in
            ManifestEntry(
                id: entry["id"] as! String,
                category: entry["category"] as! String,
                surfaces: (entry["surfaces"] as? [Any])?.compactMap { $0 as? String } ?? []
            )
        }
    }

    /// The canonical base-verb set: `editor` category, `slash` surface,
    /// minus the structurally-excluded ids above.
    private func canonicalBaseVerbIds() throws -> Set<String> {
        let manifest = try loadManifest()
        return Set(manifest
            .filter { $0.category == "editor" && $0.surfaces.contains("slash") }
            .map { $0.id }
            .filter { !Self.notTopLevelSlashVerbs.contains($0) })
    }

    func testEveryManifestSlashVerbIsCoveredOrExplicitlyOptedOut() throws {
        let manifestIds = try canonicalBaseVerbIds()
        let iosIds = Set(SlashVerbs.base.map { $0.id })
        for id in manifestIds {
            let covered = iosIds.contains(id)
            let optedOut = SlashVerbs.ManifestOptOuts.noHandlerYet.contains(id)
            XCTAssertTrue(
                covered || optedOut,
                "manifest slash verb '\(id)' is neither in SlashVerbs.base nor an explicit opt-out — a forgotten edit"
            )
        }
    }

    func testOptOutsAreActuallyAbsentFromTheManifestCoveredSet() throws {
        // An opt-out that no longer needs to opt out (a handler landed) would
        // silently stop being exercised by the test above — catch that drift
        // explicitly instead.
        let manifestIds = try canonicalBaseVerbIds()
        for id in SlashVerbs.ManifestOptOuts.noHandlerYet {
            XCTAssertTrue(manifestIds.contains(id), "'\(id)' is opted out but is no longer a manifest slash verb — remove the opt-out")
        }
    }

    func testKnownCoreVerbsAreTracedById() throws {
        // Spot-check the well-known set stays present under its manifest id
        // (regression guard: renaming an id here silently breaks tracing).
        let iosIds = Set(SlashVerbs.base.map { $0.id })
        for id in ["editor.link", "editor.tag", "editor.heading", "editor.task", "editor.collection", "editor.query"] {
            XCTAssertTrue(iosIds.contains(id), "expected manifest-traced verb '\(id)' in SlashVerbs.base")
        }
    }

    func testPlatformOnlyVerbsAreNotInTheManifest() throws {
        // The intentional non-manifest additions must NOT collide with a
        // manifest id — otherwise they'd silently mask a real command.
        let manifest = try loadManifest()
        let manifestIds = Set(manifest.map { $0.id })
        let platformOnlyIds = Set(SlashVerbs.base.map { $0.id }).subtracting(try canonicalBaseVerbIds())
        for id in platformOnlyIds {
            XCTAssertFalse(manifestIds.contains(id), "platform-only verb '\(id)' collides with a real manifest id")
        }
    }
}
