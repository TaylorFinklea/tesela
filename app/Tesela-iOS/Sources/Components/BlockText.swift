import SwiftUI

/// Renders inline block text with `[[wiki-link]]` and `**bold**` styling.
/// Same parser shape as the web's `BlockText` helper in `core.jsx`.
///
/// SwiftUI's `AttributedString` would in theory do this, but custom
/// background tints on inline spans require the lower-level approach we
/// take here. We assemble a stream of `Text` views and concatenate them
/// with `+`, which preserves wrapping behavior.
struct BlockText: View {
    let text: String

    @Environment(\.theme) private var theme

    private struct Token {
        enum Kind { case plain, wiki, bold }
        let kind: Kind
        let body: String
    }

    private var tokens: [Token] {
        let pattern = try? NSRegularExpression(pattern: #"(\[\[[^\]]+\]\]|\*\*[^*]+\*\*)"#)
        guard let re = pattern else { return [Token(kind: .plain, body: text)] }
        let ns = text as NSString
        let matches = re.matches(in: text, range: NSRange(location: 0, length: ns.length))
        var out: [Token] = []
        var cursor = 0
        for m in matches {
            if m.range.location > cursor {
                let plain = ns.substring(with: NSRange(location: cursor, length: m.range.location - cursor))
                out.append(Token(kind: .plain, body: plain))
            }
            let raw = ns.substring(with: m.range)
            if raw.hasPrefix("[[") {
                let inner = String(raw.dropFirst(2).dropLast(2))
                out.append(Token(kind: .wiki, body: inner))
            } else {
                let inner = String(raw.dropFirst(2).dropLast(2))
                out.append(Token(kind: .bold, body: inner))
            }
            cursor = m.range.location + m.range.length
        }
        if cursor < ns.length {
            out.append(Token(kind: .plain, body: ns.substring(from: cursor)))
        }
        return out
    }

    var body: some View {
        // Concatenate Text views. The `+` operator preserves wrapping.
        tokens.reduce(Text("")) { acc, tok in
            acc + textFor(tok)
        }
    }

    private func textFor(_ tok: Token) -> Text {
        switch tok.kind {
        case .plain:
            return Text(tok.body)
        case .wiki:
            // Inline wiki-link: primary-colored text with a soft accent
            // background tint. Inline `Text` can't carry a background
            // shape, so the visual treatment lives in the foreground
            // color only — full pill rendering is the `WikiLink` view
            // used in standalone contexts.
            return Text(tok.body)
                .foregroundColor(theme.accentPrimary)
                .underline()
        case .bold:
            return Text(tok.body).bold()
        }
    }
}
