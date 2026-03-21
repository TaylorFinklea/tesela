import AppKit

// MARK: - BlockView
// NSTextView subclass for a single outliner block.
// Routes keyboard input through VimEngine when available.

class BlockView: NSTextView {
    let block: Block

    // Vim integration — shared engine from OutlinerView
    var vimEngine: VimEngine?
    var onVimCommand: ((EditorCommand) -> Void)?
    var onModeChanged: ((VimMode) -> Void)?
    var onCommandPalette: (() -> Void)?

    // Block cursor state
    var isNormalMode = false {
        didSet { needsDisplay = true }
    }

    // Callbacks wired by OutlinerView
    var onTextChanged: ((String) -> Void)?
    var onEnterPressed: ((String, String) -> Void)?
    var onTabPressed: (() -> Void)?
    var onShiftTabPressed: (() -> Void)?
    var onBackspaceAtStart: (() -> Void)?
    var onArrowUpAtStart: (() -> Void)?
    var onArrowDownAtEnd: (() -> Void)?
    var onWikiLinkClicked: ((String) -> Void)?

    init(block: Block) {
        self.block = block

        let storage = NSTextStorage(string: block.text)
        let layoutMgr = NSLayoutManager()
        storage.addLayoutManager(layoutMgr)
        let container = NSTextContainer(size: NSSize(width: 300, height: 1_000_000))
        container.widthTracksTextView = true
        layoutMgr.addTextContainer(container)

        super.init(frame: NSRect(x: 0, y: 0, width: 300, height: 22), textContainer: container)
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
        allowsUndo = true
        font = .systemFont(ofSize: NSFont.systemFontSize)
        textColor = .labelColor
        isVerticallyResizable = true
        isHorizontallyResizable = false
        textContainer?.lineFragmentPadding = 2
        textContainerInset = NSSize(width: 0, height: 2)
        isAutomaticLinkDetectionEnabled = false
        isAutomaticDataDetectionEnabled = false
        delegate = self

        textStorage?.delegate = self
        if let ts = textStorage {
            BlockStyler.style(text: block.text, textStorage: ts)
            applyLinkAttributes(to: ts, text: block.text)
        }
    }

    // MARK: - Block cursor (Normal mode)

    override func drawInsertionPoint(in rect: NSRect, color: NSColor, turnedOn flag: Bool) {
        if isNormalMode {
            // Draw a filled block cursor covering the character at the insertion point
            var blockRect = rect
            if let lm = layoutManager, let tc = textContainer {
                let glyphIndex = lm.glyphIndexForCharacter(at: selectedRange().location)
                let charRect = lm.boundingRect(forGlyphRange: NSRange(location: glyphIndex, length: 1), in: tc)
                blockRect = NSRect(
                    x: charRect.origin.x + textContainerInset.width,
                    y: charRect.origin.y + textContainerInset.height,
                    width: max(charRect.width, 8),
                    height: charRect.height
                )
            }
            color.withAlphaComponent(0.4).setFill()
            blockRect.fill()
        } else {
            super.drawInsertionPoint(in: rect, color: color, turnedOn: flag)
        }
    }

    override func setNeedsDisplay(_ rect: NSRect, avoidAdditionalLayout flag: Bool) {
        super.setNeedsDisplay(bounds, avoidAdditionalLayout: flag)
    }

    override func becomeFirstResponder() -> Bool {
        let result = super.becomeFirstResponder()
        // Force immediate cursor redraw so block cursor appears on focus
        needsDisplay = true
        return result
    }

    // Keep the block cursor always visible (disable blinking in Normal mode)
    override func updateInsertionPointStateAndRestartTimer(_ restartFlag: Bool) {
        super.updateInsertionPointStateAndRestartTimer(true)
    }

    // MARK: - Key routing

    override func keyDown(with event: NSEvent) {
        guard let vim = vimEngine else {
            if event.keyCode == 51, selectedRange().location == 0, selectedRange().length == 0 {
                onBackspaceAtStart?()
                return
            }
            super.keyDown(with: event)
            return
        }

        // `:` in Normal mode → open command palette (don't send to VimEngine)
        if vim.currentMode == .normal && event.characters == ":" {
            onCommandPalette?()
            return
        }

        let previousMode = vim.currentMode
        let cmd = vim.handle(event: event)

        // Notify mode changes
        if vim.currentMode != previousMode {
            isNormalMode = (vim.currentMode == .normal)
            onModeChanged?(vim.currentMode)
        }

        // Insert mode + no Vim command → let NSTextView handle
        if previousMode == .insert && cmd == .none {
            if event.keyCode == 51, selectedRange().location == 0, selectedRange().length == 0 {
                onBackspaceAtStart?()
                return
            }
            super.keyDown(with: event)
            return
        }

        if cmd != .none {
            onVimCommand?(cmd)
        }
    }

    // MARK: - Structural overrides (Insert mode via NSTextView input system)

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

// MARK: - NSTextViewDelegate
extension BlockView: NSTextViewDelegate {
    func textView(_ textView: NSTextView, clickedOnLink link: Any, at charIndex: Int) -> Bool {
        guard let str = link as? String, str.hasPrefix("wikilink://") else { return false }
        let target = String(str.dropFirst("wikilink://".count))
        onWikiLinkClicked?(target)
        return true
    }
}

// MARK: - NSTextStorageDelegate
extension BlockView: NSTextStorageDelegate {
    nonisolated func textStorage(
        _ textStorage: NSTextStorage,
        didProcessEditing editedMask: NSTextStorageEditActions,
        range editedRange: NSRange,
        changeInLength delta: Int
    ) {
        guard editedMask.contains(.editedCharacters) else { return }
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
