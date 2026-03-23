import AppKit
import SwiftUI

// MARK: - OutlinerDelegate
@MainActor
protocol OutlinerDelegate: AnyObject {
    func outlinerDidChangeContent(blocks: [Block])
    func outlinerDidClickWikiLink(target: String)
    func outlinerDidChangeMode(mode: VimMode)
    func outlinerDidRequestCommandPalette()
    func outlinerDidRequestSlashMenu()
    func outlinerDidRequestSpaceMenu()
}

// MARK: - OutlinerView
class OutlinerView: NSView {
    var blocks: [Block] = [] {
        didSet { rebuildBlockViews() }
    }

    weak var delegate: OutlinerDelegate?
    private(set) var focusedBlockIndex: Int?
    private var vimEngine = VimEngine()
    var menuVisibilityCheck: (() -> Bool)?
    var onDismissMenuCallback: (() -> Void)?

    private var blockViews: [BlockView] = []
    private var pendingFocusIndex: Int?
    private var pendingCursorPosition: Int?
    private var lastBoundsWidth: CGFloat = 0
    private var hasInitialized = false

    override var isFlipped: Bool { true }

    override init(frame: NSRect) {
        super.init(frame: frame)
        setup()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setup()
    }

    private func setup() {
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
        autoresizingMask = [.width]

        NotificationCenter.default.addObserver(forName: .teselaSetDeadline, object: nil, queue: .main) { [weak self] _ in
            guard let self, let idx = focusedBlockIndex, idx < blockViews.count else { return }
            showDatePicker(for: "deadline", at: idx, anchorView: blockViews[idx])
        }
        NotificationCenter.default.addObserver(forName: .teselaSetScheduled, object: nil, queue: .main) { [weak self] _ in
            guard let self, let idx = focusedBlockIndex, idx < blockViews.count else { return }
            showDatePicker(for: "scheduled", at: idx, anchorView: blockViews[idx])
        }
    }

    override func layout() {
        super.layout()
        if abs(bounds.width - lastBoundsWidth) > 1 {
            lastBoundsWidth = bounds.width
            rebuildBlockViews()
        }
    }

    // MARK: - Rebuild

    func rebuildBlockViews() {
        subviews.forEach { $0.removeFromSuperview() }
        blockViews.removeAll()

        var yOffset: CGFloat = 8

        for (index, block) in blocks.enumerated() {
            let indentX = CGFloat(block.indentLevel) * 20
            let bulletX  = indentX + 8
            let textX    = indentX + 28

            // Reserve space for right-side badges (tags + task properties)
            let badgeCount = block.tags.count
                + (block.deadline != nil ? 1 : 0)
                + (block.scheduled != nil ? 1 : 0)
                + (block.effort != nil ? 1 : 0)
            let badgeWidth: CGFloat = badgeCount > 0 ? min(CGFloat(badgeCount) * 80 + 8, 280) : 0
            let priorityWidth: CGFloat = block.priority != nil ? 22 : 0
            let textWidth = max(bounds.width - textX - 12 - badgeWidth - priorityWidth, 80)

            // Todo indicator or bullet
            let (bulletSymbol, bulletColor): (String, NSColor) = {
                if let state = block.todoState {
                    switch state {
                    case .todo:  return (state.displayChar, .secondaryLabelColor)
                    case .doing: return (state.displayChar, .systemOrange)
                    case .done:  return (state.displayChar, .systemGreen)
                    }
                }
                return (block.indentLevel == 0 ? "•" : "◦", .tertiaryLabelColor)
            }()
            let bullet = NSTextField(labelWithString: bulletSymbol)
            bullet.font = .systemFont(ofSize: NSFont.systemFontSize)
            bullet.textColor = bulletColor
            bullet.isEditable = false
            bullet.isBordered = false
            bullet.drawsBackground = false
            bullet.frame = NSRect(x: bulletX, y: yOffset, width: 16, height: 22)
            addSubview(bullet)

            // Priority indicator between bullet and text
            var actualTextX = textX
            if let priority = block.priority {
                let priLabel = NSTextField(labelWithString: priority.displayChar)
                priLabel.font = .systemFont(ofSize: 10)
                priLabel.isEditable = false
                priLabel.isBordered = false
                priLabel.drawsBackground = false
                priLabel.frame = NSRect(x: textX, y: yOffset, width: 18, height: 22)
                addSubview(priLabel)
                actualTextX = textX + 18
            }

            let view = BlockView(block: block)
            view.frame = NSRect(x: actualTextX, y: yOffset, width: textWidth, height: 22)
            wireCallbacks(for: view, at: index)
            addSubview(view)
            blockViews.append(view)

            let height = blockHeight(for: view)
            view.frame.size.height = height
            bullet.frame.size.height = height

            // Right-side badges: deadline, scheduled, effort, tags
            var badgeX = actualTextX + textWidth + 6

            if let deadline = block.deadline {
                let pill = makeDeadlineBadge(deadline)
                pill.frame.origin = NSPoint(x: badgeX, y: yOffset + (height - 18) / 2)
                addSubview(pill)
                badgeX += pill.frame.width + 4
            }

            if let scheduled = block.scheduled {
                let pill = makeDateBadge("📅 \(formatDateShort(scheduled))", color: .secondaryLabelColor)
                pill.frame.origin = NSPoint(x: badgeX, y: yOffset + (height - 18) / 2)
                addSubview(pill)
                badgeX += pill.frame.width + 4
            }

            if let effort = block.effort {
                let pill = makeDateBadge("⏱ \(effort)", color: .secondaryLabelColor)
                pill.frame.origin = NSPoint(x: badgeX, y: yOffset + (height - 18) / 2)
                addSubview(pill)
                badgeX += pill.frame.width + 4
            }

            for tag in block.tags {
                let pill = makeTagPill("#\(tag)")
                let pillWidth = pill.frame.width
                pill.frame = NSRect(x: badgeX, y: yOffset + (height - 18) / 2, width: pillWidth, height: 18)
                addSubview(pill)
                badgeX += pillWidth + 4
            }

            yOffset += height + 4

            // Property pills removed — properties are now visible text on their own lines
            if false {  // dead code — keeping makePropertyPill for potential future use
                var propX = textX
                let propY = yOffset - 2
                for (key, value) in block.properties.sorted(by: { $0.key < $1.key }) {
                    let pill = makePropertyPill(key: key, value: value)
                    let pillWidth = pill.frame.width
                    pill.frame.origin = NSPoint(x: propX, y: propY)
                    addSubview(pill)
                    propX += pillWidth + 4
                }
                yOffset += 22
            }
        }

        let minHeight = superview?.bounds.height ?? 400
        frame.size.height = max(yOffset + 8, minHeight)

        if let idx = pendingFocusIndex {
            let target = min(idx, blockViews.count - 1)
            if target >= 0 {
                let view = blockViews[target]
                let cursorPos = pendingCursorPosition ?? 0
                DispatchQueue.main.async { [weak self, weak view] in
                    guard let view else { return }
                    self?.window?.makeFirstResponder(view)
                    let pos = min(cursorPos, view.string.count)
                    view.setSelectedRange(NSRange(location: pos, length: 0))
                }
            }
            pendingFocusIndex = nil
            pendingCursorPosition = nil
        }

        // Start in Insert mode on initial page load only
        if !hasInitialized {
            hasInitialized = true
            vimEngine.currentMode = .insert
            delegate?.outlinerDidChangeMode(mode: .insert)
        }
        // Sync isNormalMode to all block views after every rebuild
        let isNormal = vimEngine.currentMode == .normal
        for bv in blockViews { bv.isNormalMode = isNormal }
    }

    private func blockHeight(for view: BlockView) -> CGFloat {
        guard let lm = view.layoutManager, let tc = view.textContainer else { return 22 }
        lm.ensureLayout(for: tc)
        return max(lm.usedRect(for: tc).height + 4, 22)
    }

    private func makeTagPill(_ text: String) -> NSView {
        let container = NSView()
        container.wantsLayer = true
        container.layer?.backgroundColor = NSColor.secondaryLabelColor.withAlphaComponent(0.25).cgColor
        container.layer?.cornerRadius = 4

        let label = NSTextField(labelWithString: text)
        label.font = .systemFont(ofSize: 10)
        label.textColor = .secondaryLabelColor
        label.isEditable = false
        label.isBordered = false
        label.drawsBackground = false
        label.sizeToFit()
        label.frame.origin = NSPoint(x: 6, y: 1)
        container.addSubview(label)
        container.frame.size = NSSize(width: label.frame.width + 12, height: 18)
        return container
    }

    private func makeDeadlineBadge(_ dateStr: String) -> NSView {
        let formatted = formatDateShort(dateStr)
        let isOverdue = isDateOverdue(dateStr)
        let isUrgent = isDateWithinDays(dateStr, days: 3)
        let bgColor: NSColor = isOverdue ? .systemRed.withAlphaComponent(0.25)
            : isUrgent ? .systemOrange.withAlphaComponent(0.25)
            : .secondaryLabelColor.withAlphaComponent(0.15)
        let textColor: NSColor = isOverdue ? .systemRed : isUrgent ? .systemOrange : .secondaryLabelColor
        return makeDateBadge("⚑ \(formatted)", color: textColor, bgColor: bgColor)
    }

    private func makeDateBadge(_ text: String, color: NSColor, bgColor: NSColor? = nil) -> NSView {
        let container = NSView()
        container.wantsLayer = true
        container.layer?.backgroundColor = (bgColor ?? color.withAlphaComponent(0.15)).cgColor
        container.layer?.cornerRadius = 4

        let label = NSTextField(labelWithString: text)
        label.font = .systemFont(ofSize: 10)
        label.textColor = color
        label.isEditable = false
        label.isBordered = false
        label.drawsBackground = false
        label.sizeToFit()
        label.frame.origin = NSPoint(x: 4, y: 1)
        container.addSubview(label)
        container.frame.size = NSSize(width: label.frame.width + 8, height: 18)
        return container
    }

    private func formatDateShort(_ dateStr: String) -> String {
        let inputFmt = DateFormatter()
        inputFmt.dateFormat = "yyyy-MM-dd"
        guard let date = inputFmt.date(from: dateStr) else { return dateStr }
        let outputFmt = DateFormatter()
        outputFmt.dateFormat = "MMM d"
        return outputFmt.string(from: date)
    }

    private func isDateOverdue(_ dateStr: String) -> Bool {
        let fmt = DateFormatter()
        fmt.dateFormat = "yyyy-MM-dd"
        guard let date = fmt.date(from: dateStr) else { return false }
        return date < Calendar.current.startOfDay(for: Date())
    }

    private func isDateWithinDays(_ dateStr: String, days: Int) -> Bool {
        let fmt = DateFormatter()
        fmt.dateFormat = "yyyy-MM-dd"
        guard let date = fmt.date(from: dateStr),
              let threshold = Calendar.current.date(byAdding: .day, value: days, to: Date()) else { return false }
        return date <= threshold && date >= Calendar.current.startOfDay(for: Date())
    }

    private func makePropertyPill(key: String, value: String) -> NSView {
        let container = NSView()
        container.wantsLayer = true
        container.layer?.backgroundColor = NSColor.systemPurple.withAlphaComponent(0.15).cgColor
        container.layer?.cornerRadius = 4

        let keyLabel = NSTextField(labelWithString: "\(key):")
        keyLabel.font = .boldSystemFont(ofSize: 10)
        keyLabel.textColor = .systemPurple
        keyLabel.isEditable = false
        keyLabel.isBordered = false
        keyLabel.drawsBackground = false
        keyLabel.sizeToFit()
        keyLabel.frame.origin = NSPoint(x: 6, y: 1)
        container.addSubview(keyLabel)

        let valueLabel = NSTextField(labelWithString: " \(value)")
        valueLabel.font = .systemFont(ofSize: 10)
        valueLabel.textColor = .labelColor
        valueLabel.isEditable = false
        valueLabel.isBordered = false
        valueLabel.drawsBackground = false
        valueLabel.sizeToFit()
        valueLabel.frame.origin = NSPoint(x: 6 + keyLabel.frame.width, y: 1)
        container.addSubview(valueLabel)

        container.frame.size = NSSize(
            width: keyLabel.frame.width + valueLabel.frame.width + 12,
            height: 18
        )
        return container
    }

    // MARK: - Callback wiring

    private func wireCallbacks(for view: BlockView, at index: Int) {
        // Vim integration
        view.vimEngine = vimEngine
        view.isNormalMode = (vimEngine.currentMode == .normal)
        view.onVimCommand = { [weak self] cmd in
            guard let self else { return }
            focusedBlockIndex = index
            executeVimCommand(cmd, at: index)
        }
        view.onModeChanged = { [weak self] mode in
            self?.delegate?.outlinerDidChangeMode(mode: mode)
        }

        view.onTextChanged = { [weak self] newText in
            guard let self, index < blocks.count else { return }
            blocks[index].text = newText
            let newH = blockHeight(for: view)
            if abs(view.frame.size.height - newH) > 2 {
                pendingFocusIndex = index
                rebuildBlockViews()
            }
            delegate?.outlinerDidChangeContent(blocks: blocks)
        }

        view.onEnterPressed = { [weak self] before, after in
            guard let self, index < blocks.count else { return }
            blocks[index].text = before
            let newBlock = Block(text: after, indentLevel: blocks[index].indentLevel)
            blocks.insert(newBlock, at: index + 1)
            pendingFocusIndex = index + 1
            pendingCursorPosition = 0
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)
        }

        view.onTabPressed = { [weak self] in
            guard let self, index < blocks.count else { return }
            let maxIndent = index > 0 ? blocks[index - 1].indentLevel + 1 : 0
            blocks[index].indentLevel = min(blocks[index].indentLevel + 1, maxIndent)
            pendingFocusIndex = index
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)
        }

        view.onShiftTabPressed = { [weak self] in
            guard let self, index < blocks.count else { return }
            blocks[index].indentLevel = max(blocks[index].indentLevel - 1, 0)
            pendingFocusIndex = index
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)
        }

        view.onBackspaceAtStart = { [weak self] in
            guard let self, index > 0, index < blocks.count else { return }
            blocks[index - 1].text += blocks[index].text
            blocks.remove(at: index)
            pendingFocusIndex = index - 1
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)
        }

        view.onArrowUpAtStart = { [weak self] in
            guard let self, index > 0, index - 1 < blockViews.count else { return }
            focusedBlockIndex = index - 1
            window?.makeFirstResponder(blockViews[index - 1])
        }

        view.onArrowDownAtEnd = { [weak self] in
            guard let self, index + 1 < blockViews.count else { return }
            focusedBlockIndex = index + 1
            window?.makeFirstResponder(blockViews[index + 1])
        }

        view.onWikiLinkClicked = { [weak self] target in
            self?.delegate?.outlinerDidClickWikiLink(target: target)
        }

        view.onCommandPalette = { [weak self] in
            self?.delegate?.outlinerDidRequestCommandPalette()
        }

        view.onSlashMenu = { [weak self] in
            self?.delegate?.outlinerDidRequestSlashMenu()
        }

        view.onSpaceMenu = { [weak self] in
            self?.delegate?.outlinerDidRequestSpaceMenu()
        }

        view.isMenuVisible = { [weak self] in
            guard let self else { return false }
            // Check via delegate (OutlinerView doesn't know about AppState directly)
            return menuVisibilityCheck?() ?? false
        }

        view.onDismissMenu = { [weak self] in
            self?.onDismissMenuCallback?()
        }
    }

    // MARK: - Vim command execution

    private func executeVimCommand(_ cmd: EditorCommand, at index: Int) {
        guard index < blockViews.count, index < blocks.count else { return }
        let view = blockViews[index]
        let count = vimEngine.lastCount

        // Track edits for dot-repeat
        switch cmd {
        case .deleteBlock, .deleteChar, .indentBlock, .dedentBlock,
             .delete, .change, .pasteBelow, .pasteAbove:
            vimEngine.lastEditCommand = cmd
        default: break
        }

        switch cmd {
        // Within-block motions — respect count
        case .moveLeft:           for _ in 0..<count { view.moveLeft(nil) }
        case .moveRight:          for _ in 0..<count { view.moveRight(nil) }
        case .moveWordForward:    for _ in 0..<count { view.moveWordForward(nil) }
        case .moveWordBackward:   for _ in 0..<count { view.moveWordBackward(nil) }
        case .moveWordEnd:        for _ in 0..<count { view.moveWordForward(nil) }
        case .moveLineStart:      view.moveToBeginningOfLine(nil)
        case .moveLineEnd:        view.moveToEndOfLine(nil)

        // Block navigation — respect count
        case .moveNextBlock:
            let target = min(index + count, blockViews.count - 1)
            focusedBlockIndex = target
            blockViews[target].isNormalMode = true
            window?.makeFirstResponder(blockViews[target])
        case .movePrevBlock:
            let target = max(index - count, 0)
            focusedBlockIndex = target
            blockViews[target].isNormalMode = true
            window?.makeFirstResponder(blockViews[target])
        case .moveFirstBlock:
            guard !blockViews.isEmpty else { break }
            focusedBlockIndex = 0
            blockViews[0].isNormalMode = true
            window?.makeFirstResponder(blockViews[0])
        case .moveLastBlock:
            let last = blockViews.count - 1
            guard last >= 0 else { break }
            focusedBlockIndex = last
            blockViews[last].isNormalMode = true
            window?.makeFirstResponder(blockViews[last])

        // Insert mode entry
        case .enterInsert:          break
        case .enterInsertAfter:     view.moveRight(nil)
        case .enterInsertLineStart: view.moveToBeginningOfLine(nil)
        case .enterInsertLineEnd:   view.moveToEndOfLine(nil)

        case .enterInsertNewLineBelow:
            let newBlock = Block(text: "", indentLevel: blocks[index].indentLevel)
            blocks.insert(newBlock, at: index + 1)
            pendingFocusIndex = index + 1
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)

        case .enterInsertNewLineAbove:
            let newBlock = Block(text: "", indentLevel: blocks[index].indentLevel)
            blocks.insert(newBlock, at: index)
            pendingFocusIndex = index
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)

        case .exitToNormal:
            // Collapse any visual selection
            let loc = view.selectedRange().location
            view.setSelectedRange(NSRange(location: loc, length: 0))

        // Visual mode
        case .enterVisual:
            // Anchor selection at current cursor position
            let loc = view.selectedRange().location
            view.setSelectedRange(NSRange(location: loc, length: 1))
        case .enterVisualLine:
            // Select entire block text
            view.setSelectedRange(NSRange(location: 0, length: view.string.count))

        // Indent / dedent
        case .indentBlock:
            let maxIndent = index > 0 ? blocks[index - 1].indentLevel + 1 : 0
            blocks[index].indentLevel = min(blocks[index].indentLevel + 1, maxIndent)
            pendingFocusIndex = index
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)
        case .dedentBlock:
            blocks[index].indentLevel = max(blocks[index].indentLevel - 1, 0)
            pendingFocusIndex = index
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)

        // Block-level editing — respect count
        case .deleteBlock:
            let deleteCount = min(count, blocks.count - 1) // keep at least 1 block
            guard deleteCount > 0 else { break }
            var yanked: [String] = []
            for _ in 0..<deleteCount {
                guard blocks.count > 1, index < blocks.count else { break }
                yanked.append(blocks[index].text)
                blocks.remove(at: index)
            }
            vimEngine.yankRegister = yanked.joined(separator: "\n")
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(vimEngine.yankRegister, forType: .string)
            pendingFocusIndex = min(index, blocks.count - 1)
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)

        case .yankBlock:
            let yankCount = min(count, blocks.count - index)
            let yanked = (index..<(index + yankCount)).map { blocks[$0].text }
            vimEngine.yankRegister = yanked.joined(separator: "\n")
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(vimEngine.yankRegister, forType: .string)

        case .pasteBelow:
            let text = vimEngine.yankRegister
            guard !text.isEmpty else { break }
            let lines = text.components(separatedBy: "\n")
            for (i, line) in lines.enumerated() {
                let newBlock = Block(text: line, indentLevel: blocks[index].indentLevel)
                blocks.insert(newBlock, at: index + 1 + i)
            }
            pendingFocusIndex = index + 1
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)

        case .pasteAbove:
            let text = vimEngine.yankRegister
            guard !text.isEmpty else { break }
            let lines = text.components(separatedBy: "\n")
            for (i, line) in lines.enumerated() {
                let newBlock = Block(text: line, indentLevel: blocks[index].indentLevel)
                blocks.insert(newBlock, at: index + i)
            }
            pendingFocusIndex = index
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)

        case .deleteChar:
            for _ in 0..<count { view.deleteForward(nil) }

        // Operator + motion combos — respect count
        case .delete(let motion):
            for _ in 0..<count { applyMotionSelection(motion, on: view) }
            if let range = Range(view.selectedRange(), in: view.string) {
                vimEngine.yankRegister = String(view.string[range])
            }
            view.deleteBackward(nil)

        case .change(let motion):
            for _ in 0..<count { applyMotionSelection(motion, on: view) }
            if let range = Range(view.selectedRange(), in: view.string) {
                vimEngine.yankRegister = String(view.string[range])
            }
            view.deleteBackward(nil)

        case .yank(let motion):
            let before = view.selectedRange()
            for _ in 0..<count { applyMotionSelection(motion, on: view) }
            if let range = Range(view.selectedRange(), in: view.string) {
                vimEngine.yankRegister = String(view.string[range])
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(vimEngine.yankRegister, forType: .string)
            }
            view.setSelectedRange(before)

        // Dot-repeat
        case .repeatLastChange:
            if let lastCmd = vimEngine.lastEditCommand {
                executeVimCommand(lastCmd, at: index)
            }

        // Search
        case .startSearch:
            delegate?.outlinerDidRequestCommandPalette()

        // Undo / redo
        case .undo: view.undoManager?.undo()
        case .redo: view.undoManager?.redo()

        // Todo toggle
        case .toggleTodo:
            let block = blocks[index]
            let nextState: TodoState? = {
                switch block.todoState {
                case nil:    return .todo
                case .todo:  return .doing
                case .doing: return .done
                case .done:  return nil
                }
            }()
            // Update block text: remove old prefix, add new one
            var text = block.text
            if let current = block.todoState {
                let prefix = "\(current.rawValue) "
                if text.hasPrefix(prefix) { text = String(text.dropFirst(prefix.count)) }
            }
            if let next = nextState {
                text = "\(next.rawValue) \(text)"
            }
            block.text = text
            block.todoState = nextState
            // Update the NSTextView content directly
            view.string = text
            if let ts = view.textStorage {
                BlockStyler.style(text: text, textStorage: ts)
            }
            pendingFocusIndex = index
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)

        // Date pickers
        case .setDeadline:
            showDatePicker(for: "deadline", at: index, anchorView: view)
        case .setScheduled:
            showDatePicker(for: "scheduled", at: index, anchorView: view)

        case .replaceChar, .moveUp, .moveDown:
            break

        case .none:
            break
        }
    }

    // MARK: - Date picker popover

    private var activePopover: NSPopover?

    private func showDatePicker(for propertyKey: String, at index: Int, anchorView: NSView) {
        activePopover?.close()

        let picker = NSDatePicker()
        picker.datePickerStyle = .clockAndCalendar
        picker.datePickerElements = .yearMonthDay
        picker.dateValue = existingDate(for: propertyKey, at: index) ?? Date()
        picker.sizeToFit()

        let buttonHeight: CGFloat = 32
        let padding: CGFloat = 10
        let containerWidth = picker.frame.width + padding * 2
        let containerHeight = picker.frame.height + buttonHeight + padding * 2 + 4

        let container = NSView(frame: NSRect(x: 0, y: 0, width: containerWidth, height: containerHeight))
        picker.frame.origin = NSPoint(x: padding, y: buttonHeight + padding + 4)
        container.addSubview(picker)

        // "Set" button at the bottom
        let setButton = NSButton(title: "Set \(propertyKey.capitalized)", target: nil, action: nil)
        setButton.bezelStyle = .rounded
        setButton.keyEquivalent = "\r"  // Enter key
        setButton.frame = NSRect(x: padding, y: padding, width: containerWidth - padding * 2, height: buttonHeight)
        container.addSubview(setButton)

        let vc = NSViewController()
        vc.view = container

        let popover = NSPopover()
        popover.contentViewController = vc
        popover.behavior = .transient
        popover.show(relativeTo: anchorView.bounds, of: anchorView, preferredEdge: .maxY)
        activePopover = popover

        let blockIndex = index
        let key = propertyKey

        // Apply date and close on button click or popover close
        let applyAndClose: () -> Void = { [weak self, weak popover] in
            guard let self else { return }
            let fmt = DateFormatter()
            fmt.dateFormat = "yyyy-MM-dd"
            let dateStr = fmt.string(from: picker.dateValue)
            self.applyDateProperty(key: key, value: dateStr, at: blockIndex)
            popover?.close()
            self.activePopover = nil
        }

        setButton.target = self
        setButton.action = nil
        // Use NSButton action via block wrapper
        let clickAction = DatePickerAction(handler: applyAndClose)
        setButton.target = clickAction
        setButton.action = #selector(DatePickerAction.execute)
        objc_setAssociatedObject(popover, "clickAction", clickAction, .OBJC_ASSOCIATION_RETAIN)

        NotificationCenter.default.addObserver(
            forName: NSPopover.didCloseNotification,
            object: popover,
            queue: .main
        ) { [weak self] _ in
            self?.activePopover = nil
        }
    }

    private func existingDate(for key: String, at index: Int) -> Date? {
        guard index < blocks.count else { return nil }
        let dateStr: String?
        switch key {
        case "deadline":  dateStr = blocks[index].deadline
        case "scheduled": dateStr = blocks[index].scheduled
        default: return nil
        }
        guard let str = dateStr else { return nil }
        let fmt = DateFormatter()
        fmt.dateFormat = "yyyy-MM-dd"
        return fmt.date(from: str)
    }

    private func applyDateProperty(key: String, value: String, at index: Int) {
        guard index < blocks.count else { return }
        let block = blocks[index]

        // Split text into lines and find/replace existing property line
        var lines = block.text.components(separatedBy: "\n")
        let propertyLine = "\(key):: \(value)"
        var replaced = false

        for (i, line) in lines.enumerated() {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("\(key):: ") {
                lines[i] = propertyLine
                replaced = true
                break
            }
        }

        if !replaced {
            lines.append(propertyLine)
        }

        let text = lines.joined(separator: "\n")
        block.text = text

        switch key {
        case "deadline":  block.deadline = value
        case "scheduled": block.scheduled = value
        default: break
        }

        if index < blockViews.count {
            blockViews[index].string = text
            if let ts = blockViews[index].textStorage {
                BlockStyler.style(text: text, textStorage: ts)
            }
        }
        pendingFocusIndex = index
        rebuildBlockViews()
        delegate?.outlinerDidChangeContent(blocks: blocks)
    }

    private func applyMotionSelection(_ motion: Motion, on view: BlockView) {
        switch motion {
        case .wordForward:  view.moveWordForwardAndModifySelection(nil)
        case .wordBackward: view.moveWordBackwardAndModifySelection(nil)
        case .wordEnd:      view.moveWordForwardAndModifySelection(nil)
        case .lineStart:    view.moveToBeginningOfLineAndModifySelection(nil)
        case .lineEnd:      view.moveToEndOfLineAndModifySelection(nil)
        default: break
        }
    }
}

// MARK: - DatePickerAction (target-action helper for NSButton closure)
class DatePickerAction: NSObject {
    let handler: () -> Void
    init(handler: @escaping () -> Void) { self.handler = handler }
    @objc func execute() { handler() }
}

// MARK: - OutlinerCoordinator (NSViewRepresentable)
struct OutlinerCoordinator: NSViewRepresentable {
    @Binding var blocks: [Block]
    var onContentChanged: (([Block]) -> Void)?
    var onWikiLinkClicked: ((String) -> Void)?
    var onModeChanged: ((VimMode) -> Void)?
    var onCommandPalette: (() -> Void)?
    var onSlashMenu: (() -> Void)?
    var onSpaceMenu: (() -> Void)?
    var isMenuVisible: (() -> Bool)?
    var onDismissMenu: (() -> Void)?

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.drawsBackground = false

        let outliner = OutlinerView()
        outliner.delegate = context.coordinator
        outliner.menuVisibilityCheck = isMenuVisible
        outliner.onDismissMenuCallback = onDismissMenu
        context.coordinator.outlinerView = outliner

        scrollView.documentView = outliner
        outliner.blocks = blocks
        return scrollView
    }

    func updateNSView(_ nsView: NSScrollView, context: Context) {
        guard let outliner = context.coordinator.outlinerView else { return }
        let currentIDs = outliner.blocks.map { $0.id }
        let newIDs = blocks.map { $0.id }
        guard currentIDs != newIDs else { return }
        outliner.blocks = blocks
    }

    @MainActor
    final class Coordinator: OutlinerDelegate {
        var parent: OutlinerCoordinator
        weak var outlinerView: OutlinerView?

        init(_ parent: OutlinerCoordinator) { self.parent = parent }

        func outlinerDidChangeContent(blocks: [Block]) {
            parent.onContentChanged?(blocks)
        }

        func outlinerDidClickWikiLink(target: String) {
            parent.onWikiLinkClicked?(target)
        }

        func outlinerDidChangeMode(mode: VimMode) {
            parent.onModeChanged?(mode)
        }

        func outlinerDidRequestCommandPalette() {
            parent.onCommandPalette?()
        }

        func outlinerDidRequestSlashMenu() {
            parent.onSlashMenu?()
        }

        func outlinerDidRequestSpaceMenu() {
            parent.onSpaceMenu?()
        }
    }
}
