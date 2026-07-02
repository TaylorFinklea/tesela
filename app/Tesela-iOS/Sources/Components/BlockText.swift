import SwiftUI

/// Renders a block's body. Plain prose gets inline `[[wiki-link]]`,
/// `**bold**`, `*italic*`, `` `code` ``, `~~strike~~`, and `[text](url)`
/// link styling (see `parseInlineSpans` — the shared rendering-contract
/// fixture at crates/tesela-core/tests/fixtures/inline-span-conformance.json);
/// leading ATX headings get sized up. Fenced ```` ``` ```` spans are lifted
/// out and drawn as a monospaced, themed code surface. Wiki-links are
/// encoded as tappable `tesela://page/<title>` links via `AttributedString`,
/// so callers can intercept them through
/// `.environment(\.openURL, OpenURLAction { ... })` and push the linked page
/// onto a NavigationStack. Markdown links carry their real URL and fall
/// through to `.systemAction` (opens in the default browser) at every
/// existing `OpenURLAction` call site.
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

    // MARK: - Inline-span rendering contract (tesela-pfix.6)
    //
    // The shared fixture is crates/tesela-core/tests/fixtures/inline-span-conformance.json
    // (consumed here and by web/src/lib/block-parser.ts's `parseInlineSpans`).
    // See the fixture's `_contract` header for the full scope/precedence
    // rules. Flat, non-nesting, single-line-prose only — a span's inner text
    // is never re-scanned for other markers.

    /// One flat inline span. `href` is NOT part of the shared fixture
    /// contract (display-text-only, rendering-only) — it's populated for
    /// `.link` spans so `buildAttributed` can make markdown links tappable.
    enum InlineSpanKind: String, Equatable {
        case plain, bold, italic, code, strike, link, wikilink
    }
    struct InlineSpan: Equatable {
        let kind: InlineSpanKind
        let text: String
        var href: String?

        init(kind: InlineSpanKind, text: String, href: String? = nil) {
            self.kind = kind
            self.text = text
            self.href = href
        }
    }

    /// Parse single-line prose into the flat, ordered inline-span list this
    /// view renders — the REAL production parser `buildAttributed` styles
    /// with. Precedence: code > bold > italic > strike > wikilink > link
    /// (mirrors the shared fixture and web `parseInlineSpans`).
    static func parseInlineSpans(_ source: String) -> [InlineSpan] {
        let pattern = #"`([^`\n]+?)`|\*\*([^*\n]+?)\*\*|__([^_\n]+?)__|\*([^*\n]+?)\*|~~([^~\n]+?)~~|\[\[([^\]\n]+?)\]\]|\[([^\]\n]+?)\]\(([^)\n]+?)\)"#
        guard let re = try? NSRegularExpression(pattern: pattern) else {
            return source.isEmpty ? [] : [InlineSpan(kind: .plain, text: source)]
        }
        let ns = source as NSString
        let matches = re.matches(in: source, range: NSRange(location: 0, length: ns.length))
        var spans: [InlineSpan] = []
        var cursor = 0

        func group(_ m: NSTextCheckingResult, _ index: Int) -> String? {
            let r = m.range(at: index)
            guard r.location != NSNotFound else { return nil }
            return ns.substring(with: r)
        }

        for m in matches {
            if m.range.location > cursor {
                spans.append(InlineSpan(
                    kind: .plain,
                    text: ns.substring(with: NSRange(location: cursor, length: m.range.location - cursor))
                ))
            }
            if let code = group(m, 1) {
                spans.append(InlineSpan(kind: .code, text: code))
            } else if let bold = group(m, 2) ?? group(m, 3) {
                spans.append(InlineSpan(kind: .bold, text: bold))
            } else if let italic = group(m, 4) {
                spans.append(InlineSpan(kind: .italic, text: italic))
            } else if let strike = group(m, 5) {
                spans.append(InlineSpan(kind: .strike, text: strike))
            } else if let wiki = group(m, 6) {
                spans.append(InlineSpan(kind: .wikilink, text: wiki))
            } else if let linkText = group(m, 7) {
                spans.append(InlineSpan(kind: .link, text: linkText, href: group(m, 8)))
            }
            cursor = m.range.location + m.range.length
        }
        if cursor < ns.length {
            spans.append(InlineSpan(kind: .plain, text: ns.substring(from: cursor)))
        }
        return spans
    }

    private func buildAttributed(_ text: String, allowHeading: Bool = true) -> AttributedString {
        let heading = allowHeading ? headingStyle(for: text) : nil
        let source = heading.map { String(text.dropFirst($0.prefix.count)) } ?? text

        var attributed = AttributedString()

        func appendPlain(_ plain: String) {
            guard !plain.isEmpty else { return }
            var span = AttributedString(plain)
            span.font = heading?.font
            attributed += span
        }

        for span in Self.parseInlineSpans(source) {
            switch span.kind {
            case .plain:
                appendPlain(span.text)
            case .wikilink:
                attributed += wikiAttributed(title: span.text, font: heading?.font)
            case .bold:
                var boldSpan = AttributedString(span.text)
                boldSpan.font = heading?.boldFont ?? .system(size: 15, weight: .semibold)
                attributed += boldSpan
            case .italic:
                var italicSpan = AttributedString(span.text)
                italicSpan.font = heading?.italicFont ?? .system(size: 15).italic()
                attributed += italicSpan
            case .code:
                var codeSpan = AttributedString(span.text)
                codeSpan.font = .system(size: 13, design: .monospaced)
                codeSpan.foregroundColor = theme.fgDefault
                attributed += codeSpan
            case .strike:
                var strikeSpan = AttributedString(span.text)
                strikeSpan.font = heading?.font
                strikeSpan.foregroundColor = theme.fgFaint
                strikeSpan.strikethroughStyle = .single
                attributed += strikeSpan
            case .link:
                var linkSpan = AttributedString(span.text)
                linkSpan.font = heading?.font
                linkSpan.foregroundColor = theme.accentPrimary
                linkSpan.underlineStyle = .single
                if let href = span.href, let url = URL(string: href) {
                    linkSpan.link = url
                }
                attributed += linkSpan
            }
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
