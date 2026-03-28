import AppKit

// MARK: - CompletionView
// Keyboard-navigable, filtered autocomplete list for #tags and [[page refs]].
// Shown in an NSPopover anchored at the cursor position.

class CompletionView: NSView {
    private var items: [String]
    private(set) var query: String = ""
    var selectedIndex: Int = 0 {
        didSet { needsDisplay = true }
    }
    var onSelect: ((String) -> Void)?
    var onDismiss: (() -> Void)?

    private let rowHeight: CGFloat = 24
    private let padding: CGFloat = 4
    private let horizontalPadding: CGFloat = 10
    private let maxVisibleRows = 8
    private let viewWidth: CGFloat = 240

    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }

    var filteredItems: [String] {
        if query.isEmpty { return items }
        let q = query.lowercased()
        // Prefix matches first, then contains matches
        let prefix = items.filter { $0.lowercased().hasPrefix(q) }
        let contains = items.filter { !$0.lowercased().hasPrefix(q) && $0.lowercased().contains(q) }
        return prefix + contains
    }

    init(items: [String]) {
        self.items = items
        let rowCount = min(items.count, 8)
        let height = CGFloat(rowCount) * 24 + 8
        super.init(frame: NSRect(x: 0, y: 0, width: 240, height: height))
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) not implemented")
    }

    func updateQuery(_ newQuery: String) {
        query = newQuery
        selectedIndex = 0
        resizeToFit()
        needsDisplay = true
    }

    func updateItems(_ newItems: [String]) {
        items = newItems
        selectedIndex = 0
        resizeToFit()
        needsDisplay = true
    }

    private func resizeToFit() {
        let rowCount = min(filteredItems.count, maxVisibleRows)
        let height = max(CGFloat(rowCount) * rowHeight + padding * 2, rowHeight + padding * 2)
        frame.size = NSSize(width: viewWidth, height: height)
        // Notify the popover to resize
        if let vc = (window?.contentViewController) {
            vc.preferredContentSize = frame.size
        }
    }

    // MARK: - Drawing

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)

        let visible = filteredItems
        if visible.isEmpty {
            let noResultRect = NSRect(x: horizontalPadding, y: padding + 2,
                                      width: bounds.width - horizontalPadding * 2, height: rowHeight)
            let attrs: [NSAttributedString.Key: Any] = [
                .font: NSFont.systemFont(ofSize: 12),
                .foregroundColor: NSColor.tertiaryLabelColor
            ]
            ("No matches" as NSString).draw(in: noResultRect, withAttributes: attrs)
            return
        }

        for i in 0..<min(visible.count, maxVisibleRows) {
            let item = visible[i]
            let rowRect = NSRect(x: 0, y: padding + CGFloat(i) * rowHeight,
                                 width: bounds.width, height: rowHeight)

            if i == selectedIndex {
                NSColor.controlAccentColor.withAlphaComponent(0.2).setFill()
                let highlightRect = rowRect.insetBy(dx: 2, dy: 1)
                NSBezierPath(roundedRect: highlightRect, xRadius: 4, yRadius: 4).fill()
            }

            let textRect = NSRect(x: horizontalPadding, y: rowRect.origin.y + 3,
                                  width: bounds.width - horizontalPadding * 2, height: rowHeight - 6)

            // Build attributed string with bold matching portion
            let attrStr = highlightedString(item, query: query)
            attrStr.draw(in: textRect)
        }
    }

    private func highlightedString(_ text: String, query: String) -> NSAttributedString {
        let str = NSMutableAttributedString(string: text, attributes: [
            .font: NSFont.systemFont(ofSize: 12),
            .foregroundColor: NSColor.labelColor
        ])
        if !query.isEmpty, let range = text.range(of: query, options: .caseInsensitive) {
            let nsRange = NSRange(range, in: text)
            str.addAttribute(.font, value: NSFont.boldSystemFont(ofSize: 12), range: nsRange)
        }
        return str
    }

    // MARK: - Keyboard

    override func keyDown(with event: NSEvent) {
        let visible = filteredItems
        switch event.keyCode {
        case 125, 38: // Down arrow, j
            if selectedIndex < min(visible.count, maxVisibleRows) - 1 {
                selectedIndex += 1
            }
        case 126, 40: // Up arrow, k
            if selectedIndex > 0 {
                selectedIndex -= 1
            }
        case 36: // Enter
            confirm()
        case 53: // Escape
            onDismiss?()
        default:
            // Let other keys pass through (they'll be handled by the text view)
            super.keyDown(with: event)
        }
    }

    // MARK: - Mouse

    override func mouseDown(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        let clickedRow = Int((point.y - padding) / rowHeight)
        let visible = filteredItems
        if clickedRow >= 0 && clickedRow < min(visible.count, maxVisibleRows) {
            selectedIndex = clickedRow
            confirm()
        }
    }

    private func confirm() {
        let visible = filteredItems
        guard selectedIndex >= 0 && selectedIndex < visible.count else { return }
        onSelect?(visible[selectedIndex])
    }
}
