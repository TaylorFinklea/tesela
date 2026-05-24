import Foundation

/// Inbox chip registry — Swift port of `web/src/lib/ambients/inbox/chips.ts`.
/// The chip toolbar is the user-facing bridge between simple toggles
/// (chips) and the underlying DSL string stored on the active saved
/// Query note.
///
/// Each chip declares the DSL fragment it represents and whether it's
/// on by default in a freshly-seeded Inbox. Toggling a chip rewrites
/// the DSL by composing the active fragments together; on load we
/// parse the DSL string back into a chip-state map so the toolbar
/// reflects what's actually saved.
///
/// Unknown clauses (DSL fragments not matched by any registered chip,
/// including everything in the new JQL grammar — `BETWEEN`, `LIKE`,
/// `IS NULL`, infix comparisons, `ORDER BY`, etc.) survive the round-
/// trip via `ChipState.unknownClauses` so chip-only edits never drop a
/// raw clause the user added by hand.

/// A toggleable filter exposed in the chip toolbar.
struct ChipDef: Hashable {
    /// Stable identifier for chip-state maps. Never user-visible.
    let id: String
    /// Short label rendered on the chip.
    let label: String
    /// Glyph rendered next to the label (emoji or single Unicode mark).
    let glyph: String
    /// Compact one-line explanation, used as accessibility hint / long-press preview.
    let hint: String
    /// DSL fragment(s) the chip contributes when active. Most chips are a
    /// single token. Order matters for round-tripping — keep canonical.
    let clauses: [String]
    /// Whether the chip is on by default in a freshly-seeded Inbox
    /// (matters only when no saved Query note exists yet — the seed
    /// DSL is computed from these defaults).
    let defaultOn: Bool
    /// Display category — controls grouping in the chip picker.
    /// Doesn't affect query semantics.
    let category: Category

    enum Category: String, Hashable {
        case scope
        case type
        case tags
        case dates
    }
}

/// The canonical chip set. Order matters: `dslFromChips` emits clauses
/// in this order so the round-trip is stable.
let chipRegistry: [ChipDef] = [
    // ── scope (what counts as a triage item) ────────────────────────
    ChipDef(
        id: "untriaged",
        label: "Untriaged",
        glyph: "📥",
        hint: "Only blocks without a status:: property",
        clauses: ["-has:status"],
        defaultOn: true,
        category: .scope
    ),
    ChipDef(
        id: "notHeading",
        label: "No headings",
        glyph: "🧱",
        hint: "Hide markdown section headings (### …) — they're dividers, not tasks",
        clauses: ["-is:heading"],
        defaultOn: true,
        category: .scope
    ),
    ChipDef(
        id: "notDailyPage",
        label: "No daily pages",
        glyph: "📅",
        hint: "Hide blocks on YYYY-MM-DD daily notes — journal captures aren't triage items",
        clauses: ["-on:daily-page"],
        defaultOn: true,
        category: .scope
    ),
    ChipDef(
        id: "notSystemPages",
        label: "No system pages",
        glyph: "⚙️",
        hint: "Hide blocks on Tag / Property / Query / Template pages",
        clauses: ["-on:system-pages"],
        defaultOn: true,
        category: .scope
    ),
    // ── dates (optional refinements; off by default) ─────────────────
    ChipDef(
        id: "hasScheduled",
        label: "Has scheduled",
        glyph: "🕒",
        hint: "Only blocks with a scheduled:: date",
        clauses: ["has:scheduled"],
        defaultOn: false,
        category: .dates
    ),
    ChipDef(
        id: "hasDeadline",
        label: "Has deadline",
        glyph: "⚑",
        hint: "Only blocks with a deadline:: date",
        clauses: ["has:deadline"],
        defaultOn: false,
        category: .dates
    ),
    // ── tags ─────────────────────────────────────────────────────────
    ChipDef(
        id: "untagged",
        label: "Untagged",
        glyph: "🏷️",
        hint: "Only blocks without any tags",
        clauses: ["-has:tag"],
        defaultOn: false,
        category: .tags
    ),
]

/// Live state of the chip toolbar — drives both rendering and DSL
/// composition. Fields beyond `active` capture the dynamic pieces of
/// the saved query: a multi-select Types group (composed into a single
/// `tag-in:Name1,Name2,…` clause) and per-row exclusion lists
/// (Hide-this-page / Hide-this-block, expressed as `-page:` /
/// `-block:` clauses). `unknownClauses` preserves everything else so
/// raw edits survive chip toggles.
struct ChipState: Equatable {
    var active: [String: Bool]
    var activeTypes: [String]
    var hiddenPages: [String]
    var hiddenBlocks: [String]
    var unknownClauses: [String]

    static func empty() -> ChipState {
        var active: [String: Bool] = [:]
        for chip in chipRegistry { active[chip.id] = false }
        return ChipState(
            active: active,
            activeTypes: [],
            hiddenPages: [],
            hiddenBlocks: [],
            unknownClauses: []
        )
    }
}

/// Whitespace-tokenize a DSL string. Mirrors the web's `tokenize` in
/// `chips.ts` — simple split, no awareness of quoted strings or paren
/// groups. JQL clauses with internal spaces (`status != done`, `type
/// IN (Task, Issue)`, `BETWEEN x AND y`) tokenize into multiple parts;
/// they all land in `unknownClauses` and round-trip verbatim.
private func tokenize(_ dsl: String) -> [String] {
    dsl.split(whereSeparator: { $0.isWhitespace }).map(String.init)
}

/// Parse a raw DSL string into chip state. Clauses owned by registered
/// chips flip those chips to active; everything else (except the
/// implicit `kind:block` baseline) goes into `unknownClauses` so the
/// UI can show it verbatim.
func chipsFromDsl(_ dsl: String) -> ChipState {
    let tokens = tokenize(dsl)
    var active: [String: Bool] = [:]
    for chip in chipRegistry { active[chip.id] = false }

    // Walk chip registry; a chip is active iff EVERY one of its
    // clauses appears in the token list. Remove claimed clauses from
    // `remaining` so they don't end up in `unknownClauses`.
    var remaining = Set(tokens)
    for chip in chipRegistry {
        if chip.clauses.allSatisfy({ remaining.contains($0) }) {
            active[chip.id] = true
            for c in chip.clauses { remaining.remove(c) }
        }
    }
    // Strip the implicit `kind:block` baseline from unknowns.
    remaining.remove("kind:block")

    // Pull out dynamic groups: tag-in:A,B,C → activeTypes; -page:X /
    // -block:X → exclusion lists.
    var activeTypes: [String] = []
    var hiddenPages: [String] = []
    var hiddenBlocks: [String] = []
    for tok in Array(remaining) {
        if tok.hasPrefix("tag-in:") {
            let raw = tok.dropFirst("tag-in:".count)
            let values = raw.split(separator: ",")
                .map { $0.trimmingCharacters(in: .whitespaces) }
                .filter { !$0.isEmpty }
            activeTypes.append(contentsOf: values)
            remaining.remove(tok)
        } else if tok.hasPrefix("-page:") {
            hiddenPages.append(String(tok.dropFirst("-page:".count)))
            remaining.remove(tok)
        } else if tok.hasPrefix("-block:") {
            hiddenBlocks.append(String(tok.dropFirst("-block:".count)))
            remaining.remove(tok)
        }
    }

    return ChipState(
        active: active,
        activeTypes: activeTypes,
        hiddenPages: hiddenPages,
        hiddenBlocks: hiddenBlocks,
        unknownClauses: Array(remaining)
    )
}

/// Build a DSL string from a chip state. Always prepends `kind:block`
/// (the Inbox is fundamentally a block query) and appends preserved
/// `unknownClauses` so a chip-only edit can't accidentally drop user-
/// authored raw clauses.
func dslFromChips(_ state: ChipState) -> String {
    var parts: [String] = ["kind:block"]
    for chip in chipRegistry where state.active[chip.id] == true {
        parts.append(contentsOf: chip.clauses)
    }
    if !state.activeTypes.isEmpty {
        parts.append("tag-in:\(state.activeTypes.joined(separator: ","))")
    }
    for p in state.hiddenPages { parts.append("-page:\(p)") }
    for b in state.hiddenBlocks { parts.append("-block:\(b)") }
    parts.append(contentsOf: state.unknownClauses)
    return parts.joined(separator: " ")
}

/// The default DSL for a freshly-seeded Inbox Query note. Derived from
/// the `defaultOn` flag of every chip in the registry so a change to
/// the registry automatically updates the seed.
func defaultInboxDsl() -> String {
    var active: [String: Bool] = [:]
    for chip in chipRegistry { active[chip.id] = chip.defaultOn }
    return dslFromChips(ChipState(
        active: active,
        activeTypes: [],
        hiddenPages: [],
        hiddenBlocks: [],
        unknownClauses: []
    ))
}
