import Foundation

/// Local (on-device) mirrors of the server-side Agenda + query-DSL
/// evaluation, so the `.relay` backend mode can serve the Agenda and
/// Inbox surfaces from the relay-synced sandbox notes with NO Mac HTTP
/// — the same local-first treatment `refresh`/`loadPage`/`search` got
/// in the `.relay` read paths (07ab601).
///
/// Pure functions over already-parsed `Block`s (the service's
/// `parseBlocks` output) — no I/O, no actor, fully unit-testable.
///
/// What is mirrored, and from where:
///
/// - **Agenda** mirrors `SqliteIndex::agenda_blocks`
///   (`crates/tesela-core/src/db/sqlite.rs`): candidates are blocks
///   with a `scheduled`/`deadline` property; anchor prefers
///   `scheduled`; `status:: done` rows drop unless `include_done`;
///   kind is task when the block has a `tags::` value containing
///   "task" or any `status::`; recurring blocks project forward via
///   the `recurring::`/`recurrence_done::` vocabulary in
///   `crates/tesela-core/src/recurrence.rs`; rows sort by
///   (occurrence_date, occurrence_time, block_id).
///
/// - **Query DSL** mirrors the whitespace-token subset of
///   `parse_query` + `block_matches` (`crates/tesela-core/src/query.rs`)
///   that the Inbox chip registry emits (`kind:block`, `-has:status`,
///   `-is:heading`, `-on:daily-page`, `-on:system-pages`,
///   `has:scheduled`, `has:deadline`, `-has:tag`, `tag-in:A,B`,
///   `-page:x`, `-block:x`, plus bare `key:value` property equality).
///   Tokens outside that subset (the JQL grammar — parens, `ORDER BY`,
///   infix comparisons, quoted strings) are skipped, which degrades to
///   "lets everything through" — the same graceful-degradation posture
///   the server takes for unknown `is:`/`on:` values.
enum LocalQueryEngine {

    // MARK: - ISO date helpers

    /// Mirror of `extract_iso_date` (query.rs): first `YYYY-MM-DD` run
    /// anywhere in the value — handles bare dates AND wiki-wrapped
    /// (`[[2026-04-15]]`) forms.
    static func extractIsoDate(_ value: String) -> String? {
        let bytes = Array(value.utf8)
        let n = bytes.count
        guard n >= 10 else { return nil }
        var i = 0
        while i + 10 <= n {
            let s = bytes[i..<(i + 10)]
            let b = Array(s)
            if b[4] == UInt8(ascii: "-") && b[7] == UInt8(ascii: "-")
                && b[0..<4].allSatisfy({ $0 >= 48 && $0 <= 57 })
                && b[5..<7].allSatisfy({ $0 >= 48 && $0 <= 57 })
                && b[8..<10].allSatisfy({ $0 >= 48 && $0 <= 57 })
            {
                return String(decoding: b, as: UTF8.self)
            }
            i += 1
        }
        return nil
    }

    /// Mirror of `agenda_blocks`' `parse_dated_value` closure
    /// (sqlite.rs): ISO date + optional `HH:MM` token after it.
    static func parseDatedValue(_ value: String) -> (date: String, time: String?)? {
        guard let dateStr = extractIsoDate(value) else { return nil }
        guard isoDate(from: dateStr) != nil else { return nil }
        var time: String? = nil
        if let r = value.range(of: dateStr) {
            let rest = String(value[r.upperBound...]).trimmingCharacters(in: .whitespaces)
            let b = Array(rest.utf8)
            if b.count >= 5,
               b[2] == UInt8(ascii: ":"),
               b[0...1].allSatisfy({ $0 >= 48 && $0 <= 57 }),
               b[3...4].allSatisfy({ $0 >= 48 && $0 <= 57 })
            {
                time = String(rest.prefix(5))
            }
        }
        return (dateStr, time)
    }

    /// Fixed UTC Gregorian calendar so date-only arithmetic is stable
    /// regardless of device timezone/DST.
    private static let utcCalendar: Calendar = {
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = TimeZone(identifier: "UTC")!
        return cal
    }()

    private static let isoFormatter: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd"
        f.locale = Locale(identifier: "en_US_POSIX")
        f.timeZone = TimeZone(identifier: "UTC")
        return f
    }()

    static func isoDate(from s: String) -> Date? { isoFormatter.date(from: s) }
    static func isoString(from d: Date) -> String { isoFormatter.string(from: d) }

    // MARK: - Recurrence (mirror of crates/tesela-core/src/recurrence.rs)

    struct Recurrence: Equatable {
        enum Freq: Equatable { case daily, weekly, monthly, yearly }
        enum End: Equatable {
            /// Series runs through this ISO date (inclusive).
            case until(String)
            /// Total occurrences including the first (rrule COUNT).
            case count(UInt32)
        }
        var freq: Freq
        /// >= 1. For `.daily` this is the "every N days" step.
        var interval: Int
        /// ISO weekday numbers, 1 = Monday … 7 = Sunday. Empty = anchor
        /// on the date's own weekday / day-of-month. Non-empty = BYDAY
        /// set (implies weekly cadence).
        var byWeekday: Set<Int>
        var end: End?

        static func simple(_ freq: Freq, _ interval: Int) -> Recurrence {
            Recurrence(freq: freq, interval: interval, byWeekday: [], end: nil)
        }
    }

    /// Weekday token — three-letter or full name (already lowercased).
    /// Returns ISO weekday (1 = Mon … 7 = Sun).
    private static func parseWeekday(_ tok: String) -> Int? {
        switch tok {
        case "mon", "monday": return 1
        case "tue", "tues", "tuesday": return 2
        case "wed", "wednesday": return 3
        case "thu", "thur", "thurs", "thursday": return 4
        case "fri", "friday": return 5
        case "sat", "saturday": return 6
        case "sun", "sunday": return 7
        default: return nil
        }
    }

    /// Mirror of `recurrence::parse`. Lower-cases + collapses internal
    /// whitespace; splits a trailing ` until YYYY-MM-DD` / ` count N`
    /// end clause; then matches the frequency vocabulary. Unrecognized
    /// input → nil (callers treat that as "not recurring").
    static func parseRecurrence(_ input: String) -> Recurrence? {
        let s = input.split(whereSeparator: { $0.isWhitespace }).joined(separator: " ").lowercased()

        var base = s
        var end: Recurrence.End? = nil
        if let r = s.range(of: " until ", options: .backwards) {
            let dateStr = String(s[r.upperBound...]).trimmingCharacters(in: .whitespaces)
            guard isoDate(from: dateStr) != nil else { return nil }
            base = String(s[..<r.lowerBound])
            end = .until(dateStr)
        } else if let r = s.range(of: " count ", options: .backwards) {
            guard let n = UInt32(String(s[r.upperBound...]).trimmingCharacters(in: .whitespaces)),
                  n > 0 else { return nil }
            base = String(s[..<r.lowerBound])
            end = .count(n)
        }

        guard var rec = parseFreq(base) else { return nil }
        rec.end = end
        return rec
    }

    /// Mirror of `recurrence::parse_freq` (frequency/BYDAY only).
    private static func parseFreq(_ base: String) -> Recurrence? {
        switch base {
        case "daily", "every day": return .simple(.daily, 1)
        case "weekly", "every week": return .simple(.weekly, 1)
        case "monthly", "every month": return .simple(.monthly, 1)
        case "yearly", "annually", "every year": return .simple(.yearly, 1)
        case "weekdays":
            return Recurrence(freq: .weekly, interval: 1, byWeekday: [1, 2, 3, 4, 5], end: nil)
        case "weekends":
            return Recurrence(freq: .weekly, interval: 1, byWeekday: [6, 7], end: nil)
        default: break
        }

        guard base.hasPrefix("every ") else { return nil }
        let rest = String(base.dropFirst("every ".count))

        // BYDAY: "every mon, wed, fri" — all tokens must be weekdays.
        let dayTokens = rest.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }
        if !rest.isEmpty, dayTokens.allSatisfy({ parseWeekday($0) != nil }) {
            let days = Set(dayTokens.compactMap(parseWeekday))
            return Recurrence(freq: .weekly, interval: 1, byWeekday: days, end: nil)
        }

        // "every N <unit>".
        guard let sp = rest.firstIndex(of: " ") else { return nil }
        guard let n = Int(rest[..<sp]), n > 0 else { return nil }
        let unit = String(rest[rest.index(after: sp)...])
        switch unit {
        case "day", "days": return .simple(.daily, n)
        case "week", "weeks": return .simple(.weekly, n)
        case "month", "months": return .simple(.monthly, n)
        case "year", "years": return .simple(.yearly, n)
        default: return nil
        }
    }

    /// Mirror of `recurrence::advance` — next occurrence after
    /// `current` (ISO), or nil when completing `current` exhausts the
    /// series. `doneSoFar` is the count of occurrences completed
    /// *before* this one (the `recurrence_done::` counter).
    static func advance(_ rec: Recurrence, current: String, doneSoFar: UInt32) -> String? {
        if case .count(let total)? = rec.end, doneSoFar + 1 >= total {
            return nil
        }
        guard let next = nextAfter(rec, anchor: current) else { return nil }
        if case .until(let until)? = rec.end, next > until {
            return nil
        }
        return next
    }

    /// Mirror of `recurrence::next_after` — strictly after `anchor`.
    /// Foundation's `Calendar.date(byAdding:)` clamps day-of-month the
    /// same way the Rust `add_months`/`add_years` helpers do
    /// (Jan 31 + 1 month → Feb 28/29; Feb 29 + 1 year → Feb 28).
    static func nextAfter(_ rec: Recurrence, anchor: String) -> String? {
        guard let date = isoDate(from: anchor) else { return nil }
        let cal = utcCalendar
        if !rec.byWeekday.isEmpty {
            // BYDAY stepping — scan forward from anchor+1 for the first
            // date whose weekday is in the set. At most 7 steps.
            var d = date
            for _ in 0..<7 {
                guard let nd = cal.date(byAdding: .day, value: 1, to: d) else { return nil }
                d = nd
                // Calendar weekday: 1 = Sunday … 7 = Saturday → ISO 1 = Mon … 7 = Sun.
                let iso = ((cal.component(.weekday, from: d) + 5) % 7) + 1
                if rec.byWeekday.contains(iso) { return isoString(from: d) }
            }
            return nil
        }
        let next: Date?
        switch rec.freq {
        case .daily: next = cal.date(byAdding: .day, value: rec.interval, to: date)
        case .weekly: next = cal.date(byAdding: .day, value: 7 * rec.interval, to: date)
        case .monthly: next = cal.date(byAdding: .month, value: rec.interval, to: date)
        case .yearly: next = cal.date(byAdding: .year, value: rec.interval, to: date)
        }
        return next.map(isoString(from:))
    }

    // MARK: - Agenda (mirror of SqliteIndex::agenda_blocks)

    /// Build the agenda rows contributed by one note's blocks for the
    /// inclusive `[from, to]` ISO window. The caller concatenates the
    /// per-note results and sorts with `sortAgendaRows`.
    ///
    /// `today` (ISO) drives the `overdue` flag — the server uses
    /// query-time `chrono::Local::now().date_naive()`.
    static func agendaRows(
        blocks: [Block],
        from: String,
        to: String,
        includeDone: Bool,
        today: String
    ) -> [AgendaRow] {
        var rows: [AgendaRow] = []
        for block in blocks {
            // Exact-key property map; later duplicates win, mirroring the
            // Rust HashMap insert order.
            var props: [String: String] = [:]
            for p in block.properties { props[p.key] = p.value }

            // Anchor: prefer `scheduled`, fall back to `deadline`.
            let anchor: (date: String, time: String?)
            let field: AgendaField
            if let v = props["scheduled"], let p = parseDatedValue(v) {
                anchor = p
                field = .scheduled
            } else if let v = props["deadline"], let p = parseDatedValue(v) {
                anchor = p
                field = .deadline
            } else {
                continue
            }

            // Status + done filtering (server: exact "done").
            let status = props["status"]
            if !includeDone && status == "done" { continue }

            // Task iff `tags::` contains "task" (case-insensitive) or any
            // `status::` is present; everything else is an event.
            let hasTaskTag = props["tags"]?
                .split(separator: ",")
                .contains { $0.trimmingCharacters(in: .whitespaces).lowercased() == "task" }
                ?? false
            let kind: AgendaRowKind = (hasTaskTag || props["status"] != nil) ? .task : .event

            let recurrenceStr = props["recurring"]
            let rec = recurrenceStr.flatMap(parseRecurrence)
            let doneSoFarStart = props["recurrence_done"].flatMap { UInt32($0) } ?? 0
            let blockId = "\(block.noteId):\(block.lineNumber)"

            func push(_ date: String, _ time: String?, isAnchor: Bool) {
                rows.append(AgendaRow(
                    block_id: blockId,
                    source_note_id: block.noteId,
                    occurrence_date: date,
                    occurrence_time: time,
                    kind: kind,
                    overdue: date < today,
                    recurrence: recurrenceStr,
                    is_anchor: isAnchor,
                    text: block.text,
                    status: status,
                    field: field
                ))
            }

            if let rec {
                if anchor.date >= from && anchor.date <= to {
                    push(anchor.date, anchor.time, isAnchor: true)
                }
                var current = anchor.date
                var doneSoFar = doneSoFarStart
                while true {
                    guard let next = advance(rec, current: current, doneSoFar: doneSoFar),
                          next <= to else { break }
                    doneSoFar += 1
                    if next >= from {
                        push(next, anchor.time, isAnchor: false)
                    }
                    current = next
                }
            } else {
                if anchor.date >= from && anchor.date <= to {
                    push(anchor.date, anchor.time, isAnchor: true)
                }
            }
        }
        return rows
    }

    /// Server sort: (occurrence_date, occurrence_time, block_id) —
    /// `Option<String>` ordering puts nil time before any time.
    static func sortAgendaRows(_ rows: inout [AgendaRow]) {
        rows.sort { a, b in
            if a.occurrence_date != b.occurrence_date {
                return a.occurrence_date < b.occurrence_date
            }
            if a.occurrence_time != b.occurrence_time {
                switch (a.occurrence_time, b.occurrence_time) {
                case (nil, _): return true
                case (_, nil): return false
                case let (x?, y?): return x < y
                }
            }
            return a.block_id < b.block_id
        }
    }

    // MARK: - Query DSL (whitespace-token subset of parse_query)

    struct SimpleDsl: Equatable {
        enum Kind: Equatable { case block, page }
        struct Clause: Equatable {
            let negated: Bool
            let key: String   // lowercased
            let value: String // verbatim
        }
        var kind: Kind
        var clauses: [Clause]
    }

    /// Tokenize on whitespace; `-` prefix negates; `key:value` shape
    /// only. Tokens without a colon (JQL words, parens, operators) are
    /// skipped — the saved-filter chips never emit them, and skipping
    /// degrades to match-all rather than excluding every row.
    static func parseSimpleDsl(_ dsl: String) -> SimpleDsl {
        var kind: SimpleDsl.Kind = .block
        var clauses: [SimpleDsl.Clause] = []
        for tok in dsl.split(whereSeparator: { $0.isWhitespace }) {
            var t = String(tok)
            var negated = false
            if t.hasPrefix("-") {
                negated = true
                t.removeFirst()
            }
            guard let colon = t.firstIndex(of: ":") else { continue }
            let key = String(t[..<colon]).lowercased()
            let value = String(t[t.index(after: colon)...])
            guard !key.isEmpty else { continue }
            if key == "kind" {
                kind = (value.lowercased() == "page") ? .page : .block
                continue
            }
            clauses.append(SimpleDsl.Clause(negated: negated, key: key, value: value))
        }
        return SimpleDsl(kind: kind, clauses: clauses)
    }

    /// Per-block evaluation context: the parsed block enriched with the
    /// pieces `block_matches` reads off the Rust `ParsedBlock` that the
    /// iOS `Block` doesn't carry directly.
    struct BlockContext {
        let block: Block
        /// `<noteId>:<lineNumber>` — the server's deterministic block id.
        let blockId: String
        /// Own tags: `tags::` property values first, then the trailing
        /// `#tag` cluster (stripped of `#`), deduped — mirrors the merge
        /// order in `make_block` (block.rs).
        let ownTags: [String]
        /// Tags inherited from ancestor blocks.
        let inheritedTags: [String]
        /// Block properties with `tags` removed — `make_block` pops the
        /// `tags` key out of `properties`, so server-side `has:tags` /
        /// `tags:x` property lookups never see it.
        let properties: [String: String]
        let noteId: String
        let pageNoteType: String?
    }

    /// Build evaluation contexts for one note's blocks, computing the
    /// inherited-tag chain via the same ancestor stack as the Rust
    /// parser's pass 2 (block.rs `parse_blocks`).
    static func contexts(
        blocks: [Block],
        noteId: String,
        pageNoteType: String?
    ) -> [BlockContext] {
        var stack: [(indent: Int, tags: [String])] = []
        var out: [BlockContext] = []
        for block in blocks {
            while let last = stack.last, last.indent >= block.indent {
                stack.removeLast()
            }
            var seen = Set<String>()
            let inherited = stack.flatMap { $0.tags }.filter { seen.insert($0).inserted }

            var ownSeen = Set<String>()
            var own: [String] = []
            var props: [String: String] = [:]
            for p in block.properties { props[p.key] = p.value }
            if let tagsValue = props.removeValue(forKey: "tags") {
                for t in tagsValue.split(separator: ",").map({ $0.trimmingCharacters(in: .whitespaces) }) where !t.isEmpty {
                    if ownSeen.insert(t).inserted { own.append(t) }
                }
            }
            for t in block.tags {
                let bare = t.hasPrefix("#") ? String(t.dropFirst()) : t
                if !bare.isEmpty, ownSeen.insert(bare).inserted { own.append(bare) }
            }

            out.append(BlockContext(
                block: block,
                blockId: "\(noteId):\(block.lineNumber)",
                ownTags: own,
                inheritedTags: inherited,
                properties: props,
                noteId: noteId,
                pageNoteType: pageNoteType
            ))
            stack.append((block.indent, own))
        }
        return out
    }

    /// Mirror of `is_daily_note_id` (query.rs) — canonical `YYYY-MM-DD`.
    static func isDailyNoteId(_ noteId: String) -> Bool {
        let b = Array(noteId.utf8)
        return b.count == 10
            && b[4] == UInt8(ascii: "-")
            && b[7] == UInt8(ascii: "-")
            && b[0..<4].allSatisfy { $0 >= 48 && $0 <= 57 }
            && b[5..<7].allSatisfy { $0 >= 48 && $0 <= 57 }
            && b[8..<10].allSatisfy { $0 >= 48 && $0 <= 57 }
    }

    /// Mirror of `is_system_note_type` (query.rs) — exact match.
    static func isSystemNoteType(_ noteType: String) -> Bool {
        noteType == "Tag" || noteType == "Property" || noteType == "Query" || noteType == "Template"
    }

    /// Mirror of `is_heading_text` (query.rs) — 1–6 `#`s followed by
    /// whitespace; `#hashtag` (no space) and 7+ `#`s are not headings.
    static func isHeadingText(_ text: String) -> Bool {
        let trimmed = text.drop(while: { $0.isWhitespace })
        var hashes = 0
        for ch in trimmed {
            if ch == "#" {
                hashes += 1
                if hashes > 6 { return false }
            } else {
                return hashes >= 1 && ch.isWhitespace
            }
        }
        return false
    }

    /// Mirror of `filter_matches` (query.rs) for the Eq/Ne token subset.
    static func clauseMatches(_ clause: SimpleDsl.Clause, ctx: BlockContext) -> Bool {
        let matched: Bool
        switch clause.key {
        case "tag", "type", "pagetag", "blocktag":
            let needle = clause.value.lowercased()
            let includeInherited = clause.key != "blocktag"
            let pool = includeInherited ? ctx.ownTags + ctx.inheritedTags : ctx.ownTags
            matched = pool.contains { $0.lowercased() == needle }
        case "has-link":
            let needle = "[[\(clause.value)]]".lowercased()
            matched = ctx.block.displayText.lowercased().contains(needle)
        case "has":
            let needle = clause.value.lowercased()
            matched = ctx.properties.keys.contains { $0.lowercased() == needle }
        case "page":
            matched = ctx.noteId.lowercased() == clause.value.lowercased()
        case "block":
            matched = ctx.blockId.lowercased() == clause.value.lowercased()
        case "tag-in":
            let needles = clause.value.split(separator: ",")
                .map { $0.trimmingCharacters(in: .whitespaces).lowercased() }
                .filter { !$0.isEmpty }
            if needles.isEmpty {
                matched = false
            } else {
                matched = (ctx.ownTags + ctx.inheritedTags)
                    .contains { needles.contains($0.lowercased()) }
            }
        case "on":
            switch clause.value.lowercased() {
            case "daily-page": matched = isDailyNoteId(ctx.noteId)
            case "system-pages": matched = ctx.pageNoteType.map(isSystemNoteType) ?? false
            default: matched = false
            }
        case "is":
            switch clause.value.lowercased() {
            case "heading": matched = isHeadingText(ctx.block.text)
            default: matched = false
            }
        case "text":
            matched = ctx.block.text.lowercased() == clause.value.lowercased()
        default:
            // Property lookup — case-insensitive key; missing property
            // matches Ne ("missing != value") and fails Eq.
            let actual = ctx.properties.first { $0.key.lowercased() == clause.key }?.value
            guard let actual else { return clause.negated }
            matched = actual.lowercased() == clause.value.lowercased()
        }
        return clause.negated ? !matched : matched
    }

    static func blockMatches(_ dsl: SimpleDsl, ctx: BlockContext) -> Bool {
        dsl.clauses.allSatisfy { clauseMatches($0, ctx: ctx) }
    }

    /// Build the QueryItems contributed by one note for a block-kind
    /// query. Mirrors the row construction in
    /// `SqliteIndex::execute_block_query` (sqlite.rs): the
    /// parent-breadcrumb walk, text fallback to the first raw line,
    /// `primary_tag` = first own tag, properties sans `tags`.
    static func queryItems(
        blocks: [Block],
        noteId: String,
        noteTitle: String,
        pageNoteType: String?,
        dsl: SimpleDsl
    ) -> [QueryItem] {
        let ctxs = contexts(blocks: blocks, noteId: noteId, pageNoteType: pageNoteType)
        var out: [QueryItem] = []
        for (idx, ctx) in ctxs.enumerated() {
            guard blockMatches(dsl, ctx: ctx) else { continue }

            var breadcrumb = [noteTitle]
            var crumbs: [String] = []
            var cursor = idx
            let targetIndent = ctx.block.indent
            while cursor > 0 && targetIndent > 0 {
                cursor -= 1
                if blocks[cursor].indent < targetIndent {
                    crumbs.append(blocks[cursor].text)
                    if blocks[cursor].indent == 0 { break }
                }
            }
            breadcrumb.append(contentsOf: crumbs.reversed())

            let text = ctx.block.text.isEmpty
                ? String(ctx.block.rawText.split(separator: "\n", omittingEmptySubsequences: false).first ?? "")
                : ctx.block.text

            out.append(QueryItem(
                block_id: ctx.blockId,
                page_id: noteId,
                title: noteTitle,
                text: text,
                parent_breadcrumb: breadcrumb,
                kind: .block,
                primary_tag: ctx.ownTags.first,
                properties: ctx.properties,
                page_note_type: pageNoteType
            ))
        }
        return out
    }
}
