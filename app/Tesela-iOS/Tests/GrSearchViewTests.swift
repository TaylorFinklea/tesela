import XCTest
@testable import Tesela

@MainActor
final class GrSearchViewTests: XCTestCase {
    func testInitMatchesGraphiteInboxBackendShape() {
        let view = GrSearchView(mosaic: MockMosaicService(), backend: BackendSettings())
        withExtendedLifetime(view) {
            XCTAssertTrue(true)
        }
    }

    func testSectionsGroupInPageBlockTagOrderWithCounts() {
        let results = [
            SearchResult(id: "block-a", kind: .block, title: "Block A", snippet: "alpha"),
            SearchResult(id: "page-a", kind: .page, title: "Page A", snippet: "beta"),
            SearchResult(id: "tag-a", kind: .tag, title: "Tag A", snippet: "gamma"),
            SearchResult(id: "block-b", kind: .block, title: "Block B", snippet: "delta"),
        ]

        let sections = GrSearchView.sections(for: results)

        XCTAssertEqual(sections.map(\.kind), [.page, .block, .tag])
        XCTAssertEqual(sections.map(\.title), ["Pages", "Blocks", "Tags"])
        XCTAssertEqual(sections.map(\.count), [1, 2, 1])
        XCTAssertEqual(sections.map { $0.results.map(\.id) }, [["page-a"], ["block-a", "block-b"], ["tag-a"]])
    }
}
