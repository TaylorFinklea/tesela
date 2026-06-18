import SwiftUI

/// Which inline trigger is open in the editor. `[[` links, `#` tags, and
/// `/` slash-verbs all share one detection + suggestion strip; only the
/// candidate source and the inserted text differ per kind.
enum TriggerKind: Equatable { case link, tag, slash }

/// One suggestion chip in the keyboard strip. `insert` is spliced in place
/// of the typed `trigger+query` span when the chip is tapped.
struct Suggestion: Identifiable, Equatable {
    let id: String
    let label: String
    let insert: String
    var isCreateNew: Bool = false
}

/// Pure logic for the inline autocomplete (link / tag / slash). UIKit-free
/// so it stays unit-testable and reused across surfaces.
enum LinkSuggest {
    /// If the caret sits inside an OPEN `[[…` wikilink (no closing `]]`,
    /// newline, or stray `]` between the opener and the caret), return the
    /// UTF-16 offset of the `[[` opener and the query typed so far.
    /// Offsets are UTF-16 code units to match `text_seq` / `NSRange`.
    static func detectQuery(in text: String, caretUTF16 caret: Int) -> (start: Int, query: String)? {
        let ns = text as NSString
        let c = max(0, min(caret, ns.length))
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

    /// Detect any open trigger at the caret: `[[` link (may contain spaces;
    /// bounded by `]]`/newline) — checked first — else a single
    /// whitespace-delimited token starting with `#` (tag) or `/` (slash) at
    /// line-start or after whitespace.
    static func detectTrigger(in text: String, caretUTF16 caret: Int) -> (kind: TriggerKind, start: Int, query: String)? {
        if let link = detectQuery(in: text, caretUTF16: caret) {
            return (.link, link.start, link.query)
        }
        let ns = text as NSString
        let c = max(0, min(caret, ns.length))
        // Walk back over the current non-whitespace token.
        var wordStart = c
        while wordStart > 0 {
            let ch = ns.character(at: wordStart - 1)
            if ch == 0x20 || ch == 0x0A || ch == 0x09 { break }  // space / newline / tab
            wordStart -= 1
        }
        guard wordStart < c else { return nil }
        // The trigger char must START the token and sit at line-start or
        // after whitespace (so "C#" / "http://x" / "a/b" don't trigger).
        let okBefore = wordStart == 0 || {
            let b = ns.character(at: wordStart - 1)
            return b == 0x20 || b == 0x0A || b == 0x09
        }()
        guard okBefore else { return nil }
        let first = ns.character(at: wordStart)
        let query = ns.substring(with: NSRange(location: wordStart + 1, length: c - wordStart - 1))
        if first == 0x23 { return (.tag, wordStart, query) }    // '#'
        if first == 0x2F { return (.slash, wordStart, query) }  // '/'
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

    /// Rank plain strings (tag names) for `query`. Returns the best `limit`.
    static func rankStrings(_ items: [String], query: String, limit: Int) -> [String] {
        let q = query.lowercased()
        var scored: [(item: String, score: Int)] = []
        for item in items {
            let s = score(item.lowercased(), q)
            if s > 0 { scored.append((item: item, score: s)) }
        }
        scored.sort { a, b in
            if a.score != b.score { return a.score > b.score }
            return a.item.count < b.item.count
        }
        return scored.prefix(limit).map { $0.item }
    }

    /// Crude relevance score: exact > prefix > word-start substring >
    /// substring > subsequence > 0 (no match).
    static func score(_ haystack: String, _ needle: String) -> Int {
        if needle.isEmpty { return 1 }
        if haystack == needle { return 1000 }
        if haystack.hasPrefix(needle) { return 800 }
        if let r = haystack.range(of: needle) {
            let idx = haystack.distance(from: haystack.startIndex, to: r.lowerBound)
            let wordStart = idx == 0 || haystack[haystack.index(haystack.startIndex, offsetBy: idx - 1)] == " "
            return (wordStart ? 500 : 300) - min(idx, 200)
        }
        var hi = haystack.startIndex
        for ch in needle {
            guard let found = haystack[hi...].firstIndex(of: ch) else { return 0 }
            hi = haystack.index(after: found)
        }
        return 50
    }
}

/// Drives the inline suggestion strip in the keyboard accessory. Owned by
/// `BlockRow`; the editor's coordinator updates it as the user types, and
/// the accessory renders `results` when `isActive`.
@MainActor
final class EditorAutocomplete: ObservableObject {
    /// The open trigger, or nil when inactive.
    @Published private(set) var kind: TriggerKind? = nil
    @Published private(set) var results: [Suggestion] = []
    /// The text typed after the trigger so far.
    @Published private(set) var query = ""

    /// UTF-16 offset of the trigger opener in the live block text — the
    /// start of the `trigger+query` span a chosen suggestion replaces.
    private(set) var startOffset = 0

    /// Produces suggestions for a (kind, query). Wired by the owner.
    var provider: ((TriggerKind, String) -> [Suggestion])?

    var isActive: Bool { kind != nil && !results.isEmpty }

    func update(kind: TriggerKind, start: Int, query: String) {
        self.kind = kind
        self.startOffset = start
        self.query = query
        self.results = provider?(kind, query) ?? []
    }

    func dismiss() {
        guard kind != nil else { return }
        kind = nil
        results = []
        query = ""
    }
}

/// The built-in `/` slash verbs — text-insert / opener verbs (actions like
/// indent/status stay on the toolbar). `link`/`tag` insert just the opener
/// so the respective autocomplete chains open.
enum SlashVerbs {
    static func matching(_ query: String) -> [Suggestion] {
        let items = base + [todayDate()]
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        guard !q.isEmpty else { return items }
        return items.filter { $0.label.lowercased().contains(q) || $0.id.lowercased().contains(q) }
    }

    private static let base: [Suggestion] = [
        Suggestion(id: "slash:link", label: "Link [[…]]", insert: "[["),
        Suggestion(id: "slash:tag", label: "Tag #…", insert: "#"),
        Suggestion(id: "slash:h1", label: "Heading", insert: "# "),
        Suggestion(id: "slash:h2", label: "Subheading", insert: "## "),
        Suggestion(id: "slash:quote", label: "Quote", insert: "> "),
        Suggestion(id: "slash:divider", label: "Divider", insert: "---"),
    ]

    private static func todayDate() -> Suggestion {
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd"
        f.locale = Locale(identifier: "en_US_POSIX")
        let today = f.string(from: Date())
        return Suggestion(id: "slash:date", label: "Today's date", insert: "[[\(today)]]")
    }
}
