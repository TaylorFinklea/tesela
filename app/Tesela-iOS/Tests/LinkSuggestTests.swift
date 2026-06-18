import XCTest
@testable import Tesela

/// Locks the pure `[[` page-link autocomplete logic: detecting when the
/// caret sits inside an open wikilink, and ranking page candidates.
final class LinkSuggestTests: XCTestCase {

    // MARK: detectQuery — trigger + query span

    func testOpenLinkJustAfterBrackets() {
        // "ab[[" caret at end (4) → opener at 2, empty query.
        let hit = LinkSuggest.detectQuery(in: "ab[[", caretUTF16: 4)
        XCTAssertEqual(hit?.start, 2)
        XCTAssertEqual(hit?.query, "")
    }

    func testOpenLinkWithQuery() {
        // "see [[pro" caret at end (9) → opener at 4, query "pro".
        let hit = LinkSuggest.detectQuery(in: "see [[pro", caretUTF16: 9)
        XCTAssertEqual(hit?.start, 4)
        XCTAssertEqual(hit?.query, "pro")
    }

    func testClosedLinkIsNotActive() {
        // Caret after a completed "[[Page]]" → no open link.
        XCTAssertNil(LinkSuggest.detectQuery(in: "[[Page]]", caretUTF16: 8))
    }

    func testNewlineBreaksTheLink() {
        // A newline between "[[" and the caret cancels the trigger.
        XCTAssertNil(LinkSuggest.detectQuery(in: "[[a\nb", caretUTF16: 5))
    }

    func testNoBracketsNoTrigger() {
        XCTAssertNil(LinkSuggest.detectQuery(in: "plain text", caretUTF16: 10))
    }

    func testCaretBeforeQueryEnd() {
        // "[[proj" but caret sits right after "[[" (2) → query is "" even
        // though more text follows (we only read up to the caret).
        let hit = LinkSuggest.detectQuery(in: "[[proj", caretUTF16: 2)
        XCTAssertEqual(hit?.start, 0)
        XCTAssertEqual(hit?.query, "")
    }

    func testSecondOpenLinkWins() {
        // Two openers; the caret is inside the second.
        let hit = LinkSuggest.detectQuery(in: "[[one]] and [[tw", caretUTF16: 16)
        XCTAssertEqual(hit?.query, "tw")
    }

    // MARK: rank — relevance ordering

    private func page(_ title: String, slug: String? = nil) -> Page {
        Page(id: slug ?? title.lowercased(), title: title, slug: slug ?? title.lowercased(),
             type: "note", edited: "", blocks: 0, refs: 0)
    }

    func testRankPrefersPrefixOverSubsequence() {
        let pages = [page("Pinboard"), page("Project Atlas"), page("Roadmap")]
        let ranked = LinkSuggest.rank(pages, query: "proj", limit: 10)
        XCTAssertEqual(ranked.first?.title, "Project Atlas")
    }

    func testRankDropsNonMatches() {
        let pages = [page("Alpha"), page("Beta")]
        XCTAssertTrue(LinkSuggest.rank(pages, query: "zzz", limit: 10).isEmpty)
    }

    func testRankRespectsLimit() {
        let pages = (0..<20).map { page("Note \($0)", slug: "note-\($0)") }
        XCTAssertEqual(LinkSuggest.rank(pages, query: "note", limit: 5).count, 5)
    }
}
