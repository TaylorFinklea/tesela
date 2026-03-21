import XCTest
@testable import Tesela

final class VimEngineTests: XCTestCase {
    var handler: VimKeyHandler!
    var state: VimState!

    override func setUp() {
        handler = VimKeyHandler()
        state = VimState()
    }

    // MARK: - Mode transitions

    func testStartsInNormalMode() {
        XCTAssertEqual(state.mode, .normal)
    }

    func testIEntersInsertMode() {
        let event = KeyEvent(characters: "i", modifiers: [], keyCode: 34)
        let cmd = handler.handle(event: event, state: &state)
        XCTAssertEqual(state.mode, .insert)
        XCTAssertEqual(cmd, .enterInsert)
    }

    func testEscapeReturnsToNormal() {
        state.mode = .insert
        let event = KeyEvent(characters: "\u{1B}", modifiers: [], keyCode: 53)
        let cmd = handler.handle(event: event, state: &state)
        XCTAssertEqual(state.mode, .normal)
        XCTAssertEqual(cmd, .exitToNormal)
    }

    func testVEntersVisualMode() {
        let event = KeyEvent(characters: "v", modifiers: [], keyCode: 9)
        _ = handler.handle(event: event, state: &state)
        XCTAssertEqual(state.mode, .visual)
    }

    func testShiftVEntersVisualLine() {
        let event = KeyEvent(characters: "V", modifiers: [.shift], keyCode: 9)
        _ = handler.handle(event: event, state: &state)
        XCTAssertEqual(state.mode, .visualLine)
    }

    // MARK: - Block movement

    func testJMovesNextBlock() {
        let event = KeyEvent(characters: "j", modifiers: [], keyCode: 38)
        let cmd = handler.handle(event: event, state: &state)
        XCTAssertEqual(cmd, .moveNextBlock)
    }

    func testKMovesPrevBlock() {
        let event = KeyEvent(characters: "k", modifiers: [], keyCode: 40)
        let cmd = handler.handle(event: event, state: &state)
        XCTAssertEqual(cmd, .movePrevBlock)
    }

    func testGMoveFirstBlock() {
        let event = KeyEvent(characters: "G", modifiers: [.shift], keyCode: 5)
        let cmd = handler.handle(event: event, state: &state)
        XCTAssertEqual(cmd, .moveLastBlock)
    }

    // MARK: - Operators

    func testDDDeletesBlock() {
        // First d arms operator
        let d1 = KeyEvent(characters: "d", modifiers: [], keyCode: 2)
        let cmd1 = handler.handle(event: d1, state: &state)
        XCTAssertEqual(cmd1, .none)
        XCTAssertEqual(state.mode, .operatorPending(.delete))

        // Second d completes dd
        let d2 = KeyEvent(characters: "d", modifiers: [], keyCode: 2)
        let cmd2 = handler.handle(event: d2, state: &state)
        XCTAssertEqual(cmd2, .deleteBlock)
        XCTAssertEqual(state.mode, .normal)
    }

    func testDWDeletesWord() {
        let d = KeyEvent(characters: "d", modifiers: [], keyCode: 2)
        _ = handler.handle(event: d, state: &state)

        let w = KeyEvent(characters: "w", modifiers: [], keyCode: 13)
        let cmd = handler.handle(event: w, state: &state)
        XCTAssertEqual(cmd, .delete(.wordForward))
        XCTAssertEqual(state.mode, .normal)
    }

    func testYYYanksBlock() {
        let y1 = KeyEvent(characters: "y", modifiers: [], keyCode: 16)
        _ = handler.handle(event: y1, state: &state)

        let y2 = KeyEvent(characters: "y", modifiers: [], keyCode: 16)
        let cmd = handler.handle(event: y2, state: &state)
        XCTAssertEqual(cmd, .yankBlock)
    }

    // MARK: - Insert entry points

    func testAEntersInsertAfterCursor() {
        let event = KeyEvent(characters: "a", modifiers: [], keyCode: 0)
        let cmd = handler.handle(event: event, state: &state)
        XCTAssertEqual(state.mode, .insert)
        XCTAssertEqual(cmd, .enterInsertAfter)
    }

    func testOEntersInsertNewLineBelow() {
        let event = KeyEvent(characters: "o", modifiers: [], keyCode: 31)
        let cmd = handler.handle(event: event, state: &state)
        XCTAssertEqual(state.mode, .insert)
        XCTAssertEqual(cmd, .enterInsertNewLineBelow)
    }

    func testShiftOEntersInsertNewLineAbove() {
        let event = KeyEvent(characters: "O", modifiers: [.shift], keyCode: 31)
        let cmd = handler.handle(event: event, state: &state)
        XCTAssertEqual(state.mode, .insert)
        XCTAssertEqual(cmd, .enterInsertNewLineAbove)
    }

    // MARK: - Undo/Redo

    func testUUndo() {
        let event = KeyEvent(characters: "u", modifiers: [], keyCode: 32)
        let cmd = handler.handle(event: event, state: &state)
        XCTAssertEqual(cmd, .undo)
    }

    func testCtrlRRedo() {
        let event = KeyEvent(characters: "r", modifiers: [.control], keyCode: 15)
        let cmd = handler.handle(event: event, state: &state)
        XCTAssertEqual(cmd, .redo)
    }

    // MARK: - Word motions

    func testWMoveWordForward() {
        let event = KeyEvent(characters: "w", modifiers: [], keyCode: 13)
        let cmd = handler.handle(event: event, state: &state)
        XCTAssertEqual(cmd, .moveWordForward)
    }

    func testBMoveWordBackward() {
        let event = KeyEvent(characters: "b", modifiers: [], keyCode: 11)
        let cmd = handler.handle(event: event, state: &state)
        XCTAssertEqual(cmd, .moveWordBackward)
    }

    // MARK: - Indentation

    func testGreaterIndentsBlock() {
        let event = KeyEvent(characters: ">", modifiers: [.shift], keyCode: 47)
        let cmd = handler.handle(event: event, state: &state)
        XCTAssertEqual(cmd, .indentBlock)
    }

    func testLessDedentsBlock() {
        let event = KeyEvent(characters: "<", modifiers: [.shift], keyCode: 43)
        let cmd = handler.handle(event: event, state: &state)
        XCTAssertEqual(cmd, .dedentBlock)
    }
}
