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
    var onSlashMenu: (() -> Void)?
    var onSpaceMenu: (() -> Void)?
    var onDismissMenu: (() -> Void)?
    var isMenuVisible: (() -> Bool)?

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
    var onFocused: (() -> Void)?

    // Inline autocomplete
    var isCompletionVisible: (() -> Bool)?
    /// Forward a key event to the completion popover. Returns true if consumed.
    var onCompletionKey: ((NSEvent) -> Bool)?

    /// Type tag names (lowercased) — only these tags are stripped from display text.
    /// Casual tags stay inline and are styled by BlockStyler.
    var typeTagNames: Set<String> = []

    /// Active search query for highlighting matches
    var searchQuery: String?

    init(block: Block, typeTagNames: Set<String> = []) {
        self.block = block
        self.typeTagNames = typeTagNames

        let storage = NSTextStorage(string: block.displayText(strippingOnly: typeTagNames.isEmpty ? nil : typeTagNames))
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
            let display = block.displayText(strippingOnly: typeTagNames.isEmpty ? nil : typeTagNames)
            BlockStyler.style(text: display, textStorage: ts, searchQuery: searchQuery)
            applyLinkAttributes(to: ts, text: display)
        }
    }

    // MARK: - Block cursor (Normal mode)

    override func drawInsertionPoint(in rect: NSRect, color: NSColor, turnedOn flag: Bool) {
        if isNormalMode {
            // Start from rect (always valid from NSTextView) and widen to character width
            var blockRect = rect
            blockRect.size.width = max(rect.size.width, 8)
            if let lm = layoutManager, let tc = textContainer,
               lm.numberOfGlyphs > 0,
               selectedRange().location < lm.numberOfGlyphs {
                let glyphIndex = lm.glyphIndexForCharacter(at: selectedRange().location)
                let charRect = lm.boundingRect(forGlyphRange: NSRange(location: glyphIndex, length: 1), in: tc)
                if charRect.width > 0 {
                    blockRect.size.width = charRect.width
                }
            }
            color.withAlphaComponent(0.4).setFill()
            blockRect.fill()
        } else {
            super.drawInsertionPoint(in: rect, color: color, turnedOn: flag)
        }
    }

    /// Returns the rect of the insertion point in this view's coordinate space.
    func cursorRect() -> NSRect {
        guard let lm = layoutManager, let tc = textContainer,
              lm.numberOfGlyphs > 0,
              let ts = textStorage, ts.length > 0 else {
            return NSRect(x: textContainerInset.width, y: textContainerInset.height, width: 1, height: 18)
        }
        let pos = min(selectedRange().location, ts.length - 1)
        let glyphIndex = min(lm.glyphIndexForCharacter(at: pos), lm.numberOfGlyphs - 1)
        let rect = lm.boundingRect(forGlyphRange: NSRange(location: glyphIndex, length: 1), in: tc)
        return NSRect(x: rect.origin.x + textContainerInset.width,
                      y: rect.origin.y + textContainerInset.height,
                      width: max(rect.width, 1), height: rect.height)
    }

    override func setNeedsDisplay(_ rect: NSRect, avoidAdditionalLayout flag: Bool) {
        super.setNeedsDisplay(bounds, avoidAdditionalLayout: flag)
    }

    override func becomeFirstResponder() -> Bool {
        let result = super.becomeFirstResponder()
        // Force immediate cursor redraw so block cursor appears on focus
        needsDisplay = true
        if result { onFocused?() }
        return result
    }

    // Keep the block cursor always visible (disable blinking in Normal mode)
    override func updateInsertionPointStateAndRestartTimer(_ restartFlag: Bool) {
        super.updateInsertionPointStateAndRestartTimer(true)
    }

    // MARK: - Key routing

    override func keyDown(with event: NSEvent) {
        // If a menu is visible, forward keys to the menu overlay via notification
        if isMenuVisible?() == true {
            if event.keyCode == 53 { // Escape
                onDismissMenu?()
            } else if let chars = event.characters, !chars.isEmpty {
                NotificationCenter.default.post(
                    name: .teselaMenuKeyPress,
                    object: nil,
                    userInfo: ["characters": chars]
                )
            }
            return // don't let keys reach the editor while menu is open
        }

        // Forward nav keys to completion popover (arrow, Enter, Escape)
        // Let all other keys (typing, backspace) pass through to the editor
        if isCompletionVisible?() == true {
            if onCompletionKey?(event) == true {
                return
            }
        }

        guard let vim = vimEngine else {
            if event.keyCode == 51, selectedRange().location == 0, selectedRange().length == 0 {
                onBackspaceAtStart?()
                return
            }
            super.keyDown(with: event)
            return
        }

        // `:` in Normal mode → open command palette
        if vim.currentMode == .normal && event.characters == ":" {
            onCommandPalette?()
            return
        }

        // Space in Normal mode → leader menu
        if vim.currentMode == .normal && event.characters == " " && !event.modifierFlags.contains(.shift) {
            onSpaceMenu?()
            return
        }

        // `/` in Insert mode → slash command menu
        if vim.currentMode == .insert && event.characters == "/" {
            let pos = selectedRange().location
            let prevIsSpace = pos > 0 && pos <= string.count &&
                string[string.index(string.startIndex, offsetBy: pos - 1)] == " "
            if pos == 0 || prevIsSpace {
                onSlashMenu?()
                return
            }
        }

        // ⌘Enter → toggle todo (works in any mode)
        if event.modifierFlags.contains(.command) && event.keyCode == 36 {
            onVimCommand?(.toggleTodo)
            return
        }

        // ⌘D → set deadline date picker
        if event.modifierFlags.contains(.command) && !event.modifierFlags.contains(.shift) && event.characters?.lowercased() == "d" {
            onVimCommand?(.setDeadline)
            return
        }

        // ⌘⇧D → set scheduled date picker
        if event.modifierFlags.contains(.command) && event.modifierFlags.contains(.shift) && event.characters?.lowercased() == "d" {
            onVimCommand?(.setScheduled)
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
        // Shift+Enter → newline within block (multi-line)
        if NSEvent.modifierFlags.contains(.shift) {
            super.insertNewline(sender)
            return
        }
        // Enter → split into new block
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
                BlockStyler.style(text: text, textStorage: ts, searchQuery: searchQuery)
                applyLinkAttributes(to: ts, text: text)
            }
            onTextChanged?(text)
        }
    }
}
