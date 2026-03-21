import AppKit
import Foundation

// MARK: - BlockStyler
// Applies NSAttributedString styling for [[wiki-links]] and #tags — Phase 11.3

enum BlockStyler {
    private static let wikiLinkRegex = try! NSRegularExpression(pattern: #"\[\[([^\]]+)\]\]"#)
    private static let tagRegex = try! NSRegularExpression(pattern: #"#([A-Za-z0-9_\-]+)"#)

    static func style(text: String, textStorage: NSTextStorage) {
        let nsText = text as NSString
        let fullRange = NSRange(location: 0, length: nsText.length)

        textStorage.beginEditing()
        textStorage.addAttribute(.foregroundColor, value: NSColor.labelColor, range: fullRange)
        textStorage.addAttribute(.font, value: NSFont.systemFont(ofSize: NSFont.systemFontSize), range: fullRange)

        // [[wiki-links]] → blue with underline
        wikiLinkRegex.enumerateMatches(in: text, range: fullRange) { match, _, _ in
            guard let range = match?.range else { return }
            textStorage.addAttribute(.foregroundColor, value: NSColor.systemBlue, range: range)
            textStorage.addAttribute(.underlineStyle, value: NSUnderlineStyle.single.rawValue, range: range)
        }

        // #tags → secondary label color
        tagRegex.enumerateMatches(in: text, range: fullRange) { match, _, _ in
            guard let range = match?.range else { return }
            textStorage.addAttribute(.foregroundColor, value: NSColor.secondaryLabelColor, range: range)
        }

        textStorage.endEditing()
    }
}
