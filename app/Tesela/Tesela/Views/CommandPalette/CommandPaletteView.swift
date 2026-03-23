import SwiftUI

// MARK: - CommandPaletteView
// ⌘K floating overlay: fuzzy search over pages + commands

struct CommandPaletteView: View {
    @Environment(AppState.self) private var appState
    @State private var query = ""
    @State private var results: [PaletteResult] = []
    @State private var selectedIndex = 0
    @FocusState private var isInputFocused: Bool

    private let staticCommands: [PaletteResult] = [
        .command(id: "new-page", label: "New Page", icon: "doc.badge.plus"),
        .command(id: "open-journal", label: "Open Today's Journal", icon: "calendar"),
        .command(id: "toggle-left-sidebar", label: "Toggle Left Sidebar", icon: "sidebar.left"),
        .command(id: "toggle-right-sidebar", label: "Toggle Right Sidebar", icon: "sidebar.right"),
        .command(id: "toggle-graph", label: "Open Graph View", icon: "point.3.connected.trianglepath.dotted")
    ]

    var body: some View {
        VStack(spacing: 0) {
            // Search input
            HStack(spacing: 12) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(.secondary)
                TextField("Search pages and commands…", text: $query)
                    .textFieldStyle(.plain)
                    .font(.title3)
                    .focused($isInputFocused)
                    .onKeyPress(.escape) {
                        dismiss()
                        return .handled
                    }
                    .onKeyPress(.upArrow) {
                        selectedIndex = max(0, selectedIndex - 1)
                        return .handled
                    }
                    .onKeyPress(.downArrow) {
                        selectedIndex = min(results.count - 1, selectedIndex + 1)
                        return .handled
                    }
                    .onKeyPress(.return) {
                        if selectedIndex < results.count {
                            execute(results[selectedIndex])
                        }
                        return .handled
                    }
            }
            .padding(16)

            Divider()

            // Results list
            if results.isEmpty && query.isEmpty {
                // Show static commands when no query
                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(Array(staticCommands.enumerated()), id: \.element.id) { index, result in
                            PaletteResultRow(result: result, isSelected: selectedIndex == index)
                                .onTapGesture { execute(result) }
                        }
                    }
                }
                .frame(maxHeight: 300)
            } else {
                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(Array(results.enumerated()), id: \.element.id) { index, result in
                            PaletteResultRow(result: result, isSelected: selectedIndex == index)
                                .onTapGesture { execute(result) }
                        }
                    }
                }
                .frame(maxHeight: 400)
            }
        }
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12))
        .shadow(color: .black.opacity(0.3), radius: 24, y: 8)
        .frame(width: 560)
        .onAppear {
            results = staticCommands
            isInputFocused = true
        }
        .onChange(of: query) { _, newQuery in
            Task { await search(query: newQuery) }
        }
    }

    // MARK: - Search
    private func search(query: String) async {
        guard !query.isEmpty else {
            results = staticCommands
            selectedIndex = 0
            return
        }

        let hits = (try? await appState.api.search(query: query)) ?? []
        let pageResults = hits.map { PaletteResult.page(id: $0.noteId, title: $0.title, snippet: $0.snippet) }
        let filteredCommands = staticCommands.filter {
            $0.label.localizedCaseInsensitiveContains(query)
        }

        // Try to parse the query as a natural date (e.g., "March 23", "Mar 23rd", "3/23")
        // Try to parse the query as a natural date
        var dateResults: [PaletteResult] = []
        if let dateId = DateParser.parse(query), let preview = DateParser.preview(query) {
            if !pageResults.contains(where: { $0.id == "page:\(dateId)" }) {
                dateResults.append(.page(id: dateId, title: preview, snippet: "Daily note: \(dateId)"))
            }
        }

        results = dateResults + pageResults + filteredCommands
        selectedIndex = 0
    }

    // MARK: - Execute
    private func execute(_ result: PaletteResult) {
        switch result {
        case .page(let id, _, _):
            Task {
                if let page = try? await appState.api.getNote(id: id) {
                    appState.open(page)
                }
                dismiss()
            }
        case .command(let id, _, _):
            executeCommand(id: id)
            dismiss()
        }
    }

    private func executeCommand(id: String) {
        switch id {
        case "new-page":
            appState.isShowingNewPageSheet = true
        case "open-journal":
            Task {
                if let page = try? await appState.api.getDailyNote() {
                    appState.open(page)
                }
            }
        case "toggle-left-sidebar":
            appState.isLeftSidebarVisible.toggle()
        case "toggle-right-sidebar":
            appState.isRightSidebarVisible.toggle()
        case "toggle-graph":
            appState.selectedNavItem = .graph
        default:
            break
        }
    }

    private func dismiss() {
        appState.isCommandPaletteVisible = false
    }
}

// MARK: - PaletteResult
enum PaletteResult: Identifiable {
    case page(id: String, title: String, snippet: String)
    case command(id: String, label: String, icon: String)

    var id: String {
        switch self {
        case .page(let id, _, _): "page:\(id)"
        case .command(let id, _, _): "cmd:\(id)"
        }
    }

    var label: String {
        switch self {
        case .page(_, let title, _): title
        case .command(_, let label, _): label
        }
    }
}

// MARK: - PaletteResultRow
private struct PaletteResultRow: View {
    let result: PaletteResult
    let isSelected: Bool

    @ViewBuilder
    private var icon: some View {
        switch result {
        case .page:
            Image(systemName: "doc.text")
                .foregroundStyle(.secondary)
        case .command(_, _, let icon):
            Image(systemName: icon)
                .foregroundStyle(Color.accentColor)
        }
    }

    var body: some View {
        HStack(spacing: 12) {
            icon
                .frame(width: 20)

            VStack(alignment: .leading, spacing: 2) {
                Text(result.label)
                    .font(.body)
                if case .page(_, _, let snippet) = result, !snippet.isEmpty {
                    Text(snippet)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
            }
            Spacer()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(isSelected ? Color.accentColor.opacity(0.15) : Color.clear)
    }
}
