import AppKit
import Foundation

// MARK: - BlockStyler
// Applies NSAttributedString styling for [[wiki-links]] and #tags

enum BlockStyler {
    private static let wikiLinkRegex = try! NSRegularExpression(pattern: #"\[\[([^\]]+)\]\]"#)
    private static let tagRegex = try! NSRegularExpression(pattern: #"#([A-Za-z0-9_\-]+)"#)
    private static let propertyRegex = try! NSRegularExpression(pattern: #"([A-Za-z_][A-Za-z0-9_]*):: (.+)"#)

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

        // #tags → faint inline (pills shown on right side by OutlinerView)
        tagRegex.enumerateMatches(in: text, range: fullRange) { match, _, _ in
            guard let range = match?.range else { return }
            textStorage.addAttribute(.foregroundColor, value: NSColor.tertiaryLabelColor, range: range)
        }

        // key:: value → faint inline (pills shown below block by OutlinerView)
        propertyRegex.enumerateMatches(in: text, range: fullRange) { match, _, _ in
            guard let range = match?.range else { return }
            textStorage.addAttribute(.foregroundColor, value: NSColor.tertiaryLabelColor, range: range)
        }

        textStorage.endEditing()
    }
}
