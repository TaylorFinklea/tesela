import AppKit
import Foundation

// MARK: - BlockStyler
// Applies NSAttributedString styling to display text.
// Type tags are stripped (shown as pills). Casual tags stay inline and are styled here.

enum BlockStyler {
    private static let wikiLinkRegex = try! NSRegularExpression(pattern: #"\[\[([^\]]+)\]\]"#)
    private static let hashTagRegex = try! NSRegularExpression(pattern: #"#([A-Za-z0-9_\-]+)"#)

    static func style(text: String, textStorage: NSTextStorage, searchQuery: String? = nil) {
        // Use textStorage's own length to avoid out-of-bounds when text drifts
        let storageLength = textStorage.length
        guard storageLength > 0 else { return }
        let fullRange = NSRange(location: 0, length: storageLength)

        textStorage.beginEditing()
        textStorage.addAttribute(.foregroundColor, value: NSColor.labelColor, range: fullRange)
        textStorage.addAttribute(.font, value: NSFont.systemFont(ofSize: NSFont.systemFontSize), range: fullRange)
        textStorage.removeAttribute(.backgroundColor, range: fullRange)
        textStorage.removeAttribute(.underlineStyle, range: fullRange)

        let currentText = textStorage.string

        // [[wiki-links]] → blue text on tinted blue background (pill style)
        wikiLinkRegex.enumerateMatches(in: currentText, range: fullRange) { match, _, _ in
            guard let range = match?.range else { return }
            textStorage.addAttribute(.foregroundColor, value: NSColor.systemBlue, range: range)
            textStorage.addAttribute(.backgroundColor, value: NSColor.systemBlue.withAlphaComponent(0.25), range: range)
        }

        // All tags are now stripped from display text and shown as right-side pills.
        // No inline tag styling needed.

        // Search matches → yellow highlight
        if let query = searchQuery, !query.isEmpty {
            let lowerText = currentText.lowercased()
            let lowerQuery = query.lowercased()
            var searchStart = lowerText.startIndex
            while let range = lowerText.range(of: lowerQuery, range: searchStart..<lowerText.endIndex) {
                let nsRange = NSRange(range, in: lowerText)
                if nsRange.location + nsRange.length <= storageLength {
                    textStorage.addAttribute(.backgroundColor, value: NSColor.systemYellow.withAlphaComponent(0.4), range: nsRange)
                }
                searchStart = range.upperBound
            }
        }

        textStorage.endEditing()
    }
}
