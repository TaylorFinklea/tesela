import AppKit
import SwiftUI

// MARK: - OutlinerView
// AppKit NSView that hosts all BlockView (NSTextView) instances.
// This is the single AppKit boundary for the editor — Phase 11.4

class OutlinerView: NSView {
    var blocks: [Block] = [] {
        didSet { rebuildBlockViews() }
    }

    private var blockViews: [BlockView] = []
    private var vimEngine = VimEngine()

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
    }

    func rebuildBlockViews() {
        blockViews.forEach { $0.removeFromSuperview() }
        blockViews.removeAll()

        var yOffset: CGFloat = 8
        for block in blocks {
            let view = BlockView(block: block)
            view.frame = NSRect(x: CGFloat(block.indentLevel) * 20 + 16,
                                y: yOffset,
                                width: bounds.width - CGFloat(block.indentLevel) * 20 - 32,
                                height: 28)
            addSubview(view)
            blockViews.append(view)
            yOffset += view.frame.height + 4
        }
        frame.size.height = yOffset + 8
    }
}

// MARK: - OutlinerCoordinator (NSViewRepresentable)
struct OutlinerCoordinator: NSViewRepresentable {
    @Binding var blocks: [Block]

    func makeNSView(context: Context) -> OutlinerView {
        let view = OutlinerView()
        view.blocks = blocks
        return view
    }

    func updateNSView(_ nsView: OutlinerView, context: Context) {
        nsView.blocks = blocks
    }
}
