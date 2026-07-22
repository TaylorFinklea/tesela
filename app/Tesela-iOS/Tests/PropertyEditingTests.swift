import XCTest
@testable import Tesela

@MainActor
final class PropertyEditingTests: XCTestCase {
    func testMultiSelectDeltaPreservesMembersAndEmitsIndependentChanges() {
        XCTAssertEqual(PropertyEditing.multiSelectValues("alpha, beta, alpha"), ["alpha", "beta"])
        XCTAssertEqual(
            PropertyEditing.multiSelectDelta(
                currentValue: "alpha, beta",
                selected: ["beta", "gamma"]
            ),
            PropertyListDelta(current: ["alpha", "beta"], add: ["gamma"], remove: ["alpha"])
        )
    }

    func testCheckboxToggleIsCanonical() {
        XCTAssertTrue(PropertyEditing.isChecked("TRUE"))
        XCTAssertEqual(PropertyEditing.toggledCheckboxValue("true"), "false")
        XCTAssertEqual(PropertyEditing.toggledCheckboxValue("false"), "true")
    }

    func testLinksNormalizeAndRejectUnsafeSchemes() {
        XCTAssertEqual(
            PropertyEditing.linkURL(valueType: .url, value: "example.com")?.absoluteString,
            "https://example.com"
        )
        XCTAssertNil(PropertyEditing.linkURL(valueType: .url, value: "javascript:alert(1)"))
        XCTAssertEqual(
            PropertyEditing.linkURL(valueType: .email, value: "hello@example.com")?.absoluteString,
            "mailto:hello@example.com"
        )
        XCTAssertEqual(
            PropertyEditing.linkURL(valueType: .phone, value: "+1 (312) 555-0199")?.absoluteString,
            "tel:+13125550199"
        )
    }

    func testNodeCandidatesRankAliasesWithoutReplacingCanonicalPageId() {
        let canonical = "11111111-1111-5111-8111-111111111111"
        let candidates = [
            NodePageCandidate(
                pageId: canonical,
                slug: "roadmap",
                title: "Roadmap",
                aliases: ["Plan"]
            ),
            NodePageCandidate(
                pageId: "22222222-2222-5222-8222-222222222222",
                slug: "planning",
                title: "Planning notes"
            ),
        ]

        XCTAssertEqual(
            PropertyEditing.rankNodeCandidates(candidates, query: "plan", limit: 10)
                .map(\.pageId),
            [canonical, "22222222-2222-5222-8222-222222222222"]
        )
    }

    func testNodeDirectoryPresentationFailsClosedForDeletedAndConflictingBindings() {
        let pageId = "11111111-1111-5111-8111-111111111111"
        let live = PageDirectoryEntry(
            pageId: pageId,
            loroDocId: "live",
            slug: "roadmap",
            title: "Roadmap",
            aliases: [],
            deleted: false,
            forwardToLoroDocId: nil,
            conflict: false
        )
        XCTAssertEqual(
            MockMosaicService.nodePageResolution(for: pageId, directory: [live]),
            .resolved(title: "Roadmap", slug: "roadmap")
        )

        var deleted = live
        deleted.deleted = true
        XCTAssertEqual(
            MockMosaicService.nodePageResolution(for: pageId, directory: [deleted]),
            .deleted
        )

        var conflicted = live
        conflicted.conflict = true
        XCTAssertEqual(
            MockMosaicService.nodePageResolution(for: pageId, directory: [conflicted]),
            .conflict
        )
        XCTAssertEqual(NodePageResolution.unresolved.label(for: "legacy title"), "Unresolved: legacy title")
    }

    func testNodeDirectoryResolutionTreatsAuthoritativeEmptyDirectoryAsUnresolved() {
        XCTAssertEqual(
            MockMosaicService.nodePageResolution(
                for: "11111111-1111-5111-8111-111111111111",
                directory: []
            ),
            .unresolved
        )
    }
}
