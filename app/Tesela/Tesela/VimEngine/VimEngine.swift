import AppKit

// MARK: - VimEngine
// Bridges VimState + VimKeyHandler for use in editor views

class VimEngine {
    private var state = VimState()
    private let handler = VimKeyHandler()

    /// Count from the most recently resolved command (for OutlinerView to loop)
    private(set) var lastCount: Int = 1

    func handle(event: NSEvent) -> EditorCommand {
        let keyEvent = KeyEvent.from(nsEvent: event)
        let countBeforeHandle = state.effectiveCount
        let cmd = handler.handle(event: keyEvent, state: &state)
        // For resolved commands, use pendingCount if operator was armed with a count,
        // otherwise use the count captured before the handler cleared it.
        if cmd != .none {
            lastCount = state.pendingCount > 1 ? state.pendingCount : countBeforeHandle
            state.pendingCount = 1
        }
        return cmd
    }

    var currentMode: VimMode {
        get { state.mode }
        set { state.mode = newValue }
    }

    var yankRegister: String {
        get { state.yank }
        set { state.yank = newValue }
    }

    var lastEditCommand: EditorCommand? {
        get { state.lastEditCommand }
        set { state.lastEditCommand = newValue }
    }

    var searchQuery: String {
        get { state.searchQuery }
        set { state.searchQuery = newValue }
    }
}
