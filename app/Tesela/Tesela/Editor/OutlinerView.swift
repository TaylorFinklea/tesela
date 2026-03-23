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

        // Generic command execution from slash/space menus
        NotificationCenter.default.addObserver(forName: .teselaExecuteCommand, object: nil, queue: .main) { [weak self] notification in
            guard let self,
                  let idx = focusedBlockIndex, idx < blockViews.count,
                  let commandId = notification.userInfo?["commandId"] as? String else { return }

            switch commandId {
            case "todo", "doing", "done":
                executeVimCommand(.toggleTodo, at: idx)
            case "deadline":
                showDatePicker(for: "deadline", at: idx, anchorView: blockViews[idx])
            case "scheduled":
                showDatePicker(for: "scheduled", at: idx, anchorView: blockViews[idx])
            case "block-below":
                executeVimCommand(.enterInsertNewLineBelow, at: idx)
            case "block-above":
                executeVimCommand(.enterInsertNewLineAbove, at: idx)
            case "delete-block":
                executeVimCommand(.deleteBlock, at: idx)
            case "indent":
                executeVimCommand(.indentBlock, at: idx)
            case "dedent":
                executeVimCommand(.dedentBlock, at: idx)
            case "priority":
                // TODO: priority picker UI
                break
            case "effort":
                // TODO: effort input UI
                break
            case "search":
                delegate?.outlinerDidRequestCommandPalette()
            default:
                break
            }
        }

        // Legacy individual notifications (for ⌘D/⌘⇧D shortcuts)
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

            // Task indicator or bullet (based on #Task tag + status:: property)
            let (bulletSymbol, bulletColor): (String, NSColor) = {
                if block.isTask {
                    switch block.status {
                    case "todo":  return ("☐", .secondaryLabelColor)
                    case "doing": return ("◎", .systemOrange)
                    case "done":  return ("☑", .systemGreen)
                    default:      return ("☐", .secondaryLabelColor)
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

                // Edit button (pencil) to reopen date picker
                let editBtn = makeEditDateButton(propertyKey: "deadline", blockIndex: index)
                editBtn.frame.origin = NSPoint(x: badgeX - 2, y: yOffset + (height - 14) / 2)
                addSubview(editBtn)
                badgeX += editBtn.frame.width + 4
            }

            if let scheduled = block.scheduled {
                let pill = makeDateBadge("📅 \(formatDateShort(scheduled))", color: .secondaryLabelColor)
                pill.frame.origin = NSPoint(x: badgeX, y: yOffset + (height - 18) / 2)
                addSubview(pill)
                badgeX += pill.frame.width + 4

                let editBtn = makeEditDateButton(propertyKey: "scheduled", blockIndex: index)
                editBtn.frame.origin = NSPoint(x: badgeX - 2, y: yOffset + (height - 14) / 2)
                addSubview(editBtn)
                badgeX += editBtn.frame.width + 4
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

    private func makeEditDateButton(propertyKey: String, blockIndex: Int) -> NSView {
        let btn = NSButton(title: "✎", target: nil, action: nil)
        btn.isBordered = false
        btn.font = .systemFont(ofSize: 10)
        btn.frame.size = NSSize(width: 16, height: 14)
        let action = DatePickerAction { [weak self] in
            guard let self, blockIndex < self.blockViews.count else { return }
            self.showDatePicker(for: propertyKey, at: blockIndex, anchorView: self.blockViews[blockIndex])
        }
        btn.target = action
        btn.action = #selector(DatePickerAction.execute)
        objc_setAssociatedObject(btn, "editAction", action, .OBJC_ASSOCIATION_RETAIN)
        return btn
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
            let cursorPos = blocks[index - 1].text.count
            let mergeText = blocks[index].text.trimmingCharacters(in: .whitespacesAndNewlines)
            if !mergeText.isEmpty {
                blocks[index - 1].text += " " + mergeText
            }
            blocks.remove(at: index)
            pendingCursorPosition = cursorPos
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
        case .undo:
            if let um = view.undoManager ?? view.window?.undoManager {
                um.undo()
            }
        case .redo:
            if let um = view.undoManager ?? view.window?.undoManager {
                um.redo()
            }

        // Todo toggle: cycle #Task tag + status:: property
        case .toggleTodo:
            let block = blocks[index]
            var lines = block.text.components(separatedBy: "\n")
            let firstLine = lines[0]

            if !block.isTask {
                // Not a task → add #Task tag + status:: todo
                if !firstLine.contains("#Task") {
                    lines[0] = firstLine + " #Task"
                }
                lines.append("status:: todo")
            } else {
                // Already a task → cycle status
                let nextStatus: String? = switch block.status {
                case "todo":  "doing"
                case "doing": "done"
                default: nil  // done or unknown → remove task
                }

                if let next = nextStatus {
                    // Update existing status line
                    var found = false
                    for (i, line) in lines.enumerated() {
                        if line.trimmingCharacters(in: .whitespaces).hasPrefix("status:: ") {
                            lines[i] = "status:: \(next)"
                            found = true
                            break
                        }
                    }
                    if !found { lines.append("status:: \(next)") }
                } else {
                    // Remove #Task tag and status line
                    lines[0] = lines[0].replacingOccurrences(of: " #Task", with: "")
                        .replacingOccurrences(of: "#Task ", with: "")
                        .replacingOccurrences(of: "#Task", with: "")
                    lines.removeAll { $0.trimmingCharacters(in: .whitespaces).hasPrefix("status:: ") }
                }
            }

            let text = lines.joined(separator: "\n")
            block.text = text
            block.tags = BlockParser.extractTags(from: text)
            block.properties = BlockParser.extractProperties(from: text)
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

        // Join: merge next block into current (Vim J)
        case .joinBlock:
            guard index + 1 < blocks.count else { break }
            let cursorPos = blocks[index].text.count
            let nextText = blocks[index + 1].text.trimmingCharacters(in: .whitespacesAndNewlines)
            if !nextText.isEmpty {
                blocks[index].text += " " + nextText
            }
            blocks.remove(at: index + 1)
            pendingFocusIndex = index
            pendingCursorPosition = cursorPos
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)

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

        let existingDateVal = existingDate(for: propertyKey, at: index) ?? Date()

        // Calendar picker
        let picker = NSDatePicker()
        picker.datePickerStyle = .clockAndCalendar
        picker.datePickerElements = .yearMonthDay
        picker.dateValue = existingDateVal
        picker.sizeToFit()

        // Text input field (natural language)
        let textField = NSTextField()
        textField.placeholderString = "tomorrow, +3d, fri, Mar 25…"
        textField.font = .systemFont(ofSize: NSFont.systemFontSize)
        textField.isHidden = true  // starts hidden, Tab reveals it

        // Preview label for text input
        let previewLabel = NSTextField(labelWithString: "")
        previewLabel.font = .systemFont(ofSize: 11)
        previewLabel.textColor = .secondaryLabelColor
        previewLabel.isHidden = true

        // Mode toggle hint
        let hintLabel = NSTextField(labelWithString: "Tab: switch to text input")
        hintLabel.font = .systemFont(ofSize: 10)
        hintLabel.textColor = .tertiaryLabelColor
        hintLabel.isEditable = false
        hintLabel.isBordered = false
        hintLabel.drawsBackground = false

        let padding: CGFloat = 10
        let buttonHeight: CGFloat = 32
        let hintHeight: CGFloat = 16
        let containerWidth = picker.frame.width + padding * 2
        let containerHeight = picker.frame.height + buttonHeight + hintHeight + padding * 2 + 8

        let container = NSView(frame: NSRect(x: 0, y: 0, width: containerWidth, height: containerHeight))

        // Layout from bottom: button → hint → picker/textfield
        let setButton = NSButton(title: "Set \(propertyKey.capitalized)", target: nil, action: nil)
        setButton.bezelStyle = .rounded
        setButton.keyEquivalent = "\r"
        setButton.frame = NSRect(x: padding, y: padding, width: containerWidth - padding * 2, height: buttonHeight)
        container.addSubview(setButton)

        hintLabel.frame = NSRect(x: padding, y: padding + buttonHeight + 2, width: containerWidth - padding * 2, height: hintHeight)
        container.addSubview(hintLabel)

        let contentY = padding + buttonHeight + hintHeight + 6
        picker.frame.origin = NSPoint(x: padding, y: contentY)
        container.addSubview(picker)

        textField.frame = NSRect(x: padding, y: contentY + picker.frame.height / 2 - 12, width: containerWidth - padding * 2, height: 24)
        container.addSubview(textField)

        previewLabel.frame = NSRect(x: padding, y: contentY + picker.frame.height / 2 + 16, width: containerWidth - padding * 2, height: 20)
        container.addSubview(previewLabel)

        let vc = NSViewController()
        vc.view = container

        let popover = NSPopover()
        popover.contentViewController = vc
        popover.behavior = .transient
        popover.show(relativeTo: anchorView.bounds, of: anchorView, preferredEdge: .maxY)
        activePopover = popover

        let blockIndex = index
        let key = propertyKey

        // Tab toggles between calendar and text input
        let tabAction = DatePickerAction { [weak picker, weak textField, weak hintLabel, weak previewLabel] in
            guard let picker, let textField, let hintLabel, let previewLabel else { return }
            let showingText = !textField.isHidden
            picker.isHidden = !showingText
            textField.isHidden = showingText
            previewLabel.isHidden = showingText
            hintLabel.stringValue = showingText ? "Tab: switch to text input" : "Tab: switch to calendar"
            if !showingText {
                // Text just became visible — focus it
                textField.window?.makeFirstResponder(textField)
            }
        }
        let applyRef = Box<(() -> Void)?>(nil)
        let tabMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            if event.keyCode == 48 { // Tab
                tabAction.handler()
                return nil
            }
            // Enter key → apply date (works in both calendar and text mode)
            if event.keyCode == 36 {
                applyRef.value?()
                return nil
            }
            return event
        }

        // Apply date
        let applyAndClose: () -> Void = { [weak self, weak popover, weak picker, weak textField] in
            guard let self else { return }
            if let tabMonitor { NSEvent.removeMonitor(tabMonitor) }
            let fmt = DateFormatter()
            fmt.dateFormat = "yyyy-MM-dd"

            let dateStr: String
            if let textField, !textField.isHidden, !textField.stringValue.isEmpty {
                // Text input mode — parse natural language
                if let parsed = DateParser.parse(textField.stringValue) {
                    dateStr = parsed
                } else {
                    NSSound.beep()
                    return // invalid date, don't close
                }
            } else if let picker {
                dateStr = fmt.string(from: picker.dateValue)
            } else {
                return
            }

            self.applyDateProperty(key: key, value: dateStr, at: blockIndex)
            popover?.close()
            self.activePopover = nil
        }

        applyRef.value = applyAndClose

        let clickAction = DatePickerAction(handler: applyAndClose)
        setButton.target = clickAction
        setButton.action = #selector(DatePickerAction.execute)

        // Text field Enter key also triggers apply
        let textFieldAction = DatePickerAction(handler: applyAndClose)
        textField.target = textFieldAction
        textField.action = #selector(DatePickerAction.execute)
        objc_setAssociatedObject(popover, "textFieldAction", textFieldAction, .OBJC_ASSOCIATION_RETAIN)
        objc_setAssociatedObject(popover, "clickAction", clickAction, .OBJC_ASSOCIATION_RETAIN)
        objc_setAssociatedObject(popover, "tabAction", tabAction, .OBJC_ASSOCIATION_RETAIN)

        // Live preview for text input
        NotificationCenter.default.addObserver(
            forName: NSControl.textDidChangeNotification,
            object: textField,
            queue: .main
        ) { [weak previewLabel, weak textField] _ in
            guard let previewLabel, let textField else { return }
            if let preview = DateParser.preview(textField.stringValue) {
                previewLabel.stringValue = "→ \(preview)"
                previewLabel.textColor = .secondaryLabelColor
            } else if !textField.stringValue.isEmpty {
                previewLabel.stringValue = "? unrecognized date"
                previewLabel.textColor = .systemRed
            } else {
                previewLabel.stringValue = ""
            }
        }

        NotificationCenter.default.addObserver(
            forName: NSPopover.didCloseNotification,
            object: popover,
            queue: .main
        ) { [weak self] _ in
            if let tabMonitor { NSEvent.removeMonitor(tabMonitor) }
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

        // Store as wiki-link to the date page: deadline:: [[2026-03-30]]
        let linkedValue = "[[\(value)]]"
        let propertyLine = "\(key):: \(linkedValue)"

        // Split text into lines and find/replace existing property line
        var lines = block.text.components(separatedBy: "\n")
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
                // Also apply link attributes for the [[date]] link
                let nsText = text as NSString
                let fullRange = NSRange(location: 0, length: nsText.length)
                let linkRegex = try? NSRegularExpression(pattern: #"\[\[([^\]]+)\]\]"#)
                linkRegex?.enumerateMatches(in: text, range: fullRange) { match, _, _ in
                    guard let match, let captureRange = Range(match.range(at: 1), in: text) else { return }
                    let target = String(text[captureRange])
                    ts.addAttribute(.link, value: "wikilink://\(target)", range: match.range)
                }
            }
        }
        pendingFocusIndex = index
        rebuildBlockViews()
        delegate?.outlinerDidChangeContent(blocks: blocks)

        // Ensure the daily note page exists for this date
        Task { @MainActor in
            // This creates the page if it doesn't exist (server's daily_note is create-on-demand)
            _ = try? await self.apiClient?.getDailyNote(date: value)
        }
    }

    // Weak reference to APIClient for creating daily notes
    private weak var _apiClient: AnyObject?
    var apiClient: APIClient? {
        get { _apiClient as? APIClient }
        set { _apiClient = newValue as AnyObject? }
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
    var handler: () -> Void
    init(handler: @escaping () -> Void) { self.handler = handler }
    @objc func execute() { handler() }
}

// MARK: - Box (mutable reference wrapper for forward references in closures)
class Box<T> {
    var value: T
    init(_ value: T) { self.value = value }
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
    var apiClient: APIClient?

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
        outliner.apiClient = apiClient
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
