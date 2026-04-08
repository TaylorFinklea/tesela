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
    func outlinerDidRequestPrevTile()
    func outlinerDidRequestNextTile()
    func outlinerDidRequestBlockZoom(blockIndex: Int)
    func outlinerDidUpdateSearchStatus(current: Int, total: Int)
    func outlinerDidFocusBlock(text: String, tags: [String], properties: [String: String])
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
    var tileID: String?
    var typeRegistry: [TypeDefinition] = []
    var propertyRegistry: [PropertyDef] = []
    var allTags: [String] = []
    var allPageTitles: [String] = []
    private var expandedBlockIndex: Int?
    private var typeTagNames: Set<String> = []

    private var blockViews: [BlockView] = []
    private var pendingFocusIndex: Int?
    private var pendingCursorPosition: Int?
    private var lastBoundsWidth: CGFloat = 0
    private var hasInitialized = false

    // Ghost bullet hover state
    private var ghostBullet: NSView?
    private var ghostInsertIndex: Int?

    // Structural undo/redo stacks (for block-level operations)
    private var undoStack: [([Block], Int?)] = []  // (blocks snapshot, focused index)
    private var redoStack: [([Block], Int?)] = []
    private let maxUndoDepth = 50

    private func saveUndoState() {
        undoStack.append((blocks.map { $0.deepCopy() }, focusedBlockIndex))
        if undoStack.count > maxUndoDepth { undoStack.removeFirst() }
        redoStack.removeAll()  // new action invalidates redo
    }

    override var isFlipped: Bool { true }

    override init(frame: NSRect) {
        super.init(frame: frame)
        setup()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setup()
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        for area in trackingAreas { removeTrackingArea(area) }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.mouseMoved, .mouseEnteredAndExited, .activeInKeyWindow],
            owner: self
        )
        addTrackingArea(area)
    }

    override func mouseMoved(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        updateGhostBullet(at: point)
    }

    override func mouseExited(with event: NSEvent) {
        hideGhostBullet()
    }

    override func mouseDown(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)

        // If clicking on the ghost bullet area, insert a new block
        if let insertIdx = ghostInsertIndex {
            let ghostY = ghostBullet?.frame.midY ?? 0
            if abs(point.y - ghostY) < 14 {
                hideGhostBullet()
                insertBlockAt(insertIdx)
                return
            }
        }

        hideGhostBullet()
        super.mouseDown(with: event)
    }

    private func updateGhostBullet(at point: NSPoint) {
        // Find which gap between blocks the mouse is in
        guard !blockViews.isEmpty else {
            // Show ghost at the top if no blocks exist
            showGhostBullet(y: 8, insertIndex: 0)
            return
        }

        // Check if mouse is below the last block
        if let lastView = blockViews.last {
            let lastBottom = lastView.frame.maxY + 4
            if point.y > lastBottom {
                showGhostBullet(y: lastBottom + 4, insertIndex: blocks.count)
                return
            }
        }

        // Check gaps between blocks
        for i in 0..<blockViews.count {
            let blockTop = blockViews[i].frame.minY
            let prevBottom: CGFloat = i == 0 ? 0 : blockViews[i - 1].frame.maxY + 2

            // Mouse is in the gap before this block
            if point.y >= prevBottom && point.y < blockTop {
                let gapCenter = (prevBottom + blockTop) / 2
                showGhostBullet(y: gapCenter, insertIndex: i)
                return
            }
        }

        hideGhostBullet()
    }

    private func showGhostBullet(y: CGFloat, insertIndex: Int) {
        ghostInsertIndex = insertIndex

        if ghostBullet == nil {
            let dot = NSTextField(labelWithString: "•")
            dot.font = .systemFont(ofSize: NSFont.systemFontSize)
            dot.textColor = .tertiaryLabelColor.withAlphaComponent(0.4)
            dot.isEditable = false
            dot.isBordered = false
            dot.drawsBackground = false
            dot.frame = NSRect(x: 12, y: 0, width: 16, height: 14)
            addSubview(dot)
            ghostBullet = dot
        }

        ghostBullet?.frame.origin.y = y - 7
    }

    private func hideGhostBullet() {
        ghostBullet?.removeFromSuperview()
        ghostBullet = nil
        ghostInsertIndex = nil
    }

    private func insertBlockAt(_ index: Int) {
        saveUndoState()
        let newBlock = Block(text: "")
        if index >= blocks.count {
            blocks.append(newBlock)
        } else {
            blocks.insert(newBlock, at: index)
        }
        pendingFocusIndex = index
        pendingCursorPosition = 0
        rebuildBlockViews()
        delegate?.outlinerDidChangeContent(blocks: blocks)
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
                let choices = propertyRegistry.first(where: { $0.name == "Priority" })?.values ?? ["critical", "high", "medium", "low"]
                showSelectPopover(propertyName: "Priority", choices: choices, at: idx, anchorView: blockViews[idx])
            case "effort":
                editTextProperty(name: "Effort", at: idx)
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

        // Tile focus — another tile is requesting this one to focus
        NotificationCenter.default.addObserver(forName: .teselaTileFocus, object: nil, queue: .main) { [weak self] notification in
            guard let self,
                  let targetID = notification.userInfo?["tileID"] as? String,
                  targetID == tileID else { return }
            focusFirstBlock()
        }
    }

    /// Focus the first block in this outliner (used for tile navigation).
    /// Preserves Normal mode since navigation is a Normal-mode action.
    func focusFirstBlock() {
        guard !blockViews.isEmpty else { return }
        vimEngine.currentMode = .normal
        for bv in blockViews { bv.isNormalMode = true }
        delegate?.outlinerDidChangeMode(mode: .normal)
        DispatchQueue.main.async { [weak self] in
            guard let self, !self.blockViews.isEmpty else { return }
            self.window?.makeFirstResponder(self.blockViews[0])
            self.focusedBlockIndex = 0
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

        // MARK: Baseline alignment system
        // Type tags become right-aligned pills; casual tags stay inline
        typeTagNames = Set(typeRegistry.map { $0.name.lowercased() })

        var yOffset: CGFloat = 8
        var blockPositions: [(y: CGFloat, height: CGFloat, indent: Int, bulletCenterX: CGFloat)] = []

        for (index, block) in blocks.enumerated() {
            // Skip blocks whose FIRST LINE is purely a property (status:: done, priority:: high)
            // These are metadata continuation lines, not user-visible content
            let firstLine = block.text.components(separatedBy: "\n").first ?? block.text
            let trimmedFirstLine = firstLine.trimmingCharacters(in: .whitespaces)
            let isPropertyOnly = trimmedFirstLine.range(of: #"^[A-Za-z_][A-Za-z0-9_]*:: "#, options: .regularExpression) != nil
            if isPropertyOnly {
                // Still need a BlockView placeholder for index consistency
                let emptyView = BlockView(block: block, typeTagNames: typeTagNames)
                emptyView.frame = NSRect(x: 0, y: yOffset, width: 0, height: 0)
                emptyView.isHidden = true
                blockViews.append(emptyView)
                continue
            }

            let indentX = CGFloat(block.indentLevel) * 20
            let bulletX  = indentX + 12
            let textX    = indentX + 32

            // All tags shown as right-side pills (stripped from editor text)
            let allTags = block.tags

            // Reserve space for right-side badges (all tags + task properties)
            let badgeCount = allTags.count
                + (block.deadline != nil ? 1 : 0)
                + (block.scheduled != nil ? 1 : 0)
                + (block.effort != nil ? 1 : 0)
            let badgeWidth: CGFloat = badgeCount > 0 ? min(CGFloat(badgeCount) * 80 + 8, 280) : 0
            let priorityWidth: CGFloat = block.priority != nil ? 22 : 0
            let textWidth = max(bounds.width - textX - 12 - badgeWidth - priorityWidth, 80)

            // MARK: Bullet creation
            // Bullet — custom icon + color per type tag, or default bullet
            let matchedType = block.tags
                .compactMap { tag in typeRegistry.first(where: { $0.name.lowercased() == tag.lowercased() }) }
                .first
            let bulletSymbol = matchedType.flatMap { $0.icon.isEmpty ? nil : $0.icon } ?? (block.indentLevel == 0 ? "•" : "◦")
            let bulletColor: NSColor = {
                guard let hex = matchedType?.color, !hex.isEmpty, hex != "#808080" else { return .tertiaryLabelColor }
                var str = hex.trimmingCharacters(in: .whitespacesAndNewlines)
                if str.hasPrefix("#") { str.removeFirst() }
                guard str.count == 6 else { return .tertiaryLabelColor }
                var rgb: UInt64 = 0
                Scanner(string: str).scanHexInt64(&rgb)
                return NSColor(red: CGFloat((rgb >> 16) & 0xFF) / 255,
                               green: CGFloat((rgb >> 8) & 0xFF) / 255,
                               blue: CGFloat(rgb & 0xFF) / 255, alpha: 1)
            }()
            // --- BASELINE-ALIGNED LAYOUT ---
            // All inline elements (bullet, status, text, pills) align to a shared baseline
            let baselineY = yOffset + 11  // visual center of the first text line

            let blockIndex = index
            let bullet = BulletView(symbol: bulletSymbol, tintColor: bulletColor)
            // Offset the bullet glyph so its optical center lands on the shared text baseline.
            bullet.frame = NSRect(x: bulletX, y: baselineY - 7, width: 16, height: 14)
            bullet.onLeftClick = { [weak self] in
                self?.delegate?.outlinerDidRequestBlockZoom(blockIndex: blockIndex)
            }
            bullet.onShowProperties = { [weak self] in
                guard let self else { return }
                if expandedBlockIndex == blockIndex {
                    expandedBlockIndex = nil
                } else {
                    expandedBlockIndex = blockIndex
                }
                rebuildBlockViews()
            }
            addSubview(bullet)

            // MARK: Status icon positioning
            // Task status icon — aligned to baseline
            var actualTextX = textX
            if block.isTask {
                let statusChar: String = switch block.status {
                case "todo":  "☐"
                case "doing": "◎"
                case "done":  "☑"
                default:      "☐"
                }
                let statusColor: NSColor = if let priority = block.priority {
                    switch priority {
                    case .critical: .systemRed
                    case .high:     .systemOrange
                    case .medium:   .secondaryLabelColor
                    case .low:      .systemBlue
                    }
                } else {
                    switch block.status {
                    case "done":  .systemGreen
                    case "doing": .systemOrange
                    default:      .secondaryLabelColor
                    }
                }
                let statusLabel = NSTextField(labelWithString: statusChar)
                let statusFont = NSFont.systemFont(ofSize: NSFont.systemFontSize - 1)
                statusLabel.font = statusFont
                statusLabel.textColor = statusColor
                statusLabel.isEditable = false
                statusLabel.isBordered = false
                statusLabel.drawsBackground = false
                statusLabel.sizeToFit()
                statusLabel.frame = baselineAlignedLabelFrame(
                    for: statusLabel,
                    font: statusFont,
                    baselineY: baselineY,
                    x: bulletX + 18,
                    width: 16,
                    minHeight: 16
                )
                addSubview(statusLabel)
                actualTextX = bulletX + 36
            }

            // MARK: BlockView creation and height measurement
            let view = BlockView(block: block, typeTagNames: typeTagNames)
            let activeSearch = vimEngine.searchQuery.isEmpty ? nil : vimEngine.searchQuery
            view.searchQuery = activeSearch
            if activeSearch != nil, let ts = view.textStorage {
                BlockStyler.style(text: ts.string, textStorage: ts, searchQuery: activeSearch)
            }
            view.frame = NSRect(x: actualTextX, y: yOffset, width: textWidth, height: 22)
            wireCallbacks(for: view, at: index)
            addSubview(view)
            blockViews.append(view)

            let height = blockHeight(for: view)
            view.frame.size.height = height

            // MARK: Right-side badges
            // Right-side badges — all aligned to baseline
            let badgeFont = NSFont.systemFont(ofSize: 10)
            let pillY = baselineAlignedPillY(
                font: badgeFont,
                baselineY: baselineY,
                height: 18
            )
            let editBtnY = baselineY - 7
            // Right-align badges from the view edge (build right to left)
            var rightEdge = bounds.width - 16

            if let effort = block.effort {
                let pill = makeDateBadge("⏱ \(effort)", color: .secondaryLabelColor)
                rightEdge -= pill.frame.width
                pill.frame.origin = NSPoint(x: rightEdge, y: pillY)
                addSubview(pill)
                rightEdge -= 4
            }

            if let scheduled = block.scheduled {
                let editBtn = makeEditDateButton(propertyKey: "scheduled", blockIndex: index)
                rightEdge -= editBtn.frame.width
                editBtn.frame.origin = NSPoint(x: rightEdge, y: editBtnY)
                addSubview(editBtn)
                rightEdge -= 2

                let pill = makeDateBadge("📅 \(formatDateShort(scheduled))", color: .secondaryLabelColor)
                rightEdge -= pill.frame.width
                pill.frame.origin = NSPoint(x: rightEdge, y: pillY)
                addSubview(pill)
                rightEdge -= 4
            }

            if let deadline = block.deadline {
                let editBtn = makeEditDateButton(propertyKey: "deadline", blockIndex: index)
                rightEdge -= editBtn.frame.width
                editBtn.frame.origin = NSPoint(x: rightEdge, y: editBtnY)
                addSubview(editBtn)
                rightEdge -= 2

                let pill = makeDeadlineBadge(deadline)
                rightEdge -= pill.frame.width
                pill.frame.origin = NSPoint(x: rightEdge, y: pillY)
                addSubview(pill)
                rightEdge -= 4
            }

            // MARK: Tag text rendering
            // Tags as right-aligned plain text (like Logseq)
            if !allTags.isEmpty {
                let blockIdx = index
                let tagText = allTags.map { "#\($0)" }.joined(separator: "  ")
                let tagLabel = NSTextField(labelWithString: tagText)
                let tagFont = NSFont.systemFont(ofSize: 12)
                tagLabel.font = tagFont
                tagLabel.textColor = .systemBlue
                tagLabel.isEditable = false
                tagLabel.isBordered = false
                tagLabel.drawsBackground = false
                tagLabel.alignment = .right
                tagLabel.sizeToFit()
                // Right-align to the view edge (or left of badges if present)
                rightEdge -= tagLabel.frame.width
                let tagX = rightEdge
                tagLabel.frame = baselineAlignedLabelFrame(
                    for: tagLabel,
                    font: tagFont,
                    baselineY: baselineY,
                    x: tagX,
                    width: tagLabel.frame.width,
                    minHeight: 16
                )
                // Click navigates to tag page (first tag)
                let firstTag = allTags[0]
                let clickAction = DatePickerAction { [weak self] in
                    self?.delegate?.outlinerDidClickWikiLink(target: firstTag.lowercased())
                }
                let clickRecognizer = NSClickGestureRecognizer(target: clickAction, action: #selector(DatePickerAction.execute))
                tagLabel.addGestureRecognizer(clickRecognizer)
                objc_setAssociatedObject(tagLabel, "tagClickAction", clickAction, .OBJC_ASSOCIATION_RETAIN)
                // Right-click to remove
                let menu = NSMenu()
                for tag in allTags {
                    let removeAction = DatePickerAction { [weak self] in
                        self?.removeTag(tag, at: blockIdx)
                    }
                    let item = NSMenuItem(title: "Remove #\(tag)", action: #selector(DatePickerAction.execute), keyEquivalent: "")
                    item.target = removeAction
                    menu.addItem(item)
                    objc_setAssociatedObject(item, "removeAction", removeAction, .OBJC_ASSOCIATION_RETAIN)
                }
                tagLabel.menu = menu
                addSubview(tagLabel)
            }

            // MARK: Block position recording for threading
            blockPositions.append((y: yOffset, height: height, indent: block.indentLevel, bulletCenterX: bulletX + 8))
            yOffset += height + 4

            // MARK: Expanded property display
            // Expanded block: show inherited properties
            if expandedBlockIndex == index && !block.tags.isEmpty {
                let inheritedProps = resolveInheritedProperties(for: block)
                if !inheritedProps.isEmpty {
                    // "Properties" header
                    let headerLabel = NSTextField(labelWithString: "▼ Properties")
                    headerLabel.font = .systemFont(ofSize: 11)
                    headerLabel.textColor = .secondaryLabelColor
                    headerLabel.isEditable = false
                    headerLabel.isBordered = false
                    headerLabel.drawsBackground = false
                    headerLabel.frame = NSRect(x: textX + 8, y: yOffset, width: 200, height: 18)
                    addSubview(headerLabel)
                    yOffset += 20

                    for (propDef, currentValue) in inheritedProps {
                        let icon = propertyTypeIcon(propDef.valueType)
                        let iconLabel = NSTextField(labelWithString: icon)
                        iconLabel.font = .systemFont(ofSize: 10)
                        iconLabel.textColor = .secondaryLabelColor
                        iconLabel.isEditable = false
                        iconLabel.isBordered = false
                        iconLabel.drawsBackground = false
                        iconLabel.frame = NSRect(x: textX + 8, y: yOffset, width: 20, height: 18)
                        addSubview(iconLabel)

                        let nameLabel = NSTextField(labelWithString: propDef.name)
                        nameLabel.font = .boldSystemFont(ofSize: 11)
                        nameLabel.textColor = .labelColor
                        nameLabel.isEditable = false
                        nameLabel.isBordered = false
                        nameLabel.drawsBackground = false
                        nameLabel.frame = NSRect(x: textX + 30, y: yOffset, width: 100, height: 18)
                        addSubview(nameLabel)

                        let valueText = currentValue ?? "Empty"
                        let valueLabel = NSTextField(labelWithString: valueText)
                        valueLabel.font = .systemFont(ofSize: 11)
                        valueLabel.textColor = currentValue != nil ? .labelColor : .tertiaryLabelColor
                        valueLabel.isEditable = false
                        valueLabel.isBordered = false
                        valueLabel.drawsBackground = false
                        valueLabel.frame = NSRect(x: textX + 140, y: yOffset, width: 200, height: 18)

                        // Make value clickable for editing
                        let propName = propDef.name
                        let propType = propDef.valueType
                        let propChoices = propDef.values
                        let blockIdx = index
                        let editAction = DatePickerAction { [weak self] in
                            self?.editProperty(name: propName, valueType: propType, choices: propChoices, at: blockIdx)
                        }
                        let editClick = NSClickGestureRecognizer(target: editAction, action: #selector(DatePickerAction.execute))
                        valueLabel.addGestureRecognizer(editClick)
                        objc_setAssociatedObject(valueLabel, "editAction", editAction, .OBJC_ASSOCIATION_RETAIN)

                        addSubview(valueLabel)
                        yOffset += 20
                    }
                    yOffset += 8
                }
            }

        }

        // MARK: Thread line drawing
        // Draw indent thread lines connecting parent bullets to children
        drawThreadLines(blockPositions: blockPositions)

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

    // MARK: - Block property expansion helpers

    private func resolveInheritedProperties(for block: Block) -> [(PropertyDef, String?)] {
        var result: [(PropertyDef, String?)] = []
        for tagName in block.tags {
            if let typeDef = typeRegistry.first(where: { $0.name.lowercased() == tagName.lowercased() }) {
                for prop in typeDef.properties {
                    // Check if block has a value for this property
                    let value = block.properties[prop.name] ?? block.properties[prop.name.lowercased()]
                    result.append((prop, value))
                }
            }
        }
        return result
    }

    private func propertyTypeIcon(_ valueType: String) -> String {
        switch valueType {
        case "text", "select": return "T"
        case "number": return "N°"
        case "date", "datetime": return "📅"
        case "checkbox": return "☑"
        case "url": return "🔗"
        case "node": return "→"
        default: return "T"
        }
    }

    private func editProperty(name: String, valueType: String, choices: [String]?, at index: Int) {
        guard index < blocks.count, index < blockViews.count else { return }
        let block = blocks[index]
        let anchorView = blockViews[index]

        // Ensure our window is frontmost before showing any popover/alert
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)

        if valueType == "date" || valueType == "datetime" {
            showDatePicker(for: name.lowercased(), at: index, anchorView: anchorView)
        } else if valueType == "select", let choices, !choices.isEmpty {
            showSelectPopover(propertyName: name, choices: choices, at: index, anchorView: anchorView)
        } else if valueType == "node" {
            showNodePicker(propertyName: name, at: index, anchorView: anchorView)
        } else {
            // Text/number: inline input alert
            let alert = NSAlert()
            alert.messageText = "Set \(name)"
            alert.addButton(withTitle: "OK")
            alert.addButton(withTitle: "Cancel")
            let input = NSTextField(frame: NSRect(x: 0, y: 0, width: 200, height: 24))
            input.stringValue = block.properties[name] ?? block.properties[name.lowercased()] ?? ""
            alert.accessoryView = input
            if alert.runModal() == .alertFirstButtonReturn {
                applyBlockProperty(name: name, value: input.stringValue, at: index)
            }
        }
    }

    private func showNodePicker(propertyName: String, at index: Int, anchorView: NSView) {
        activePopover?.close()
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)

        let completionView = CompletionView(items: allPageTitles)
        completionView.onSelect = { [weak self] selected in
            self?.activePopover?.close()
            self?.activePopover = nil
            self?.applyBlockProperty(name: propertyName, value: "[[\(selected)]]", at: index)
        }
        completionView.onDismiss = { [weak self] in
            self?.activePopover?.close()
            self?.activePopover = nil
        }

        let vc = NSViewController()
        vc.view = completionView

        let popover = NSPopover()
        popover.contentViewController = vc
        popover.behavior = .transient
        popover.show(relativeTo: anchorView.bounds, of: anchorView, preferredEdge: .maxY)
        activePopover = popover
        popover.contentViewController?.view.window?.makeFirstResponder(completionView)
    }

    private func editTextProperty(name: String, at index: Int) {
        guard index < blocks.count, index < blockViews.count else { return }
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)

        let alert = NSAlert()
        alert.messageText = "Set \(name)"
        alert.addButton(withTitle: "OK")
        alert.addButton(withTitle: "Cancel")
        let input = NSTextField(frame: NSRect(x: 0, y: 0, width: 200, height: 24))
        input.stringValue = blocks[index].properties[name] ?? blocks[index].properties[name.lowercased()] ?? ""
        alert.accessoryView = input
        if alert.runModal() == .alertFirstButtonReturn {
            applyBlockProperty(name: name, value: input.stringValue, at: index)
        }
    }

    private func showSelectPopover(propertyName: String, choices: [String], at index: Int, anchorView: NSView) {
        activePopover?.close()
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)

        // Pre-select current value if set
        let currentValue = blocks[index].properties[propertyName] ?? blocks[index].properties[propertyName.lowercased()]
        let initialIndex = currentValue.flatMap { val in choices.firstIndex(of: val) } ?? 0

        let listView = SelectListView(choices: choices, selectedIndex: initialIndex)
        listView.onSelect = { [weak self] choice in
            self?.activePopover?.close()
            self?.activePopover = nil
            self?.applyBlockProperty(name: propertyName, value: choice, at: index)
        }
        listView.onDismiss = { [weak self] in
            self?.activePopover?.close()
            self?.activePopover = nil
        }

        let vc = NSViewController()
        vc.view = listView

        let popover = NSPopover()
        popover.contentViewController = vc
        popover.behavior = .transient
        popover.show(relativeTo: anchorView.bounds, of: anchorView, preferredEdge: .maxY)
        activePopover = popover

        // Make the list view first responder so it receives key events
        popover.contentViewController?.view.window?.makeFirstResponder(listView)
    }

    private func applyBlockProperty(name: String, value: String, at index: Int) {
        guard index < blocks.count else { return }
        let block = blocks[index]
        let key = name.lowercased()

        var lines = block.text.components(separatedBy: "\n")
        var replaced = false
        for (i, line) in lines.enumerated() {
            if line.trimmingCharacters(in: .whitespaces).lowercased().hasPrefix("\(key):: ") {
                lines[i] = "\(key):: \(value)"
                replaced = true
                break
            }
        }
        if !replaced {
            lines.append("\(key):: \(value)")
        }
        block.text = lines.joined(separator: "\n")
        block.tags = BlockParser.extractTags(from: block.text)
        block.properties = BlockParser.extractProperties(from: block.text)

        pendingFocusIndex = index
        rebuildBlockViews()
        delegate?.outlinerDidChangeContent(blocks: blocks)
    }

    private func makeTagPill(_ text: String, removable: Bool = false) -> NSView {
        let container = NSView()
        container.wantsLayer = true
        container.layer?.backgroundColor = NSColor.secondaryLabelColor.withAlphaComponent(0.15).cgColor
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

        var totalWidth = label.frame.width + 12
        if removable {
            let xLabel = NSTextField(labelWithString: "×")
            xLabel.font = .systemFont(ofSize: 9)
            xLabel.textColor = .tertiaryLabelColor
            xLabel.isEditable = false
            xLabel.isBordered = false
            xLabel.drawsBackground = false
            xLabel.sizeToFit()
            xLabel.frame.origin = NSPoint(x: label.frame.maxX + 2, y: 1)
            xLabel.tag = 999  // marker for finding the × button
            container.addSubview(xLabel)
            totalWidth = xLabel.frame.maxX + 4
        }

        container.frame.size = NSSize(width: totalWidth, height: 18)
        return container
    }

    private func removeTag(_ tag: String, at index: Int) {
        guard index < blocks.count else { return }
        // Remove from the tags array (source of truth)
        blocks[index].tags.removeAll { $0 == tag }
        // Remove from storage text too
        var text = blocks[index].text
        text = text.replacingOccurrences(of: " #\(tag)", with: "")
        text = text.replacingOccurrences(of: "#\(tag) ", with: "")
        text = text.replacingOccurrences(of: "#\(tag)", with: "")
        blocks[index].text = text.trimmingCharacters(in: .whitespaces)
        pendingFocusIndex = index
        rebuildBlockViews()
        delegate?.outlinerDidChangeContent(blocks: blocks)
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

    private func baselineAlignedLabelFrame(
        for label: NSTextField,
        font: NSFont,
        baselineY: CGFloat,
        x: CGFloat,
        width: CGFloat,
        minHeight: CGFloat
    ) -> NSRect {
        let measuredHeight = max(label.frame.height, font.boundingRectForFont.height)
        let height = max(minHeight, ceil(measuredHeight))
        let y = baselineY - ceil(font.ascender) - floor((height - measuredHeight) / 2)
        return NSRect(x: x, y: y, width: width, height: height)
    }

    private func baselineAlignedPillY(font: NSFont, baselineY: CGFloat, height: CGFloat) -> CGFloat {
        baselineY - ceil(font.ascender) - floor((height - font.boundingRectForFont.height) / 2)
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
        // Track focus on click/tab into a block
        view.onFocused = { [weak self] in
            guard let self else { return }
            focusedBlockIndex = index
            if index < blocks.count {
                let b = blocks[index]
                delegate?.outlinerDidFocusBlock(text: b.displayText, tags: b.tags, properties: b.properties)
            }
        }
        // Inline autocomplete — forward nav keys to CompletionView
        view.isCompletionVisible = { [weak self] in
            self?.activeCompletionPopover != nil
        }
        view.onCompletionKey = { [weak self] event in
            guard let self, let cv = activeCompletionView else { return false }
            switch event.keyCode {
            case 125, 126: // Down arrow, Up arrow
                cv.keyDown(with: event)
                return true
            case 36: // Enter
                cv.keyDown(with: event)
                return true
            case 53: // Escape
                dismissCompletion()
                return true
            default:
                return false // let typing pass through to editor
            }
        }
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

            // Defer autocomplete check — cursor position is stale during textStorage delegate
            DispatchQueue.main.async { [weak self] in
                self?.checkForCompletion(in: view, at: index)
            }

            let oldTags = Set(blocks[index].tags)
            blocks[index].updateDisplayText(newText, typeTagNames: nil)
            // Tags are managed by the pill UI (add via autocomplete, remove via ×)
            // Don't re-extract from text — block.tags is the source of truth
            let newProps = BlockParser.extractProperties(from: blocks[index].text)
            blocks[index].priority = Priority(rawValue: newProps["priority"] ?? "")
            blocks[index].properties = newProps
            let newTags = Set(blocks[index].tags)

            let newH = blockHeight(for: view)
            let tagsChanged = oldTags != newTags
            if abs(view.frame.size.height - newH) > 2 || tagsChanged {
                pendingFocusIndex = index
                // Preserve cursor position across rebuild (display text may be shorter after tag stripping)
                let cursorPos = min(view.selectedRange().location, blocks[index].displayText(strippingOnly: nil).count)
                pendingCursorPosition = cursorPos
                rebuildBlockViews()
            }
            delegate?.outlinerDidChangeContent(blocks: blocks)
        }

        view.onEnterPressed = { [weak self] before, after in
            guard let self, index < blocks.count else { return }
            saveUndoState()
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
            saveUndoState()
            let maxIndent = index > 0 ? blocks[index - 1].indentLevel + 1 : 0
            blocks[index].indentLevel = min(blocks[index].indentLevel + 1, maxIndent)
            pendingFocusIndex = index
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)
        }

        view.onShiftTabPressed = { [weak self] in
            guard let self, index < blocks.count else { return }
            saveUndoState()
            blocks[index].indentLevel = max(blocks[index].indentLevel - 1, 0)
            pendingFocusIndex = index
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)
        }

        view.onBackspaceAtStart = { [weak self] in
            guard let self, index > 0, index < blocks.count else { return }
            saveUndoState()
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

        // Save undo state before structural mutations
        switch cmd {
        case .deleteBlock, .indentBlock, .dedentBlock, .joinBlock,
             .pasteBelow, .pasteAbove, .toggleTodo,
             .enterInsertNewLineBelow, .enterInsertNewLineAbove:
            saveUndoState()
        default: break
        }

        // Track edits for dot-repeat
        switch cmd {
        case .deleteBlock, .deleteChar, .indentBlock, .dedentBlock,
             .delete, .change, .pasteBelow, .pasteAbove,
             .toggleTodo, .enterInsertNewLineBelow, .enterInsertNewLineAbove,
             .joinBlock:
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

        // Visual mode — extend selection
        case .visualExtendLeft:   view.moveLeftAndModifySelection(nil)
        case .visualExtendRight:  view.moveRightAndModifySelection(nil)
        case .visualExtendWordForward:  view.moveWordForwardAndModifySelection(nil)
        case .visualExtendWordBackward: view.moveWordBackwardAndModifySelection(nil)
        case .visualExtendLineStart:    view.moveToBeginningOfLineAndModifySelection(nil)
        case .visualExtendLineEnd:      view.moveToEndOfLineAndModifySelection(nil)
        case .visualExtendBlockDown:
            // Extend selection to end of current block, then into next
            view.moveToEndOfDocumentAndModifySelection(nil)
        case .visualExtendBlockUp:
            view.moveToBeginningOfDocumentAndModifySelection(nil)

        // Visual mode — operators on selection
        case .visualDelete:
            let sel = view.selectedRange()
            guard sel.length > 0 else { break }
            let selectedText = (view.string as NSString).substring(with: sel)
            vimEngine.yankRegister = selectedText
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(selectedText, forType: .string)
            view.replaceCharacters(in: sel, with: "")
            view.onTextChanged?(view.string)
            vimEngine.lastEditCommand = .visualDelete
        case .visualYank:
            let sel = view.selectedRange()
            guard sel.length > 0 else { break }
            let selectedText = (view.string as NSString).substring(with: sel)
            vimEngine.yankRegister = selectedText
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(selectedText, forType: .string)
            // Move cursor to start of selection and deselect
            view.setSelectedRange(NSRange(location: sel.location, length: 0))
        case .visualChange:
            let sel = view.selectedRange()
            guard sel.length > 0 else { break }
            let selectedText = (view.string as NSString).substring(with: sel)
            vimEngine.yankRegister = selectedText
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(selectedText, forType: .string)
            view.replaceCharacters(in: sel, with: "")
            view.onTextChanged?(view.string)
            vimEngine.lastEditCommand = .visualChange

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
                // Use the NEW count (e.g., 5. after dw should repeat 5 times)
                let repeatCount = count
                for _ in 0..<repeatCount {
                    executeVimCommand(lastCmd, at: index)
                }
            }

        // Search
        case .startSearch:
            showSearchBar()
        case .searchNext:
            jumpToNextMatch()
        case .searchPrev:
            jumpToPrevMatch()

        // Undo / redo — structural stack first, then NSTextView
        case .undo:
            if !undoStack.isEmpty {
                redoStack.append((blocks.map { $0.deepCopy() }, focusedBlockIndex))
                let (saved, savedFocus) = undoStack.removeLast()
                blocks = saved
                pendingFocusIndex = savedFocus ?? 0
                rebuildBlockViews()
                delegate?.outlinerDidChangeContent(blocks: blocks)
            } else if let um = view.undoManager ?? view.window?.undoManager {
                um.undo()
            }
        case .redo:
            if !redoStack.isEmpty {
                undoStack.append((blocks.map { $0.deepCopy() }, focusedBlockIndex))
                let (saved, savedFocus) = redoStack.removeLast()
                blocks = saved
                pendingFocusIndex = savedFocus ?? 0
                rebuildBlockViews()
                delegate?.outlinerDidChangeContent(blocks: blocks)
            } else if let um = view.undoManager ?? view.window?.undoManager {
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
            view.string = block.displayText
            if let ts = view.textStorage {
                BlockStyler.style(text: block.displayText, textStorage: ts)
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

        // Section navigation (tile jumping)
        case .prevSection:
            delegate?.outlinerDidRequestPrevTile()
        case .nextSection:
            delegate?.outlinerDidRequestNextTile()

        case .replaceChar, .moveUp, .moveDown:
            break

        case .none:
            break
        }
    }

    // MARK: - Thread lines (indent hierarchy visualization)

    private func drawThreadLines(blockPositions: [(y: CGFloat, height: CGFloat, indent: Int, bulletCenterX: CGFloat)]) {
        guard blockPositions.count > 1 else { return }

        // For each block with children, draw a vertical thread line at
        // the first child's actual bullet X center. The line connects
        // all siblings visually, like Logseq's threading.
        for i in 0..<blockPositions.count {
            let parentIndent = blockPositions[i].indent
            guard i + 1 < blockPositions.count, blockPositions[i + 1].indent > parentIndent else { continue }

            // Find the last block before indent returns to parent level
            var lastChild = i + 1
            for j in (i + 2)..<blockPositions.count {
                if blockPositions[j].indent <= parentIndent { break }
                lastChild = j
            }

            // Use the first child's actual bullet center X
            let lineX = blockPositions[i + 1].bulletCenterX

            // Y: from parent's baseline down to the last child's baseline
            // baseline = y + 11 (center of first text line)
            let startY = blockPositions[i].y + 11 + 4  // just below parent baseline
            let endY = blockPositions[lastChild].y + 11  // at last child baseline

            guard endY > startY else { continue }

            let line = NSView(frame: NSRect(x: lineX, y: startY, width: 1, height: endY - startY))
            line.wantsLayer = true
            line.layer?.backgroundColor = NSColor.tertiaryLabelColor.withAlphaComponent(0.25).cgColor
            addSubview(line)
        }
    }

    // MARK: - Inline autocomplete (#tags and [[page refs]])

    private enum CompletionTrigger { case tag, pageRef }
    private struct CompletionContext {
        let trigger: CompletionTrigger
        let blockIndex: Int
        let triggerPosition: Int  // position of # or [[ in the text
        var query: String
    }

    private var activeCompletionPopover: NSPopover?
    private var activeCompletionView: CompletionView?
    private var activeCompletion: CompletionContext?

    /// Called from onTextChanged to detect and manage autocomplete.
    private func checkForCompletion(in view: BlockView, at index: Int) {
        guard vimEngine.currentMode == .insert else {
            dismissCompletion()
            return
        }

        let text = view.string
        guard !text.isEmpty else { dismissCompletion(); return }
        let cursorPos = view.selectedRange().location
        guard cursorPos > 0, cursorPos <= text.count else { dismissCompletion(); return }

        let before = String(text.prefix(cursorPos))

        // Check for [[ trigger
        if let bracketIdx = before.range(of: "[[", options: .backwards) {
            let afterBrackets = String(before[bracketIdx.upperBound...])
            // No closing ]] yet, and no newlines in query
            if !afterBrackets.contains("]]") && !afterBrackets.contains("\n") {
                let query = afterBrackets
                let triggerPos = before.distance(from: before.startIndex, to: bracketIdx.lowerBound)
                if let ctx = activeCompletion, ctx.trigger == .pageRef, ctx.blockIndex == index {
                    // Update existing
                    activeCompletion?.query = query
                    activeCompletionView?.updateQuery(query)
                } else {
                    showCompletion(trigger: .pageRef, query: query, triggerPosition: triggerPos, blockIndex: index, anchorView: view)
                }
                return
            }
        }

        // Check for # trigger
        if let hashIdx = before.lastIndex(of: "#") {
            let posInString = before.distance(from: before.startIndex, to: hashIdx)
            // # must be at start or preceded by whitespace
            let validStart = posInString == 0 || before[before.index(before: hashIdx)].isWhitespace
            if validStart {
                let afterHash = String(before[before.index(after: hashIdx)...])
                // Query is alphanumeric/dash/underscore only, no spaces
                let validQuery = afterHash.allSatisfy { $0.isLetter || $0.isNumber || $0 == "-" || $0 == "_" }
                if validQuery {
                    if let ctx = activeCompletion, ctx.trigger == .tag, ctx.blockIndex == index {
                        activeCompletion?.query = afterHash
                        activeCompletionView?.updateQuery(afterHash)
                    } else {
                        showCompletion(trigger: .tag, query: afterHash, triggerPosition: posInString, blockIndex: index, anchorView: view)
                    }
                    return
                }
            }
        }

        // No trigger found
        dismissCompletion()
    }

    private func showCompletion(trigger: CompletionTrigger, query: String, triggerPosition: Int, blockIndex: Int, anchorView: BlockView) {
        dismissCompletion()

        // Both # and [[ search all pages — tags are just pages
        let items = allPageTitles

        guard !items.isEmpty || trigger == .tag else { return }

        let completionView = CompletionView(items: items)
        completionView.showCreateOption = (trigger == .tag)
        completionView.updateQuery(query)
        completionView.onSelect = { [weak self] selected in
            self?.insertCompletion(selected, trigger: trigger, blockIndex: blockIndex)
        }
        completionView.onDismiss = { [weak self] in
            self?.dismissCompletion()
        }

        activeCompletion = CompletionContext(trigger: trigger, blockIndex: blockIndex, triggerPosition: triggerPosition, query: query)
        activeCompletionView = completionView

        let vc = NSViewController()
        vc.view = completionView

        let popover = NSPopover()
        popover.contentViewController = vc
        popover.behavior = .semitransient
        popover.show(relativeTo: anchorView.cursorRect(), of: anchorView, preferredEdge: .maxY)
        activeCompletionPopover = popover
        // BlockView keeps focus — keys forwarded via onCompletionKey callback
    }

    private func dismissCompletion() {
        activeCompletionPopover?.close()
        activeCompletionPopover = nil
        activeCompletionView = nil
        activeCompletion = nil
    }

    private func insertCompletion(_ selected: String, trigger: CompletionTrigger, blockIndex: Int) {
        guard blockIndex < blockViews.count,
              let ctx = activeCompletion else { return }
        let view = blockViews[blockIndex]

        let triggerPos = ctx.triggerPosition
        let cursorPos = view.selectedRange().location

        switch trigger {
        case .tag:
            // Don't insert #tag into the editor — add to block.tags and remove the typed #query
            let range = NSRange(location: triggerPos, length: cursorPos - triggerPos)
            view.replaceCharacters(in: range, with: "")
            view.setSelectedRange(NSRange(location: triggerPos, length: 0))

            // Add tag to the block's tags array (source of truth)
            if !blocks[blockIndex].tags.contains(selected) {
                blocks[blockIndex].tags.append(selected)
            }
            // Update storage text to include the new tag
            blocks[blockIndex].text = view.string
            let hashTags = blocks[blockIndex].tags.map { "#\($0)" }
            blocks[blockIndex].text += (blocks[blockIndex].text.isEmpty ? "" : " ") + hashTags.joined(separator: " ")

            // Rebuild to show the new pill
            pendingFocusIndex = blockIndex
            pendingCursorPosition = triggerPos
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)

        case .pageRef:
            let replacement = "[[\(selected)]]"
            let range = NSRange(location: triggerPos, length: cursorPos - triggerPos)
            view.replaceCharacters(in: range, with: replacement)
            let newPos = triggerPos + (replacement as NSString).length
            view.setSelectedRange(NSRange(location: newPos, length: 0))
            view.onTextChanged?(view.string)
        }

        dismissCompletion()

        // Trigger text change handlers
        view.onTextChanged?(view.string)
    }

    // MARK: - In-page search (/pattern, n, N)

    private var searchBar: NSTextField?
    private var searchMatches: [(blockIndex: Int, range: NSRange)] = []
    private var currentMatchIndex: Int = 0

    private func showSearchBar() {
        if searchBar != nil { return }
        // Add to the scroll view so it stays fixed at the bottom of the visible area
        guard let scrollView = enclosingScrollView ?? superview else { return }
        let bar = NSTextField()
        bar.placeholderString = "Search…"
        bar.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        bar.isBordered = true
        bar.drawsBackground = true
        bar.wantsLayer = true
        bar.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        let svBounds = scrollView.bounds
        bar.frame = NSRect(x: 8, y: svBounds.height - 28, width: svBounds.width - 16, height: 24)
        bar.autoresizingMask = [.width, .minYMargin]
        scrollView.addSubview(bar)
        searchBar = bar
        window?.makeFirstResponder(bar)

        // Handle Enter and Escape via event monitor (NSTextField action unreliable in scroll views)
        let monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self, weak bar] event in
            guard let self, let bar else { return event }
            // Only intercept when search bar is focused
            guard bar.window?.firstResponder == bar.currentEditor() else { return event }
            if event.keyCode == 36 { // Enter
                let query = bar.stringValue
                if !query.isEmpty {
                    self.performSearch(query)
                } else {
                    self.dismissSearchBar(clearMatches: true)
                }
                return nil
            }
            if event.keyCode == 53 { // Escape
                self.dismissSearchBar(clearMatches: true)
                return nil
            }
            return event
        }
        objc_setAssociatedObject(bar, "searchMonitor", monitor as AnyObject, .OBJC_ASSOCIATION_RETAIN)
    }

    private func dismissSearchBar(clearMatches: Bool = false) {
        if let monitor = objc_getAssociatedObject(searchBar as Any, "searchMonitor") {
            NSEvent.removeMonitor(monitor)
        }
        searchBar?.removeFromSuperview()
        searchBar = nil
        if clearMatches {
            searchMatches = []
            currentMatchIndex = 0
            vimEngine.searchQuery = ""
            delegate?.outlinerDidUpdateSearchStatus(current: 0, total: 0)
            if let idx = focusedBlockIndex { pendingFocusIndex = idx }
            rebuildBlockViews()
        }
        // Restore focus to the last focused block
        if let idx = focusedBlockIndex, idx < blockViews.count {
            window?.makeFirstResponder(blockViews[idx])
        }
    }

    private func performSearch(_ query: String) {
        vimEngine.searchQuery = query
        searchMatches = []
        let lowerQuery = query.lowercased()

        // Search display text (what the user sees in the editor)
        for (i, block) in blocks.enumerated() {
            let display = block.displayText(strippingOnly: nil)
            let text = display.lowercased()
            var searchRange = text.startIndex..<text.endIndex
            while let range = text.range(of: lowerQuery, range: searchRange) {
                let nsRange = NSRange(range, in: text)
                searchMatches.append((blockIndex: i, range: nsRange))
                searchRange = range.upperBound..<text.endIndex
            }
        }

        // Dismiss first, then rebuild to show highlights, then jump
        dismissSearchBar()
        // Rebuild to apply search highlighting to all blocks
        if let idx = focusedBlockIndex { pendingFocusIndex = idx }
        rebuildBlockViews()
        currentMatchIndex = 0
        if !searchMatches.isEmpty {
            jumpToMatch(at: 0)
        }
    }

    private func jumpToNextMatch() {
        guard !searchMatches.isEmpty else {
            // If no matches but we have a query, re-search
            if !vimEngine.searchQuery.isEmpty { performSearch(vimEngine.searchQuery) }
            return
        }
        currentMatchIndex = (currentMatchIndex + 1) % searchMatches.count
        jumpToMatch(at: currentMatchIndex)
    }

    private func jumpToPrevMatch() {
        guard !searchMatches.isEmpty else { return }
        currentMatchIndex = (currentMatchIndex - 1 + searchMatches.count) % searchMatches.count
        jumpToMatch(at: currentMatchIndex)
    }

    private func jumpToMatch(at matchIndex: Int) {
        guard matchIndex < searchMatches.count else { return }
        let match = searchMatches[matchIndex]
        guard match.blockIndex < blockViews.count else { return }
        let view = blockViews[match.blockIndex]
        focusedBlockIndex = match.blockIndex
        window?.makeFirstResponder(view)
        // Position cursor at the match
        view.setSelectedRange(NSRange(location: match.range.location, length: 0))
        view.scrollRangeToVisible(match.range)
        delegate?.outlinerDidUpdateSearchStatus(current: matchIndex + 1, total: searchMatches.count)
    }

    // MARK: - Date picker popover

    private var activePopover: NSPopover?

    private func showDatePicker(for propertyKey: String, at index: Int, anchorView: NSView) {
        activePopover?.close()
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)

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
        let tabMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            if event.keyCode == 48 { // Tab
                tabAction.handler()
                return nil
            }
            // Enter handled by setButton.keyEquivalent (calendar) and textField.action (text mode)
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
            let display = blocks[index].displayText
            blockViews[index].string = display
            if let ts = blockViews[index].textStorage {
                BlockStyler.style(text: display, textStorage: ts)
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
// MARK: - BulletView (handles left-click drill-in + right-click context menu)
class BulletView: NSView {
    var onLeftClick: (() -> Void)?
    var onShowProperties: (() -> Void)?

    // Map icon names to SF Symbols
    private static let sfSymbolMap: [String: String] = [
        "☑": "checkmark.square",
        "🗂": "folder",
        "👤": "person",
        "📄": "doc",
        "📅": "calendar",
        "⚑": "flag",
        "📋": "list.clipboard",
        "💡": "lightbulb",
        "🔗": "link",
        "⭐": "star",
        "🏷": "tag",
    ]

    init(symbol: String, tintColor: NSColor = .tertiaryLabelColor) {
        super.init(frame: .zero)

        let sfName = Self.sfSymbolMap[symbol] ?? symbol
        let isBulletDot = (symbol == "•" || symbol == "◦")
        if !isBulletDot, let img = NSImage(systemSymbolName: sfName, accessibilityDescription: nil) {
            let config = NSImage.SymbolConfiguration(pointSize: 11, weight: .regular)
            let imageView = NSImageView()
            imageView.image = img.withSymbolConfiguration(config)
            imageView.contentTintColor = tintColor
            // Fill the BulletView frame — parent positions us at baseline
            imageView.frame = NSRect(x: 1, y: 0, width: 14, height: 14)
            addSubview(imageView)
        } else {
            let label = NSTextField(labelWithString: symbol)
            label.font = .systemFont(ofSize: 12)
            label.textColor = tintColor
            label.isEditable = false
            label.isBordered = false
            label.drawsBackground = false
            // Fill the BulletView frame — parent positions us at baseline
            label.frame = NSRect(x: 0, y: 0, width: 16, height: 14)
            addSubview(label)
        }
    }

    required init?(coder: NSCoder) { fatalError() }

    override func mouseDown(with event: NSEvent) {
        onLeftClick?()
    }

    override func rightMouseDown(with event: NSEvent) {
        let menu = NSMenu()

        let drillIn = NSMenuItem(title: "Drill In", action: #selector(handleDrillIn), keyEquivalent: "")
        drillIn.target = self
        menu.addItem(drillIn)

        let props = NSMenuItem(title: "Show Properties", action: #selector(handleShowProperties), keyEquivalent: "")
        props.target = self
        menu.addItem(props)

        NSMenu.popUpContextMenu(menu, with: event, for: self)
    }

    @objc private func handleDrillIn() { onLeftClick?() }
    @objc private func handleShowProperties() { onShowProperties?() }
}

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
    var onPrevTile: (() -> Void)?
    var onNextTile: (() -> Void)?
    var onBlockZoom: ((Int) -> Void)?
    var onSearchStatus: ((Int, Int) -> Void)?  // (current, total)
    var onFocusedBlock: ((String, [String], [String: String]) -> Void)?  // (text, tags, props)
    var tileID: String?
    var apiClient: APIClient?
    var typeRegistry: [TypeDefinition] = []
    var propertyRegistry: [PropertyDef] = []
    var allTags: [String] = []
    var allPageTitles: [String] = []

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
        outliner.tileID = tileID
        outliner.typeRegistry = typeRegistry
        outliner.propertyRegistry = propertyRegistry
        outliner.allTags = allTags
        outliner.allPageTitles = allPageTitles
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

        func outlinerDidRequestPrevTile() {
            parent.onPrevTile?()
        }

        func outlinerDidRequestNextTile() {
            parent.onNextTile?()
        }

        func outlinerDidRequestBlockZoom(blockIndex: Int) {
            parent.onBlockZoom?(blockIndex)
        }

        func outlinerDidUpdateSearchStatus(current: Int, total: Int) {
            parent.onSearchStatus?(current, total)
        }

        func outlinerDidFocusBlock(text: String, tags: [String], properties: [String: String]) {
            parent.onFocusedBlock?(text, tags, properties)
        }
    }
}
