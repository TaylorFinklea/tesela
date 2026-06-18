import SwiftUI

/// Pure logic for the in-editor `[[` page-link autocomplete (and, later,
/// `#` tags / `/` slash — same trigger/rank machinery). Kept free of UIKit
/// so it is unit-testable and reused across surfaces.
enum LinkSuggest {
    /// If the caret sits inside an OPEN `[[…` wikilink (no closing `]]`,
    /// newline, or stray `]` between the opener and the caret), return the
    /// UTF-16 offset of the `[[` opener and the query typed so far.
    ///
    /// Offsets are UTF-16 code units to match `text_seq` / `NSRange`.
    static func detectQuery(in text: String, caretUTF16 caret: Int) -> (start: Int, query: String)? {
        let ns = text as NSString
        let c = max(0, min(caret, ns.length))
        // Walk back from the char just left of the caret looking for "[[".
        // Bail on a newline or a `]` first — the caret isn't in an open link.
        var i = c - 1
        while i >= 1 {
            let ch = ns.character(at: i)
            if ch == 0x0A { return nil }        // newline
            if ch == 0x5D { return nil }        // ']' — a closed/!open link
            let prev = ns.character(at: i - 1)
            if prev == 0x5B && ch == 0x5B {     // "[["
                let start = i - 1
                let qRange = NSRange(location: start + 2, length: c - (start + 2))
                return (start, ns.substring(with: qRange))
            }
            i -= 1
        }
        return nil
    }

    /// Rank pages for `query` over title + slug. Returns the best `limit`.
    static func rank(_ pages: [Page], query: String, limit: Int) -> [Page] {
        let q = query.lowercased()
        var scored: [(page: Page, score: Int)] = []
        for page in pages {
            let s = max(score(page.title.lowercased(), q), score(page.slug.lowercased(), q))
            if s > 0 { scored.append((page: page, score: s)) }
        }
        scored.sort { a, b in
            if a.score != b.score { return a.score > b.score }
            return a.page.title.count < b.page.title.count
        }
        return scored.prefix(limit).map { $0.page }
    }

    /// Crude relevance score: exact > prefix > word-start substring >
    /// substring > subsequence > 0 (no match). Enough for a page picker.
    static func score(_ haystack: String, _ needle: String) -> Int {
        if needle.isEmpty { return 1 }
        if haystack == needle { return 1000 }
        if haystack.hasPrefix(needle) { return 800 }
        if let r = haystack.range(of: needle) {
            let idx = haystack.distance(from: haystack.startIndex, to: r.lowerBound)
            let wordStart = idx == 0 || haystack[haystack.index(haystack.startIndex, offsetBy: idx - 1)] == " "
            return (wordStart ? 500 : 300) - min(idx, 200)
        }
        // Subsequence: every needle char appears in order.
        var hi = haystack.startIndex
        for ch in needle {
            guard let found = haystack[hi...].firstIndex(of: ch) else { return 0 }
            hi = haystack.index(after: found)
        }
        return 50
    }
}

/// Drives the `[[` suggestions strip shown in the keyboard accessory.
/// Owned by `BlockRow`; the editor's coordinator updates it as the user
/// types, and the accessory renders `results` when `isActive`.
@MainActor
final class LinkAutocomplete: ObservableObject {
    @Published var isActive = false
    @Published private(set) var results: [Page] = []
    /// The text typed after `[[` so far. Surfaced so the accessory can offer
    /// a "create [[query]]" row for a page that doesn't exist yet.
    @Published private(set) var query = ""

    /// UTF-16 offset of the `[[` opener in the live block text — the start
    /// of the `[[query` span the chosen link replaces.
    private(set) var startOffset = 0

    /// Supplies candidates for a query. Wired by the owner from the
    /// service's `searchablePages`.
    var search: ((String) -> [Page])?

    func update(start: Int, query: String) {
        startOffset = start
        self.query = query
        results = search?(query) ?? []
        // Active when we have matches OR a non-empty query (so the
        // create-new row can offer to make a brand-new page link).
        isActive = !results.isEmpty || !query.isEmpty
    }

    func dismiss() {
        guard isActive else { return }
        isActive = false
        results = []
        query = ""
    }
}
