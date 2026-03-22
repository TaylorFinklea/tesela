import SwiftUI

// MARK: - ContentArea
// Center pane: shows the current page in the outliner editor

struct ContentArea: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        Group {
            if let page = appState.currentPage {
                PageEditorView(page: page)
            } else {
                switch appState.selectedNavItem {
                case .tiles:
                    TilesView()
                case .pages:
                    PageListView()
                case .graph:
                    GraphView()
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - EmptyStateView
private struct EmptyStateView: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        VStack(spacing: 12) {
            Image(systemName: "doc.text")
                .font(.system(size: 48))
                .foregroundStyle(.tertiary)
            Text("No page selected")
                .font(.title3)
                .foregroundStyle(.secondary)
            Text("Select a page from the sidebar or press ⌘K to search")
                .font(.caption)
                .foregroundStyle(.tertiary)
                .multilineTextAlignment(.center)
            Button("Open Today's Journal") {
                Task {
                    if let page = try? await appState.api.getDailyNote() {
                        appState.open(page)
                    }
                }
            }
            .buttonStyle(.bordered)
            .padding(.top, 8)
        }
        .padding()
    }
}

// MARK: - PageEditorView
// Block outliner editor with 500ms debounced auto-save.
struct PageEditorView: View {
    let page: Page
    @Environment(AppState.self) private var appState
    @State private var blocks: [Block] = []
    @State private var saveTask: Task<Void, Never>?
    @State private var showDeleteConfirm = false
    @State private var vimMode: VimMode = .insert

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Title + toolbar
            HStack {
                Text(page.title)
                    .font(.title2)
                    .bold()
                    .lineLimit(1)
                Spacer()
                Button {
                    appState.toggleFavorite(page.id)
                } label: {
                    Image(systemName: appState.favoritePageIds.contains(page.id) ? "star.fill" : "star")
                        .foregroundStyle(appState.favoritePageIds.contains(page.id) ? .yellow : .secondary)
                }
                .buttonStyle(.borderless)
                .help("Toggle favorite")
                Text(page.modifiedAt, style: .relative)
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                Button(role: .destructive) {
                    showDeleteConfirm = true
                } label: {
                    Image(systemName: "trash")
                }
                .buttonStyle(.borderless)
                .help("Delete page")
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 12)

            Divider()

            OutlinerCoordinator(
                blocks: $blocks,
                onContentChanged: { updatedBlocks in
                    blocks = updatedBlocks  // sync SwiftUI state with outliner
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
                onModeChanged: { mode in
                    vimMode = mode
                },
                onCommandPalette: {
                    appState.isCommandPaletteVisible = true
                }
            )
            .padding(.horizontal, 8)
            .overlay(alignment: .bottomTrailing) {
                Text(vimMode.displayName)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundStyle(vimModeColor)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(vimModeColor.opacity(0.12), in: RoundedRectangle(cornerRadius: 4))
                    .padding(12)
            }
        }
        .onAppear { loadBlocks() }
        .onChange(of: page.id) { _, _ in
            saveTask?.cancel()
            loadBlocks()
        }
        .alert("Delete \"\(page.title)\"?", isPresented: $showDeleteConfirm) {
            Button("Delete", role: .destructive) {
                Task { await appState.deletePage(page) }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This permanently deletes the page and its file. This cannot be undone.")
        }
    }

    private var vimModeColor: Color {
        switch vimMode {
        case .normal: .primary
        case .insert: .green
        case .visual, .visualLine: .blue
        case .operatorPending: .orange
        }
    }

    private func loadBlocks() {
        let parsed = BlockParser.flatten(blocks: BlockParser.parse(markdown: page.body))
        if parsed.isEmpty {
            let text = strippedBody(page.body)
            blocks = [Block(text: text)]
        } else {
            blocks = parsed
        }
    }

    private func strippedBody(_ body: String) -> String {
        var lines = body.components(separatedBy: "\n")
        if let first = lines.first, first.hasPrefix("# ") {
            lines.removeFirst()
        }
        return lines.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func scheduleAutoSave(markdown: String) {
        saveTask?.cancel()
        saveTask = Task {
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled else { return }
            await appState.updatePage(id: page.id, newBody: markdown)
        }
    }
}
