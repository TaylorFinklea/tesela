import XCTest
@testable import Tesela

@MainActor
final class DashboardWidgetsTests: XCTestCase {
    func testDefaultLayoutStartsWithPerfectedBuiltins() {
        XCTAssertEqual(
            DashboardWidgetLayout.defaultLayout.placements.map(\.id),
            ["builtin:agenda", "builtin:inbox", "builtin:sync-health"]
        )
    }

    func testNormalizationDeduplicatesAndPreservesUnavailableSources() {
        let layout = DashboardWidgetLayout(
            version: 1,
            placements: [
                DashboardWidgetPlacement(id: "query:alpha", fallbackTitle: "Alpha", collapsed: true),
                DashboardWidgetPlacement(id: "query:alpha", fallbackTitle: "Duplicate", collapsed: false),
                DashboardWidgetPlacement(id: "view:missing", fallbackTitle: "Missing view", collapsed: false),
                DashboardWidgetPlacement(id: "invalid", fallbackTitle: "Invalid", collapsed: false),
            ]
        ).normalized()

        XCTAssertEqual(layout.placements.map(\.id), ["query:alpha", "view:missing"])
        XCTAssertTrue(layout.placements[0].collapsed)
        XCTAssertEqual(layout.placements[1].fallbackTitle, "Missing view")
    }

    func testAddRemoveMoveAndCollapseAreStableIDOperations() {
        let candidate = DashboardWidgetCandidate(
            id: "query:weekly", sourceKind: .query, sourceID: "weekly",
            title: "Weekly", subtitle: "Query note", icon: "calendar"
        )
        let added = DashboardWidgetLayout.defaultLayout.adding(candidate)
        XCTAssertEqual(added.placements.last?.id, candidate.id)
        XCTAssertEqual(added.adding(candidate), added, "adding an existing stable id is idempotent")

        let moved = added.moving(candidate.id, by: -1)
        XCTAssertEqual(moved.placements[moved.placements.count - 2].id, candidate.id)

        let collapsed = moved.toggling(candidate.id)
        XCTAssertTrue(collapsed.placements.first(where: { $0.id == candidate.id })?.collapsed == true)

        let removed = collapsed.removing(candidate.id)
        XCTAssertFalse(removed.placements.contains(where: { $0.id == candidate.id }))
    }

    func testLayoutPersistsAnExplicitlyEmptyDashboard() throws {
        let suiteName = "DashboardWidgetsTests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let empty = DashboardWidgetLayout(version: 1, placements: [])
        empty.save(defaults: defaults)

        XCTAssertEqual(DashboardWidgetLayout.load(defaults: defaults), empty)
    }

    func testSavedViewRevisionChangesWithProjectionDependencies() {
        let base = SavedView(
            id: "view-1", name: "Work", dsl: "tag:work", order: 10,
            builtin: false, displayMode: "list", displayGroupBy: nil, displayShowDone: nil
        )
        var changed = base
        changed.displayGroupBy = "status"

        XCTAssertNotEqual(base.dashboardRevision, changed.dashboardRevision)
        XCTAssertEqual(DashboardQueryProjection(view: base).id, "view:view-1")
        XCTAssertEqual(DashboardQueryProjection(view: SavedView.fallbackInbox).id, "builtin:inbox")
    }
}
