import XCTest
@testable import Tesela

final class BlockParserTests: XCTestCase {

    // MARK: - Parsing

    func testParsesSingleBlock() {
        let md = "- Hello world"
        let blocks = BlockParser.parse(markdown: md)
        XCTAssertEqual(blocks.count, 1)
        XCTAssertEqual(blocks[0].text, "Hello world")
    }

    func testParsesMultipleRootBlocks() {
        let md = """
        - First block
        - Second block
        - Third block
        """
        let blocks = BlockParser.parse(markdown: md)
        XCTAssertEqual(blocks.count, 3)
        XCTAssertEqual(blocks[0].text, "First block")
        XCTAssertEqual(blocks[2].text, "Third block")
    }

    func testParsesNestedBlocks() {
        let md = """
        - Parent
          - Child
            - Grandchild
        """
        let blocks = BlockParser.parse(markdown: md)
        XCTAssertEqual(blocks.count, 1)
        XCTAssertEqual(blocks[0].children.count, 1)
        XCTAssertEqual(blocks[0].children[0].text, "Child")
        XCTAssertEqual(blocks[0].children[0].children.count, 1)
        XCTAssertEqual(blocks[0].children[0].children[0].text, "Grandchild")
    }

    func testExtractsTags() {
        let tags = BlockParser.extractTags(from: "Hello #world and #swift")
        XCTAssertEqual(Set(tags), Set(["world", "swift"]))
    }

    func testExtractsTodoState() {
        let (state, text) = BlockParser.extractTodo(from: "TODO Buy groceries")
        XCTAssertEqual(state, .todo)
        XCTAssertEqual(text, "Buy groceries")
    }

    func testExtractsDoneState() {
        let (state, _) = BlockParser.extractTodo(from: "DONE Finished task")
        XCTAssertEqual(state, .done)
    }

    func testNoTodoState() {
        let (state, text) = BlockParser.extractTodo(from: "Regular block")
        XCTAssertNil(state)
        XCTAssertEqual(text, "Regular block")
    }

    func testExtractsWikiLinks() {
        let links = BlockParser.extractWikiLinks(from: "See [[Page A]] and [[Another Page]]")
        XCTAssertEqual(Set(links), Set(["Page A", "Another Page"]))
    }

    // MARK: - Serialization

    func testRoundTrip() {
        let original = """
        - First
          - Nested
        - Second
        """
        let blocks = BlockParser.parse(markdown: original)
        let serialized = BlockParser.serialize(blocks: blocks)
        let reparsed = BlockParser.parse(markdown: serialized)

        XCTAssertEqual(reparsed.count, 2)
        XCTAssertEqual(reparsed[0].text, "First")
        XCTAssertEqual(reparsed[0].children.count, 1)
        XCTAssertEqual(reparsed[1].text, "Second")
    }

    func testSerializesSingleBlock() {
        let block = Block(text: "Hello")
        let result = BlockParser.serialize(blocks: [block])
        XCTAssertEqual(result, "- Hello")
    }

    func testSerializesNestedBlocks() {
        let child = Block(text: "Child", indentLevel: 1)
        let parent = Block(text: "Parent", children: [child])
        let result = BlockParser.serialize(blocks: [parent])
        let expected = "- Parent\n  - Child"
        XCTAssertEqual(result, expected)
    }

    // MARK: - Edge cases

    func testEmptyMarkdown() {
        let blocks = BlockParser.parse(markdown: "")
        XCTAssertTrue(blocks.isEmpty)
    }

    func testSkipsNonBlockLines() {
        let md = """
        # This is a heading
        - This is a block
        Regular prose line
        - Another block
        """
        let blocks = BlockParser.parse(markdown: md)
        XCTAssertEqual(blocks.count, 2)
    }

    func testTagsNotIncludedInBlockText() {
        let md = "- Meeting notes #work #important"
        let blocks = BlockParser.parse(markdown: md)
        XCTAssertEqual(blocks.count, 1)
        XCTAssertEqual(Set(blocks[0].tags), Set(["work", "important"]))
        XCTAssertFalse(blocks[0].text.contains("#work"))
    }
}
