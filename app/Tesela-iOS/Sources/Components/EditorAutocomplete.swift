import SwiftUI

/// Which inline trigger is open in the editor. `[[` links, `#` tags, and
/// `/` slash-verbs all share one detection + suggestion strip; only the
/// candidate source and the inserted text differ per kind.
enum TriggerKind: Equatable { case link, tag, slash, nlp }

/// What tapping a suggestion chip DOES. Most chips still splice text
/// (`.insertText`); registry-driven property verbs + inline-NLP lifts route
/// to the STRUCTURED converging seam instead (`.setProperty`/`.setStatus`) or
/// open the date sheet (`.openDateSheet`). The single dispatch point
/// (`BlockRow.commitSuggestion`) branches on this.
enum SuggestionAction: Equatable {
    /// Splice `Suggestion.insert` at the trigger span (today's behavior).
    case insertText
    /// Write a structured property (key, value) via the typed per-key seam,
    /// then remove the matched trigger text. NEVER spliced into `text_seq`.
    case setProperty(key: String, value: String)
    /// Same as `.setProperty` but specifically the status key — kept distinct
    /// so a `/todo`/`/doing` verb reads as a status set at the call site.
    case setStatus(choice: String)
    /// Open the date sheet preset to `field` (scheduled/deadline). The matched
    /// trigger text is removed and the sheet takes over.
    case openDateSheet(field: DateField)
}

/// One suggestion chip in the keyboard strip. For `.insertText` actions
/// `insert` is spliced in place of the typed `trigger+query` span when the
/// chip is tapped; for the structured actions `insert` is ignored and the
/// `action` drives a typed property write / date sheet instead.
struct Suggestion: Identifiable, Equatable {
    let id: String
    let label: String
    let insert: String
    var isCreateNew: Bool = false
    /// What the chip does when tapped. Defaults to the legacy splice so every
    /// existing call site (link/tag/format verbs) keeps inserting text.
    var action: SuggestionAction = .insertText
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
    /// start of the `trigger+query` span a chosen suggestion replaces. For the
    /// `.nlp` kind this is the start of the matched trigger text.
    private(set) var startOffset = 0

    /// Produces suggestions for a (kind, query). Wired by the owner.
    var provider: ((TriggerKind, String) -> [Suggestion])?

    /// Inline-NLP detector (P5.5): given the live text + caret, returns a lift
    /// candidate or nil. Wired by the owner (captures the block's tags +
    /// registry). The coordinator calls it when no `[[`/`#`/`/` trigger is open.
    var nlpDetector: ((_ text: String, _ caretUTF16: Int) -> InlineNLP.Hit?)?

    var isActive: Bool { kind != nil && !results.isEmpty }

    func update(kind: TriggerKind, start: Int, query: String) {
        self.kind = kind
        self.startOffset = start
        self.query = query
        self.results = provider?(kind, query) ?? []
    }

    /// Surface an inline-NLP lift (P5.5) in the same strip. Carries the exact
    /// span (`start`/`length`) to remove on apply so the dispatch removes the
    /// matched prose token rather than `start…caret`.
    func updateNLP(_ hit: InlineNLP.Hit) {
        self.kind = .nlp
        self.startOffset = hit.start
        self.query = ""
        self.results = [hit.suggestion]
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
///
/// Base-verb-set unification (tesela-cmdd.5): `manifestVerbs` ids trace 1:1
/// to `web/src/lib/command-manifest.json`'s `editor` category entries whose
/// `surfaces` include `slash` — the SAME set web's `BlockEditor.getSlashTree`
/// builds from `commandRegistry.availableOn('slash', …)` (mirrors
/// `slash-tree.ts`'s doc comment: "the 8 insertion verbs"). Two manifest
/// entries are structurally excluded, matching web exactly:
/// `editor.property` (invoked from the `/p` submenu leaf, never a top-level
/// verb — no `slashKey` on web either) and `editor.widget` (leader-only,
/// dropped from slash). `editor.template` is an EXPLICIT opt-out
/// (`ManifestOptOuts.noHandlerYet`): iOS has no template-picker UI, so it's
/// deliberately absent rather than silently missing — see
/// `ManifestSlashVerbsTests`, which asserts every OTHER manifest slash verb
/// has a corresponding id here. `platformOnlyVerbs` are the intentional
/// non-manifest additions (Subheading/Quote/Divider have no web command at
/// all) — kept separate so they read as a deliberate platform capability,
/// not scope creep into the traced set.
enum SlashVerbs {
    /// Manifest ids with no MCP/iOS handler yet — tracked here (not just a
    /// comment) so `ManifestSlashVerbsTests` can assert the gap is
    /// intentional rather than a forgotten command.
    enum ManifestOptOuts {
        static let noHandlerYet: Set<String> = ["editor.template"]
    }

    static func matching(_ query: String) -> [Suggestion] {
        let items = base
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        guard !q.isEmpty else { return items }
        return items.filter { $0.label.lowercased().contains(q) || $0.id.lowercased().contains(q) }
    }

    /// `todayDate()` is recomputed per call (embeds today's date), so `base`
    /// is a computed property rather than a static array.
    static var base: [Suggestion] { manifestVerbs + [todayDate()] + platformOnlyVerbs }

    /// Manifest-traced verbs — id == the `web/src/lib/command-manifest.json`
    /// entry it corresponds to.
    private static let manifestVerbs: [Suggestion] = [
        Suggestion(id: "editor.link", label: "Link [[…]]", insert: "[["),
        Suggestion(id: "editor.tag", label: "Tag #…", insert: "#"),
        Suggestion(id: "editor.heading", label: "Heading", insert: "# "),
        Suggestion(id: "editor.task", label: "Task", insert: "tags:: Task"),
        Suggestion(id: "editor.collection", label: "Collection", insert: "\ncollection:: []\nview:: cards"),
        Suggestion(id: "editor.query", label: "Query", insert: "\nquery:: type = \nview:: table"),
    ]

    /// Intentional non-manifest additions — no web command exists for these
    /// (tesela-cmdd.5's "explicit platform flag", not a forgotten edit).
    private static let platformOnlyVerbs: [Suggestion] = [
        Suggestion(id: "ios.subheading", label: "Subheading", insert: "## "),
        Suggestion(id: "ios.quote", label: "Quote", insert: "> "),
        Suggestion(id: "ios.divider", label: "Divider", insert: "---"),
    ]

    /// Also manifest-traced (`editor.date`); execution differs deliberately
    /// from web (which opens a date picker) — iOS inserts a `[[today]]` link,
    /// an existing, working, lower-friction affordance for the same
    /// "insert today" intent. The id ties it to the manifest for
    /// traceability even though the mechanism diverges.
    private static func todayDate() -> Suggestion {
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd"
        f.locale = Locale(identifier: "en_US_POSIX")
        let today = f.string(from: Date())
        return Suggestion(id: "editor.date", label: "Today's date", insert: "[[\(today)]]")
    }

    /// Registry-derived slash verbs for the block being edited (P5.4). Each
    /// `select`/`multi-select` property contributes a `/<choice>` verb (e.g.
    /// `/p1`, `/todo`) AND a `/<prop> <choice>` verb (e.g. `/status doing`),
    /// both routing to the STRUCTURED seam (`.setStatus` for status,
    /// `.setProperty` otherwise — NOT text). Each `date` property contributes a
    /// `/<prop>` verb (`/scheduled`, `/deadline`) that opens the date sheet
    /// preset to its field. The format verbs (`/h1`, `/link`, …) are kept
    /// separately by the caller.
    ///
    /// Verbs are derived from the resolved defs across `tags` (first def per
    /// lowercased name wins). Choices come from the resolved def — so a Task's
    /// `Status` yields `[todo, doing, done, blocked]`, a Project's
    /// `[planned, active, shipped]`.
    static func registryVerbs(tags: [String], registry: PropertyRegistry) -> [Suggestion] {
        var seen = Set<String>()
        var out: [Suggestion] = []
        for tag in tags {
            let clean = tag.hasPrefix("#") ? String(tag.dropFirst()) : tag
            for def in registry.resolvedDefs(forTag: clean) {
                let key = def.name.lowercased()
                if seen.contains(key) { continue }
                seen.insert(key)
                switch def.valueType {
                case .select, .multiSelect:
                    let isStatus = key == "status"
                    for choice in def.choices {
                        let action: SuggestionAction = isStatus
                            ? .setStatus(choice: choice)
                            : .setProperty(key: key, value: choice)
                        // /<choice> — e.g. /p1, /todo.
                        out.append(Suggestion(
                            id: "slash:prop:\(key):\(choice)",
                            label: "\(choice)",
                            insert: "",
                            action: action
                        ))
                        // /<prop> <choice> — e.g. /status doing (filtered by the
                        // same query matcher, so typing "status" surfaces these).
                        out.append(Suggestion(
                            id: "slash:propq:\(key):\(choice)",
                            label: "\(def.name) → \(choice)",
                            insert: "",
                            action: action
                        ))
                    }
                case .date:
                    let field: DateField = key == "deadline" ? .deadline
                        : (key == "scheduled" ? .scheduled : .deadline)
                    out.append(Suggestion(
                        id: "slash:date:\(key)",
                        label: "\(def.name)…",
                        insert: "",
                        action: .openDateSheet(field: field)
                    ))
                default:
                    break
                }
            }
        }
        return out
    }

    /// Merge the built-in format verbs with the registry-derived property
    /// verbs, then FUZZY-rank by `query`. Mirrors web `flattenedSlashFilter`
    /// (`web/src/lib/editor/slash-filter.ts`): score each label via `scoreFuzzy`,
    /// keep `score > 0`, sort by score desc with a stable tie-break on the
    /// original index — so `/pri` (prefix), `/prio`, and subsequence typos all
    /// surface the priority verbs, not just literal substrings. Empty query →
    /// the full list in original order. This is the single source the `.slash`
    /// provider returns.
    static func matchingWithRegistry(_ query: String, tags: [String], registry: PropertyRegistry) -> [Suggestion] {
        let items = base + registryVerbs(tags: tags, registry: registry)
        let q = query.trimmingCharacters(in: .whitespaces)
        guard !q.isEmpty else { return items }
        // Explicit element type so the chained map/filter/sorted/map doesn't blow
        // the Swift type-checker's inference budget (SourceKit flagged "unable to
        // type-check in reasonable time"; xcodebuild compiled it but borderline).
        let scored: [(item: Suggestion, index: Int, score: Int)] = items.enumerated()
            .map { (i, item) in (item: item, index: i, score: scoreFuzzy(item.label, q)) }
        return scored
            .filter { $0.score > 0 }
            .sorted { $0.score != $1.score ? $0.score > $1.score : $0.index < $1.index }
            .map { $0.item }
    }

    /// Tiered fuzzy score mirroring web `scoreFuzzy` (`web/src/lib/fuzzy.ts`):
    /// prefix > word-start substring > substring > subsequence > 0 (no match).
    /// Case-insensitive. Word-start separators match web's `/[\s_/-]/`.
    static func scoreFuzzy(_ label: String, _ filter: String) -> Int {
        let f = filter.lowercased()
        if f.isEmpty { return 0 }
        let l = label.lowercased()
        // Prefix.
        if l.hasPrefix(f) {
            return 1000 + (label.count == filter.count ? 50 : 0)
        }
        // Substring (first occurrence).
        if let r = l.range(of: f) {
            let sIdx = l.distance(from: l.startIndex, to: r.lowerBound)
            let wordStart: Bool
            if sIdx == 0 {
                wordStart = true
            } else {
                let prev = Array(l)[sIdx - 1]
                wordStart = prev == " " || prev == "\t" || prev == "\n"
                    || prev == "_" || prev == "/" || prev == "-"
            }
            return (wordStart ? 500 : 200) - sIdx
        }
        // Subsequence — chars in order, possibly with gaps.
        let lChars = Array(l)
        var li = 0
        var positions: [Int] = []
        for fc in f {
            while li < lChars.count && lChars[li] != fc { li += 1 }
            if li >= lChars.count { return 0 }
            positions.append(li)
            li += 1
        }
        guard let first = positions.first, let last = positions.last else { return 0 }
        return max(1, 50 - (last - first))
    }
}

/// JSON wire shape for `detectNlpLifts`'s `registry_json` argument — mirrors
/// the shared `DetectSpec` the fixture (`nlp-lift-conformance.json`) and
/// `tesela_core::nlp_lift::Registry` both use.
private struct NLPRegistrySpec: Encodable {
    let defaultDateProperty: String
    let properties: [NLPPropertySpec]

    enum CodingKeys: String, CodingKey {
        case properties
        case defaultDateProperty = "default_date_property"
    }
}

private struct NLPPropertySpec: Encodable {
    let key: String
    let valueType: String
    let choices: [String]
    let triggers: [String]

    enum CodingKeys: String, CodingKey {
        case key, choices, triggers
        case valueType = "value_type"
    }
}

/// JSON wire shape for `detectNlpLifts`'s return value.
private struct NLPDetectResult: Decodable {
    let stripped: String
    let props: [NLPLiftedProp]
}

private struct NLPLiftedProp: Decodable {
    let key: String
    let value: String
}

/// Inline NLP detection (P5.5): scans the just-typed tail/token of the live
/// block text for a property `nl_trigger` token or a confident `DateParser`
/// phrase, and surfaces a one-tap "lift into a structured property" suggestion
/// in the SAME chip strip the slash/link/tag triggers use. Conservative —
/// only an EXACT `nl_trigger` token or a confident `DateParser.parse`; never
/// auto-applies (the user taps to lift; declining leaves the text as prose).
///
/// The blur-time whole-block lift (`detectLifts`, below) delegates to the
/// shared Rust `tesela_core::nlp_lift::detect_task_tokens` via the
/// `detectNlpLifts` FFI call (tesela-ug7) rather than a native Swift
/// reimplementation — it's the surface the `nlp-lift-conformance.json`
/// fixture pins. The live caret-anchored chip suggestion (`detect`) and its
/// derived per-keystroke highlight (`detectHighlightRanges`) stay native
/// Swift: they're an iOS-only UX (no web equivalent), not covered by that
/// fixture, and reusing the whole-block FFI call per keystroke/boundary
/// would add FFI-crossing overhead to a hot path for no conformance benefit.
enum InlineNLP {

    /// One detected lift candidate: which UTF-16 span to remove on apply and
    /// the suggestion chip to offer.
    struct Hit: Equatable {
        /// UTF-16 offset where the matched trigger text starts.
        let start: Int
        /// UTF-16 length of the matched trigger text (removed on apply).
        let length: Int
        let suggestion: Suggestion
    }

    /// Detect a lift candidate ending at the caret. Returns the FIRST clear
    /// match found, preferring a property `nl_trigger` token (most specific)
    /// then a `DateParser` phrase over the current line's tail. `nil` when no
    /// confident match — plain prose offers nothing.
    static func detect(
        in text: String,
        caretUTF16 caret: Int,
        tags: [String],
        registry: PropertyRegistry,
        today: Date = Date()
    ) -> Hit? {
        let ns = text as NSString
        let c = max(0, min(caret, ns.length))
        guard c > 0 else { return nil }

        // Literal ranges (wiki links, markdown links/images, inline code, bare
        // URLs) that no lift may be detected inside — mirrors web's
        // `task-tokens.ts` `literalRanges`/`overlaps` pre-claim, closing the
        // gap where a `p1` inside a pasted URL lifted on iOS only.
        let literal = literalRanges(in: text)

        // Resolve the block's property defs once (first def per name wins).
        var defs: [PropertyDef] = []
        var seen = Set<String>()
        for tag in tags {
            let clean = tag.hasPrefix("#") ? String(tag.dropFirst()) : tag
            for def in registry.resolvedDefs(forTag: clean) {
                let key = def.name.lowercased()
                if seen.contains(key) { continue }
                seen.insert(key)
                defs.append(def)
            }
        }
        guard !defs.isEmpty else { return nil }

        // The token immediately before the caret (whitespace-delimited).
        var tokenStart = c
        while tokenStart > 0 {
            let ch = ns.character(at: tokenStart - 1)
            if ch == 0x20 || ch == 0x0A || ch == 0x09 { break }
            tokenStart -= 1
        }
        let token = ns.substring(with: NSRange(location: tokenStart, length: c - tokenStart))
        let tokenLower = token.lowercased()

        // (a) Exact nl_trigger token → a property lift. A select property whose
        // nl_trigger equals a choice sets that choice (e.g. priority `p1`);
        // otherwise the trigger is the property and the token its value.
        if !tokenLower.isEmpty, !overlapsAny(NSRange(location: tokenStart, length: c - tokenStart), literal) {
            for def in defs where def.valueType != .date {
                // Only lift when the typed token IS one of the property's
                // CHOICES — so the value is meaningful (e.g. priority p1, status
                // doing). A keyword nl_trigger on a non-select / number / text
                // property does NOT lift to the literal word.
                guard def.nlTriggers.contains(tokenLower),
                      let value = def.choices.first(where: { $0.lowercased() == tokenLower })
                else { continue }
                let key = def.name.lowercased()
                let action: SuggestionAction = key == "status"
                    ? .setStatus(choice: value)
                    : .setProperty(key: key, value: value)
                let sugg = Suggestion(
                    id: "nlp:prop:\(key):\(value)",
                    label: "\(def.name): \(value)",
                    insert: "",
                    action: action
                )
                return Hit(start: tokenStart, length: c - tokenStart, suggestion: sugg)
            }
        }

        // (b) Date phrase via DateParser over the current line's tail. Only the
        // date properties on the block participate (so a block with no date
        // prop offers no date lift). We try progressively shorter line-tails
        // (longest first) so "due tomorrow" beats a bare "tomorrow".
        let dateDefs = defs.filter { $0.valueType == .date }
        guard !dateDefs.isEmpty else { return nil }

        // Current line span (after the last newline before the caret).
        var lineStart = c
        while lineStart > 0 {
            if ns.character(at: lineStart - 1) == 0x0A { break }
            lineStart -= 1
        }
        // Date-intent keywords: bare-token date prepositions/keywords plus the
        // resolved date properties' nl_triggers (e.g. due/deadline/scheduled).
        // A candidate tail that is NOT line-start and is NOT preceded by one of
        // these — and whose parse infers no field — is a bare weekday/relative
        // token mid-prose and must NOT offer a lift (over-offer guard).
        var dateIntentWords: Set<String> = ["on", "by", "at", "due", "scheduled", "deadline"]
        for def in dateDefs {
            for t in def.nlTriggers { dateIntentWords.insert(t.lowercased()) }
        }

        // Walk word boundaries from lineStart up to the caret; try the tail
        // beginning at each boundary, longest first.
        var starts: [Int] = []
        var i = lineStart
        starts.append(lineStart)
        while i < c {
            let ch = ns.character(at: i)
            if ch == 0x20 || ch == 0x09 {
                if i + 1 < c { starts.append(i + 1) }
            }
            i += 1
        }
        for s in starts {
            let tail = ns.substring(with: NSRange(location: s, length: c - s))
                .trimmingCharacters(in: .whitespaces)
            guard !tail.isEmpty else { continue }
            guard !overlapsAny(NSRange(location: s, length: c - s), literal) else { continue }
            guard let parsed = DateParser.parse(tail, today: today) else { continue }
            // Clear date intent gate: only offer the lift when the tail begins
            // at line-start (a), OR is immediately preceded by a date
            // preposition/keyword (b), OR DateParser inferred a field — a
            // keyword-led parse like "deadline may 23" (c). A bare weekday/
            // relative token sitting mid-sentence after an ordinary word
            // offers nothing.
            let atLineStart = s <= lineStart
            let precededByIntent: Bool = {
                guard !atLineStart else { return false }
                // The whitespace-delimited word ending just before `s`.
                var wEnd = s
                while wEnd > lineStart, isLineSpace(ns.character(at: wEnd - 1)) { wEnd -= 1 }
                var wStart = wEnd
                while wStart > lineStart, !isLineSpace(ns.character(at: wStart - 1)) { wStart -= 1 }
                guard wEnd > wStart else { return false }
                let prevWord = ns.substring(with: NSRange(location: wStart, length: wEnd - wStart)).lowercased()
                return dateIntentWords.contains(prevWord)
            }()
            // Trailing-date rule (Taylor's locked decision): a date phrase at the
            // END of the block/capture text (nothing but whitespace, if any,
            // follows the caret) lifts the type's DEFAULT date property EVEN
            // without an intent word — matching how Taylor types "p1 tomorrow".
            // Scoped to TRAILING only: a date mid-prose ("call her tomorrow about
            // p1") still requires an intent word, which limits false positives.
            let isTrailing: Bool = {
                var k = c
                while k < ns.length {
                    let ch = ns.character(at: k)
                    guard ch == 0x20 || ch == 0x09 || ch == 0x0A else { return false }
                    k += 1
                }
                return true
            }()
            guard atLineStart || precededByIntent || parsed.field != nil || isTrailing else { continue }
            // Write only a date field the resolved type DECLARES: DateParser's
            // inferred field if declared, else the type's DEFAULT/primary date
            // property — the FIRST date-typed property in the type's
            // tag_properties order (Deadline for Task) — else skip.
            let declared = Set(dateDefs.map { $0.name.lowercased() })
            let inferred = parsed.field?.rawValue.lowercased()
            let fieldName: String
            if let inferred, declared.contains(inferred) {
                fieldName = inferred
            } else if let primary = dateDefs.first?.name.lowercased() {
                fieldName = primary
            } else {
                continue
            }
            let value = parsed.time.map { "\(parsed.date) \($0)" } ?? parsed.date
            let label = "\(fieldName.capitalized): \(value)"
            // A confident parse → offer setting the structured date directly
            // (no sheet needed; the user already typed the phrase).
            let sugg = Suggestion(
                id: "nlp:date:\(fieldName):\(value)",
                label: label,
                insert: "",
                action: .setProperty(key: fieldName, value: value)
            )
            return Hit(start: s, length: c - s, suggestion: sugg)
        }
        return nil
    }

    /// A space or tab (0x20 / 0x09). Used to find the word preceding a
    /// candidate tail without crossing a newline.
    private static func isLineSpace(_ ch: unichar) -> Bool {
        ch == 0x20 || ch == 0x09
    }

    // MARK: - Literal-range guard (mirror of web task-tokens.ts `literalRanges`)

    private static let wikiLinkRe = try! NSRegularExpression(pattern: "\\[\\[[^\\]]*\\]\\]")
    private static let mdLinkRe = try! NSRegularExpression(pattern: "!?\\[[^\\]]*\\]\\([^)]*\\)")
    private static let inlineCodeRe = try! NSRegularExpression(pattern: "`[^`]*`")
    private static let bareUrlRe = try! NSRegularExpression(pattern: "\\bhttps?://\\S+")

    /// Ranges (`[[wiki links]]`, markdown links/images, inline `` `code` ``,
    /// bare URLs) that no lift may be detected inside — mirrors web's
    /// `task-tokens.ts` `literalRanges`, so a `p1` inside a pasted URL, or a
    /// date word inside a `[[wiki link]]`, is never lifted on iOS either.
    static func literalRanges(in text: String) -> [NSRange] {
        let ns = text as NSString
        let full = NSRange(location: 0, length: ns.length)
        var ranges: [NSRange] = []
        for re in [wikiLinkRe, mdLinkRe, inlineCodeRe, bareUrlRe] {
            for m in re.matches(in: text, range: full) {
                ranges.append(m.range)
            }
        }
        return ranges
    }

    /// Whether `range` overlaps any range in `ranges`.
    private static func overlapsAny(_ range: NSRange, _ ranges: [NSRange]) -> Bool {
        guard range.length > 0 else { return false }
        let a0 = range.location
        let a1 = range.location + range.length
        return ranges.contains { r in
            let b0 = r.location
            let b1 = r.location + r.length
            return a0 < b1 && a1 > b0
        }
    }

    /// Blur-time WHOLE-BLOCK lift (P-surface-parity): scan the whole block for
    /// EVERY liftable token — not just the one at the caret — strip them, and
    /// return the cleaned text plus the structured props to set. Mirrors web
    /// `detectTaskTokens` + the editor's blur handler
    /// (`BlockEditor.svelte` ~1734): auto-lift on blur, no tap required.
    ///
    /// Built on the same confident, gated `detect` used for the live preview
    /// chip: it repeatedly finds the earliest lift candidate anywhere in the
    /// text (by probing word-end boundaries), records its prop, strips its span,
    /// and re-scans the shortened text — so multiple tokens (`p1 ... due
    /// tomorrow`) all lift. Double spaces left by a strip are collapsed and the
    /// result is trimmed (matching web). Returns `(text, [])` unchanged when no
    /// confident token is found (plain prose lifts nothing).
    static func detectLifts(
        in text: String,
        tags: [String],
        registry: PropertyRegistry,
        today: Date = Date()
    ) -> (stripped: String, props: [(key: String, value: String)]) {
        guard let spec = detectSpec(tags: tags, registry: registry),
              let specData = try? JSONEncoder().encode(spec),
              let specJSON = String(data: specData, encoding: .utf8)
        else { return (text, []) }
        let json = detectNlpLifts(text: text, registryJson: specJSON, anchorDate: fmt(today))
        guard let data = json.data(using: .utf8),
              let result = try? JSONDecoder().decode(NLPDetectResult.self, from: data)
        else { return (text, []) }
        return (result.stripped, result.props.map { ($0.key, $0.value) })
    }

    /// The `{key, value_type, choices, triggers}` registry spec (JSON) the
    /// `detectNlpLifts` FFI call takes — resolved from the block's DIRECT
    /// tags, matching `detect`/`firstLift`'s own def resolution above. The
    /// default date property is the FIRST date-typed property in
    /// `tag_properties` order (mirrors `detect`'s `dateDefs.first` fallback
    /// — see `nlp_lift_conformance.rs`'s module docs on why this trivially
    /// agrees with web's `default_date_property` frontmatter when a type
    /// declares only one date property). `nil` when the tags resolve no
    /// properties (detection off for this block).
    private static func detectSpec(tags: [String], registry: PropertyRegistry) -> NLPRegistrySpec? {
        var defs: [PropertyDef] = []
        var seen = Set<String>()
        for tag in tags {
            let clean = tag.hasPrefix("#") ? String(tag.dropFirst()) : tag
            for def in registry.resolvedDefs(forTag: clean) {
                let key = def.name.lowercased()
                if seen.contains(key) { continue }
                seen.insert(key)
                defs.append(def)
            }
        }
        guard !defs.isEmpty else { return nil }
        let defaultDateProperty = defs.first(where: { $0.valueType == .date })?.name.lowercased() ?? "scheduled"
        let properties = defs.map {
            NLPPropertySpec(
                key: $0.name.lowercased(),
                valueType: $0.valueType.rawValue,
                choices: $0.choices,
                triggers: $0.nlTriggers.map { $0.lowercased() }
            )
        }
        return NLPRegistrySpec(defaultDateProperty: defaultDateProperty, properties: properties)
    }

    /// Live-highlight spans (P-surface-parity, iOS): the UTF-16 ranges of EVERY
    /// inline-NLP token that WOULD lift on commit — the same `p2` / `due
    /// tomorrow` spans `detectLifts` strips — so the editor can color them as
    /// the user types (mirrors web's inline token highlight via `cm-decorations`).
    /// Built on the identical gated `detect` + `firstLift` the lift uses, so the
    /// highlight and the eventual lift never disagree by construction.
    ///
    /// Returns ranges in ORIGINAL-string coordinates: it finds the earliest
    /// candidate, records its span, then MASKS that span with equal-length
    /// spaces (offsets unchanged — spaces can't re-match a priority token or a
    /// date parse) and re-scans, so multiple tokens (`p1 … due tomorrow`) all
    /// surface. Empty when nothing would lift (plain prose / no liftable type).
    static func detectHighlightRanges(
        in text: String,
        tags: [String],
        registry: PropertyRegistry,
        today: Date = Date()
    ) -> [NSRange] {
        var ranges: [NSRange] = []
        let mut = NSMutableString(string: text)
        var guardCount = 0
        while guardCount < 64 {
            guardCount += 1
            guard let hit = firstLift(in: mut as String, tags: tags, registry: registry, today: today)
            else { break }
            guard hit.length > 0,
                  hit.start >= 0,
                  hit.start + hit.length <= mut.length else { break }
            // `detect` returns a date span beginning at the candidate boundary
            // (it trims only for PARSING), so the span can carry leading
            // whitespace — which, once an earlier token has been space-masked,
            // would swallow that already-consumed region. Trim the span to the
            // meaningful token before recording AND masking.
            let trimmed = Self.trimWhitespaceRange(NSRange(location: hit.start, length: hit.length), in: mut)
            guard trimmed.length > 0 else { break }
            ranges.append(trimmed)
            let blanks = String(repeating: " ", count: trimmed.length)
            mut.replaceCharacters(in: trimmed, with: blanks)
        }
        return ranges
    }

    /// Shrink `range` past leading/trailing whitespace (space/tab/newline) in
    /// `ns`. Returns a zero-length range at `range.location` when it is all
    /// whitespace.
    private static func trimWhitespaceRange(_ range: NSRange, in ns: NSString) -> NSRange {
        func isWS(_ ch: unichar) -> Bool { ch == 0x20 || ch == 0x09 || ch == 0x0A }
        var start = range.location
        var end = range.location + range.length
        while start < end, isWS(ns.character(at: start)) { start += 1 }
        while end > start, isWS(ns.character(at: end - 1)) { end -= 1 }
        return NSRange(location: start, length: end - start)
    }

    /// The earliest-starting lift candidate ANYWHERE in `text`, found by probing
    /// `detect` (which is caret-anchored) at every word-end boundary. `nil` when
    /// no boundary yields a confident match.
    private static func firstLift(
        in text: String,
        tags: [String],
        registry: PropertyRegistry,
        today: Date
    ) -> Hit? {
        let ns = text as NSString
        var best: Hit? = nil
        var i = 1
        while i <= ns.length {
            let prev = ns.character(at: i - 1)
            let prevIsSpace = prev == 0x20 || prev == 0x0A || prev == 0x09
            let atEnd = i == ns.length
            let nextIsSpace = !atEnd && {
                let cur = ns.character(at: i)
                return cur == 0x20 || cur == 0x0A || cur == 0x09
            }()
            // A word-end boundary: a non-space char followed by end-of-text or
            // whitespace.
            if !prevIsSpace, atEnd || nextIsSpace,
               let hit = detect(in: text, caretUTF16: i, tags: tags, registry: registry, today: today) {
                // Prefer the earliest-starting hit; among ties on `start` (two
                // different caret boundaries both anchoring at the same word,
                // e.g. "due thu" vs "due thu at 8"), prefer the LONGER match —
                // the more complete parse — so a later boundary that extends an
                // earlier one's phrase isn't shadowed by the shorter one found
                // first. Without this, "due thu at 8" split into a bare "due
                // thu" lift plus a stray "at 8" lift on the next pass.
                if best == nil || hit.start < best!.start
                    || (hit.start == best!.start && hit.length > best!.length) {
                    best = hit
                }
            }
            i += 1
        }
        return best
    }
}
