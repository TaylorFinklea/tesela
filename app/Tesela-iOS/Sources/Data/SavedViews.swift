import Foundation

/// One saved view from the synced views registry (saved-views spec,
/// 2026-06-10). The app-side model behind the Inbox tab's view-switcher
/// chips: in `.relay` mode it mirrors the FFI `ViewRecord` (the registry
/// is a dedicated Loro doc read/written through the engine); in `.http`
/// mode it round-trips the server's `/views` JSON (snake_case fields,
/// `tesela_sync::ViewRecord`'s serde shape).
struct SavedView: Identifiable, Equatable, Hashable, Codable {
    /// Stable view id — UUID string for user views; the fixed
    /// `builtin-inbox` constant for the seeded Inbox.
    var id: String
    /// Display name ("Inbox", "This week", …).
    var name: String
    /// The query DSL string the view executes.
    var dsl: String
    /// Sort position in the view switcher (ties break by id).
    var order: Int64
    /// Built-ins are editable but never deletable.
    var builtin: Bool
    /// Result rendering: "list" | "table" | "kanban". iOS renders every
    /// mode as a list; the preference is stored for web.
    var displayMode: String
    /// Optional grouping key (kanban columns / table groups). Not edited
    /// on iOS — preserved through updates.
    var displayGroupBy: String?
    /// Optional "include done items" toggle. Not edited on iOS —
    /// preserved through updates.
    var displayShowDone: Bool?
    /// tesela-ya4.4 — table column display config (hide/reorder/sort).
    /// iOS STORES this (round-trips it through both the FFI `.relay` path
    /// and the `.http` JSON path) but does NOT render it — the native
    /// table view is a later bead (ya4.6).
    var displayTableConfig: SavedViewTableConfig?

    enum CodingKeys: String, CodingKey {
        case id, name, dsl, order, builtin
        case displayMode = "display_mode"
        case displayGroupBy = "display_group_by"
        case displayShowDone = "display_show_done"
        case displayTableConfig = "display_table_config"
    }

    /// The engine's fixed id for the seeded builtin Inbox
    /// (`tesela_sync::INBOX_VIEW_ID`).
    static let builtinInboxId = "builtin-inbox"

    /// Offline/mock stand-in for the seeded builtin Inbox. Mirrors
    /// `tesela_core::query::INBOX_VIEW_DSL` verbatim so a device that
    /// can't reach its registry (mock mode, unwired seam, HTTP failure)
    /// still triages against the canonical default.
    static let fallbackInbox = SavedView(
        id: builtinInboxId,
        name: "Inbox",
        dsl: "status:backlog,todo -has:scheduled -has:deadline",
        order: 0,
        builtin: true,
        displayMode: "list",
        displayGroupBy: nil,
        displayShowDone: nil
    )

    /// Bridge from the FFI record (the `.relay` read path).
    init(ffi: ViewRecord) {
        self.init(
            id: ffi.id,
            name: ffi.name,
            dsl: ffi.dsl,
            order: ffi.order,
            builtin: ffi.builtin,
            displayMode: ffi.displayMode,
            displayGroupBy: ffi.displayGroupBy,
            displayShowDone: ffi.displayShowDone,
            displayTableConfig: ffi.displayTableConfig.map(SavedViewTableConfig.init(ffi:))
        )
    }

    init(
        id: String,
        name: String,
        dsl: String,
        order: Int64,
        builtin: Bool,
        displayMode: String,
        displayGroupBy: String?,
        displayShowDone: Bool?,
        displayTableConfig: SavedViewTableConfig? = nil
    ) {
        self.id = id
        self.name = name
        self.dsl = dsl
        self.order = order
        self.builtin = builtin
        self.displayMode = displayMode
        self.displayGroupBy = displayGroupBy
        self.displayShowDone = displayShowDone
        self.displayTableConfig = displayTableConfig
    }

    /// Bridge to the FFI record (the `.relay` write path).
    var ffiRecord: ViewRecord {
        ViewRecord(
            id: id,
            name: name,
            dsl: dsl,
            order: order,
            builtin: builtin,
            displayMode: displayMode,
            displayGroupBy: displayGroupBy,
            displayShowDone: displayShowDone,
            displayTableConfig: displayTableConfig?.ffiRecord
        )
    }
}

/// tesela-ya4.4 — table column display config (hide/reorder/sort), the
/// app-side mirror of `tesela_sync::TableColumnConfig` / the FFI
/// `TableColumnConfig` record. Same dual-mode contract as `SavedView`
/// itself: Codable for the `.http` JSON path (snake_case keys matching
/// the server's serde shape), and bridges to/from the FFI record for the
/// `.relay` path. iOS stores this opaquely — it is never read for
/// rendering (the native table view is ya4.6).
struct SavedViewTableConfig: Equatable, Hashable, Codable {
    var hidden: [String]
    var order: [String]
    var sortBy: String?
    var sortDir: String?

    enum CodingKeys: String, CodingKey {
        case hidden, order
        case sortBy = "sort_by"
        case sortDir = "sort_dir"
    }

    init(ffi: TableColumnConfig) {
        self.init(hidden: ffi.hidden, order: ffi.order, sortBy: ffi.sortBy, sortDir: ffi.sortDir)
    }

    init(hidden: [String], order: [String], sortBy: String?, sortDir: String?) {
        self.hidden = hidden
        self.order = order
        self.sortBy = sortBy
        self.sortDir = sortDir
    }

    /// Bridge to the FFI record (the `.relay` write path).
    var ffiRecord: TableColumnConfig {
        TableColumnConfig(hidden: hidden, order: order, sortBy: sortBy, sortDir: sortDir)
    }
}

/// Pure helpers for the Views surface — validation, fragment insertion,
/// and selection persistence. Kept off the service/view so they unit-test
/// without a backend.
enum SavedViewLogic {
    /// Validate a view's query DSL before save — the iOS mirror of the
    /// server's `validate_dsl` (routes/views.rs): the parser is
    /// deliberately TOTAL (liberal, drops unrecognized syntax), so
    /// "unparseable" means a NON-EMPTY input from which the parser
    /// recognized ZERO predicates — saving it would silently create a
    /// match-everything view. Carve-outs (server parity): a lone
    /// `kind:…` selector, and an `ORDER BY` that ACTUALLY parsed a sort
    /// field (`parsed.sort != nil`, mirroring the server's
    /// `parsed.sort.is_none()` gate) — STRUCTURAL, never a substring:
    /// in `.relay` mode this check is the only gate (the engine's
    /// views_upsert doesn't validate DSL), and a substring would let
    /// "reorder bytes" persist a match-everything view fleet-wide.
    ///
    /// tesela-vp9.5: when the query IS rejected, the message prefers the
    /// FIRST `QueryDiagnostic`'s hint + dropped snippet (real, span-
    /// located feedback from `parseSimpleDslWithDiagnostics`) over the
    /// old one-size-fits-all copy; the generic fallback only fires when
    /// there are no diagnostics at all (e.g. a token stream that tokenizes
    /// to nothing, like stray punctuation). A query that recognizes at
    /// least one real predicate is still saveable even if it also has
    /// dropped garbage elsewhere — diagnostics only change the MESSAGE
    /// shown when already-invalid, never make a previously-valid query
    /// invalid.
    /// Returns the error message, or nil when the DSL is saveable.
    static func dslValidationError(_ dsl: String) -> String? {
        let trimmed = dsl.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            return "Query must not be empty"
        }
        let (parsed, diagnostics) = LocalQueryEngine.parseSimpleDslWithDiagnostics(trimmed)
        let mentionsKind = trimmed.lowercased().contains("kind:")
        guard LocalQueryEngine.isEmptyExpr(parsed.expr) && parsed.sort == nil && !mentionsKind else {
            return nil
        }
        if let first = diagnostics.first {
            return "\(first.hint) — near “\(first.got)”"
        }
        return "No filters recognized in “\(trimmed)” — use key:value "
            + "filters like status:todo, tag:project, -has:scheduled"
    }

    /// Toggle a chip's JQL predicate clause in/out of the query string.
    /// The chip inserters are one-tap writers INTO the text (DSL-first
    /// editing, spec decision 2). tesela-vp9.5: parse-aware — "present"
    /// means the clause's parsed predicate (canonicalized, see
    /// `QueryAuthoring.canonicalPredicate`) structurally equals one of
    /// `dsl`'s top-level AND atoms, so a multi-token clause
    /// (`status IS NULL`) is matched as ONE unit, and an equivalent
    /// legacy phrasing (`-has:status`) the user typed by hand still
    /// registers as active. Toggle-off removes the matching top-level
    /// segment via its token span (`QueryAuthoring.topLevelSegments`);
    /// toggle-on appends the clause space-separated (implicit AND).
    /// Everything else in the string survives verbatim.
    static func toggleFragment(_ jqlClause: String, in dsl: String) -> String {
        let target = QueryAuthoring.canonicalPredicate(LocalQueryEngine.parseSimpleDsl(jqlClause).expr)
        let (tokens, bytes) = LocalQueryEngine.tokenizeDsl(dsl)
        let segments = QueryAuthoring.topLevelSegments(tokens)

        for seg in segments {
            let segText = String(decoding: bytes[seg.start..<seg.end], as: UTF8.self)
            let segExpr = LocalQueryEngine.parseSimpleDsl(segText).expr
            if QueryAuthoring.canonicalPredicate(segExpr) == target {
                let before = String(decoding: bytes[0..<seg.start], as: UTF8.self)
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                let after = String(decoding: bytes[seg.end...], as: UTF8.self)
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                return [before, after].filter { !$0.isEmpty }.joined(separator: " ")
            }
        }

        // Not present (or present only in a shape this best-effort
        // segmenter can't isolate, e.g. inside an OR/paren group — left
        // untouched rather than risk corrupting a hand-written query).
        if fragmentActive(jqlClause, in: dsl) { return dsl }
        let trimmed = dsl.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? jqlClause : trimmed + " " + jqlClause
    }

    /// Is the chip's JQL predicate clause currently active in the DSL —
    /// present (in canonical form) among `dsl`'s top-level AND atoms?
    /// Drives the inserter chip's active styling. tesela-vp9.5:
    /// parse-aware, replacing the old whitespace-token membership check
    /// (which false-negatived on any multi-token JQL clause and
    /// false-positived when a clause's individual words happened to
    /// appear elsewhere in the string).
    static func fragmentActive(_ jqlClause: String, in dsl: String) -> Bool {
        let target = QueryAuthoring.canonicalPredicate(LocalQueryEngine.parseSimpleDsl(jqlClause).expr)
        let atoms = QueryAuthoring.topLevelAtoms(LocalQueryEngine.parseSimpleDsl(dsl).expr)
        return atoms.contains { QueryAuthoring.canonicalPredicate($0) == target }
    }

    /// UserDefaults key for the persisted active-view selection, scoped
    /// per backend (mirroring how the relay cursors scope per identity —
    /// `RelayTicker.inboundCursorKey`) so switching mosaics/backends
    /// doesn't bleed one registry's selection into another's.
    static func selectionKey(scope: String) -> String {
        "tesela.views.activeViewId.\(scope)"
    }

    /// Backend identity for selection scoping: the mode for mock/relay,
    /// mode + server URL for HTTP (two different Macs are two different
    /// registries).
    static func selectionScope(mode: String, serverURL: String) -> String {
        mode == "http" ? "http|\(serverURL)" : mode
    }

    /// Resolve which view should be active: the persisted id when it
    /// still exists, else the builtin Inbox (the spec's default
    /// selection), else the first view. `nil` only for an empty list
    /// (which `fetchViews` never returns — it falls back to the builtin).
    static func resolveSelection(views: [SavedView], persisted: String?) -> String? {
        if let persisted, views.contains(where: { $0.id == persisted }) {
            return persisted
        }
        if views.contains(where: { $0.id == SavedView.builtinInboxId }) {
            return SavedView.builtinInboxId
        }
        return views.first?.id
    }

    /// Deterministic switcher order: `(order, id)` — the engine's and the
    /// server's sort, re-applied client-side after local upserts.
    static func sorted(_ views: [SavedView]) -> [SavedView] {
        views.sorted { ($0.order, $0.id) < ($1.order, $1.id) }
    }
}
