import AppKit
import Foundation

// MARK: - BlockStyler
// Applies NSAttributedString styling to display text.
// Tags and properties are no longer in the display text (clean separation).

enum BlockStyler {
    private static let wikiLinkRegex = try! NSRegularExpression(pattern: #"\[\[([^\]]+)\]\]"#)

    static func style(text: String, textStorage: NSTextStorage) {
        let nsText = text as NSString
        let fullRange = NSRange(location: 0, length: nsText.length)

        textStorage.beginEditing()
        textStorage.addAttribute(.foregroundColor, value: NSColor.labelColor, range: fullRange)
        textStorage.addAttribute(.font, value: NSFont.systemFont(ofSize: NSFont.systemFontSize), range: fullRange)
        textStorage.removeAttribute(.backgroundColor, range: fullRange)
        textStorage.removeAttribute(.underlineStyle, range: fullRange)

        // [[wiki-links]] → blue text on tinted blue background (pill style)
        wikiLinkRegex.enumerateMatches(in: text, range: fullRange) { match, _, _ in
            guard let range = match?.range else { return }
            textStorage.addAttribute(.foregroundColor, value: NSColor.systemBlue, range: range)
            textStorage.addAttribute(.backgroundColor, value: NSColor.systemBlue.withAlphaComponent(0.25), range: range)
        }

        textStorage.endEditing()
    }
}
