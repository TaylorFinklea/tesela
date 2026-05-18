import SwiftUI

/// Renders inline block text with `[[wiki-link]]` and `**bold**`
/// styling. Wiki-links are encoded as tappable `tesela://page/<title>`
/// links via `AttributedString`, so callers can intercept them through
/// `.environment(\.openURL, OpenURLAction { ... })` and push the
/// linked page onto a NavigationStack.
struct BlockText: View {
    let text: String

    @Environment(\.theme) private var theme

    var body: some View {
        Text(buildAttributed())
    }

    private func buildAttributed() -> AttributedString {
        var attributed = AttributedString()
        let pattern = try? NSRegularExpression(pattern: #"(\[\[[^\]]+\]\]|\*\*[^*]+\*\*)"#)
        guard let re = pattern else {
            return AttributedString(text)
        }
        let ns = text as NSString
        let matches = re.matches(in: text, range: NSRange(location: 0, length: ns.length))
        var cursor = 0
        for m in matches {
            if m.range.location > cursor {
                let plain = ns.substring(with: NSRange(location: cursor, length: m.range.location - cursor))
                attributed += AttributedString(plain)
            }
            let raw = ns.substring(with: m.range)
            if raw.hasPrefix("[[") {
                let title = String(raw.dropFirst(2).dropLast(2))
                attributed += wikiAttributed(title: title)
            } else {
                let inner = String(raw.dropFirst(2).dropLast(2))
                var boldSpan = AttributedString(inner)
                boldSpan.font = .system(size: 15, weight: .semibold)
                attributed += boldSpan
            }
            cursor = m.range.location + m.range.length
        }
        if cursor < ns.length {
            attributed += AttributedString(ns.substring(from: cursor))
        }
        return attributed
    }

    /// Build a tappable AttributedString span for a wiki-link. The
    /// link uses `tesela://page/<slug>` so callers can route via
    /// `OpenURLAction`.
    private func wikiAttributed(title: String) -> AttributedString {
        var span = AttributedString(title)
        span.foregroundColor = theme.accentPrimary
        span.underlineStyle = .single
        let slug = title
            .lowercased()
            .replacingOccurrences(of: " ", with: "-")
            .addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? title
        if let url = URL(string: "tesela://page/\(slug)") {
            span.link = url
        }
        return span
    }
}

/// Helper for callers that want to handle `tesela://page/<slug>` taps.
/// Returns the page slug if the URL matches; otherwise nil. Used in
/// the `OpenURLAction` closure.
enum TeselaLink {
    static func pageSlug(from url: URL) -> String? {
        guard url.scheme == "tesela", url.host == "page" else { return nil }
        let path = url.pathComponents.dropFirst() // drop leading "/"
        guard let last = path.last else { return nil }
        return last.removingPercentEncoding ?? last
    }
}
