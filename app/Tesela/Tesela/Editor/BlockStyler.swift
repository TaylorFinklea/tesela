import AppKit
import Foundation

// MARK: - BlockStyler
// Applies NSAttributedString styling for [[wiki-links]], #tags, and properties

enum BlockStyler {
    private static let wikiLinkRegex = try! NSRegularExpression(pattern: #"\[\[([^\]]+)\]\]"#)
    private static let tagRegex = try! NSRegularExpression(pattern: #"#([A-Za-z0-9_\-]+)"#)
    private static let propertyRegex = try! NSRegularExpression(pattern: #"([A-Za-z_][A-Za-z0-9_]*):: (.+)"#)
    private static let todoPrefix = try! NSRegularExpression(pattern: #"^(TODO|DOING|DONE) "#)

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

        // key:: value properties → styled visibly (secondary color, smaller font)
        propertyRegex.enumerateMatches(in: text, range: fullRange) { match, _, _ in
            guard let match else { return }
            let keyRange = match.range(at: 1)
            textStorage.addAttribute(.foregroundColor, value: NSColor.systemPurple, range: keyRange)
            textStorage.addAttribute(.font, value: NSFont.boldSystemFont(ofSize: NSFont.systemFontSize - 1), range: keyRange)
            let valueRange = match.range(at: 2)
            textStorage.addAttribute(.foregroundColor, value: NSColor.secondaryLabelColor, range: valueRange)
            textStorage.addAttribute(.font, value: NSFont.systemFont(ofSize: NSFont.systemFontSize - 1), range: valueRange)
            // Style the `:: ` separator
            let sepStart = keyRange.location + keyRange.length
            let sepLen = valueRange.location - sepStart
            if sepLen > 0 {
                let sepRange = NSRange(location: sepStart, length: sepLen)
                textStorage.addAttribute(.foregroundColor, value: NSColor.tertiaryLabelColor, range: sepRange)
                textStorage.addAttribute(.font, value: NSFont.systemFont(ofSize: NSFont.systemFontSize - 1), range: sepRange)
            }
        }

        // TODO/DOING/DONE prefix → hidden (icon shown in bullet area by OutlinerView)
        todoPrefix.enumerateMatches(in: text, range: fullRange) { match, _, _ in
            guard let range = match?.range else { return }
            textStorage.addAttribute(.foregroundColor, value: NSColor.clear, range: range)
            textStorage.addAttribute(.font, value: NSFont.systemFont(ofSize: 1), range: range)
        }

        textStorage.endEditing()
    }
}
