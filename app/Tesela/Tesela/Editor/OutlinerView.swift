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
            let textWidth = max(bounds.width - textX - 12, 80)

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
            yOffset += height + 4
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

        switch cmd {
        // Within-block motions → NSTextView methods
        case .moveLeft:           view.moveLeft(nil)
        case .moveRight:          view.moveRight(nil)
        case .moveWordForward:    view.moveWordForward(nil)
        case .moveWordBackward:   view.moveWordBackward(nil)
        case .moveWordEnd:        view.moveWordForward(nil)
        case .moveLineStart:      view.moveToBeginningOfLine(nil)
        case .moveLineEnd:        view.moveToEndOfLine(nil)

        // Block navigation
        case .moveNextBlock:
            let next = min(index + 1, blockViews.count - 1)
            focusedBlockIndex = next
            blockViews[next].isNormalMode = true
            window?.makeFirstResponder(blockViews[next])
        case .movePrevBlock:
            let prev = max(index - 1, 0)
            focusedBlockIndex = prev
            blockViews[prev].isNormalMode = true
            window?.makeFirstResponder(blockViews[prev])
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
        case .enterInsert:          break // cursor stays
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

        case .exitToNormal: break // mode already changed

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

        // Block-level editing
        case .deleteBlock:
            guard blocks.count > 1 else { break }
            vimEngine.yankRegister = blocks[index].text
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(blocks[index].text, forType: .string)
            blocks.remove(at: index)
            pendingFocusIndex = min(index, blocks.count - 1)
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)

        case .yankBlock:
            vimEngine.yankRegister = blocks[index].text
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(blocks[index].text, forType: .string)

        case .pasteBelow:
            let text = vimEngine.yankRegister
            guard !text.isEmpty else { break }
            let newBlock = Block(text: text, indentLevel: blocks[index].indentLevel)
            blocks.insert(newBlock, at: index + 1)
            pendingFocusIndex = index + 1
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)

        case .pasteAbove:
            let text = vimEngine.yankRegister
            guard !text.isEmpty else { break }
            let newBlock = Block(text: text, indentLevel: blocks[index].indentLevel)
            blocks.insert(newBlock, at: index)
            pendingFocusIndex = index
            rebuildBlockViews()
            delegate?.outlinerDidChangeContent(blocks: blocks)

        case .deleteChar:
            view.deleteForward(nil)

        // Operator + motion combos
        case .delete(let motion):
            applyMotionSelection(motion, on: view)
            if let range = Range(view.selectedRange(), in: view.string) {
                vimEngine.yankRegister = String(view.string[range])
            }
            view.deleteBackward(nil)

        case .change(let motion):
            applyMotionSelection(motion, on: view)
            if let range = Range(view.selectedRange(), in: view.string) {
                vimEngine.yankRegister = String(view.string[range])
            }
            view.deleteBackward(nil)
            // Mode already changed to insert by VimKeyHandler

        case .yank(let motion):
            let before = view.selectedRange()
            applyMotionSelection(motion, on: view)
            if let range = Range(view.selectedRange(), in: view.string) {
                vimEngine.yankRegister = String(view.string[range])
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(vimEngine.yankRegister, forType: .string)
            }
            view.setSelectedRange(before)

        // Undo / redo
        case .undo: view.undoManager?.undo()
        case .redo: view.undoManager?.redo()

        // Deferred to Phase 11.5
        case .enterVisual, .enterVisualLine, .startSearch,
             .repeatLastChange, .replaceChar, .moveUp, .moveDown:
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
        default: break // innerWord, aroundWord, etc. → Phase 11.5
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
