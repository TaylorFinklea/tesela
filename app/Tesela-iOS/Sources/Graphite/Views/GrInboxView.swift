import SwiftUI

/// Graphite Inbox — now the SAVED-VIEWS surface (saved-views spec,
/// 2026-06-10): the tab is a view switcher whose chips are the synced
/// views registry, with the builtin Inbox as the default selection. The
/// selected chip's DSL drives the same query path as before
/// (`MockMosaicService.executeQuery(_:)` — `LocalQueryEngine` over the
/// relay-synced sandbox in `.relay`, `POST /search/query` in `.http`),
/// and triage actions still write `status::` via `setBlockProperty`.
///
/// Registry reads: `.relay` → the engine's views doc through the
/// shell-wired seam (re-read on `refreshTick`, which the relay tick bumps
/// when the registry doc syncs in); `.http` → `GET /views` (re-read on
/// `viewsTick`, bumped by the `views_changed` WS event). Selection
/// persists in UserDefaults, scoped per backend
/// (`SavedViewLogic.selectionKey`).
///
/// Long-press a chip for edit / move / delete (builtins hide delete);
/// the "+" chip opens the DSL-first editor (`GrViewEditorSheet`).
struct GrInboxView: View {
    @ObservedObject var mosaic: MockMosaicService
    var backend: BackendSettings? = nil

    @Environment(\.theme) private var theme

    @State private var views: [SavedView] = []
    @State private var activeViewId: String = SavedView.builtinInboxId
    @State private var rows: [QueryItem] = []
    @State private var loading = false
    @State private var navigationPath = NavigationPath()
    @State private var editorTarget: EditorTarget? = nil

    /// Soft cap mirroring the web's `ROW_CAP` so a legacy mosaic with
    /// thousands of matching blocks doesn't choke the renderer.
    private let rowCap = 200

    private struct EditorTarget: Identifiable {
        let id: String
        /// nil = creating a new view.
        let view: SavedView?
    }

    var body: some View {
        NavigationStack(path: $navigationPath) {
            VStack(spacing: 0) {
                GrHeader(title: activeView?.name ?? "Views", subtitle: subtitle)
                viewChipBar
                content
            }
            .background(theme.bg)
            .navigationDestination(for: GrPageRoute.self) { route in
                GrPageView(slug: route.slug, mosaic: mosaic, path: $navigationPath)
                    .environment(\.theme, theme)
            }
        }
        .task { await load() }
        // Re-query when a refresh pass lands (relay tick / WS event) —
        // same signal that freshens Daily. The relay tick also carries
        // the views REGISTRY doc, so this re-reads the switcher too.
        .onChange(of: mosaic.refreshTick) { _, _ in Task { await load() } }
        // `.http` registry changes arrive as the `views_changed` WS
        // event → `viewsTick` (no note refresh involved).
        .onChange(of: mosaic.viewsTick) { _, _ in Task { await load() } }
        .sheet(item: $editorTarget) { target in
            GrViewEditorSheet(
                existing: target.view,
                siblings: views,
                onSave: { record, isNew in
                    mosaic.enqueueBackendMutation { reservation in
                        try await saveView(
                            record,
                            isNew: isNew,
                            reservation: reservation
                        )
                    }
                },
                onDelete: { id in
                    mosaic.enqueueBackendMutation { reservation in
                        try await deleteView(id: id, reservation: reservation)
                    }
                },
                propertyRegistry: mosaic.propertyRegistry
            )
            .environment(\.theme, theme)
            .preferredColorScheme(.dark)
        }
    }

    private var activeView: SavedView? {
        views.first(where: { $0.id == activeViewId })
    }

    private var isInboxActive: Bool {
        activeViewId == SavedView.builtinInboxId
    }

    private var subtitle: String {
        if rows.isEmpty {
            return isInboxActive ? "TRIAGE" : "SAVED VIEW"
        }
        if isInboxActive {
            return "\(headerCount) unsorted"
        }
        return rows.count == 1 ? "1 result" : "\(headerCount) results"
    }

    private var headerCount: String {
        rows.count >= rowCap ? "\(rowCap)+" : "\(rows.count)"
    }

    /// UserDefaults key for the persisted selection, scoped per backend
    /// so switching mosaics doesn't bleed selections across registries.
    private var selectionKey: String {
        SavedViewLogic.selectionKey(
            scope: SavedViewLogic.selectionScope(
                mode: backend?.mode.rawValue ?? "mock",
                serverURL: backend?.serverURL ?? ""
            )
        )
    }

    // ── View switcher (.gr-chipbar over the views registry) ─────────────

    private var viewChipBar: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 7) {
                ForEach(views) { view in
                    savedViewChip(view)
                }
                newViewChip
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 11)
        }
        .scrollClipDisabled()
        .overlay(alignment: .bottom) {
            Rectangle().fill(theme.lineSoft).frame(height: 1)
        }
    }

    /// One saved-view chip with its context menu. Extracted to keep
    /// the chip bar's view-builder shallow and to give the compiler a
    /// narrower expression to type-check.
    @ViewBuilder
    private func savedViewChip(_ view: SavedView) -> some View {
        let isSelected = view.id == activeViewId
        GrChip(
            label: view.name,
            active: isSelected,
            action: { select(view.id) },
            accessibilityLabelOverride: a11ySavedViewLabel(view, isSelected: isSelected),
            accessibilityHint: "Double-tap to switch to this view. Long-press for edit, move, or delete.",
            accessibilityIdentifier: "gr-saved-view-\(view.id)"
        )
        .accessibilityAddTraits(isSelected ? .isSelected : [])
        .contextMenu {
            Button {
                editorTarget = EditorTarget(id: view.id, view: view)
            } label: {
                Label("Edit view", systemImage: "pencil")
            }
            Button {
                move(view.id, by: -1)
            } label: {
                Label("Move left", systemImage: "arrow.left")
            }
            .disabled(views.first?.id == view.id)
            Button {
                move(view.id, by: 1)
            } label: {
                Label("Move right", systemImage: "arrow.right")
            }
            .disabled(views.last?.id == view.id)
            if !view.builtin {
                Button(role: .destructive) {
                    mosaic.enqueueBackendMutation { reservation in
                        try? await deleteView(id: view.id, reservation: reservation)
                    }
                } label: {
                    Label("Delete view", systemImage: "trash")
                }
            }
        }
    }

    /// The "+ New" chip that opens the view editor. Has no context
    /// menu (it can only create, never reorder/delete).
    private var newViewChip: some View {
        GrChip(
            label: "+ New",
            action: { editorTarget = EditorTarget(id: "new", view: nil) },
            accessibilityLabelOverride: "New saved view",
            accessibilityHint: "Opens the editor to create a new saved view.",
            accessibilityIdentifier: "gr-new-view"
        )
    }

    /// VoiceOver label for a saved-view chip. "Selected" is appended
    /// when the chip is the active view, so users hear the same state
    /// the visual styling conveys.
    private func a11ySavedViewLabel(_ view: SavedView, isSelected: Bool) -> String {
        let builtinPrefix = view.builtin ? "Built-in saved view: " : "Saved view: "
        return builtinPrefix + view.name + (isSelected ? ", selected" : "")
    }

    // ── Content ─────────────────────────────────────────────────────────

    @ViewBuilder
    private var content: some View {
        if loading && rows.isEmpty {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if rows.isEmpty {
            emptyState
        } else if activeView?.displayMode == "kanban" {
            kanbanContent
        } else if activeView?.displayMode == "table" {
            tableContent
        } else {
            List {
                if let mode = activeView?.displayMode, mode != "list" {
                    displayModeNote(mode)
                        .listRowInsets(EdgeInsets(top: 6, leading: 18, bottom: 2, trailing: 18))
                        .listRowBackground(Color.clear)
                        .listRowSeparator(.hidden)
                }
                Section {
                    ForEach(rows) { row in
                        inboxCard(row)
                            .listRowInsets(EdgeInsets(top: 4, leading: 14, bottom: 4, trailing: 14))
                            .listRowBackground(Color.clear)
                            .listRowSeparator(.hidden)
                            .swipeActions(edge: .leading, allowsFullSwipe: false) {
                                Button {
                                    triage(row, status: "todo")
                                } label: {
                                    Label("Todo", systemImage: "circle")
                                }
                                .tint(theme.fgMuted)
                                Button {
                                    triage(row, status: "doing")
                                } label: {
                                    Label("Doing", systemImage: "circle.lefthalf.filled")
                                }
                                .tint(theme.accentPrimary)
                            }
                            .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                                Button {
                                    triage(row, status: "done")
                                } label: {
                                    Label("Done", systemImage: "checkmark.circle.fill")
                                }
                                .tint(.green)
                            }
                    }
                }
            }
            .listStyle(.plain)
            .scrollContentBackground(.hidden)
            .background(theme.bg)
            .refreshable { await load() }
        }
    }

    @ViewBuilder
    private var emptyState: some View {
        if isInboxActive {
            ContentUnavailableView {
                Label("Views clear", systemImage: "checkmark.circle")
            } description: {
                Text("Nothing matches the Views query right now.")
            }
            .background(theme.bg)
        } else {
            ContentUnavailableView {
                Label("No matches", systemImage: "line.3.horizontal.decrease.circle")
            } description: {
                Text("Nothing matches “\(activeView?.dsl ?? "")”.")
            }
            .background(theme.bg)
        }
    }

    /// Honest note for a display mode with no native iOS render path.
    /// Kanban (`kanbanContent`) and table (`tableContent`) each have their
    /// own native surface and never reach this branch — it's a defensive
    /// fallback for any future/unrecognized `display_mode` value, so an
    /// unexpected mode still renders SOMETHING honest rather than silently
    /// looking like plain "list".
    private func displayModeNote(_ mode: String) -> some View {
        Text("\(mode) view — shown as a list on iOS; full layout on web")
            .font(.system(size: 10.5, design: .monospaced))
            .foregroundStyle(theme.fgFaint)
    }

    // ── Kanban (tesela-ya4.5) ────────────────────────────────────────────

    /// First positive `tag:X` filter in the active view's DSL — drives the
    /// tag-scoped vs. data-derived group-by candidate resolution (decision
    /// 3c / `KanbanLogic.candidateProperties`).
    private var kanbanTagName: String? {
        // Parens matter: `activeView?.dsl.flatMap(...)` optional-chains the
        // WHOLE `dsl.flatMap(...)` expression, so `dsl` inside it resolves
        // as a plain (non-optional) `String` and `.flatMap` picks the
        // `Sequence` overload (over `Character`) instead of `Optional`'s.
        (activeView?.dsl).flatMap(KanbanLogic.inferredTag(fromDsl:))
    }

    private var kanbanCandidates: [PropertyDef] {
        KanbanLogic.candidateProperties(tagName: kanbanTagName, items: rows, registry: mosaic.propertyRegistry)
    }

    private func kanbanResolveDef(_ name: String) -> PropertyDef? {
        KanbanLogic.resolveDef(name, tagName: kanbanTagName, registry: mosaic.propertyRegistry)
    }

    /// The resolved group-by property name — spec decision 3, iOS subset
    /// (a → c → honest nil; GrInboxView is always a saved-view context, so
    /// (b)'s per-surface pref never applies here).
    private var kanbanGroupByName: String? {
        KanbanLogic.resolveGroupBy(
            displayGroupBy: activeView?.displayGroupBy,
            candidates: kanbanCandidates,
            resolveDef: kanbanResolveDef
        )
    }

    private var kanbanGroupByDef: PropertyDef? {
        kanbanGroupByName.flatMap(kanbanResolveDef)
    }

    @ViewBuilder
    private var kanbanContent: some View {
        if let def = kanbanGroupByDef, let name = kanbanGroupByName {
            GrKanbanBoard(
                items: rows,
                groupByDef: def,
                groupByProp: name,
                onOpen: { item in
                    guard !item.page_id.isEmpty else { return }
                    navigationPath.append(GrPageRoute(slug: item.page_id))
                },
                onMove: { item, value in
                    moveCard(item, groupByProp: name, value: value)
                }
            )
        } else {
            // Decision 3(d) — honest empty state. Never silently fall back
            // to the list under a kanban toggle.
            VStack {
                Spacer()
                Text(
                    "No groupable select property found for this view. Add a "
                    + "select property with choices, or set a group-by on this view."
                )
                .font(.system(size: 12.5))
                .foregroundStyle(theme.fgMuted)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
                Spacer()
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(theme.bg)
        }
    }

    /// Move a card to another kanban column: writes `groupByProp` via the
    /// same block-granular `setBlockProperty` triage already uses, then
    /// re-runs the active query so the board reflects the new column.
    /// `.relay` already bumps `refreshTick` internally on a successful
    /// write, but `.http` has no equivalent push — re-querying explicitly
    /// covers both backends uniformly.
    private func moveCard(_ item: QueryItem, groupByProp: String, value: String) {
        guard let bid = item.block_id else { return }
        mosaic.enqueueBackendMutation { reservation in
            do {
                try await mosaic.setBlockProperty(
                    blockId: bid,
                    key: groupByProp.lowercased(),
                    value: value,
                    reservation: reservation
                )
                await runActiveQuery()
            } catch {
                // Silent — the sheet already dismissed; the next refresh reconciles.
            }
        }
    }

    // ── Table (tesela-ya4.6) ─────────────────────────────────────────────

    /// Resolved columns before the stored config's hide/reorder is applied
    /// — `kanbanTagName` (the DSL's first positive `tag:X`) drives the same
    /// tag-scoped-vs-data-derived resolution `TableLogic.resolveColumns`
    /// mirrors from web `resolveTableColumns`.
    private var tableResolvedColumns: [PropertyDef] {
        TableLogic.resolveColumns(tagName: kanbanTagName, items: rows, registry: mosaic.propertyRegistry)
    }

    /// Final visible/ordered columns — hidden columns dropped, `order`
    /// override applied (`TableLogic.applyConfig`, mirror of web
    /// `applyTableConfig`). Honors the saved view's stored
    /// `displayTableConfig` when present; a nil config renders every
    /// resolved column in its natural order.
    private var tableColumns: [PropertyDef] {
        TableLogic.applyConfig(tableResolvedColumns, config: activeView?.displayTableConfig)
    }

    /// The stored sort (`displayTableConfig.sortBy`/`sortDir`), seeded as
    /// `GrTableView`'s INITIAL local sort — only when it names a column
    /// that actually resolves, mirroring the web table's "a sort pinned to
    /// a since-hidden/removed column has no effect" posture (looked up
    /// against `tableResolvedColumns`, not the filtered `tableColumns`).
    private var tableStoredSort: (column: String, direction: TableSortDirection)? {
        guard let config = activeView?.displayTableConfig,
              let sortBy = config.sortBy, !sortBy.isEmpty,
              let dirRaw = config.sortDir, let dir = TableSortDirection(rawValue: dirRaw),
              tableResolvedColumns.contains(where: { $0.name == sortBy }) else {
            return nil
        }
        return (sortBy, dir)
    }

    @ViewBuilder
    private var tableContent: some View {
        if tableColumns.isEmpty {
            // Mirror kanban's decision-3(d) honesty — never silently render
            // a table with zero columns; say why instead.
            VStack {
                Spacer()
                Text(
                    "No columns resolved for this view. Add properties to the "
                    + "scoped type, or make sure the query's blocks carry some."
                )
                .font(.system(size: 12.5))
                .foregroundStyle(theme.fgMuted)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
                Spacer()
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(theme.bg)
        } else {
            GrTableView(
                items: rows,
                columns: tableColumns,
                storedSort: tableStoredSort,
                onOpen: { item in
                    guard !item.page_id.isEmpty else { return }
                    navigationPath.append(GrPageRoute(slug: item.page_id))
                }
            )
        }
    }

    // ── Card (.grm-icard) ───────────────────────────────────────────────

    private func inboxCard(_ row: QueryItem) -> some View {
        HStack(alignment: .top, spacing: 12) {
            if isTaskRow(row) {
                TaskStatusMarker(
                    status: row.properties["status"],
                    priority: row.properties["priority"],
                    size: 18
                ) {
                    let next = row.properties["status"] == "done" ? "todo" : "done"
                    triage(row, status: next)
                }
                .frame(width: 30, alignment: .center)
                .contentShape(Rectangle().inset(by: -8))
                .padding(.top, 1)
            } else {
                ZStack {
                    RoundedRectangle(cornerRadius: 8)
                        .fill(theme.bg4)
                        .frame(width: 30, height: 30)
                    GrIcon(name: sourceIcon(row), size: 15)
                        .foregroundStyle(theme.fgSubtle)
                }
            }
            VStack(alignment: .leading, spacing: 7) {
                Text(row.text.isEmpty ? "(empty block)" : row.text)
                    .font(.system(size: 14))
                    .foregroundStyle(theme.fgDefault)
                    .lineSpacing(2)
                    .multilineTextAlignment(.leading)
                HStack(spacing: 8) {
                    metaPill("in \(row.title.isEmpty ? row.page_id : row.title)")
                    if let tag = row.primary_tag {
                        metaPill("#\(tag)")
                    }
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 13)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(theme.bg2)
        .overlay(
            RoundedRectangle(cornerRadius: 11)
                .stroke(theme.lineSoft, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 11))
        .contentShape(Rectangle())
        .onTapGesture {
            guard !row.page_id.isEmpty else { return }
            navigationPath.append(GrPageRoute(slug: row.page_id))
        }
    }

    private func metaPill(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 10.5, design: .monospaced))
            .foregroundStyle(theme.fgSubtle)
            .lineLimit(1)
            .truncationMode(.middle)
            .padding(.horizontal, 7)
            .padding(.vertical, 2)
            .background(theme.bg4)
            .clipShape(RoundedRectangle(cornerRadius: 5))
    }

    private func sourceIcon(_ row: QueryItem) -> String {
        switch row.page_note_type?.lowercased() {
        case "daily": return "calendar"
        case "project": return "folder"
        case "person": return "user"
        default: return "file-text"
        }
    }

    /// A row is a task iff it carries a `status::` property or a `tags::`
    /// value containing "task" — same rule as the agenda/LocalQueryEngine.
    /// Task rows get the shared status marker instead of the source icon.
    private func isTaskRow(_ row: QueryItem) -> Bool {
        if row.properties["status"] != nil { return true }
        return (row.properties["tags"] ?? "")
            .split(separator: ",")
            .contains { $0.trimmingCharacters(in: .whitespaces).lowercased() == "task" }
    }

    // ── Data load + actions ─────────────────────────────────────────────

    private func load() async {
        loading = true
        defer { loading = false }
        let fetched = await mosaic.fetchViews()
        views = fetched
        let persisted = UserDefaults.standard.string(forKey: selectionKey)
        activeViewId = SavedViewLogic.resolveSelection(views: fetched, persisted: persisted)
            ?? SavedView.builtinInboxId
        await runActiveQuery()
    }

    private func runActiveQuery() async {
        let dsl = activeView?.dsl ?? SavedView.fallbackInbox.dsl
        let result = await mosaic.executeQuery(dsl)
        var collected: [QueryItem] = []
        outer: for g in result.groups {
            for item in g.items where item.kind == .block {
                collected.append(item)
                if collected.count >= rowCap { break outer }
            }
        }
        rows = collected
    }

    private func select(_ id: String) {
        activeViewId = id
        UserDefaults.standard.set(id, forKey: selectionKey)
        Task { await runActiveQuery() }
    }

    private func saveView(
        _ record: SavedView,
        isNew: Bool,
        reservation: MockMosaicService.BackendMutationReservation
    ) async throws {
        try await mosaic.saveView(
            record,
            isNew: isNew,
            reservation: reservation
        )
        await load()
        if isNew {
            select(record.id)
        }
    }

    private func deleteView(
        id: String,
        reservation: MockMosaicService.BackendMutationReservation
    ) async throws {
        try await mosaic.deleteView(id: id, reservation: reservation)
        if activeViewId == id {
            UserDefaults.standard.removeObject(forKey: selectionKey)
        }
        await load()
    }

    /// Move a chip one slot left/right and persist the new order
    /// (`POST /views/reorder` in `.http`; order upserts through the
    /// engine seam in `.relay`).
    private func move(_ id: String, by delta: Int) {
        guard let idx = views.firstIndex(where: { $0.id == id }) else { return }
        let target = idx + delta
        guard views.indices.contains(target) else { return }
        var reordered = views
        reordered.swapAt(idx, target)
        views = reordered  // optimistic; load() below re-syncs
        mosaic.enqueueBackendMutation { reservation in
            try? await mosaic.reorderViews(
                reordered,
                reservation: reservation
            )
            await load()
        }
    }

    private func triage(_ row: QueryItem, status: String) {
        guard let bid = row.block_id else { return }
        mosaic.enqueueBackendMutation { reservation in
            do {
                try await mosaic.setBlockProperty(
                    blockId: bid,
                    key: "status",
                    value: status,
                    reservation: reservation
                )
                // Optimistically drop the row — the user is ripping through
                // the list and wants immediate feedback (mirrors InboxView).
                rows.removeAll { $0.id == row.id }
            } catch {
                // Silent — refresh recovers on next load.
            }
        }
    }
}
