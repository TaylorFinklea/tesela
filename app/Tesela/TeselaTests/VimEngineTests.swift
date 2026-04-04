import XCTest
@testable import Tesela

final class VimEngineTests: XCTestCase {
    var handler: VimKeyHandler!
    var state: VimState!

    override func setUp() {
        handler = VimKeyHandler()
        state = VimState()
    }

    // MARK: - Helpers

    private func key(_ characters: String, modifiers: KeyEvent.ModifierFlags = [], keyCode: UInt16 = 0) -> KeyEvent {
        KeyEvent(characters: characters, modifiers: modifiers, keyCode: keyCode)
    }

    @discardableResult
    private func send(_ characters: String, modifiers: KeyEvent.ModifierFlags = []) -> EditorCommand {
        handler.handle(event: key(characters, modifiers: modifiers), state: &state)
    }

    // MARK: - Initial state

    func testStartsInNormalMode() {
        XCTAssertEqual(state.mode, .normal)
    }

    func testInitialCountIsNil() {
        XCTAssertNil(state.count)
    }

    func testEffectiveCountIsOneWhenCountIsNil() {
        XCTAssertEqual(state.effectiveCount, 1)
    }

    func testInitialYankRegisterIsEmpty() {
        XCTAssertEqual(state.yank, "")
    }

    // MARK: - Normal mode: h/l/j/k motions

    func testHMovesLeft() {
        XCTAssertEqual(send("h"), .moveLeft)
    }

    func testLMovesRight() {
        XCTAssertEqual(send("l"), .moveRight)
    }

    func testJMovesNextBlock() {
        XCTAssertEqual(send("j"), .moveNextBlock)
    }

    func testKMovesPrevBlock() {
        XCTAssertEqual(send("k"), .movePrevBlock)
    }

    // MARK: - Normal mode: word motions

    func testWMoveWordForward() {
        XCTAssertEqual(send("w"), .moveWordForward)
    }

    func testBMoveWordBackward() {
        XCTAssertEqual(send("b"), .moveWordBackward)
    }

    func testEMoveWordEnd() {
        XCTAssertEqual(send("e"), .moveWordEnd)
    }

    // MARK: - Normal mode: line motions

    func testZeroMovesLineStart() {
        XCTAssertEqual(send("0"), .moveLineStart)
    }

    func testDollarMovesLineEnd() {
        XCTAssertEqual(send("$"), .moveLineEnd)
    }

    // MARK: - Normal mode: block extremes

    func testGCapitalMovesLastBlock() {
        XCTAssertEqual(send("G"), .moveLastBlock)
    }

    func testSmallGMovesFirstBlock() {
        // g with no preceding count should return .moveFirstBlock
        XCTAssertEqual(send("g"), .moveFirstBlock)
    }

    func testSmallGWithCountDoesNotMoveFirstBlock() {
        // When count is accumulated, "g" matches the `where state.count == nil` guard so it
        // hits the default branch and returns .none
        send("3")  // accumulate count=3
        XCTAssertEqual(send("g"), .none)
    }

    // MARK: - Normal mode: section navigation

    func testOpenBracePrevSection() {
        XCTAssertEqual(send("{"), .prevSection)
    }

    func testCloseBraceNextSection() {
        XCTAssertEqual(send("}"), .nextSection)
    }

    // MARK: - Mode transitions: insert entry points

    func testIEntersInsertMode() {
        let cmd = send("i")
        XCTAssertEqual(cmd, .enterInsert)
        XCTAssertEqual(state.mode, .insert)
    }

    func testAEntersInsertAfterCursor() {
        let cmd = send("a")
        XCTAssertEqual(cmd, .enterInsertAfter)
        XCTAssertEqual(state.mode, .insert)
    }

    func testShiftIEntersInsertLineStart() {
        let cmd = send("I")
        XCTAssertEqual(cmd, .enterInsertLineStart)
        XCTAssertEqual(state.mode, .insert)
    }

    func testShiftAEntersInsertLineEnd() {
        let cmd = send("A")
        XCTAssertEqual(cmd, .enterInsertLineEnd)
        XCTAssertEqual(state.mode, .insert)
    }

    func testOEntersInsertNewLineBelow() {
        let cmd = send("o")
        XCTAssertEqual(cmd, .enterInsertNewLineBelow)
        XCTAssertEqual(state.mode, .insert)
    }

    func testShiftOEntersInsertNewLineAbove() {
        let cmd = send("O")
        XCTAssertEqual(cmd, .enterInsertNewLineAbove)
        XCTAssertEqual(state.mode, .insert)
    }

    // MARK: - Mode transitions: visual

    func testVEntersVisualMode() {
        let cmd = send("v")
        XCTAssertEqual(cmd, .enterVisual)
        XCTAssertEqual(state.mode, .visual)
    }

    func testShiftVEntersVisualLineMode() {
        let cmd = send("V")
        XCTAssertEqual(cmd, .enterVisualLine)
        XCTAssertEqual(state.mode, .visualLine)
    }

    // MARK: - Insert mode: Escape exits to normal

    func testEscapeFromInsertReturnsToNormal() {
        send("i")
        let cmd = send("\u{1B}")
        XCTAssertEqual(cmd, .exitToNormal)
        XCTAssertEqual(state.mode, .normal)
    }

    func testInsertModePassesThroughRegularKeys() {
        send("i")
        XCTAssertEqual(send("a"), .none)
        XCTAssertEqual(send("z"), .none)
        XCTAssertEqual(send("1"), .none)
        // Mode stays in insert
        XCTAssertEqual(state.mode, .insert)
    }

    func testInsertModeDoesNotAccumulateCount() {
        send("i")
        send("3")
        send("5")
        XCTAssertEqual(state.mode, .insert)  // still in insert, not normal
    }

    // MARK: - Count prefix accumulation

    func testSingleDigitAccumulates() {
        send("3")
        XCTAssertEqual(state.count, 3)
        XCTAssertEqual(state.effectiveCount, 3)
    }

    func testMultiDigitAccumulates() {
        send("1")
        send("2")
        XCTAssertEqual(state.count, 12)
    }

    func testThreeDigitCountAccumulates() {
        send("1")
        send("0")
        send("0")
        XCTAssertEqual(state.count, 100)
    }

    func testCountClearsAfterCommand() {
        send("3")
        send("j")  // resolved command
        XCTAssertNil(state.count)
    }

    func testZeroAloneIsLineStartNotDigit() {
        // "0" when count is nil should be treated as moveLineStart, not digit accumulation
        let cmd = send("0")
        XCTAssertEqual(cmd, .moveLineStart)
        XCTAssertNil(state.count)
    }

    func testZeroAfterDigitAccumulates() {
        // "3" then "0" → count should become 30
        send("3")
        let cmd = send("0")
        XCTAssertEqual(cmd, .none)  // still accumulating
        XCTAssertEqual(state.count, 30)
    }

    func testCountDoesNotReturnCommand() {
        let cmd = send("5")
        XCTAssertEqual(cmd, .none)
        XCTAssertEqual(state.count, 5)
    }

    // MARK: - Edit commands

    func testXDeletesChar() {
        XCTAssertEqual(send("x"), .deleteChar)
    }

    func testUUndo() {
        XCTAssertEqual(send("u"), .undo)
    }

    func testCtrlRRedo() {
        XCTAssertEqual(send("r", modifiers: [.control]), .redo)
    }

    func testPPastesBelow() {
        XCTAssertEqual(send("p"), .pasteBelow)
    }

    func testShiftPPastesAbove() {
        XCTAssertEqual(send("P"), .pasteAbove)
    }

    func testDotRepeatLastChange() {
        XCTAssertEqual(send("."), .repeatLastChange)
    }

    func testJoinBlock() {
        XCTAssertEqual(send("J"), .joinBlock)
    }

    func testToggleTodo() {
        XCTAssertEqual(send("t"), .toggleTodo)
    }

    // MARK: - Indentation

    func testGreaterIndentsBlock() {
        XCTAssertEqual(send(">"), .indentBlock)
    }

    func testLessDedentsBlock() {
        XCTAssertEqual(send("<"), .dedentBlock)
    }

    // MARK: - Search

    func testSlashStartsSearch() {
        XCTAssertEqual(send("/"), .startSearch)
    }

    func testNSearchNext() {
        XCTAssertEqual(send("n"), .searchNext)
    }

    func testShiftNSearchPrev() {
        XCTAssertEqual(send("N"), .searchPrev)
    }

    // MARK: - Operator pending: delete

    func testDArmsDeleteOperator() {
        let cmd = send("d")
        XCTAssertEqual(cmd, .none)
        XCTAssertEqual(state.mode, .operatorPending(.delete))
    }

    func testDDDeletesBlock() {
        send("d")
        let cmd = send("d")
        XCTAssertEqual(cmd, .deleteBlock)
        XCTAssertEqual(state.mode, .normal)
    }

    func testDWDeletesWordForward() {
        send("d")
        let cmd = send("w")
        XCTAssertEqual(cmd, .delete(.wordForward))
        XCTAssertEqual(state.mode, .normal)
    }

    func testDBDeletesWordBackward() {
        send("d")
        let cmd = send("b")
        XCTAssertEqual(cmd, .delete(.wordBackward))
        XCTAssertEqual(state.mode, .normal)
    }

    func testDEDeletesWordEnd() {
        send("d")
        let cmd = send("e")
        XCTAssertEqual(cmd, .delete(.wordEnd))
        XCTAssertEqual(state.mode, .normal)
    }

    func testDDollarDeletesLineEnd() {
        send("d")
        let cmd = send("$")
        XCTAssertEqual(cmd, .delete(.lineEnd))
        XCTAssertEqual(state.mode, .normal)
    }

    func testDZeroDeletesLineStart() {
        send("d")
        let cmd = send("0")
        XCTAssertEqual(cmd, .delete(.lineStart))
        XCTAssertEqual(state.mode, .normal)
    }

    func testDUnknownMotionReturnsNoneAndGoesNormal() {
        send("d")
        let cmd = send("z")  // not a known motion
        XCTAssertEqual(cmd, .none)
        XCTAssertEqual(state.mode, .normal)
    }

    func testDEscapeCancelsOperatorAndGoesNormal() {
        send("d")
        XCTAssertEqual(state.mode, .operatorPending(.delete))
        // Escape key: the handler returns .none and resets mode to normal
        let cmd = send("\u{1B}")
        XCTAssertEqual(state.mode, .normal)
        // Command is .none (no edit was performed)
        XCTAssertEqual(cmd, .none)
    }

    // MARK: - Operator pending: change

    func testCArmsChangeOperator() {
        let cmd = send("c")
        XCTAssertEqual(cmd, .none)
        XCTAssertEqual(state.mode, .operatorPending(.change))
    }

    func testCCDeletesBlockAndEntersInsert() {
        send("c")
        let cmd = send("c")
        // cc = deleteBlock (then editor enters insert)
        XCTAssertEqual(cmd, .deleteBlock)
        XCTAssertEqual(state.mode, .insert)
    }

    func testCWChangesWordForward() {
        send("c")
        let cmd = send("w")
        XCTAssertEqual(cmd, .change(.wordForward))
        XCTAssertEqual(state.mode, .insert)
    }

    func testCEChangesWordEnd() {
        send("c")
        let cmd = send("e")
        XCTAssertEqual(cmd, .change(.wordEnd))
        XCTAssertEqual(state.mode, .insert)
    }

    func testCUnknownMotionReturnsNoneAndGoesNormal() {
        send("c")
        let cmd = send("z")
        XCTAssertEqual(cmd, .none)
        XCTAssertEqual(state.mode, .normal)
    }

    // MARK: - Operator pending: yank

    func testYArmsYankOperator() {
        let cmd = send("y")
        XCTAssertEqual(cmd, .none)
        XCTAssertEqual(state.mode, .operatorPending(.yank))
    }

    func testYYYanksBlock() {
        send("y")
        let cmd = send("y")
        XCTAssertEqual(cmd, .yankBlock)
        XCTAssertEqual(state.mode, .normal)
    }

    func testYWYanksWordForward() {
        send("y")
        let cmd = send("w")
        XCTAssertEqual(cmd, .yank(.wordForward))
        XCTAssertEqual(state.mode, .normal)
    }

    func testYEYanksWordEnd() {
        send("y")
        let cmd = send("e")
        XCTAssertEqual(cmd, .yank(.wordEnd))
        XCTAssertEqual(state.mode, .normal)
    }

    func testYUnknownMotionReturnsNoneAndGoesNormal() {
        send("y")
        let cmd = send("z")
        XCTAssertEqual(cmd, .none)
        XCTAssertEqual(state.mode, .normal)
    }

    // MARK: - Count preserved through operator

    func testCountPreservedThroughDeleteOperator() {
        send("3")
        XCTAssertEqual(state.count, 3)
        send("d")  // arm operator — pendingCount should capture 3
        XCTAssertEqual(state.pendingCount, 3)
    }

    func testCountPreservedThroughYankOperator() {
        send("5")
        send("y")
        XCTAssertEqual(state.pendingCount, 5)
    }

    func testCountPreservedThroughChangeOperator() {
        send("2")
        send("c")
        XCTAssertEqual(state.pendingCount, 2)
    }

    // MARK: - Visual mode motions

    func testVisual_hExtendsLeft() {
        send("v")
        XCTAssertEqual(send("h"), .visualExtendLeft)
    }

    func testVisual_lExtendsRight() {
        send("v")
        XCTAssertEqual(send("l"), .visualExtendRight)
    }

    func testVisual_wExtendsWordForward() {
        send("v")
        XCTAssertEqual(send("w"), .visualExtendWordForward)
    }

    func testVisual_eAlsoExtendsWordForward() {
        send("v")
        XCTAssertEqual(send("e"), .visualExtendWordForward)
    }

    func testVisual_bExtendsWordBackward() {
        send("v")
        XCTAssertEqual(send("b"), .visualExtendWordBackward)
    }

    func testVisual_zeroExtendsLineStart() {
        send("v")
        XCTAssertEqual(send("0"), .visualExtendLineStart)
    }

    func testVisual_dollarExtendsLineEnd() {
        send("v")
        XCTAssertEqual(send("$"), .visualExtendLineEnd)
    }

    func testVisual_jExtendsBlockDown() {
        send("v")
        XCTAssertEqual(send("j"), .visualExtendBlockDown)
    }

    func testVisual_kExtendsBlockUp() {
        send("v")
        XCTAssertEqual(send("k"), .visualExtendBlockUp)
    }

    // MARK: - Visual mode: operations on selection

    func testVisual_dDeletesSelection() {
        send("v")
        let cmd = send("d")
        XCTAssertEqual(cmd, .visualDelete)
        XCTAssertEqual(state.mode, .normal)
    }

    func testVisual_xAlsoDeletesSelection() {
        send("v")
        let cmd = send("x")
        XCTAssertEqual(cmd, .visualDelete)
        XCTAssertEqual(state.mode, .normal)
    }

    func testVisual_yYanksSelection() {
        send("v")
        let cmd = send("y")
        XCTAssertEqual(cmd, .visualYank)
        XCTAssertEqual(state.mode, .normal)
    }

    func testVisual_cChangesSelection() {
        send("v")
        let cmd = send("c")
        XCTAssertEqual(cmd, .visualChange)
        XCTAssertEqual(state.mode, .insert)
    }

    func testVisual_sAlsoChangesSelection() {
        send("v")
        let cmd = send("s")
        XCTAssertEqual(cmd, .visualChange)
        XCTAssertEqual(state.mode, .insert)
    }

    func testVisual_unknownKeyReturnsNone() {
        send("v")
        XCTAssertEqual(send("z"), .none)
        XCTAssertEqual(state.mode, .visual)  // mode unchanged
    }

    // MARK: - Visual mode: exit

    func testVisual_escapeExitsToNormal() {
        send("v")
        let cmd = send("\u{1B}")
        XCTAssertEqual(cmd, .exitToNormal)
        XCTAssertEqual(state.mode, .normal)
    }

    func testVisual_vExitsToNormal() {
        send("v")
        let cmd = send("v")
        XCTAssertEqual(cmd, .exitToNormal)
        XCTAssertEqual(state.mode, .normal)
    }

    func testVisualLine_escapeExitsToNormal() {
        send("V")
        let cmd = send("\u{1B}")
        XCTAssertEqual(cmd, .exitToNormal)
        XCTAssertEqual(state.mode, .normal)
    }

    // MARK: - Visual line: indentation

    func testVisualLine_greaterIndentsBlock() {
        send("V")
        XCTAssertEqual(state.mode, .visualLine)
        let cmd = send(">")
        XCTAssertEqual(cmd, .indentBlock)
    }

    func testVisualLine_lesssDedentsBlock() {
        send("V")
        XCTAssertEqual(state.mode, .visualLine)
        let cmd = send("<")
        XCTAssertEqual(cmd, .dedentBlock)
    }

    func testVisual_greaterReturnsNone() {
        // In visual (non-line) mode, > is not handled
        send("v")
        XCTAssertEqual(state.mode, .visual)
        let cmd = send(">")
        XCTAssertEqual(cmd, .none)
    }

    // MARK: - State reuse / sequential commands

    func testSequentialNormalCommands() {
        XCTAssertEqual(send("j"), .moveNextBlock)
        XCTAssertEqual(send("k"), .movePrevBlock)
        XCTAssertEqual(send("h"), .moveLeft)
        XCTAssertEqual(send("l"), .moveRight)
    }

    func testInsertEscapeBackToNormalAllowsMotions() {
        send("i")
        send("\u{1B}")
        XCTAssertEqual(state.mode, .normal)
        XCTAssertEqual(send("j"), .moveNextBlock)
    }

    func testDoubleOperatorArmsAndResolves() {
        // d→operatorPending, dd→deleteBlock, back to normal, d again→operatorPending
        send("d")
        send("d")
        XCTAssertEqual(state.mode, .normal)
        send("d")
        XCTAssertEqual(state.mode, .operatorPending(.delete))
    }

    // MARK: - appendCount and resetCount helpers

    func testAppendCountBuildsValue() {
        state.appendCount(digit: 4)
        XCTAssertEqual(state.count, 4)
        state.appendCount(digit: 2)
        XCTAssertEqual(state.count, 42)
    }

    func testResetCountNilsCount() {
        state.appendCount(digit: 7)
        state.resetCount()
        XCTAssertNil(state.count)
    }

    func testEffectiveCountReturnsAccumulatedCount() {
        state.appendCount(digit: 9)
        XCTAssertEqual(state.effectiveCount, 9)
    }

    // MARK: - Default / unknown keys in normal mode

    func testUnknownKeyInNormalModeReturnsNone() {
        XCTAssertEqual(send("q"), .none)
        XCTAssertEqual(send("Z"), .none)
        XCTAssertEqual(send("~"), .none)
    }

    func testNormalModeRKeyWithoutCtrlIsNone() {
        // r without ctrl should fall to default
        XCTAssertEqual(send("r"), .none)
    }

    // MARK: - VimState default values

    func testPendingCountDefaultsToOne() {
        XCTAssertEqual(state.pendingCount, 1)
    }

    func testSearchQueryDefaultsToEmpty() {
        XCTAssertEqual(state.searchQuery, "")
    }

    func testLastEditCommandDefaultsToNil() {
        XCTAssertNil(state.lastEditCommand)
    }

    // MARK: - VimMode displayName

    func testNormalModeDisplayName() {
        XCTAssertEqual(VimMode.normal.displayName, "NORMAL")
    }

    func testInsertModeDisplayName() {
        XCTAssertEqual(VimMode.insert.displayName, "INSERT")
    }

    func testVisualModeDisplayName() {
        XCTAssertEqual(VimMode.visual.displayName, "VISUAL")
    }

    func testVisualLineModeDisplayName() {
        XCTAssertEqual(VimMode.visualLine.displayName, "VISUAL LINE")
    }

    func testOperatorPendingDisplayName() {
        XCTAssertEqual(VimMode.operatorPending(.delete).displayName, "OPERATOR")
    }

    // MARK: - Motion equality

    func testMotionEquality() {
        XCTAssertEqual(Motion.wordForward, Motion.wordForward)
        XCTAssertNotEqual(Motion.wordForward, Motion.wordBackward)
        XCTAssertEqual(Motion.count(3, .wordForward), Motion.count(3, .wordForward))
        XCTAssertNotEqual(Motion.count(3, .wordForward), Motion.count(4, .wordForward))
    }

    // MARK: - EditorCommand equality

    func testEditorCommandEquality() {
        XCTAssertEqual(EditorCommand.moveLeft, EditorCommand.moveLeft)
        XCTAssertNotEqual(EditorCommand.moveLeft, EditorCommand.moveRight)
        XCTAssertEqual(EditorCommand.delete(.wordForward), EditorCommand.delete(.wordForward))
        XCTAssertNotEqual(EditorCommand.delete(.wordForward), EditorCommand.delete(.wordBackward))
    }
}
