import AppKit
import SwiftUI

// MARK: - OutlinerDelegate
@MainActor
protocol OutlinerDelegate: AnyObject {
    func outlinerDidChangeContent(blocks: [Block])
    func outlinerDidClickWikiLink(target: String)
    func outlinerDidChangeMode(mode: VimMode)
    func outlinerDidRequestCommandPalette()
}

// MARK: - OutlinerView
class OutlinerView: NSView {
    var blocks: [Block] = [] {
        didSet { rebuildBlockViews() }
    }

    weak var delegate: OutlinerDelegate?
    private(set) var focusedBlockIndex: Int?
    private var vimEngine = VimEngine()

    private var blockViews: [BlockView] = []
    private var pendingFocusIndex: Int?
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

            // Reserve space for tag pills on the right
            let tagPillsWidth: CGFloat = block.tags.isEmpty ? 0 : min(CGFloat(block.tags.count) * 80 + 8, 200)
            let textWidth = max(bounds.width - textX - 12 - tagPillsWidth, 80)

            let bulletSymbol = block.indentLevel == 0 ? "•" : "◦"
            let bullet = NSTextField(labelWithString: bulletSymbol)
            bullet.font = .systemFont(ofSize: NSFont.systemFontSize)
            bullet.textColor = .tertiaryLabelColor
            bullet.isEditable = false
            bullet.isBordered = false
            bullet.drawsBackground = false
            bullet.frame = NSRect(x: bulletX, y: yOffset, width: 16, height: 22)
            addSubview(bullet)

            let view = BlockView(block: block)
            view.frame = NSRect(x: textX, y: yOffset, width: textWidth, height: 22)
            wireCallbacks(for: view, at: index)
            addSubview(view)
            blockViews.append(view)

            let height = blockHeight(for: view)
            view.frame.size.height = height
            bullet.frame.size.height = height

            // Tag pills on the right
            if !block.tags.isEmpty {
                var tagX = textX + textWidth + 6
                for tag in block.tags {
                    let pill = makeTagPill("#\(tag)")
                    let pillWidth = pill.frame.width
                    let pillHeight: CGFloat = 18
                    pill.frame = NSRect(x: tagX, y: yOffset + (height - pillHeight) / 2, width: pillWidth, height: pillHeight)
                    addSubview(pill)
                    tagX += pillWidth + 4
                }
            }

            yOffset += height + 4

            // Property pills below block row
            if !block.properties.isEmpty {
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
                DispatchQueue.main.async { [weak self, weak view] in
                    guard let view else { return }
                    self?.window?.makeFirstResponder(view)
                }
            }
            pendingFocusIndex = nil
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

        case .replaceChar, .moveUp, .moveDown:
            break

        case .none:
            break
        }
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

// MARK: - OutlinerCoordinator (NSViewRepresentable)
struct OutlinerCoordinator: NSViewRepresentable {
    @Binding var blocks: [Block]
    var onContentChanged: (([Block]) -> Void)?
    var onWikiLinkClicked: ((String) -> Void)?
    var onModeChanged: ((VimMode) -> Void)?
    var onCommandPalette: (() -> Void)?

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.drawsBackground = false

        let outliner = OutlinerView()
        outliner.delegate = context.coordinator
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
    }
}
