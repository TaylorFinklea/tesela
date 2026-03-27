import AppKit

// MARK: - SelectListView
// Keyboard-navigable list for select property popovers.
// Arrow keys / j/k to move, Enter to confirm, Escape to dismiss.

class SelectListView: NSView {
    let choices: [String]
    var selectedIndex: Int {
        didSet { needsDisplay = true }
    }
    var onSelect: ((String) -> Void)?
    var onDismiss: (() -> Void)?

    private let rowHeight: CGFloat = 28
    private let padding: CGFloat = 4
    private let horizontalPadding: CGFloat = 8
    private let width: CGFloat = 200

    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }

    init(choices: [String], selectedIndex: Int = 0) {
        self.choices = choices
        self.selectedIndex = selectedIndex
        let height = CGFloat(choices.count) * 28 + 8
        super.init(frame: NSRect(x: 0, y: 0, width: 200, height: height))
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) not implemented")
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)

        for (i, choice) in choices.enumerated() {
            let rowRect = NSRect(x: 0, y: padding + CGFloat(i) * rowHeight,
                                 width: bounds.width, height: rowHeight)

            // Highlight selected row
            if i == selectedIndex {
                NSColor.controlAccentColor.withAlphaComponent(0.2).setFill()
                let highlightRect = rowRect.insetBy(dx: 2, dy: 1)
                NSBezierPath(roundedRect: highlightRect, xRadius: 4, yRadius: 4).fill()
            }

            // Draw text
            let textRect = NSRect(x: horizontalPadding, y: rowRect.origin.y + 4,
                                  width: bounds.width - horizontalPadding * 2, height: rowHeight - 8)
            let attrs: [NSAttributedString.Key: Any] = [
                .font: NSFont.systemFont(ofSize: NSFont.systemFontSize),
                .foregroundColor: NSColor.labelColor
            ]
            (choice as NSString).draw(in: textRect, withAttributes: attrs)
        }
    }

    // MARK: - Keyboard

    override func keyDown(with event: NSEvent) {
        switch event.keyCode {
        case 125, 38: // Down arrow, j
            selectedIndex = min(choices.count - 1, selectedIndex + 1)
        case 126, 40: // Up arrow, k
            selectedIndex = max(0, selectedIndex - 1)
        case 36: // Enter
            confirm()
        case 53: // Escape
            onDismiss?()
        default:
            super.keyDown(with: event)
        }
    }

    // MARK: - Mouse

    override func mouseDown(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        let clickedRow = Int((point.y - padding) / rowHeight)
        if clickedRow >= 0 && clickedRow < choices.count {
            selectedIndex = clickedRow
            confirm()
        }
    }

    private func confirm() {
        guard selectedIndex >= 0 && selectedIndex < choices.count else { return }
        onSelect?(choices[selectedIndex])
    }
}
