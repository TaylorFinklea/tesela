import AppKit

// MARK: - BlockView
// NSTextView subclass for a single outliner block.
// Handles inline editing, structural key intercepts, and live syntax styling.

class BlockView: NSTextView {
    let block: Block

    // Callbacks wired by OutlinerView
    var onTextChanged: ((String) -> Void)?
    var onEnterPressed: ((String, String) -> Void)?   // (textBefore, textAfter)
    var onTabPressed: (() -> Void)?
    var onShiftTabPressed: (() -> Void)?
    var onBackspaceAtStart: (() -> Void)?
    var onArrowUpAtStart: (() -> Void)?
    var onArrowDownAtEnd: (() -> Void)?
    var onWikiLinkClicked: ((String) -> Void)?

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
        isRichText = true
        font = .systemFont(ofSize: NSFont.systemFontSize)
        textColor = .labelColor
        isVerticallyResizable = true
        isHorizontallyResizable = false
        textContainer?.widthTracksTextView = true
        textContainer?.lineFragmentPadding = 2
        textContainerInset = NSSize(width: 0, height: 2)
        isAutomaticLinkDetectionEnabled = false
        isAutomaticDataDetectionEnabled = false
        delegate = self

        string = block.text
        textStorage?.delegate = self
        if let ts = textStorage {
            BlockStyler.style(text: block.text, textStorage: ts)
            applyLinkAttributes(to: ts, text: block.text)
        }
    }

    // MARK: - Structural key overrides

    override func insertNewline(_ sender: Any?) {
        let loc = selectedRange().location
        let s = string
        let before = String(s.prefix(loc))
        let after = String(s.suffix(s.count - loc))
        onEnterPressed?(before, after)
    }

    override func insertTab(_ sender: Any?) {
        onTabPressed?()
    }

    override func insertBacktab(_ sender: Any?) {
        onShiftTabPressed?()
    }

    override func keyDown(with event: NSEvent) {
        // Backspace at position 0 with no selection → merge with previous block
        if event.keyCode == 51,
           selectedRange().location == 0,
           selectedRange().length == 0 {
            onBackspaceAtStart?()
            return
        }
        super.keyDown(with: event)
    }

    override func moveUp(_ sender: Any?) {
        let before = selectedRange().location
        super.moveUp(sender)
        if selectedRange().location == before {
            onArrowUpAtStart?()
        }
    }

    override func moveDown(_ sender: Any?) {
        let before = selectedRange().location
        super.moveDown(sender)
        if selectedRange().location == before {
            onArrowDownAtEnd?()
        }
    }

    // MARK: - Wiki-link link attributes

    private func applyLinkAttributes(to textStorage: NSTextStorage, text: String) {
        let nsText = text as NSString
        let fullRange = NSRange(location: 0, length: nsText.length)
        let regex = try? NSRegularExpression(pattern: #"\[\[([^\]]+)\]\]"#)
        regex?.enumerateMatches(in: text, range: fullRange) { match, _, _ in
            guard let match, let captureRange = Range(match.range(at: 1), in: text) else { return }
            let target = String(text[captureRange])
            textStorage.addAttribute(.link, value: "wikilink://\(target)", range: match.range)
        }
    }
}

// MARK: - NSTextViewDelegate (wiki-link click handling)
extension BlockView: NSTextViewDelegate {
    func textView(_ textView: NSTextView, clickedOnLink link: Any, at charIndex: Int) -> Bool {
        guard let str = link as? String, str.hasPrefix("wikilink://") else { return false }
        let target = String(str.dropFirst("wikilink://".count))
        onWikiLinkClicked?(target)
        return true
    }
}

// MARK: - NSTextStorageDelegate (live syntax re-styling)
// NSTextStorageDelegate is not @MainActor, but AppKit guarantees main-thread delivery.
extension BlockView: NSTextStorageDelegate {
    nonisolated func textStorage(
        _ textStorage: NSTextStorage,
        didProcessEditing editedMask: NSTextStorageEditActions,
        range editedRange: NSRange,
        changeInLength delta: Int
    ) {
        guard editedMask.contains(.editedCharacters) else { return }
        // Capture the String value (Sendable) before crossing into @MainActor context.
        // Use self.textStorage inside to stay within actor isolation.
        let text = textStorage.string
        MainActor.assumeIsolated {
            if let ts = self.textStorage {
                BlockStyler.style(text: text, textStorage: ts)
                applyLinkAttributes(to: ts, text: text)
            }
            onTextChanged?(text)
        }
    }
}
