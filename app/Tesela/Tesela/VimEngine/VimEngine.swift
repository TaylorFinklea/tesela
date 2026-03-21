import AppKit

// MARK: - VimEngine
// Bridges VimState + VimKeyHandler for use in editor views

class VimEngine {
    private var state = VimState()
    private let handler = VimKeyHandler()

    func handle(event: NSEvent) -> EditorCommand {
        let keyEvent = KeyEvent.from(nsEvent: event)
        return handler.handle(event: keyEvent, state: &state)
    }

    var currentMode: VimMode {
        get { state.mode }
        set { state.mode = newValue }
    }

    var yankRegister: String {
        get { state.yank }
        set { state.yank = newValue }
    }
}
