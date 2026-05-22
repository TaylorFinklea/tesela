import SwiftUI

/// Inbox — the triage surface. Lists open tasks across today and
/// yesterday plus any blocks tagged `#inbox`. Tapping a task toggles
/// it done (writes back via the same mosaic.toggleTask path). Long-
/// press surfaces the block context menu.
///
/// Per the user's ask: "an inbox to triage todos and stuff."
struct InboxView: View {
    @ObservedObject var mosaic: MockMosaicService
    @ObservedObject var backend: BackendSettings
    /// Optional — wires the top-bar gear button to the shared Settings
    /// sheet, mirroring DailyView and LibraryView.
    var appearance: AppearanceController? = nil
    var syncState: SyncState? = nil
    var transcription: TranscriptionStore? = nil

    @Environment(\.theme) private var theme
    @EnvironmentObject private var mosaicRegistry: MosaicRegistry
    @State private var showSettings: Bool = false
    @State private var showMosaicSwitcher: Bool = false

    var body: some View {
        NavigationStack {
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
                    .refreshable {
                        await mosaic.refresh(from: backend.backend)
                    }
            }
            .background(theme.bg)
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
    }

    @ViewBuilder
    private var content: some View {
        if openTasksToday.isEmpty && openTasksYesterday.isEmpty && inboxTagged.isEmpty {
            ContentUnavailableView {
                Label("Inbox is clear", systemImage: "tray")
            } description: {
                Text("No tasks need triage. New captures and open todos will show up here.")
            }
            .background(theme.bg)
        } else {
            List {
                if !openTasksToday.isEmpty {
                    section(title: "Today · open tasks", count: openTasksToday.count, blocks: openTasksToday)
                }
                if !openTasksYesterday.isEmpty {
                    section(title: "Yesterday · still open", count: openTasksYesterday.count, blocks: openTasksYesterday)
                }
                if !inboxTagged.isEmpty {
                    section(title: "Tagged #inbox", count: inboxTagged.count, blocks: inboxTagged)
                }
            }
            .listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
            .background(theme.bg)
        }
    }

    private func section(title: String, count: Int, blocks: [Block]) -> some View {
        Section {
            ForEach(blocks) { block in
                InboxRow(block: block) {
                    mosaic.toggleTask(id: block.id)
                }
                .listRowBackground(theme.bg2)
            }
        } header: {
            HStack {
                Text(title.uppercased())
                    .font(.system(size: 10, design: .monospaced))
                    .tracking(1.2)
                    .foregroundStyle(theme.fgFaint)
                Spacer()
                Text("\(count)")
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }
        }
    }

    // MARK: - Slices of the mosaic

    private var openTasksToday: [Block] {
        mosaic.todayBlocks.filter { $0.kind == .task && !$0.done }
    }

    private var openTasksYesterday: [Block] {
        mosaic.yesterdayBlocks.filter { $0.kind == .task && !$0.done }
    }

    /// Blocks tagged with `#inbox` in either source. Pulled from the
    /// trailing-cluster `tags` field on each block.
    private var inboxTagged: [Block] {
        let all = mosaic.todayBlocks + mosaic.yesterdayBlocks
        return all.filter { block in
            block.tags.contains(where: { $0.lowercased() == "#inbox" }) ||
            block.properties.contains(where: { p in
                p.key.lowercased() == "tags" &&
                p.value.lowercased().split(separator: ",").contains { $0.trimmingCharacters(in: .whitespaces).lowercased() == "inbox" }
            })
        }
    }
}

/// One row in the Inbox list. Renders a task with checkbox, block
/// text, and the small tag chip strip below.
struct InboxRow: View {
    let block: Block
    var onToggle: () -> Void = {}

    @Environment(\.theme) private var theme

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            checkbox
            VStack(alignment: .leading, spacing: 4) {
                BlockText(text: block.text)
                    .font(.system(size: 14.5))
                    .foregroundStyle(theme.fgDefault)
                    .strikethrough(block.done, color: theme.fgSubtle)
                if !block.tags.isEmpty {
                    HStack(spacing: 4) {
                        ForEach(block.tags, id: \.self) { TagChip(value: $0) }
                    }
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.vertical, 2)
        .contentShape(Rectangle())
        .onTapGesture {
            if block.kind == .task { onToggle() }
        }
        .contextMenu {
            BlockContextMenu(blockId: block.id)
        }
    }

    @ViewBuilder
    private var checkbox: some View {
        if block.kind == .task {
            ZStack {
                RoundedRectangle(cornerRadius: 3)
                    .stroke(theme.typeTask, lineWidth: 1.5)
                    .frame(width: 16, height: 16)
                if block.done {
                    RoundedRectangle(cornerRadius: 3)
                        .fill(theme.typeTask)
                        .frame(width: 16, height: 16)
                    Icon(name: .check, size: 11, lineWidth: 2.5)
                        .foregroundStyle(theme.bg)
                }
            }
            .padding(.top, 2)
        } else {
            Text("·")
                .font(.system(size: 12, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
                .frame(width: 16, alignment: .center)
                .padding(.top, 2)
        }
    }
}
