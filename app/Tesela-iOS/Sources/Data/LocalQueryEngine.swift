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
/// - **Query DSL** mirrors the legacy-colon subset of `parse_query` +
///   `block_matches` (`crates/tesela-core/src/query.rs`), gated by the
///   shared conformance fixture
///   (`crates/tesela-core/tests/fixtures/query-conformance.json`,
///   consumed by `QueryConformanceTests`): `kind:` prefix, `-`/`NOT`
///   negation, `key:value` equality, quoted values (`tag:"To Read"`),
///   comparison ops (`priority:>=3`, `deadline:<=2026-05-01`, and the
///   infix forms `key >= v`), tight-comma multi-value OR
///   (`status:backlog,todo` — commas touching whitespace end the
///   list, mirroring `peek_tight_comma_continuation`), the legacy
///   loose `tag-in:A,B`, `has:`/`-has:` presence, `is:heading`,
///   `on:daily-page` / `on:system-pages`, `text:`, `page:`/`block:`,
///   and the empty-value drop (`status:` degrades toward match-all).
///   The full-JQL remainder (`OR`, parens, `IN (…)`, `LIKE`,
///   `BETWEEN`, `IS NULL`, `ORDER BY`) stays server-side; those
///   tokens drop out, degrading toward "lets everything through" —
///   the same posture the server takes for unknown `is:`/`on:`
///   values.
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

    // MARK: - Query DSL (legacy-colon subset of parse_query)

    struct SimpleDsl: Equatable {
        enum Kind: Equatable { case block, page }
        /// Comparison operator — the `QueryOp` subset the local engine
        /// evaluates (`Like`/`NotLike` stay server-side).
        enum Op: Equatable { case eq, ne, gt, lt, gte, lte }
        /// One parsed clause. Mirrors Rust's `Predicate`: `cmp` is
        /// `Predicate::Cmp` (negation = the wrapping `Not`), `inList`
        /// is `Predicate::In` (from `key:v1,v2` tight-comma sugar or
        /// the legacy `tag-in:a,b` shape — OR within the key).
        enum Clause: Equatable {
            case cmp(negated: Bool, key: String, op: Op, value: String)
            case inList(negated: Bool, key: String, values: [String])

            var negated: Bool {
                switch self {
                case .cmp(let n, _, _, _), .inList(let n, _, _): return n
                }
            }
        }
        var kind: Kind
        var clauses: [Clause]
        /// Mirror of Rust `Query.sort`: the pre-composed `ORDER BY`
        /// string ("deadline desc", "status, deadline desc"), or nil when
        /// no ORDER BY with at least one field parsed. Not evaluated
        /// locally (sorting stays server-side) — surfaced so validation
        /// can apply the server's structural sort-only carve-out
        /// (`SavedViewLogic.dslValidationError`).
        var sort: String? = nil
    }

    /// Mirror of `tokenize` (query.rs): punctuation + quoted strings +
    /// hyphen-keeping words, each token carrying its byte span in the
    /// source so the parser can detect adjacency (tight commas, value
    /// slurping across `:` / `-` runs).
    private enum DslToken: Equatable {
        case word(String)
        case quoted(String)
        case lparen, rparen, comma, colon, minus
        case op(SimpleDsl.Op)
    }

    private struct SpannedDslToken {
        let tok: DslToken
        /// UTF-8 byte offsets into the source; `end` exclusive.
        let start: Int
        let end: Int
    }

    private static func tokenizeDsl(_ input: String) -> (tokens: [SpannedDslToken], bytes: [UInt8]) {
        let bytes = Array(input.utf8)
        var tokens: [SpannedDslToken] = []
        var i = 0
        func isWordByte(_ b: UInt8) -> Bool {
            (b >= 48 && b <= 57) || (b >= 65 && b <= 90) || (b >= 97 && b <= 122)
                || b == UInt8(ascii: "_") || b == UInt8(ascii: "-")
        }
        func isSpaceByte(_ b: UInt8) -> Bool {
            b == 0x20 || (b >= 0x09 && b <= 0x0D)
        }
        while i < bytes.count {
            let b = bytes[i]
            if isSpaceByte(b) {
                i += 1
                continue
            }
            let start = i
            let tok: DslToken
            switch b {
            case UInt8(ascii: "("): tok = .lparen; i += 1
            case UInt8(ascii: ")"): tok = .rparen; i += 1
            case UInt8(ascii: ","): tok = .comma; i += 1
            case UInt8(ascii: ":"): tok = .colon; i += 1
            case UInt8(ascii: "="): tok = .op(.eq); i += 1
            case UInt8(ascii: "!") where i + 1 < bytes.count && bytes[i + 1] == UInt8(ascii: "="):
                tok = .op(.ne); i += 2
            case UInt8(ascii: "<") where i + 1 < bytes.count && bytes[i + 1] == UInt8(ascii: "="):
                tok = .op(.lte); i += 2
            case UInt8(ascii: ">") where i + 1 < bytes.count && bytes[i + 1] == UInt8(ascii: "="):
                tok = .op(.gte); i += 2
            case UInt8(ascii: "<"): tok = .op(.lt); i += 1
            case UInt8(ascii: ">"): tok = .op(.gt); i += 1
            case UInt8(ascii: "\""):
                // `"…"` literal; unterminated quote runs to the end.
                var j = i + 1
                while j < bytes.count && bytes[j] != UInt8(ascii: "\"") { j += 1 }
                tok = .quoted(String(decoding: bytes[(i + 1)..<j], as: UTF8.self))
                i = j < bytes.count ? j + 1 : j
            case UInt8(ascii: "-"): tok = .minus; i += 1
            case let w where isWordByte(w):
                var j = i
                while j < bytes.count && isWordByte(bytes[j]) { j += 1 }
                tok = .word(String(decoding: bytes[i..<j], as: UTF8.self))
                i = j
            default:
                // Unknown byte — skip silently, mirroring the Rust
                // tokenizer's malformed-input posture.
                i += 1
                continue
            }
            tokens.append(SpannedDslToken(tok: tok, start: start, end: i))
        }
        return (tokens, bytes)
    }

    /// Flat-AND mirror of the Rust recursive-descent parser
    /// (`Parser` in query.rs) for the subset the local engine
    /// evaluates. Structural mirroring notes:
    ///   * `parseClauses` ≈ `parse_and` — implicit AND between atoms;
    ///     a token that can't start a unary (stray comma, `or`, `)`)
    ///     terminates parsing, dropping the remainder exactly like
    ///     Rust's `parse_and` break.
    ///   * `parseUnary` ≈ `parse_unary` — `-`/`NOT` toggle negation
    ///     (Rust stacks `Not` via recursion; a parity flag is
    ///     equivalent), and a dropped predicate (`kind:…`, bareword,
    ///     empty value) re-synchronizes at the next unary-starting
    ///     token with the negation still pending, mirroring how
    ///     Rust's outer `Not` wraps whatever the inner retry returns.
    ///   * `OR` / parens / JQL (`IN (…)`, `LIKE`, `BETWEEN`,
    ///     `IS NULL`) aren't evaluated locally: paren tokens are
    ///     skipped (so a single parenthesized group degrades to its
    ///     inner clauses), JQL keywords drop as barewords, and `or`
    ///     ends the clause list.
    ///   * `ORDER BY` stops the clause list (mirroring Rust
    ///     `parse_unary`'s stop) so `parseOrderBy` can surface the
    ///     sort spec at the top level — sorting itself stays
    ///     server-side; the spec exists for validation parity.
    private struct DslParser {
        let tokens: [SpannedDslToken]
        let bytes: [UInt8]
        var pos = 0
        var kind: SimpleDsl.Kind = .block

        var peek: DslToken? { pos < tokens.count ? tokens[pos].tok : nil }

        func peekKeyword(_ kw: String) -> Bool {
            if case .word(let w)? = peek { return w.lowercased() == kw }
            return false
        }

        /// Mirror of `peek_order_by` — is the upcoming two-token
        /// sequence `ORDER BY` (case-insensitive)? Stops expression
        /// parsing so the trailing sort clause is picked up by
        /// `parseOrderBy` at the top level.
        var peekOrderBy: Bool {
            guard case .word(let order)? = peek, order.lowercased() == "order" else {
                return false
            }
            guard pos + 1 < tokens.count, case .word(let by) = tokens[pos + 1].tok else {
                return false
            }
            return by.lowercased() == "by"
        }

        /// Mirror of `peek_starts_unary` — `(`, `-`, or a word that
        /// isn't the `or`/`and` keyword.
        var peekStartsUnary: Bool {
            switch peek {
            case .lparen, .minus: return true
            case .word: return !peekKeyword("or") && !peekKeyword("and")
            default: return false
            }
        }

        mutating func parseClauses() -> [SimpleDsl.Clause] {
            var clauses: [SimpleDsl.Clause] = []
            // First unary is unconditional (Rust parse_and's `left`);
            // if it fails the whole expression is empty (match-all).
            guard let first = parseUnary() else { return clauses }
            clauses.append(first)
            while true {
                if peekKeyword("and") {
                    pos += 1
                } else if !peekStartsUnary {
                    break
                }
                guard let next = parseUnary() else { break }
                clauses.append(next)
            }
            return clauses
        }

        mutating func parseUnary() -> SimpleDsl.Clause? {
            var negated = false
            while true {
                // Mirror of Rust `parse_unary`'s ORDER BY stop: without
                // it, ORDER / BY / the field names would be consumed as
                // dropped barewords and the sort never populated.
                if peekOrderBy { return nil }
                if peekKeyword("not") || peek == .minus {
                    pos += 1
                    negated.toggle()
                    continue
                }
                if peek == .lparen {
                    // Grouping isn't evaluated locally — skip the paren
                    // so `(tag:x)` degrades to its inner clause.
                    pos += 1
                    continue
                }
                let startPos = pos
                if let clause = parsePredicate(negated: negated) { return clause }
                if pos == startPos { return nil }
                // Predicate consumed but produced nothing (`kind:…`,
                // bareword, empty value) — eat an optional AND and
                // retry at the next unary-starting token.
                if peekKeyword("and") { pos += 1 }
                if !peekStartsUnary { return nil }
            }
        }

        /// Mirror of `parse_predicate` for the legacy shapes
        /// (`key:value`, `key:>=N`, `key:v1,v2`, `tag-in:a,b,c`,
        /// `has:foo`) plus infix ops. JQL keyword forms fall through
        /// to the bareword drop.
        mutating func parsePredicate(negated: Bool) -> SimpleDsl.Clause? {
            guard pos < tokens.count else { return nil }
            let keyTok = tokens[pos].tok
            pos += 1
            guard case .word(let rawKey) = keyTok else { return nil }
            let key = rawKey.lowercased()

            // `kind:` is meta — consume the value, set kind, no clause.
            if key == "kind" {
                if peek == .colon { pos += 1 }
                if let v = parseValue() {
                    kind = (v.lowercased() == "page" || v.lowercased() == "pages") ? .page : .block
                }
                return nil
            }

            // Legacy `tag-in:a,b,c` — whitespace-tolerant comma list on
            // the stripped key, mirroring `parse_comma_list_until_whitespace`.
            if key.hasSuffix("-in"), peek == .colon {
                pos += 1
                let realKey = String(key.dropLast("-in".count))
                return .inList(negated: negated, key: realKey, values: parseCommaListUntilBoundary())
            }

            // Infix `key = v` / `key >= v` / … — no empty-value drop and
            // no comma sugar on this path (mirrors Rust).
            if case .op(let infixOp)? = peek {
                pos += 1
                let value = parseValue() ?? ""
                return .cmp(negated: negated, key: key, op: infixOp, value: value)
            }

            // Legacy colon syntax: `key:value`, `key:>=N`, `key:v1,v2`.
            if peek == .colon {
                pos += 1
                var op = SimpleDsl.Op.eq
                // `consume_legacy_colon_op` accepts !=, <=, >=, <, > —
                // never a bare `=` after the colon.
                if case .op(let colonOp)? = peek, colonOp != .eq {
                    op = colonOp
                    pos += 1
                }
                let value = parseValue() ?? ""
                // Empty value drops the clause (degrade toward
                // match-all); `has:` is the one legitimate no-value key.
                if key != "has" && value.isEmpty { return nil }
                // Tight-comma multi-value sugar — Eq only.
                if op == .eq && !value.isEmpty {
                    var values = [value]
                    while peekTightCommaContinuation() {
                        pos += 1 // consume ','
                        if let v = parseValue(), !v.isEmpty {
                            values.append(v)
                        } else {
                            break
                        }
                    }
                    if values.count > 1 {
                        return .inList(negated: negated, key: key, values: values)
                    }
                }
                return .cmp(negated: negated, key: key, op: op, value: value)
            }

            // Bareword with no operator (including the JQL keyword
            // grammar) — dropped silently, same as Rust.
            return nil
        }

        /// Mirror of `parse_value`: a quoted literal is self-contained;
        /// otherwise slurp every token contiguous with the first word
        /// (values containing `:`, `-`, comparison glyphs) until a
        /// whitespace gap or a quoted/paren/comma token.
        mutating func parseValue() -> String? {
            if case .quoted(let s)? = peek {
                pos += 1
                return s
            }
            guard pos < tokens.count else { return nil }
            let first = tokens[pos]
            pos += 1
            guard case .word(let w) = first.tok else { return nil }
            var buf = w
            var endOffset = first.end
            while pos < tokens.count {
                let span = tokens[pos]
                if span.start != endOffset { break } // whitespace gap
                switch span.tok {
                case .word, .colon, .minus, .op:
                    buf += String(decoding: bytes[span.start..<span.end], as: UTF8.self)
                    endOffset = span.end
                    pos += 1
                default:
                    return buf // quoted / paren / comma end the value
                }
            }
            return buf
        }

        /// Mirror of `peek_tight_comma_continuation` — a `,` with no
        /// whitespace on either side, followed by a value token. A
        /// loose comma is stray punctuation that ends the list.
        func peekTightCommaContinuation() -> Bool {
            guard pos > 0, pos < tokens.count, tokens[pos].tok == .comma else { return false }
            let comma = tokens[pos]
            guard tokens[pos - 1].end == comma.start else { return false }
            guard pos + 1 < tokens.count else { return false }
            let next = tokens[pos + 1]
            guard next.start == comma.end else { return false }
            switch next.tok {
            case .word, .quoted: return true
            default: return false
            }
        }

        /// Mirror of `parse_order_by` (query.rs): a trailing
        /// `ORDER BY field [ASC|DESC] [, field [ASC|DESC]] …` clause,
        /// pre-composed into the comma-separated string shape Rust's
        /// `Query.sort` carries (lowercased keys; omitted direction =
        /// ascending). nil when no `ORDER BY` sits at the cursor OR no
        /// field word followed it — "order by" alone is NOT a sort,
        /// which is what makes the validation carve-out structural.
        mutating func parseOrderBy() -> String? {
            guard peekOrderBy else { return nil }
            pos += 2 // consume ORDER BY
            var parts: [String] = []
            while pos < tokens.count {
                guard case .word(let key) = tokens[pos].tok else { break }
                pos += 1
                var part = key.lowercased()
                if peekKeyword("desc") {
                    pos += 1
                    part += " desc"
                } else if peekKeyword("asc") {
                    pos += 1
                    part += " asc"
                }
                parts.append(part)
                guard peek == .comma else { break }
                pos += 1 // consume comma
            }
            return parts.isEmpty ? nil : parts.joined(separator: ", ")
        }

        /// Mirror of `parse_comma_list_until_whitespace` (the legacy
        /// `tag-in:` list) — words/quoted + commas until any other
        /// token; empty entries (trailing comma) stripped.
        mutating func parseCommaListUntilBoundary() -> [String] {
            var out: [String] = []
            loop: while pos < tokens.count {
                switch tokens[pos].tok {
                case .word, .quoted:
                    if let v = parseValue() { out.append(v) }
                case .comma:
                    pos += 1
                default:
                    break loop
                }
            }
            return out.filter { !$0.isEmpty }
        }
    }

    /// Parse a DSL string into the flat-AND clause list the local
    /// matcher evaluates. Gated by the shared conformance fixture —
    /// every supported shape must match Rust's `parse_query` +
    /// `block_matches` exactly.
    static func parseSimpleDsl(_ dsl: String) -> SimpleDsl {
        let (tokens, bytes) = tokenizeDsl(dsl)
        var parser = DslParser(tokens: tokens, bytes: bytes)
        let clauses = parser.parseClauses()
        // Same top-level sequencing as Rust `parse_query`: the sort is
        // parsed at wherever expression parsing stopped.
        let sort = parser.parseOrderBy()
        return SimpleDsl(kind: parser.kind, clauses: clauses, sort: sort)
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

    /// Mirror of `is_iso_date` (query.rs) — `YYYY-MM-DD` prefix shape
    /// (longer strings pass when the first 10 bytes match, same as Rust).
    private static func isIsoDateShaped(_ s: String) -> Bool {
        let b = Array(s.utf8)
        return b.count >= 10
            && b[4] == UInt8(ascii: "-")
            && b[7] == UInt8(ascii: "-")
            && b[0..<4].allSatisfy { $0 >= 48 && $0 <= 57 }
            && b[5..<7].allSatisfy { $0 >= 48 && $0 <= 57 }
            && b[8..<10].allSatisfy { $0 >= 48 && $0 <= 57 }
    }

    /// Byte-wise lexicographic order, mirroring Rust's `str::cmp`.
    private static func lexicographic(_ a: String, _ b: String) -> ComparisonResult {
        let ab = Array(a.utf8)
        let bb = Array(b.utf8)
        var i = 0
        let n = min(ab.count, bb.count)
        while i < n {
            if ab[i] != bb[i] {
                return ab[i] < bb[i] ? .orderedAscending : .orderedDescending
            }
            i += 1
        }
        if ab.count == bb.count { return .orderedSame }
        return ab.count < bb.count ? .orderedAscending : .orderedDescending
    }

    /// Mirror of `compare` (query.rs): number → ISO date → case-folded
    /// string, in that order.
    static func compareValues(_ a: String, _ b: String) -> ComparisonResult {
        if let an = Double(a.trimmingCharacters(in: .whitespaces)),
           let bn = Double(b.trimmingCharacters(in: .whitespaces)) {
            if an < bn { return .orderedAscending }
            if an > bn { return .orderedDescending }
            return .orderedSame
        }
        if isIsoDateShaped(a) && isIsoDateShaped(b) {
            return lexicographic(a, b) // ISO dates sort lexicographically
        }
        return lexicographic(a.lowercased(), b.lowercased())
    }

    /// Mirror of `apply_op` (query.rs) for the local op subset.
    static func applyOp(_ actual: String, _ op: SimpleDsl.Op, _ expected: String) -> Bool {
        switch op {
        case .eq: return actual.lowercased() == expected.lowercased()
        case .ne: return actual.lowercased() != expected.lowercased()
        case .gt: return compareValues(actual, expected) == .orderedDescending
        case .lt: return compareValues(actual, expected) == .orderedAscending
        case .gte: return compareValues(actual, expected) != .orderedAscending
        case .lte: return compareValues(actual, expected) != .orderedDescending
        }
    }

    /// Mirror of `pred_matches` (query.rs): `cmp` routes to the per-key
    /// matcher; `inList` is OR over per-value Eq through the same
    /// matcher (so `status:a,b` ≡ `tag-in:`-style membership exactly).
    static func clauseMatches(_ clause: SimpleDsl.Clause, ctx: BlockContext) -> Bool {
        switch clause {
        case .cmp(let negated, let key, let op, let value):
            let matched = cmpMatches(key: key, op: op, value: value, ctx: ctx)
            return negated ? !matched : matched
        case .inList(let negated, let key, let values):
            let matched = values.contains { cmpMatches(key: key, op: .eq, value: $0, ctx: ctx) }
            return negated ? !matched : matched
        }
    }

    /// Mirror of `filter_matches` (query.rs) for one `key OP value`.
    private static func cmpMatches(key: String, op: SimpleDsl.Op, value: String, ctx: BlockContext) -> Bool {
        switch key {
        case "tag", "type", "pagetag", "blocktag":
            let needle = value.lowercased()
            let includeInherited = key != "blocktag"
            let pool = includeInherited ? ctx.ownTags + ctx.inheritedTags : ctx.ownTags
            let hasTag = pool.contains { $0.lowercased() == needle }
            return presence(hasTag, op)
        case "has-link":
            let needle = "[[\(value)]]".lowercased()
            return presence(ctx.block.displayText.lowercased().contains(needle), op)
        case "has":
            let needle = value.lowercased()
            return presence(ctx.properties.keys.contains { $0.lowercased() == needle }, op)
        case "page":
            return presence(ctx.noteId.lowercased() == value.lowercased(), op)
        case "block":
            return presence(ctx.blockId.lowercased() == value.lowercased(), op)
        case "tag-in":
            // Legacy comma-valued filter shape (kept for callers that
            // build clauses directly; the parser desugars `tag-in:` to
            // `inList` on `tag`).
            let needles = value.split(separator: ",")
                .map { $0.trimmingCharacters(in: .whitespaces).lowercased() }
                .filter { !$0.isEmpty }
            let matched = !needles.isEmpty && (ctx.ownTags + ctx.inheritedTags)
                .contains { needles.contains($0.lowercased()) }
            return presence(matched, op)
        case "on":
            let matched: Bool
            switch value.lowercased() {
            case "daily-page": matched = isDailyNoteId(ctx.noteId)
            case "system-pages": matched = ctx.pageNoteType.map(isSystemNoteType) ?? false
            default: matched = false // unknown on: degrades gracefully
            }
            return presence(matched, op)
        case "is":
            let matched = value.lowercased() == "heading"
                ? isHeadingText(ctx.block.text)
                : false // unknown is: degrades gracefully
            return presence(matched, op)
        case "text":
            return applyOp(ctx.block.text, op, value)
        default:
            // Property lookup — case-insensitive key; missing property
            // matches Ne ("missing != value") and fails everything else.
            let actual = ctx.properties.first { $0.key.lowercased() == key }?.value
            guard let actual else { return op == .ne }
            return applyOp(actual, op, value)
        }
    }

    /// Eq/Ne over a boolean presence test; comparison ops aren't
    /// meaningful for these keys (Rust returns `false`).
    private static func presence(_ matched: Bool, _ op: SimpleDsl.Op) -> Bool {
        switch op {
        case .eq: return matched
        case .ne: return !matched
        default: return false
        }
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
