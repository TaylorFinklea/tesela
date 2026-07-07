import Foundation

/// JQL syntax-highlight span detector for `query::` lines (tesela-vp9.6) —
/// a thin adapter over `LocalQueryEngine.tokenizeDsl` (tesela-vp9.4) +
/// `QueryAuthoring.buildPreviewSpans` (tesela-vp9.5, itself built on
/// `QueryAuthoring.classifyTokens`), so "what colors" in the block
/// editor's `query::` lines never drifts from "what the saved-view
/// editor's token-preview row colors" or "what the parser actually
/// reads" — zero second classifier. Pure Foundation (no UIKit/SwiftUI),
/// like `QueryAuthoring` itself, so it stays unit-testable without a
/// live `UITextView`.
///
/// A "line" here is a `\n`-delimited run within a BLOCK's own text (a
/// single block can carry multiple lines — e.g. the `/query` slash verb
/// inserts `"\nquery:: type = \nview:: table"` into one block, see
/// `EditorAutocomplete.SlashVerbs.manifestVerbs`). A line is a `query::`
/// line when, after skipping leading spaces/tabs, it starts with
/// `"query::"` case-insensitively — mirrors the existing property-line
/// convention (`GrPageView.queryInfo`'s `text.lowercased().hasPrefix
/// ("query::")`, `MockMosaicService.fetchInboxDsl`'s line scan).
///
/// `LocalQueryEngine.tokenizeDsl`/`QueryAuthoring.buildPreviewSpans` both
/// work in UTF-8 BYTE offsets (matching the Rust-mirrored tokenizer);
/// `NSTextStorage`/`NSRange` (what the shared `InlineNLPHighlighter`
/// painter mechanics consume) are UTF-16. `detectSpans` converts every
/// span through a per-line byte→UTF-16 offset table so a multibyte
/// character anywhere in a `query::` line's quoted values (`"café"`)
/// never desyncs a later token's highlight range.
enum JQLLineHighlight {

    /// One JQL syntax span: a UTF-16 `NSRange` into the FULL text (ready
    /// for `NSTextStorage.addAttribute`) plus the syntax kind that
    /// governs its color — reuses `QueryAuthoring.PreviewTokenKind`
    /// (key/operator/value/string/number/paren) directly rather than
    /// inventing a parallel vocabulary.
    struct HighlightSpan: Equatable {
        let range: NSRange
        let kind: QueryAuthoring.PreviewTokenKind
    }

    private static let prefix = "query::"
    private static let prefixUTF16Length = prefix.utf16.count  // 7, all-ASCII

    /// The UTF-16 `NSRange` of every WHOLE LINE (leading indent through
    /// the line's end, excluding the trailing `\n`) in `text` that is a
    /// `query::` line. Used by the block editor to SUPPRESS NLP-lift
    /// detection on these lines — a `query::` line gets JQL coloring
    /// INSTEAD of NLP coloring, so a date word inside a quoted query
    /// value (`query:: title LIKE "tomorrow"`) never reads as an NLP
    /// date token.
    static func queryLineRanges(in text: String) -> [NSRange] {
        var ranges: [NSRange] = []
        forEachLine(in: text as NSString) { ns, lineRange in
            if isQueryLine(ns, lineRange) {
                ranges.append(lineRange)
            }
        }
        return ranges
    }

    /// JQL syntax highlight spans for every `query::` line in `text`, in
    /// the FULL text's UTF-16 `NSRange` space. Non-`query::` lines and
    /// whitespace gaps between tokens contribute nothing (mirrors
    /// `QueryAuthoring.buildPreviewSpans`'s `kind == nil` gap spans,
    /// which are dropped here rather than painted).
    static func detectSpans(in text: String) -> [HighlightSpan] {
        var spans: [HighlightSpan] = []
        let ns = text as NSString
        forEachLine(in: ns) { ns, lineRange in
            guard isQueryLine(ns, lineRange) else { return }
            let lineEnd = lineRange.location + lineRange.length
            let contentStart = firstNonSpace(ns, lineRange)
            let dslStart = contentStart + prefixUTF16Length
            guard dslStart <= lineEnd else { return }
            let dslText = ns.substring(with: NSRange(location: dslStart, length: lineEnd - dslStart))
            let table = utf16OffsetTable(for: dslText)
            for p in QueryAuthoring.buildPreviewSpans(dslText) {
                guard let kind = p.kind, p.start < p.end else { continue }
                let u16Start = table[p.start]
                let u16End = table[p.end]
                guard u16End > u16Start else { continue }
                let range = NSRange(location: dslStart + u16Start, length: u16End - u16Start)
                spans.append(HighlightSpan(range: range, kind: kind))
            }
        }
        return spans
    }

    // MARK: - Line scanning

    /// Walk `ns` line by line (split on `\n`, trailing newline excluded
    /// from each `lineRange`), invoking `body` with each line's UTF-16
    /// `NSRange`.
    private static func forEachLine(in ns: NSString, _ body: (NSString, NSRange) -> Void) {
        let length = ns.length
        var lineStart = 0
        var i = 0
        while i <= length {
            if i == length || ns.character(at: i) == 0x0A {
                body(ns, NSRange(location: lineStart, length: i - lineStart))
                lineStart = i + 1
            }
            i += 1
        }
    }

    /// The UTF-16 offset of the first non-space/tab character in
    /// `lineRange`, or the line's end when it's all whitespace.
    private static func firstNonSpace(_ ns: NSString, _ lineRange: NSRange) -> Int {
        let end = lineRange.location + lineRange.length
        var i = lineRange.location
        while i < end {
            let ch = ns.character(at: i)
            guard ch == 0x20 || ch == 0x09 else { break }
            i += 1
        }
        return i
    }

    /// Whether `lineRange` (after skipping leading spaces/tabs) starts
    /// with `"query::"`, case-insensitively.
    private static func isQueryLine(_ ns: NSString, _ lineRange: NSRange) -> Bool {
        let end = lineRange.location + lineRange.length
        let contentStart = firstNonSpace(ns, lineRange)
        guard end - contentStart >= prefixUTF16Length else { return false }
        let candidate = ns.substring(with: NSRange(location: contentStart, length: prefixUTF16Length))
        return candidate.lowercased() == prefix
    }

    /// Byte-offset → UTF-16-offset lookup for `s`: `table[i]` is the
    /// UTF-16 offset of the UTF-8 byte at index `i` (meaningful only at
    /// Unicode scalar boundaries — every span `tokenizeDsl` produces
    /// starts/ends on one, since its tokenizer only ever splits on
    /// single-byte ASCII structural characters). `table.count ==
    /// s.utf8.count + 1`, so a span `[start, end)` in `tokenizeDsl`'s
    /// byte space converts via `table[start]..<table[end]` in UTF-16
    /// space.
    private static func utf16OffsetTable(for s: String) -> [Int] {
        var table = [Int](repeating: 0, count: s.utf8.count + 1)
        var byteIdx = 0
        var utf16Idx = 0
        for scalar in s.unicodeScalars {
            let piece = String(scalar)
            let byteLen = piece.utf8.count
            let utf16Len = piece.utf16.count
            for k in 0..<byteLen { table[byteIdx + k] = utf16Idx }
            byteIdx += byteLen
            utf16Idx += utf16Len
        }
        table[byteIdx] = utf16Idx
        return table
    }
}
