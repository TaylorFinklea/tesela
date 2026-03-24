import SwiftUI

// MARK: - TilesView
// Scrollable timeline of daily notes with inline editing.
// Each tile is a fully editable OutlinerCoordinator — like Logseq's journal page.

struct TilesView: View {
    @Environment(AppState.self) private var appState
    @State private var dailyNotes: [Page] = []

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                TodayHeader(dailyNotes: dailyNotes, onCreated: { await loadDailyNotes() })
                    .padding(.horizontal, 24)
                    .padding(.top, 16)

                ForEach(dailyNotes) { page in
                    EditableTileCard(page: page)
                }
            }
            .padding(.bottom, 24)
        }
        .task { await loadDailyNotes() }
        .refreshable { await loadDailyNotes() }
    }

    private func loadDailyNotes() async {
        let notes = (try? await appState.api.listNotes(tag: "daily", limit: 100)) ?? []
        dailyNotes = notes.sorted { $0.id > $1.id }
    }
}

// MARK: - TodayHeader
private struct TodayHeader: View {
    let dailyNotes: [Page]
    var onCreated: () async -> Void
    @Environment(AppState.self) private var appState

    private var todayID: String {
        let fmt = DateFormatter()
        fmt.dateFormat = "yyyy-MM-dd"
        return fmt.string(from: Date())
    }

    var body: some View {
        if !dailyNotes.contains(where: { $0.id == todayID }) {
            Button {
                Task {
                    if let _ = try? await appState.api.getDailyNote() {
                        await onCreated()
                    }
                }
            } label: {
                HStack {
                    Image(systemName: "plus.square.dashed")
                    Text("Lay today's tile")
                        .font(.headline)
                }
                .foregroundStyle(Color.accentColor)
            }
            .buttonStyle(.plain)
            .padding(.bottom, 12)
        }
    }
}

// MARK: - EditableTileCard
// Each tile has its own OutlinerCoordinator with independent editing + auto-save.
private struct EditableTileCard: View {
    let page: Page
    @Environment(AppState.self) private var appState
    @State private var blocks: [Block] = []
    @State private var saveTask: Task<Void, Never>?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Divider()
                .padding(.horizontal, 24)

            // Date header — click to open full page editor
            Button {
                appState.open(page)
            } label: {
                Text(formattedDate)
                    .font(.title3)
                    .bold()
                    .foregroundStyle(.primary)
            }
            .buttonStyle(.plain)
            .padding(.horizontal, 24)
            .padding(.top, 16)
            .padding(.bottom, 4)

            // Inline editor
            OutlinerCoordinator(
                blocks: $blocks,
                onContentChanged: { updatedBlocks in
                    blocks = updatedBlocks
                    let markdown = BlockParser.serializeFlat(blocks: updatedBlocks)
                    scheduleAutoSave(markdown: markdown)
                },
                onWikiLinkClicked: { target in
                    if let linked = appState.pages.first(where: {
                        $0.title.lowercased() == target.lowercased()
                    }) {
                        appState.open(linked)
                    }
                },
                onModeChanged: { _ in },
                onCommandPalette: {
                    appState.isCommandPaletteVisible = true
                },
                onSlashMenu: {
                    appState.isSlashMenuVisible = true
                },
                onSpaceMenu: {
                    appState.isSpaceMenuVisible = true
                },
                isMenuVisible: {
                    appState.isSlashMenuVisible || appState.isSpaceMenuVisible
                },
                onDismissMenu: {
                    appState.isSlashMenuVisible = false
                    appState.isSpaceMenuVisible = false
                },
                apiClient: appState.api
            )
            .frame(minHeight: 30, maxHeight: 300)
            .padding(.horizontal, 8)
            .padding(.bottom, 16)
        }
        .onAppear { loadBlocks() }
    }

    private func loadBlocks() {
        let parsed = BlockParser.flatten(blocks: BlockParser.parse(markdown: page.body))
        if parsed.isEmpty {
            blocks = [Block(text: "")]
        } else {
            blocks = parsed
        }
    }

    private func scheduleAutoSave(markdown: String) {
        saveTask?.cancel()
        saveTask = Task {
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled else { return }
            await appState.updatePage(id: page.id, newBody: markdown)
        }
    }

    private var formattedDate: String {
        let inputFmt = DateFormatter()
        inputFmt.dateFormat = "yyyy-MM-dd"
        guard let date = inputFmt.date(from: page.id) else { return page.title }
        let outputFmt = DateFormatter()
        outputFmt.dateFormat = "EEEE, MMMM d, yyyy"
        return outputFmt.string(from: date)
    }
}
