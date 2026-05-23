import SwiftUI

/// Inbox — the triage surface. Lists every block across the mosaic
/// that doesn't yet carry a `status::` property, drives the GTD-style
/// flow of ripping through untriaged captures with single-key actions.
///
/// Mirrors the web v5 Inbox ambient and the v4 sidebar widget — same
/// `kind:block -has:status` query, same `isInboxable` post-filter
/// (drops daily-page blocks and system page types: Tag, Property,
/// Query, Template). Triaging a row to any status (todo / doing /
/// done) makes it drop out of the inbox.
///
/// iOS triage idioms:
///   - tap row → open source page focused on the block
///   - leading swipe → `todo` / `doing`
///   - trailing swipe → `done` (a.k.a. "archive from inbox")
struct InboxView: View {
    @ObservedObject var mosaic: MockMosaicService
    @ObservedObject var backend: BackendSettings
    var appearance: AppearanceController? = nil
    var syncState: SyncState? = nil
    var transcription: TranscriptionStore? = nil

    @Environment(\.theme) private var theme
    @EnvironmentObject private var mosaicRegistry: MosaicRegistry
    @State private var showSettings: Bool = false
    @State private var showMosaicSwitcher: Bool = false
    @State private var rows: [QueryItem] = []
    @State private var loading = false
    @State private var navigationPath = NavigationPath()

    /// Soft cap so a legacy mosaic with thousands of untriaged blocks
    /// doesn't choke the renderer on first open. Mirrors the web's
    /// `ROW_CAP`. Future virtualization can lift this.
    private let rowCap = 200

    var body: some View {
        NavigationStack(path: $navigationPath) {
            VStack(spacing: 0) {
                TabHeader(
                    title: "Inbox",
                    syncStatus: TabHeader.SyncDotState(mosaic.connection),
                    onTapSettings: { showSettings = true },
                    onTapMosaic: { showMosaicSwitcher = true }
                )
                ConnectionBanner(connection: mosaic.connection) {
                    Task { await mosaic.refresh(from: backend.backend) }
                }
                content
                    .refreshable { await load() }
            }
            .background(theme.bg)
            .navigationDestination(for: DailyPageRoute.self) { route in
                if let page = route.resolvedPage(mosaic) {
                    PageViewWrapper(
                        page: page,
                        mosaic: mosaic,
                        syncState: SyncState()
                    )
                    .environment(\.theme, theme)
                } else {
                    placeholderPage(slug: route.slug)
                }
            }
            .sheet(isPresented: $showMosaicSwitcher) {
                MosaicSwitcherSheet(registry: mosaicRegistry)
                    .environment(\.theme, theme)
            }
            .sheet(isPresented: $showSettings) {
                if let appearance, let syncState {
                    SettingsView(
                        appearance: appearance,
                        mosaic: mosaic,
                        syncState: syncState,
                        backend: backend,
                        transcription: transcription
                    )
                    .environment(\.theme, theme)
                    .environment(\.density, appearance.density)
                } else {
                    Text("Settings unavailable.")
                        .padding()
                }
            }
        }
        .task { await load() }
    }

    // MARK: - Content

    @ViewBuilder
    private var content: some View {
        if loading && rows.isEmpty {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(theme.bg)
        } else if rows.isEmpty {
            ContentUnavailableView {
                Label("Inbox clear", systemImage: "checkmark.circle")
            } description: {
                Text("Every block has a status — nothing to triage right now.")
            }
            .background(theme.bg)
        } else {
            List {
                Section {
                    ForEach(rows) { row in
                        InboxRow(row: row) {
                            navigationPath.append(DailyPageRoute(slug: row.page_id))
                        }
                        .listRowBackground(theme.bg2)
                        .swipeActions(edge: .leading, allowsFullSwipe: false) {
                            Button {
                                Task { await triage(row, status: "todo") }
                            } label: {
                                Label("Todo", systemImage: "circle")
                            }
                            .tint(theme.fgDefault)
                            Button {
                                Task { await triage(row, status: "doing") }
                            } label: {
                                Label("Doing", systemImage: "circle.lefthalf.filled")
                            }
                            .tint(theme.accentPrimary)
                        }
                        .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                            Button {
                                Task { await triage(row, status: "done") }
                            } label: {
                                Label("Done", systemImage: "checkmark.circle.fill")
                            }
                            .tint(.green)
                        }
                    }
                } header: {
                    HStack {
                        Text("UNTRIAGED")
                            .font(.system(size: 10, design: .monospaced))
                            .tracking(1.2)
                            .foregroundStyle(theme.fgFaint)
                        Spacer()
                        Text(headerCount)
                            .font(.system(size: 10, design: .monospaced))
                            .foregroundStyle(theme.fgFaint)
                    }
                }
            }
            .listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
            .background(theme.bg)
        }
    }

    private var headerCount: String {
        if rows.count >= rowCap { return "showing \(rowCap)+" }
        return "\(rows.count)"
    }

    // MARK: - Data load + actions

    /// Drop daily-page blocks and system-page-type blocks. Mirrors
    /// `isInboxableRow` in the web `QueryWidgetView.svelte` and the
    /// v5 Inbox ambient. Keeps the surface focused on captures + real
    /// project blocks rather than every loose bullet in the mosaic.
    private static let TRIAGED_PAGE_TYPES: Set<String> = ["Tag", "Property", "Query", "Template"]
    private static let DATE_ID_RE = try! NSRegularExpression(pattern: #"^\d{4}-\d{2}-\d{2}$"#)
    /// `### Raw Strings`, `## Lists`, `# Header` — markdown headings
    /// the user uses as section dividers inside reference pages, not
    /// triage items. Band-aid until the saved-query inbox redesign
    /// lets the user shape the filter themselves.
    private static let HEADING_RE = try! NSRegularExpression(pattern: #"^#{1,6}\s"#)

    private func isInboxable(_ item: QueryItem) -> Bool {
        guard item.kind == .block else { return false }
        let pid = item.page_id
        let pidRange = NSRange(pid.startIndex..<pid.endIndex, in: pid)
        if Self.DATE_ID_RE.firstMatch(in: pid, range: pidRange) != nil { return false }
        if let t = item.page_note_type, Self.TRIAGED_PAGE_TYPES.contains(t) { return false }
        let trimmed = item.text.trimmingCharacters(in: .whitespaces)
        let textRange = NSRange(trimmed.startIndex..<trimmed.endIndex, in: trimmed)
        if Self.HEADING_RE.firstMatch(in: trimmed, range: textRange) != nil { return false }
        return true
    }

    private func load() async {
        loading = true
        defer { loading = false }
        let result = await mosaic.executeQuery("kind:block -has:status")
        var collected: [QueryItem] = []
        outer: for g in result.groups {
            for item in g.items where isInboxable(item) {
                collected.append(item)
                if collected.count >= rowCap { break outer }
            }
        }
        rows = collected
    }

    private func triage(_ row: QueryItem, status: String) async {
        guard let bid = row.block_id else { return }
        do {
            try await mosaic.setBlockProperty(blockId: bid, key: "status", value: status)
            // Optimistically remove the row instead of refetching — the
            // user is going through a list and wants immediate feedback.
            rows.removeAll { $0.id == row.id }
        } catch {
            // Silent — connection banner surfaces server failures.
        }
    }

    private func placeholderPage(slug: String) -> some View {
        VStack(spacing: 12) {
            Text("Page not found")
                .font(.system(size: 20, weight: .semibold))
                .foregroundStyle(theme.fgDefault)
            Text("Nothing in the mosaic has the id `\(slug)`.")
                .font(.system(size: 13, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 24)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(theme.bg)
    }
}

// ── Row view ──────────────────────────────────────────────────────────

private struct InboxRow: View {
    let row: QueryItem
    let onTap: () -> Void

    @Environment(\.theme) private var theme

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Text("·")
                .font(.system(size: 12, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
                .frame(width: 12, alignment: .center)
                .padding(.top, 2)
            VStack(alignment: .leading, spacing: 4) {
                BlockText(text: row.text.isEmpty ? "(empty block)" : row.text)
                    .font(.system(size: 14))
                    .foregroundStyle(theme.fgDefault)
                HStack(spacing: 6) {
                    Text("in \(row.title.isEmpty ? row.page_id : row.title)")
                        .font(.system(size: 10))
                        .foregroundStyle(theme.fgFaint)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    if let tag = row.primary_tag {
                        Text("#\(tag)")
                            .font(.system(size: 10))
                            .foregroundStyle(theme.fgFaint.opacity(0.7))
                    }
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.vertical, 2)
        .contentShape(Rectangle())
        .onTapGesture { onTap() }
    }
}
