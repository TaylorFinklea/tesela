import XCTest
@testable import Tesela

final class BlockParserTests: XCTestCase {

    // MARK: - extractTags

    func testExtractsTagAfterSpace() {
        let tags = BlockParser.extractTags(from: "Hello #world today")
        XCTAssertEqual(tags, ["world"])
    }

    func testExtractsTagAtEndOfLine() {
        let tags = BlockParser.extractTags(from: "Hello #world")
        XCTAssertEqual(tags, ["world"])
    }

    func testExtractsMultipleTags() {
        let tags = BlockParser.extractTags(from: "Hello #world and #swift")
        XCTAssertEqual(Set(tags), Set(["world", "swift"]))
    }

    func testExtractsNoTagsFromPlainText() {
        let tags = BlockParser.extractTags(from: "No tags here at all")
        XCTAssertTrue(tags.isEmpty)
    }

    func testExtractsTagWithDashes() {
        let tags = BlockParser.extractTags(from: "Task #follow-up later")
        XCTAssertEqual(tags, ["follow-up"])
    }

    func testExtractsTagWithUnderscores() {
        let tags = BlockParser.extractTags(from: "Note #my_project done")
        XCTAssertEqual(tags, ["my_project"])
    }

    func testExtractsTagWithSlash() {
        let tags = BlockParser.extractTags(from: "Nested #projects/tesela work")
        XCTAssertEqual(tags, ["projects/tesela"])
    }

    func testExtractsTagStartingWithNumber() {
        let tags = BlockParser.extractTags(from: "Ship #2026plan soon")
        XCTAssertEqual(tags, ["2026plan"])
    }

    func testTagExtractionIsCaseSensitive() {
        // Tag names preserve the exact casing from the source text
        let tags = BlockParser.extractTags(from: "Meeting #Work and #work")
        XCTAssertEqual(Set(tags), Set(["Work", "work"]))
    }

    func testTagFollowedByPunctuation() {
        // Tags followed by punctuation are still extracted
        let tags = BlockParser.extractTags(from: "See #project, and #task.")
        XCTAssertEqual(Set(tags), Set(["project", "task"]))
    }

    func testBareHashIsNotATag() {
        // A bare "#" with no following characters is not a tag
        let tags = BlockParser.extractTags(from: "Version #")
        XCTAssertTrue(tags.isEmpty)
    }

    func testHashFollowedBySpaceIsNotATag() {
        let tags = BlockParser.extractTags(from: "Some # text")
        XCTAssertTrue(tags.isEmpty)
    }

    // MARK: - extractTagsLive

    func testLiveExtractsTagAfterSpace() {
        let tags = BlockParser.extractTagsLive(from: "Hello #world today")
        XCTAssertEqual(tags, ["world"])
    }

    func testLiveDoesNotExtractTagAtEndOfLine() {
        // While the user is still typing #world, it should NOT be extracted
        let tags = BlockParser.extractTagsLive(from: "Hello #world")
        XCTAssertTrue(tags.isEmpty)
    }

    func testLiveExtractsCompletedTagsButNotTrailingOne() {
        // #work is followed by a space so it's complete; #wip is at end so it's pending
        let tags = BlockParser.extractTagsLive(from: "Note #work and #wip")
        XCTAssertEqual(tags, ["work"])
        XCTAssertFalse(tags.contains("wip"))
    }

    func testLiveExtractsMultipleTagsWhenAllHaveTrailingSpace() {
        let tags = BlockParser.extractTagsLive(from: "#alpha #beta text")
        XCTAssertEqual(Set(tags), Set(["alpha", "beta"]))
    }

    func testLiveExtractsNoTagsFromPlainText() {
        let tags = BlockParser.extractTagsLive(from: "No tags here")
        XCTAssertTrue(tags.isEmpty)
    }

    func testLiveTagWithDashes() {
        let tags = BlockParser.extractTagsLive(from: "see #follow-up note")
        XCTAssertEqual(tags, ["follow-up"])
    }

    func testLiveTagWithUnderscores() {
        let tags = BlockParser.extractTagsLive(from: "check #my_project again")
        XCTAssertEqual(tags, ["my_project"])
    }

    func testLiveTagWithSlash() {
        let tags = BlockParser.extractTagsLive(from: "see #projects/tesela next")
        XCTAssertEqual(tags, ["projects/tesela"])
    }

    func testLiveTagStartingWithNumber() {
        let tags = BlockParser.extractTagsLive(from: "track #2026plan next")
        XCTAssertEqual(tags, ["2026plan"])
    }

    // MARK: - extractProperties

    func testExtractsSingleProperty() {
        let props = BlockParser.extractProperties(from: "priority:: high")
        XCTAssertEqual(props["priority"], "high")
    }

    func testExtractsMultipleProperties() {
        let text = "Some text\npriority:: high\ndeadline:: [[2026-01-01]]"
        let props = BlockParser.extractProperties(from: text)
        XCTAssertEqual(props["priority"], "high")
        XCTAssertEqual(props["deadline"], "[[2026-01-01]]")
    }

    func testExtractsNoPropertiesFromPlainText() {
        let props = BlockParser.extractProperties(from: "Just a plain block")
        XCTAssertTrue(props.isEmpty)
    }

    func testExtractsPropertyWithSpacesInValue() {
        let props = BlockParser.extractProperties(from: "effort:: 1 hour 30 min")
        XCTAssertEqual(props["effort"], "1 hour 30 min")
    }

    func testExtractsPropertyWithUnderscoredKey() {
        let props = BlockParser.extractProperties(from: "my_key:: some value")
        XCTAssertEqual(props["my_key"], "some value")
    }

    func testPropertyRequiresDoubleColon() {
        // Single colon should not be extracted as a property
        let props = BlockParser.extractProperties(from: "time: 9am")
        XCTAssertNil(props["time"])
    }

    func testExtractsPropertyOnContinuationLine() {
        let text = "Block text\nstatus:: todo"
        let props = BlockParser.extractProperties(from: text)
        XCTAssertEqual(props["status"], "todo")
    }

    // MARK: - parse

    func testParsesSingleBlock() {
        let blocks = BlockParser.parse(markdown: "- Hello world")
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

    func testParseRootBlockHasIndentLevelZero() {
        let blocks = BlockParser.parse(markdown: "- Root")
        XCTAssertEqual(blocks[0].indentLevel, 0)
    }

    func testParseChildBlockHasIndentLevelOne() {
        let md = """
        - Parent
          - Child
        """
        let blocks = BlockParser.parse(markdown: md)
        // Children in the tree: indentLevel is set by flatten, but parse reflects the raw indent
        XCTAssertEqual(blocks[0].children[0].indentLevel, 1)
    }

    func testParseExtractsTagsFromBlock() {
        let md = "- Meeting notes #work #important"
        let blocks = BlockParser.parse(markdown: md)
        XCTAssertEqual(Set(blocks[0].tags), Set(["work", "important"]))
    }

    func testParseExtractsPriorityProperty() {
        let md = "- Task item\n  priority:: high"
        let blocks = BlockParser.parse(markdown: md)
        XCTAssertEqual(blocks[0].priority, Priority.high)
    }

    func testParseExtractsDeadlineStrippingWikiLink() {
        let md = "- Task item\n  deadline:: [[2026-03-30]]"
        let blocks = BlockParser.parse(markdown: md)
        XCTAssertEqual(blocks[0].deadline, "2026-03-30")
    }

    func testParseExtractsScheduledStrippingWikiLink() {
        let md = "- Task item\n  scheduled:: [[2026-04-01]]"
        let blocks = BlockParser.parse(markdown: md)
        XCTAssertEqual(blocks[0].scheduled, "2026-04-01")
    }

    func testParseExtractsEffortProperty() {
        let md = "- Task item\n  effort:: 30m"
        let blocks = BlockParser.parse(markdown: md)
        XCTAssertEqual(blocks[0].effort, "30m")
    }

    func testParseContinuationLineAppendsToBlockText() {
        let md = "- Block text\n  continuation line"
        let blocks = BlockParser.parse(markdown: md)
        XCTAssertEqual(blocks.count, 1)
        XCTAssertTrue(blocks[0].text.contains("continuation line"))
    }

    func testParseIgnoresBlankLines() {
        let md = """
        - First

        - Second
        """
        let blocks = BlockParser.parse(markdown: md)
        XCTAssertEqual(blocks.count, 2)
    }

    func testParseSiblingBlocksAtSameIndent() {
        let md = """
        - Parent
          - Child A
          - Child B
        """
        let blocks = BlockParser.parse(markdown: md)
        XCTAssertEqual(blocks[0].children.count, 2)
        XCTAssertEqual(blocks[0].children[0].text, "Child A")
        XCTAssertEqual(blocks[0].children[1].text, "Child B")
    }

    func testParseEmptyMarkdownReturnsEmptyArray() {
        let blocks = BlockParser.parse(markdown: "")
        XCTAssertTrue(blocks.isEmpty)
    }

    func testParseTextContentIsPreserved() {
        let md = "- This block has #tag text"
        let blocks = BlockParser.parse(markdown: md)
        // Raw text includes the tag
        XCTAssertTrue(blocks[0].text.contains("#tag"))
    }

    // MARK: - flatten

    func testFlattenSingleBlock() {
        let block = Block(text: "Root")
        let flat = BlockParser.flatten(blocks: [block])
        XCTAssertEqual(flat.count, 1)
        XCTAssertEqual(flat[0].indentLevel, 0)
    }

    func testFlattenSetsIndentLevelFromDepth() {
        let child = Block(text: "Child")
        let parent = Block(text: "Parent", children: [child])
        let flat = BlockParser.flatten(blocks: [parent])
        XCTAssertEqual(flat.count, 2)
        XCTAssertEqual(flat[0].indentLevel, 0)
        XCTAssertEqual(flat[1].indentLevel, 1)
    }

    func testFlattenDeepNesting() {
        let grandchild = Block(text: "Grandchild")
        let child = Block(text: "Child", children: [grandchild])
        let parent = Block(text: "Parent", children: [child])
        let flat = BlockParser.flatten(blocks: [parent])
        XCTAssertEqual(flat.count, 3)
        XCTAssertEqual(flat[0].indentLevel, 0)
        XCTAssertEqual(flat[1].indentLevel, 1)
        XCTAssertEqual(flat[2].indentLevel, 2)
    }

    func testFlattenOrderIsDepthFirst() {
        let child1 = Block(text: "Child1")
        let child2 = Block(text: "Child2")
        let parent = Block(text: "Parent", children: [child1, child2])
        let flat = BlockParser.flatten(blocks: [parent])
        XCTAssertEqual(flat[0].text, "Parent")
        XCTAssertEqual(flat[1].text, "Child1")
        XCTAssertEqual(flat[2].text, "Child2")
    }

    func testFlattenMultipleRootsWithChildren() {
        let childA = Block(text: "ChildA")
        let rootA = Block(text: "RootA", children: [childA])
        let rootB = Block(text: "RootB")
        let flat = BlockParser.flatten(blocks: [rootA, rootB])
        XCTAssertEqual(flat.count, 3)
        XCTAssertEqual(flat[0].text, "RootA")
        XCTAssertEqual(flat[0].indentLevel, 0)
        XCTAssertEqual(flat[1].text, "ChildA")
        XCTAssertEqual(flat[1].indentLevel, 1)
        XCTAssertEqual(flat[2].text, "RootB")
        XCTAssertEqual(flat[2].indentLevel, 0)
    }

    // MARK: - serializeFlat

    func testSerializeFlatSingleBlock() {
        let block = Block(text: "Hello", indentLevel: 0)
        let result = BlockParser.serializeFlat(blocks: [block])
        XCTAssertEqual(result, "- Hello")
    }

    func testSerializeFlatUsesIndentLevel() {
        let parent = Block(text: "Parent", indentLevel: 0)
        let child = Block(text: "Child", indentLevel: 1)
        let result = BlockParser.serializeFlat(blocks: [parent, child])
        let lines = result.components(separatedBy: "\n")
        XCTAssertEqual(lines[0], "- Parent")
        XCTAssertEqual(lines[1], "  - Child")
    }

    func testSerializeFlatMultilineBlock() {
        let block = Block(text: "First line\nstatus:: todo", indentLevel: 0)
        let result = BlockParser.serializeFlat(blocks: [block])
        let lines = result.components(separatedBy: "\n")
        XCTAssertEqual(lines[0], "- First line")
        XCTAssertEqual(lines[1], "  status:: todo")
    }

    func testSerializeFlatMultilineBlockAtIndentOne() {
        // Continuation lines are indented relative to their block's indentLevel
        let block = Block(text: "Block text\nkey:: value", indentLevel: 1)
        let result = BlockParser.serializeFlat(blocks: [block])
        let lines = result.components(separatedBy: "\n")
        XCTAssertEqual(lines[0], "  - Block text")
        XCTAssertEqual(lines[1], "    key:: value")
    }

    func testSerializeFlatRoundTrip() {
        let md = """
        - First
          - Nested
        - Second
        """
        let tree = BlockParser.parse(markdown: md)
        let flat = BlockParser.flatten(blocks: tree)
        let serialized = BlockParser.serializeFlat(blocks: flat)
        let reparsed = BlockParser.parse(markdown: serialized)

        XCTAssertEqual(reparsed.count, 2)
        XCTAssertEqual(reparsed[0].text, "First")
        XCTAssertEqual(reparsed[0].children.count, 1)
        XCTAssertEqual(reparsed[0].children[0].text, "Nested")
        XCTAssertEqual(reparsed[1].text, "Second")
    }

    func testSerializeFlatRoundTripWithTags() {
        let md = "- Meeting #work #important"
        let tree = BlockParser.parse(markdown: md)
        let flat = BlockParser.flatten(blocks: tree)
        let serialized = BlockParser.serializeFlat(blocks: flat)
        let reparsed = BlockParser.parse(markdown: serialized)

        XCTAssertEqual(reparsed.count, 1)
        XCTAssertEqual(Set(reparsed[0].tags), Set(["work", "important"]))
    }

    // MARK: - stripWikiLink

    func testStripWikiLinkRemovesBrackets() {
        let result = BlockParser.stripWikiLink("[[2026-03-30]]")
        XCTAssertEqual(result, "2026-03-30")
    }

    func testStripWikiLinkReturnsNilForNilInput() {
        let result = BlockParser.stripWikiLink(nil)
        XCTAssertNil(result)
    }

    func testStripWikiLinkReturnsOriginalWhenNoBrackets() {
        let result = BlockParser.stripWikiLink("plain-value")
        XCTAssertEqual(result, "plain-value")
    }

    func testStripWikiLinkReturnsNilForEmptyBrackets() {
        let result = BlockParser.stripWikiLink("[[]]")
        XCTAssertNil(result)
    }

    func testStripWikiLinkDoesNotStripPartialBrackets() {
        // Only strips when BOTH [[ prefix and ]] suffix are present
        let onlyOpen = BlockParser.stripWikiLink("[[value")
        XCTAssertEqual(onlyOpen, "[[value")

        let onlyClose = BlockParser.stripWikiLink("value]]")
        XCTAssertEqual(onlyClose, "value]]")
    }

    func testStripWikiLinkWithPageName() {
        let result = BlockParser.stripWikiLink("[[My Page Name]]")
        XCTAssertEqual(result, "My Page Name")
    }

    // MARK: - strippedWikiLink (non-optional)

    func testStrippedWikiLinkRemovesBrackets() {
        let result = BlockParser.strippedWikiLink("[[2026-03-30]]")
        XCTAssertEqual(result, "2026-03-30")
    }

    func testStrippedWikiLinkReturnsOriginalWhenNoBrackets() {
        let result = BlockParser.strippedWikiLink("plain-value")
        XCTAssertEqual(result, "plain-value")
    }

    func testStrippedWikiLinkWithPageName() {
        let result = BlockParser.strippedWikiLink("[[Some Page]]")
        XCTAssertEqual(result, "Some Page")
    }

    func testStrippedWikiLinkDoesNotStripPartialBrackets() {
        let onlyOpen = BlockParser.strippedWikiLink("[[value")
        XCTAssertEqual(onlyOpen, "[[value")

        let onlyClose = BlockParser.strippedWikiLink("value]]")
        XCTAssertEqual(onlyClose, "value]]")
    }

    func testStrippedWikiLinkEmptyString() {
        let result = BlockParser.strippedWikiLink("")
        XCTAssertEqual(result, "")
    }

    // MARK: - Legacy: extractTodo

    func testExtractsTodoState() {
        let (state, text) = BlockParser.extractTodo(from: "TODO Buy groceries")
        XCTAssertEqual(state, "TODO")
        XCTAssertEqual(text, "Buy groceries")
    }

    func testExtractsDoneState() {
        let (state, _) = BlockParser.extractTodo(from: "DONE Finished task")
        XCTAssertEqual(state, "DONE")
    }

    func testExtractsDoingState() {
        let (state, text) = BlockParser.extractTodo(from: "DOING Working on it")
        XCTAssertEqual(state, "DOING")
        XCTAssertEqual(text, "Working on it")
    }

    func testNoTodoState() {
        let (state, text) = BlockParser.extractTodo(from: "Regular block")
        XCTAssertNil(state)
        XCTAssertEqual(text, "Regular block")
    }

    // MARK: - Serialization (tree-based)

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
        XCTAssertEqual(result, "- Parent\n  - Child")
    }

    // MARK: - extractWikiLinks

    func testExtractsWikiLinks() {
        let links = BlockParser.extractWikiLinks(from: "See [[Page A]] and [[Another Page]]")
        XCTAssertEqual(Set(links), Set(["Page A", "Another Page"]))
    }
}

// MARK: - BlockDisplayTextTests

final class BlockDisplayTextTests: XCTestCase {

    // MARK: - displayText (strips all tags)

    func testDisplayTextStripsTagAtEnd() {
        let block = Block(text: "hello #tag")
        XCTAssertEqual(block.displayText, "hello")
    }

    func testDisplayTextStripsMultipleTags() {
        let block = Block(text: "text #a #b")
        XCTAssertEqual(block.displayText, "text")
    }

    func testDisplayTextLeavesPlainTextUntouched() {
        let block = Block(text: "plain text")
        XCTAssertEqual(block.displayText, "plain text")
    }

    func testDisplayTextStripsTagWithNoTrailingSpace() {
        let block = Block(text: "note#tag")
        // The regex requires a leading space/word boundary before #
        // Pattern is \s*#[A-Za-z]... so "note#tag" — the tag part (#tag) has no preceding \s
        // Verify actual behavior: the regex \s*#[A-Za-z0-9_\-]* will still match #tag
        // even without a leading space because \s* matches zero spaces.
        let result = block.displayText
        XCTAssertFalse(result.contains("#tag"))
    }

    func testDisplayTextEmptyString() {
        let block = Block(text: "")
        XCTAssertEqual(block.displayText, "")
    }

    func testDisplayTextUsesFirstLineOnly() {
        let block = Block(text: "First line #tag\nstatus:: todo")
        // displayText only strips from first line; no property continuation leaked
        let result = block.displayText
        XCTAssertFalse(result.contains("status::"))
        XCTAssertFalse(result.contains("#tag"))
        XCTAssertEqual(result, "First line")
    }

    func testDisplayTextStripsTagInMiddleOfText() {
        let block = Block(text: "before #tag after")
        XCTAssertEqual(block.displayText, "before after")
    }

    func testDisplayTextTrimsWhitespace() {
        // After stripping a leading or trailing tag, extra spaces are trimmed
        let block = Block(text: "  #tag some text  ")
        let result = block.displayText
        XCTAssertFalse(result.hasPrefix(" "))
        XCTAssertFalse(result.hasSuffix(" "))
    }

    // MARK: - updateDisplayText

    func testUpdateDisplayTextPreservesTagsFromArray() {
        let block = Block(text: "My note #work", tags: ["work"])
        block.updateDisplayText("My updated note")
        XCTAssertTrue(block.text.contains("#work"))
    }

    func testUpdateDisplayTextDoesNotDuplicateTags() {
        // If the new display text accidentally contains the tag already,
        // updateDisplayText cleans it out before appending from the array.
        let block = Block(text: "My note #work", tags: ["work"])
        block.updateDisplayText("My updated note #work")
        // Should appear exactly once
        let occurrences = block.text.components(separatedBy: "#work").count - 1
        XCTAssertEqual(occurrences, 1)
    }

    func testUpdateDisplayTextPreservesMultipleTags() {
        let block = Block(text: "Note #alpha #beta", tags: ["alpha", "beta"])
        block.updateDisplayText("Updated note")
        XCTAssertTrue(block.text.contains("#alpha"))
        XCTAssertTrue(block.text.contains("#beta"))
    }

    func testUpdateDisplayTextWithNoTags() {
        let block = Block(text: "Plain note", tags: [])
        block.updateDisplayText("Updated plain note")
        XCTAssertEqual(block.text, "Updated plain note")
    }

    func testUpdateDisplayTextPreservesPropertyContinuationLines() {
        let block = Block(text: "Task\nstatus:: todo\npriority:: high", tags: [])
        block.updateDisplayText("Updated Task")
        XCTAssertTrue(block.text.contains("status:: todo"))
        XCTAssertTrue(block.text.contains("priority:: high"))
    }

    func testUpdateDisplayTextCleansLeakedTagsFromDisplayInput() {
        // Old saved block might have tag in display; updateDisplayText should strip it
        let block = Block(text: "Work item #work", tags: ["work"])
        block.updateDisplayText("Work item #work")
        let occurrences = block.text.components(separatedBy: "#work").count - 1
        XCTAssertEqual(occurrences, 1)
    }

    func testUpdateDisplayTextEmptyDisplayWithTags() {
        let block = Block(text: "#task", tags: ["task"])
        block.updateDisplayText("")
        // With empty display, tags should still be appended without a leading space
        XCTAssertTrue(block.text.hasPrefix("#task"))
    }

    func testUpdateDisplayTextTrimsWhitespace() {
        let block = Block(text: "Note #work", tags: ["work"])
        block.updateDisplayText("  Note with spaces  ")
        // The result should be trimmed before appending tags
        XCTAssertFalse(block.text.hasPrefix("  "))
    }
}
