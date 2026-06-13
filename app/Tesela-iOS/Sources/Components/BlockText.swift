import SwiftUI

/// Renders a block's body. Plain prose gets inline `[[wiki-link]]`,
/// `**bold**`, and `*italic*` styling; leading ATX headings get sized up.
/// Fenced ```` ``` ```` spans are lifted out and drawn
/// as a monospaced, themed code surface. Wiki-links are encoded as
/// tappable `tesela://page/<title>` links via `AttributedString`, so
/// callers can intercept them through
/// `.environment(\.openURL, OpenURLAction { ... })` and push the
/// linked page onto a NavigationStack.
///
/// A block with no code fence renders as a single `Text` — exactly as
/// before — so the common case keeps its lightweight layout.
struct BlockText: View {
    let text: String

    @Environment(\.theme) private var theme

    /// A run of the block: either prose (inline-parsed) or a fenced
    /// code span (rendered verbatim, no tag/wikilink parsing).
    private enum Segment {
        case prose(String)
        case code(language: String?, body: String)
    }

    var body: some View {
        let segments = parseSegments(text)
        if segments.count == 1, case .prose(let only) = segments[0] {
            // No code fence — single Text, no enclosing stack.
            Text(buildAttributed(only))
        } else {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(Array(segments.enumerated()), id: \.offset) { index, segment in
                    switch segment {
                    case .prose(let prose):
                        Text(buildAttributed(prose, allowHeading: index == 0))
                            .frame(maxWidth: .infinity, alignment: .leading)
                    case .code(let language, let codeBody):
                        codeSurface(language: language, body: codeBody)
                    }
                }
            }
        }
    }

    // MARK: - Fence parsing

    /// Split `text` into prose and fenced-code segments. The opening
    /// fence is a line beginning with ```` ``` ```` (anything after it on
    /// that line is the language token); the closing fence is a line
    /// that is exactly ```` ``` ````. An unclosed fence runs to the end
    /// of the block, matching CommonMark.
    private func parseSegments(_ text: String) -> [Segment] {
        // Fast path: no fence marker anywhere — return the text as-is so
        // a fence-free block does zero extra work and zero reflow.
        guard text.contains("```") else { return [.prose(text)] }

        var segments: [Segment] = []
        var proseLines: [String] = []
        var codeLines: [String] = []
        var codeLanguage: String?
        var inCode = false

        func flushProse() {
            guard !proseLines.isEmpty else { return }
            segments.append(.prose(proseLines.joined(separator: "\n")))
            proseLines.removeAll()
        }
        func flushCode() {
            segments.append(.code(language: codeLanguage, body: codeLines.joined(separator: "\n")))
            codeLines.removeAll()
            codeLanguage = nil
        }

        // Keep empty subsequences so blank lines inside code (and prose
        // spacing) survive the round-trip.
        let lines = text.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        for line in lines {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if inCode {
                if trimmed == "```" {
                    flushCode()
                    inCode = false
                } else {
                    codeLines.append(line)
                }
            } else if trimmed.hasPrefix("```") {
                flushProse()
                let language = trimmed.dropFirst(3).trimmingCharacters(in: .whitespaces)
                codeLanguage = language.isEmpty ? nil : language
                inCode = true
            } else {
                proseLines.append(line)
            }
        }
        if inCode {
            flushCode()
        } else {
            flushProse()
        }
        return segments
    }

    // MARK: - Code surface

    private func codeSurface(language: String?, body: String) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            if let language {
                Text(language.uppercased())
                    .font(.system(size: 9, weight: .semibold, design: .monospaced))
                    .tracking(0.8)
                    .foregroundStyle(theme.fgFaint)
            }
            Text(body)
                .font(.system(size: 13, design: .monospaced))
                .foregroundStyle(theme.fgDefault)
                .strikethrough(false)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(theme.bg3)
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(theme.lineSoft, lineWidth: 1)
        )
    }

    // MARK: - Inline prose parsing

    private struct HeadingStyle {
        let prefix: String
        let font: Font
        let boldFont: Font
        let italicFont: Font
    }

    private func headingStyle(for text: String) -> HeadingStyle? {
        if text.hasPrefix("### ") {
            return HeadingStyle(
                prefix: "### ",
                font: .system(size: 16, weight: .semibold),
                boldFont: .system(size: 16, weight: .semibold),
                italicFont: .system(size: 16, weight: .semibold).italic()
            )
        }
        if text.hasPrefix("## ") {
            return HeadingStyle(
                prefix: "## ",
                font: .system(size: 19, weight: .bold),
                boldFont: .system(size: 19, weight: .bold),
                italicFont: .system(size: 19, weight: .bold).italic()
            )
        }
        if text.hasPrefix("# ") {
            return HeadingStyle(
                prefix: "# ",
                font: .system(size: 22, weight: .bold),
                boldFont: .system(size: 22, weight: .bold),
                italicFont: .system(size: 22, weight: .bold).italic()
            )
        }
        return nil
    }

    private func buildAttributed(_ text: String, allowHeading: Bool = true) -> AttributedString {
        let heading = allowHeading ? headingStyle(for: text) : nil
        let source = heading.map { String(text.dropFirst($0.prefix.count)) } ?? text

        var attributed = AttributedString()
        let pattern = try? NSRegularExpression(pattern: #"(\[\[[^\]]+\]\]|\*\*[^*]+\*\*|\*[^*]+\*)"#)
        guard let re = pattern else {
            var fallback = AttributedString(source)
            fallback.font = heading?.font
            return fallback
        }
        let ns = source as NSString
        let matches = re.matches(in: source, range: NSRange(location: 0, length: ns.length))
        var cursor = 0

        func appendPlain(_ plain: String) {
            var span = AttributedString(plain)
            span.font = heading?.font
            attributed += span
        }

        for m in matches {
            if m.range.location > cursor {
                let plain = ns.substring(with: NSRange(location: cursor, length: m.range.location - cursor))
                appendPlain(plain)
            }
            let raw = ns.substring(with: m.range)
            if raw.hasPrefix("[[") {
                let title = String(raw.dropFirst(2).dropLast(2))
                attributed += wikiAttributed(title: title, font: heading?.font)
            } else if raw.hasPrefix("**") {
                let inner = String(raw.dropFirst(2).dropLast(2))
                var boldSpan = AttributedString(inner)
                boldSpan.font = heading?.boldFont ?? .system(size: 15, weight: .semibold)
                attributed += boldSpan
            } else {
                let inner = String(raw.dropFirst().dropLast())
                var italicSpan = AttributedString(inner)
                italicSpan.font = heading?.italicFont ?? .system(size: 15).italic()
                attributed += italicSpan
            }
            cursor = m.range.location + m.range.length
        }
        if cursor < ns.length {
            appendPlain(ns.substring(from: cursor))
        }
        return attributed
    }

    /// Build a tappable AttributedString span for a wiki-link. The
    /// link uses `tesela://page/<slug>` so callers can route via
    /// `OpenURLAction`.
    private func wikiAttributed(title: String, font: Font? = nil) -> AttributedString {
        var span = AttributedString(title)
        span.font = font
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
