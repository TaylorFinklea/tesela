import SwiftUI

/// Graphite Inbox — the triage surface, re-themed over the SAME
/// `MockMosaicService.executeQuery(_:)` + inbox-DSL helpers the legacy
/// `InboxView` uses. No data layer is rebuilt: the active saved filter's
/// `query::` DSL is resolved via `fetchInboxDsl(slug:)` (falling back to
/// `defaultInboxDsl()`), parsed to chip state via `chipsFromDsl`, run
/// through `executeQuery`, and triage actions write `status::` via
/// `setBlockProperty`. Chip toggles rebuild the DSL via `dslFromChips`
/// and persist through `saveInboxDsl`.
///
/// Only the chrome is new: a `GrHeader`, a `GrChip` filter bar, and the
/// mobile `.grm-icard` rows with leading-swipe (todo/doing) and
/// trailing-swipe (done) triage. The active filter slug persists via
/// `@AppStorage`, matching the web's `localStorage` key.
struct GrInboxView: View {
    @ObservedObject var mosaic: MockMosaicService
    var backend: BackendSettings? = nil

    @Environment(\.theme) private var theme

    @State private var rows: [QueryItem] = []
    @State private var loading = false
    @State private var chipState: ChipState = chipsFromDsl(defaultInboxDsl())
    @State private var navigationPath = NavigationPath()

    @AppStorage("tesela.inbox.activeFilterSlug") private var activeSlug: String = "inbox"

    /// Soft cap mirroring the web's `ROW_CAP` so a legacy mosaic with
    /// thousands of untriaged blocks doesn't choke the renderer.
    private let rowCap = 200

    var body: some View {
        NavigationStack(path: $navigationPath) {
            VStack(spacing: 0) {
                GrHeader(title: "Inbox", subtitle: subtitle)
                chipBar
                content
            }
            .background(theme.bg)
            .navigationDestination(for: GrPageRoute.self) { route in
                GrPageView(slug: route.slug, mosaic: mosaic, path: $navigationPath)
                    .environment(\.theme, theme)
            }
        }
        .task { await load() }
    }

    private var subtitle: String {
        rows.isEmpty ? "TRIAGE" : "\(headerCount) unsorted"
    }

    private var headerCount: String {
        rows.count >= rowCap ? "\(rowCap)+" : "\(rows.count)"
    }

    // ── Chip bar (.gr-chipbar over the inbox-DSL registry) ──────────────

    private var chipBar: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 7) {
                ForEach(chipRegistry, id: \.id) { chip in
                    GrChip(
                        label: chip.label,
                        active: chipState.active[chip.id] == true
                    ) {
                        chipState.active[chip.id] = !(chipState.active[chip.id] ?? false)
                        Task { await commitChipState() }
                    }
                }
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 11)
        }
        .scrollClipDisabled()
        .overlay(alignment: .bottom) {
            Rectangle().fill(theme.lineSoft).frame(height: 1)
        }
    }

    // ── Content ─────────────────────────────────────────────────────────

    @ViewBuilder
    private var content: some View {
        if loading && rows.isEmpty {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
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
                        inboxCard(row)
                            .listRowInsets(EdgeInsets(top: 4, leading: 14, bottom: 4, trailing: 14))
                            .listRowBackground(Color.clear)
                            .listRowSeparator(.hidden)
                            .swipeActions(edge: .leading, allowsFullSwipe: false) {
                                Button {
                                    Task { await triage(row, status: "todo") }
                                } label: {
                                    Label("Todo", systemImage: "circle")
                                }
                                .tint(theme.fgMuted)
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
                }
            }
            .listStyle(.plain)
            .scrollContentBackground(.hidden)
            .background(theme.bg)
            .refreshable { await load() }
        }
    }

    // ── Card (.grm-icard) ───────────────────────────────────────────────

    private func inboxCard(_ row: QueryItem) -> some View {
        HStack(alignment: .top, spacing: 12) {
            ZStack {
                RoundedRectangle(cornerRadius: 8)
                    .fill(theme.bg4)
                    .frame(width: 30, height: 30)
                GrIcon(name: sourceIcon(row), size: 15)
                    .foregroundStyle(theme.fgSubtle)
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

    // ── Data load + actions (same mutations as InboxView) ───────────────

    private func load() async {
        loading = true
        defer { loading = false }
        let dsl = await mosaic.fetchInboxDsl(slug: activeSlug) ?? MockMosaicService.defaultInboxDsl()
        chipState = chipsFromDsl(dsl)
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

    private func commitChipState() async {
        let newDsl = dslFromChips(chipState)
        try? await mosaic.saveInboxDsl(slug: activeSlug, dsl: newDsl)
        await load()
    }

    private func triage(_ row: QueryItem, status: String) async {
        guard let bid = row.block_id else { return }
        do {
            try await mosaic.setBlockProperty(blockId: bid, key: "status", value: status)
            // Optimistically drop the row — the user is ripping through
            // the list and wants immediate feedback (mirrors InboxView).
            rows.removeAll { $0.id == row.id }
        } catch {
            // Silent — refresh recovers on next load.
        }
    }
}
