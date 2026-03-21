import AppKit

// MARK: - BlockView
// NSTextView subclass for a single outliner block — Phase 11.4

class BlockView: NSTextView {
    let block: Block

    init(block: Block) {
        self.block = block
        super.init(frame: .zero, textContainer: nil)
        setup()
    }

    required init?(coder: NSCoder) {
        fatalError("Use init(block:)")
    }

    private func setup() {
        isEditable = true
        isSelectable = true
        drawsBackground = false
        isRichText = false
        font = NSFont.systemFont(ofSize: NSFont.systemFontSize)
        string = block.text
        isVerticallyResizable = true
        isHorizontallyResizable = false
        textContainer?.widthTracksTextView = true
    }
}

// MARK: - VimEngine
// Bridges VimState + VimKeyHandler for use in OutlinerView
class VimEngine {
    private var state = VimState()
    private let handler = VimKeyHandler()

    func handle(event: NSEvent) -> EditorCommand {
        let keyEvent = KeyEvent.from(nsEvent: event)
        return handler.handle(event: keyEvent, state: &state)
    }

    var currentMode: VimMode { state.mode }
}
