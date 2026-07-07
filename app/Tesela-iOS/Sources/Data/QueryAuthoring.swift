import Foundation

/// Pure JQL-authoring logic for the iOS saved-view editor (tesela-vp9.5).
///
/// Ports the web `query-input/{classify,caret-context,completion,
/// overlay-spans}.ts` modules (tesela-vp9.2) onto `LocalQueryEngine`'s
/// internal tokenizer (tesela-vp9.4) — the SAME token stream the parser
/// consumes, so "what completes / what highlights" never drifts from
/// "what parses". Best-effort, advisory only, same spirit as
/// `LocalQueryEngine.QueryDiagnostic` (spec decision 3): a lightweight
/// state machine over the token stream, NOT a second parser. It only
/// needs to be right often enough to look correct and place the
/// completion popup sensibly — `LocalQueryEngine.parseSimpleDsl` stays
/// the sole source of truth for what a query MEANS.
enum QueryAuthoring {

    // MARK: - Token roles (mirror web's classify.ts TokenRole)

    enum TokenRole: Equatable {
        case key, keyword, operatorRole, value, paren, comma
    }

    struct ClassifiedToken {
        let span: LocalQueryEngine.SpannedDslToken
        let role: TokenRole
        /// Lowercased predicate key this token is scoped to — set on the
        /// operator/value/comma tokens of a `key OP value` predicate, and
        /// on the `IN`/`LIKE`/`BETWEEN`/`IS` keyword that introduces a
        /// value. `nil` for key tokens themselves, combinators
        /// (`AND`/`OR`/`NOT`), and punctuation outside any predicate's
        /// value region.
        let key: String?
    }

    private enum ClassifyState { case key, op, val }

    private static let valueKeywords: Set<String> = ["in", "like", "between", "is"]
    private static let keywords: Set<String> = [
        "and", "or", "not", "in", "like", "between", "is", "null", "empty",
        "order", "by", "asc", "desc",
    ]

    /// Classify every token in `tokens` (the output of
    /// `LocalQueryEngine.tokenizeDsl`) into a role + governing key. See
    /// the type doc for what "best-effort" means here — it is not the
    /// grammar.
    static func classifyTokens(_ tokens: [LocalQueryEngine.SpannedDslToken]) -> [ClassifiedToken] {
        var out: [ClassifiedToken] = []
        var state: ClassifyState = .key
        var activeKey: String? = nil
        var pendingBetween = false
        var inOrderBy = false

        for (i, sp) in tokens.enumerated() {
            switch sp.tok {
            case .lparen, .rparen:
                out.append(ClassifiedToken(span: sp, role: .paren, key: state == .val ? activeKey : nil))
                if case .rparen = sp.tok { state = .key }
                continue
            case .comma:
                out.append(ClassifiedToken(span: sp, role: .comma, key: inOrderBy ? nil : activeKey))
                if !inOrderBy { state = .val }
                continue
            case .colon, .op:
                out.append(ClassifiedToken(span: sp, role: .operatorRole, key: activeKey))
                state = .val
                continue
            case .minus:
                out.append(ClassifiedToken(span: sp, role: state == .key ? .keyword : .operatorRole, key: activeKey))
                continue
            case .quoted:
                out.append(ClassifiedToken(span: sp, role: .value, key: activeKey))
                state = .key
                continue
            case .word(let w):
                let lower = w.lowercased()
                if keywords.contains(lower) {
                    out.append(ClassifiedToken(span: sp, role: .keyword, key: valueKeywords.contains(lower) ? activeKey : nil))
                    switch lower {
                    case "order":
                        if i + 1 < tokens.count, case .word(let by) = tokens[i + 1].tok, by.lowercased() == "by" {
                            inOrderBy = true
                        }
                        state = .key
                        activeKey = nil
                    case "by":
                        state = .key
                    case "and", "or":
                        if pendingBetween {
                            pendingBetween = false
                            state = .val
                        } else {
                            state = .key
                            activeKey = nil
                        }
                    case "in", "like":
                        state = .val
                    case "between":
                        state = .val
                        pendingBetween = true
                    case "is":
                        state = .val
                    case "null", "empty":
                        state = .key
                    case "asc", "desc":
                        state = .key
                    default:
                        break // "not" — leave state untouched.
                    }
                    continue
                }
                if inOrderBy {
                    out.append(ClassifiedToken(span: sp, role: .key, key: nil))
                    state = .key
                    continue
                }
                if state == .val {
                    out.append(ClassifiedToken(span: sp, role: .value, key: activeKey))
                    state = .key
                } else {
                    out.append(ClassifiedToken(span: sp, role: .key, key: nil))
                    activeKey = lower
                    state = .op
                }
            }
        }
        return out
    }

    // MARK: - Caret context (mirror caret-context.ts)

    enum CompletionTier: Equatable { case key, operatorTier, value, none }

    struct CaretContext {
        let tier: CompletionTier
        /// `[from, to)` byte range in the source string an accepted item
        /// replaces.
        let from: Int
        let to: Int
        /// Already-typed text in `[from, to)` — available for prefix
        /// filtering by a caller (unused today; the completion strip
        /// shows the full tier candidate list).
        let prefix: String
        /// Lowercased governing predicate key. Only meaningful for tier
        /// `.value`.
        let key: String?
    }

    private static func keywordIntroducesValue(_ word: String) -> Bool {
        let w = word.lowercased()
        return w == "in" || w == "like" || w == "between"
    }

    /// Caret-context classification for the completion strip. Given the
    /// raw source string and a byte-offset cursor, decides which of the
    /// three completion tiers (spec decision 4) applies. `cursor` is
    /// text.utf8.count in every current call site — see
    /// `GrViewEditorSheet`'s doc note on why iOS uses end-of-text as the
    /// working caret (a SwiftUI `TextField` doesn't expose a real caret
    /// without a `UIViewRepresentable` migration the spec defers).
    static func caretContext(_ input: String, cursor: Int) -> CaretContext {
        let none = CaretContext(tier: .none, from: 0, to: 0, prefix: "", key: nil)
        let bytes = Array(input.utf8)
        guard cursor >= 0, cursor <= bytes.count else { return none }
        // Mid-word caret — don't interrupt editing inside an existing
        // token (mirrors the pre-vp9.2 web `dslKeySuggestions` convention).
        if cursor < bytes.count {
            let b = bytes[cursor]
            let isSpace = b == 0x20 || (b >= 0x09 && b <= 0x0D)
            if !isSpace { return none }
        }

        let (tokens, _) = LocalQueryEngine.tokenizeDsl(input)
        let classified = classifyTokens(tokens)

        // The partial word being typed, if the cursor sits immediately
        // after a word token with no gap.
        let partial = classified.first { c in
            if case .word = c.span.tok, c.span.end == cursor { return true }
            return false
        }
        let from = partial?.span.start ?? cursor
        let prefix: String = {
            if let partial, case .word(let w) = partial.span.tok { return w }
            return ""
        }()

        // The nearest classified token strictly before the partial word
        // (or the caret, when there's no partial word).
        var prev: ClassifiedToken? = nil
        for c in classified {
            if c.span.start >= from { break }
            prev = c
        }

        guard let prev else {
            return CaretContext(tier: .key, from: from, to: cursor, prefix: prefix, key: nil)
        }

        switch prev.role {
        case .key:
            let key: String? = {
                if case .word(let w) = prev.span.tok { return w.lowercased() }
                return nil
            }()
            return CaretContext(tier: .operatorTier, from: from, to: cursor, prefix: prefix, key: key)
        case .operatorRole, .comma:
            return CaretContext(tier: .value, from: from, to: cursor, prefix: prefix, key: prev.key)
        case .paren:
            if let k = prev.key {
                return CaretContext(tier: .value, from: from, to: cursor, prefix: prefix, key: k)
            }
            return CaretContext(tier: .key, from: from, to: cursor, prefix: prefix, key: nil)
        case .keyword:
            let w: String = {
                if case .word(let ww) = prev.span.tok { return ww }
                return ""
            }()
            if keywordIntroducesValue(w) {
                return CaretContext(tier: .value, from: from, to: cursor, prefix: prefix, key: prev.key)
            }
            return CaretContext(tier: .key, from: from, to: cursor, prefix: prefix, key: nil)
        case .value:
            // A predicate just finished — the natural next thing is a
            // new predicate (implicit AND) or an explicit combinator.
            return CaretContext(tier: .key, from: from, to: cursor, prefix: prefix, key: nil)
        }
    }

    // MARK: - Completion candidates (mirror completion.ts, spec decision 4)

    struct CompletionItem: Equatable, Identifiable {
        var id: String { label }
        let label: String
        let secondary: String?
    }

    /// Meta keys every query understands, per the vp9 spec's decision 4
    /// list — distinct from registered properties.
    static let metaKeys: [String] = [
        "type", "kind", "tag", "status", "has", "is", "on", "text", "page", "block",
    ]

    /// The full operator/combinator/sort-keyword menu offered right after
    /// a key (spec decision 4 + the task's tier-(b) list) — a fixed menu,
    /// not filtered by what's grammatically valid at the exact caret
    /// position (the spec'd simplification).
    static let operatorItems: [String] = [
        "=", "!=", "<", "<=", ">", ">=", ":",
        "IN", "NOT IN", "LIKE", "NOT LIKE", "BETWEEN",
        "IS NULL", "IS NOT NULL", "AND", "OR", "ORDER BY", "ASC", "DESC",
    ]

    private static func isSelectType(_ vt: PropertyType) -> Bool {
        vt == .select || vt == .multiSelect
    }

    /// Build the full completion candidate list for `ctx`'s tier.
    /// Returns `[]` for tier `.none`, for a VALUE tier whose key isn't a
    /// select-typed property (and isn't `type`/`kind`), or when there's
    /// nothing to offer.
    static func buildCompletions(
        _ ctx: CaretContext,
        properties: [String: PropertyDef],
        typeNames: [String]
    ) -> [CompletionItem] {
        switch ctx.tier {
        case .key:
            var seen = Set<String>()
            var items: [CompletionItem] = []
            for def in properties.values.sorted(by: { $0.name.lowercased() < $1.name.lowercased() }) {
                let lower = def.name.lowercased()
                if seen.contains(lower) { continue }
                seen.insert(lower)
                items.append(CompletionItem(label: def.name, secondary: PROPERTY_TYPE_LABELS[def.valueType]))
            }
            for key in metaKeys {
                if seen.contains(key) { continue }
                seen.insert(key)
                items.append(CompletionItem(label: key, secondary: "meta"))
            }
            return items
        case .operatorTier:
            return operatorItems.map { CompletionItem(label: $0, secondary: nil) }
        case .value:
            guard let key = ctx.key?.lowercased(), !key.isEmpty else { return [] }
            if key == "type" || key == "kind" {
                return typeNames.map { CompletionItem(label: $0, secondary: nil) }
            }
            if let def = properties[key], isSelectType(def.valueType), !def.choices.isEmpty {
                return def.choices.map { CompletionItem(label: $0, secondary: nil) }
            }
            return []
        case .none:
            return []
        }
    }

    /// Splice an accepted completion item into `input` at `ctx`'s
    /// `[from, to)` byte range. Insertion shape depends on tier: KEY
    /// items become `key:` (ready to type/pick a value — the familiar
    /// colon-DSL shorthand, matching the pre-vp9.2 web
    /// `applyDslSuggestion` default); OPERATOR items get a trailing space
    /// (except bare `:`, which stays tight against the key); VALUE items
    /// are quoted when they contain whitespace, followed by a trailing
    /// space so the user can keep typing the next predicate.
    static func applyCompletion(_ input: String, _ ctx: CaretContext, _ item: String) -> (text: String, cursor: Int) {
        let bytes = Array(input.utf8)
        let from = max(0, min(ctx.from, bytes.count))
        let to = max(from, min(ctx.to, bytes.count))
        let insertion: String
        switch ctx.tier {
        case .key:
            insertion = "\(item):"
        case .operatorTier:
            insertion = item == ":" ? ":" : "\(item) "
        case .value:
            let needsQuotes = item.contains(where: { $0.isWhitespace })
            insertion = (needsQuotes ? "\"\(item)\"" : item) + " "
        case .none:
            insertion = item
        }
        let insertionBytes = Array(insertion.utf8)
        var newBytes = Array(bytes[0..<from])
        newBytes.append(contentsOf: insertionBytes)
        newBytes.append(contentsOf: bytes[to...])
        let text = String(decoding: newBytes, as: UTF8.self)
        return (text, from + insertionBytes.count)
    }

    // MARK: - Token preview spans (mirror overlay-spans.ts)

    /// Coloring bucket for the read-only token-preview row (spec item 2):
    /// key/operator/value/string/number/paren.
    enum PreviewTokenKind: Equatable {
        case key, operatorKind, value, string, number, paren
    }

    struct PreviewSpan {
        let start: Int
        let end: Int
        let text: String
        /// `nil` for a whitespace gap between tokens (or the whole
        /// string, when input is empty/all-whitespace) — renders with no
        /// role color.
        let kind: PreviewTokenKind?
        /// True when this span overlaps a `QueryDiagnostic` — the
        /// preview row underlines it.
        let diagnostic: Bool
    }

    private static func previewKind(for token: ClassifiedToken) -> PreviewTokenKind {
        switch token.role {
        case .key: return .key
        case .keyword, .operatorRole, .comma: return .operatorKind
        case .paren: return .paren
        case .value:
            switch token.span.tok {
            case .quoted: return .string
            case .word(let w): return Double(w) != nil ? .number : .value
            default: return .value
            }
        }
    }

    private static func diagnosticsOverlap(_ start: Int, _ end: Int, _ diagnostics: [LocalQueryEngine.QueryDiagnostic]) -> Bool {
        diagnostics.contains { start < $0.end && end > $0.start }
    }

    /// Build the span list the token-preview row renders — spans cover
    /// the ENTIRE string (including whitespace gaps), so the row's text
    /// content matches the real input glyph-for-glyph.
    static func buildPreviewSpans(
        _ input: String,
        diagnostics: [LocalQueryEngine.QueryDiagnostic] = []
    ) -> [PreviewSpan] {
        let bytes = Array(input.utf8)
        let (tokens, _) = LocalQueryEngine.tokenizeDsl(input)
        let classified = classifyTokens(tokens)
        var spans: [PreviewSpan] = []
        var cursor = 0

        func push(_ start: Int, _ end: Int, _ kind: PreviewTokenKind?) {
            let text = String(decoding: bytes[start..<end], as: UTF8.self)
            spans.append(PreviewSpan(start: start, end: end, text: text, kind: kind, diagnostic: diagnosticsOverlap(start, end, diagnostics)))
        }

        for c in classified {
            if c.span.start > cursor { push(cursor, c.span.start, nil) }
            push(c.span.start, c.span.end, previewKind(for: c))
            cursor = c.span.end
        }
        if cursor < bytes.count { push(cursor, bytes.count, nil) }
        return spans
    }

    // MARK: - Canonical-form predicate equality (spec item 4: chips → JQL)

    /// Reduce a `BoolExpr` to a canonical form so structurally-different
    /// but semantically-identical negations compare equal — specifically
    /// `NOT (key OP value)` folds to `key OP' value` for the invertible
    /// `eq`/`ne` pair, so a legacy `-has:status` (parses to
    /// `.not(.atom(.cmp(has, eq, status)))`) and its JQL equivalent
    /// `status IS NULL` (parses directly to
    /// `.atom(.cmp(has, ne, status))`) canonicalize to the SAME tree.
    /// Only single-predicate `NOT` wrappers fold; compound `NOT (a AND
    /// b)` stays as-is (best-effort — chips never emit that shape).
    static func canonicalPredicate(_ expr: LocalQueryEngine.SimpleDsl.BoolExpr) -> LocalQueryEngine.SimpleDsl.BoolExpr {
        if case .not(let inner) = expr,
           case .atom(.cmp(let key, let op, let value)) = inner,
           let flipped = flipEqNe(op) {
            return .atom(.cmp(key: key, op: flipped, value: value))
        }
        return expr
    }

    private static func flipEqNe(_ op: LocalQueryEngine.SimpleDsl.Op) -> LocalQueryEngine.SimpleDsl.Op? {
        switch op {
        case .eq: return .ne
        case .ne: return .eq
        default: return nil
        }
    }

    /// The top-level AND atoms of a parsed expression — `args` for
    /// `.and(args)` (including the empty list for the identity
    /// `.and([])`), or the expression itself as a single-element list for
    /// any other top-level shape (a lone predicate/`NOT`/`OR`/paren).
    static func topLevelAtoms(_ expr: LocalQueryEngine.SimpleDsl.BoolExpr) -> [LocalQueryEngine.SimpleDsl.BoolExpr] {
        if case .and(let args) = expr { return args }
        return [expr]
    }

    // MARK: - Top-level segments (byte spans for chip toggle-off)

    /// One top-level "unary" run in a token stream — a single predicate,
    /// possibly multi-token (`NOT is:heading`, `status IS NULL`,
    /// `deadline BETWEEN a AND b`) — with its source byte span.
    struct TopLevelSegment {
        let tokens: [LocalQueryEngine.SpannedDslToken]
        var start: Int { tokens.first?.start ?? 0 }
        var end: Int { tokens.last?.end ?? 0 }
    }

    private static let segmentKeywords = keywords

    /// Split a token stream into top-level unary segments — mirrors
    /// where `DslParser.parseAnd`/`parseUnary` (LocalQueryEngine.swift,
    /// private) would split, without re-implementing the full
    /// recursive-descent grammar: a new segment opens at a token that
    /// `peekStartsUnary` would accept (`(`, `-`, or a word that isn't
    /// `and`/`or`) arriving in KEY position, and stays open until the
    /// predicate's value completes. Best-effort: parenthesized groups are
    /// kept intact as one segment (not recursed into) — sufficient for
    /// the flat, unparenthesized clauses the chip registry emits.
    static func topLevelSegments(_ tokens: [LocalQueryEngine.SpannedDslToken]) -> [TopLevelSegment] {
        var state: ClassifyState = .key
        var pendingBetween = false
        var unaryOpen = false
        var segments: [TopLevelSegment] = []
        var current: [LocalQueryEngine.SpannedDslToken] = []

        func startsUnary(_ tok: LocalQueryEngine.DslToken) -> Bool {
            switch tok {
            case .lparen, .minus: return true
            case .word(let w):
                let l = w.lowercased()
                return l != "and" && l != "or"
            default: return false
            }
        }
        func closeCurrentIfNeeded() {
            if !current.isEmpty {
                segments.append(TopLevelSegment(tokens: current))
                current = []
            }
        }

        for sp in tokens {
            let openingNewUnary = state == .key && !unaryOpen && startsUnary(sp.tok)
            if openingNewUnary {
                closeCurrentIfNeeded()
                unaryOpen = true
            }

            switch sp.tok {
            case .lparen:
                current.append(sp)
            case .rparen:
                current.append(sp)
                state = .key
                unaryOpen = false
            case .comma:
                current.append(sp)
                state = .val
            case .colon, .op:
                current.append(sp)
                state = .val
            case .minus:
                // Prefix negation — state stays `.key` so the operand
                // right after it is still read as the predicate's key.
                current.append(sp)
            case .quoted:
                current.append(sp)
                state = .key
                unaryOpen = false
            case .word(let w):
                current.append(sp)
                let lower = w.lowercased()
                if segmentKeywords.contains(lower) {
                    switch lower {
                    case "order", "by":
                        state = .key
                    case "and", "or":
                        if pendingBetween {
                            pendingBetween = false
                            state = .val
                        } else {
                            state = .key
                            unaryOpen = false
                        }
                    case "in", "like":
                        state = .val
                    case "between":
                        state = .val
                        pendingBetween = true
                    case "is":
                        state = .val
                    case "null", "empty":
                        state = .key
                        unaryOpen = false
                    case "asc", "desc":
                        state = .key
                        unaryOpen = false
                    default:
                        break // "not" — leave state as-is.
                    }
                } else if state == .val {
                    state = .key
                    unaryOpen = false
                } else {
                    state = .op
                }
            }
        }
        closeCurrentIfNeeded()
        return segments
    }
}
