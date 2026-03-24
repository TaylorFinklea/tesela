import Foundation
import AppKit

// MARK: - KeyEvent
// Abstraction over NSEvent for testability (no UI dependency)
struct KeyEvent: Equatable, Sendable {
    let characters: String
    let modifiers: ModifierFlags
    let keyCode: UInt16

    struct ModifierFlags: OptionSet, Sendable {
        let rawValue: UInt
        static let command  = ModifierFlags(rawValue: 1 << 0)
        static let shift    = ModifierFlags(rawValue: 1 << 1)
        static let control  = ModifierFlags(rawValue: 1 << 2)
        static let option   = ModifierFlags(rawValue: 1 << 3)
    }

    static func from(nsEvent: NSEvent) -> KeyEvent {
        var flags: ModifierFlags = []
        if nsEvent.modifierFlags.contains(.command) { flags.insert(.command) }
        if nsEvent.modifierFlags.contains(.shift) { flags.insert(.shift) }
        if nsEvent.modifierFlags.contains(.control) { flags.insert(.control) }
        if nsEvent.modifierFlags.contains(.option) { flags.insert(.option) }
        // Use charactersIgnoringModifiers for Ctrl combos so Ctrl+R gives "r" not "\u{12}"
        let chars = flags.contains(.control)
            ? (nsEvent.charactersIgnoringModifiers ?? "")
            : (nsEvent.characters ?? "")
        return KeyEvent(
            characters: chars,
            modifiers: flags,
            keyCode: nsEvent.keyCode
        )
    }
}

// MARK: - EditorCommand
// High-level commands produced by VimKeyHandler, consumed by OutlinerView
enum EditorCommand: Equatable, Sendable {
    // Movement
    case moveLeft
    case moveRight
    case moveUp               // within block text (or prev block if at start)
    case moveDown             // within block text (or next block if at end)
    case movePrevBlock        // k — jump to previous block
    case moveNextBlock        // j — jump to next block
    case moveWordForward
    case moveWordBackward
    case moveWordEnd
    case moveLineStart
    case moveLineEnd
    case moveFirstBlock
    case moveLastBlock

    // Mode changes
    case enterInsert
    case enterInsertAfter
    case enterInsertLineStart
    case enterInsertLineEnd
    case enterInsertNewLineBelow
    case enterInsertNewLineAbove
    case enterVisual
    case enterVisualLine
    case exitToNormal

    // Edit
    case deleteChar
    case deleteBlock
    case yankBlock
    case pasteBelow
    case pasteAbove
    case indentBlock
    case dedentBlock
    case joinBlock
    case replaceChar(Character)
    case undo
    case redo
    case repeatLastChange

    // Operator + motion combos (resolved by VimKeyHandler)
    case delete(Motion)
    case change(Motion)
    case yank(Motion)

    // App
    case startSearch
    case toggleTodo
    case prevSection   // { — jump to previous tile/section
    case nextSection   // } — jump to next tile/section
    case setDeadline
    case setScheduled
    case none
}

// MARK: - Motion
indirect enum Motion: Equatable, Sendable {
    case wordForward
    case wordBackward
    case wordEnd
    case lineStart
    case lineEnd
    case innerWord
    case aroundWord
    case innerQuote(Character)
    case aroundQuote(Character)
    case innerParen
    case aroundParen
    case count(Int, Motion)  // 3w = count(3, .wordForward)
}

// MARK: - VimKeyHandler
// Pure function: (VimState, KeyEvent) → (VimState, EditorCommand)
// Zero UI dependencies — fully testable

struct VimKeyHandler: Sendable {
    func handle(event: KeyEvent, state: inout VimState) -> EditorCommand {
        switch state.mode {
        case .normal:
            return handleNormal(event: event, state: &state)
        case .insert:
            return handleInsert(event: event, state: &state)
        case .visual, .visualLine:
            return handleVisual(event: event, state: &state)
        case .operatorPending(let op):
            return handleOperator(op: op, event: event, state: &state)
        }
    }

    // MARK: - Normal mode
    private func handleNormal(event: KeyEvent, state: inout VimState) -> EditorCommand {
        let ch = event.characters
        let ctrl = event.modifiers.contains(.control)

        // Digit accumulation
        if let digit = Int(ch), ch.count == 1, ch != "0" || state.count != nil {
            state.appendCount(digit: digit)
            return .none
        }

        defer { state.resetCount() }

        switch ch {
        // Movement
        case "h": return .moveLeft
        case "l": return .moveRight
        case "j": return .moveNextBlock
        case "k": return .movePrevBlock
        case "w": return .moveWordForward
        case "b": return .moveWordBackward
        case "e": return .moveWordEnd
        case "0": return .moveLineStart
        case "$": return .moveLineEnd
        case "g" where state.count == nil: return .moveFirstBlock  // gg handled below
        case "G": return .moveLastBlock

        // Mode transitions
        case "i":
            state.mode = .insert
            return .enterInsert
        case "a":
            state.mode = .insert
            return .enterInsertAfter
        case "I":
            state.mode = .insert
            return .enterInsertLineStart
        case "A":
            state.mode = .insert
            return .enterInsertLineEnd
        case "o":
            state.mode = .insert
            return .enterInsertNewLineBelow
        case "O":
            state.mode = .insert
            return .enterInsertNewLineAbove
        case "J": return .joinBlock
        case "v":
            state.mode = .visual
            return .enterVisual
        case "V":
            state.mode = .visualLine
            return .enterVisualLine

        // Edit
        case "x": return .deleteChar
        case "u": return .undo
        case "p": return .pasteBelow
        case "P": return .pasteAbove
        case ".": return .repeatLastChange
        case "/": return .startSearch
        case "t": return .toggleTodo

        // Section navigation (tile jumping)
        case "{": return .prevSection
        case "}": return .nextSection

        // Indentation
        case ">": return .indentBlock
        case "<": return .dedentBlock

        // Operators (arm pending) — preserve count for the resolved command
        case "d":
            state.pendingCount = state.effectiveCount
            state.mode = .operatorPending(.delete)
            return .none
        case "c":
            state.pendingCount = state.effectiveCount
            state.mode = .operatorPending(.change)
            return .none
        case "y":
            state.pendingCount = state.effectiveCount
            state.mode = .operatorPending(.yank)
            return .none

        // Ctrl combos
        case "r" where ctrl: return .redo

        default:
            return .none
        }
    }

    // MARK: - Insert mode
    private func handleInsert(event: KeyEvent, state: inout VimState) -> EditorCommand {
        if event.characters == "\u{1B}" {  // Escape
            state.mode = .normal
            return .exitToNormal
        }
        // All other keys fall through to NSTextView
        return .none
    }

    // MARK: - Visual mode
    private func handleVisual(event: KeyEvent, state: inout VimState) -> EditorCommand {
        if event.characters == "\u{1B}" {
            state.mode = .normal
            return .exitToNormal
        }
        switch event.characters {
        case "d":
            state.mode = .normal
            return .deleteBlock
        case "y":
            state.mode = .normal
            return .yankBlock
        default:
            return .none
        }
    }

    // MARK: - Operator pending
    private func handleOperator(op: Operator, event: KeyEvent, state: inout VimState) -> EditorCommand {
        let ch = event.characters
        state.mode = .normal

        // Operator + operator = line-wise (dd, cc, yy)
        switch (op, ch) {
        case (.delete, "d"): return .deleteBlock
        case (.yank, "y"): return .yankBlock
        case (.change, "c"):
            state.mode = .insert
            return .deleteBlock  // simplification: delete block then insert

        // Motions
        case (.delete, "w"): return .delete(.wordForward)
        case (.delete, "b"): return .delete(.wordBackward)
        case (.delete, "e"): return .delete(.wordEnd)
        case (.delete, "$"): return .delete(.lineEnd)
        case (.delete, "0"): return .delete(.lineStart)

        case (.change, "w"):
            state.mode = .insert
            return .change(.wordForward)
        case (.change, "e"):
            state.mode = .insert
            return .change(.wordEnd)

        case (.yank, "w"): return .yank(.wordForward)
        case (.yank, "e"): return .yank(.wordEnd)

        // Text objects
        case (_, "i") where ch == "i":
            // Need another char — re-arm (simplified: iw only)
            return .none  // Phase 11.5 extension point

        default:
            return .none
        }
    }
}
