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

    enum CodingKeys: String, CodingKey {
        case id, name, dsl, order, builtin
        case displayMode = "display_mode"
        case displayGroupBy = "display_group_by"
        case displayShowDone = "display_show_done"
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
            displayShowDone: ffi.displayShowDone
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
        displayShowDone: Bool?
    ) {
        self.id = id
        self.name = name
        self.dsl = dsl
        self.order = order
        self.builtin = builtin
        self.displayMode = displayMode
        self.displayGroupBy = displayGroupBy
        self.displayShowDone = displayShowDone
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
            displayShowDone: displayShowDone
        )
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
    /// `kind:…` selector and a bare `ORDER BY` clause are valid queries
    /// with an empty predicate tree. Returns the error message, or nil
    /// when the DSL is saveable.
    static func dslValidationError(_ dsl: String) -> String? {
        let trimmed = dsl.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            return "Query must not be empty"
        }
        let parsed = LocalQueryEngine.parseSimpleDsl(trimmed)
        let lowered = trimmed.lowercased()
        let mentionsKind = lowered.contains("kind:")
        let mentionsOrderBy = lowered.contains("order by")
        if parsed.clauses.isEmpty && !mentionsKind && !mentionsOrderBy {
            return "No filters recognized in “\(trimmed)” — use key:value "
                + "filters like status:todo, tag:project, -has:scheduled"
        }
        return nil
    }

    /// Toggle a chip's DSL fragment in/out of the query string. The chip
    /// inserters are one-tap writers INTO the text (DSL-first editing,
    /// spec decision 2): if every whitespace token of `fragment` is
    /// already present the fragment's tokens are removed; otherwise the
    /// missing tokens are appended. Everything the user typed by hand is
    /// preserved verbatim (token-level edit, no re-canonicalization).
    static func toggleFragment(_ fragment: String, in dsl: String) -> String {
        let fragmentTokens = fragment.split(whereSeparator: \.isWhitespace).map(String.init)
        guard !fragmentTokens.isEmpty else { return dsl }
        var tokens = dsl.split(whereSeparator: \.isWhitespace).map(String.init)
        let allPresent = fragmentTokens.allSatisfy { tokens.contains($0) }
        if allPresent {
            for f in fragmentTokens {
                if let idx = tokens.firstIndex(of: f) {
                    tokens.remove(at: idx)
                }
            }
        } else {
            for f in fragmentTokens where !tokens.contains(f) {
                tokens.append(f)
            }
        }
        return tokens.joined(separator: " ")
    }

    /// Is the fragment currently active in the DSL (every token present)?
    /// Drives the inserter chip's active styling.
    static func fragmentActive(_ fragment: String, in dsl: String) -> Bool {
        let fragmentTokens = fragment.split(whereSeparator: \.isWhitespace).map(String.init)
        guard !fragmentTokens.isEmpty else { return false }
        let tokens = Set(dsl.split(whereSeparator: \.isWhitespace).map(String.init))
        return fragmentTokens.allSatisfy { tokens.contains($0) }
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
