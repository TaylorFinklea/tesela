import XCTest
@testable import Tesela

final class GrCommandTests: XCTestCase {
    func testEmptyQueryReturnsWholeCatalog() {
        XCTAssertEqual(GrCommand.matching("").count, GrCommand.catalog.count)
        XCTAssertEqual(GrCommand.matching("   ").count, GrCommand.catalog.count)
    }

    func testFiltersByLabel() {
        let hits = GrCommand.matching("agenda")
        XCTAssertEqual(hits.map(\.id), ["goto.agenda"])
    }

    func testFiltersByHint() {
        // "deadlines" only appears in the Agenda command's hint.
        XCTAssertEqual(GrCommand.matching("deadline").map(\.id), ["goto.agenda"])
    }

    func testNoMatchIsEmpty() {
        XCTAssertTrue(GrCommand.matching("zzzzz").isEmpty)
    }

    func testNavigationAndActionsPresent() {
        let ids = Set(GrCommand.catalog.map(\.id))
        for id in ["goto.daily", "goto.agenda", "goto.inbox", "goto.library",
                   "goto.search", "action.refresh", "open.settings"] {
            XCTAssertTrue(ids.contains(id), "missing command \(id)")
        }
    }
}
