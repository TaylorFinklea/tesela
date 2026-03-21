import SwiftUI

// MARK: - PageListView
// Shows all pages in the content area when "Pages" is selected in sidebar.
// Includes a search bar (FTS5 via server) and tag filter chips.

struct PageListView: View {
    @Environment(AppState.self) private var appState
    @State private var selectedTag: String?
    @State private var searchQuery = ""
    @State private var searchResults: [SearchHit]?
    @State private var searchTask: Task<Void, Never>?
    @FocusState private var isSearchFocused: Bool

    var filteredPages: [Page] {
        guard let tag = selectedTag else { return appState.pages }
        return appState.pages.filter { $0.metadata.tags.contains(tag) }
    }

    var body: some View {
        VStack(spacing: 0) {
            // Search bar
            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(.tertiary)
                TextField("Search pages…", text: $searchQuery)
                    .textFieldStyle(.plain)
                    .focused($isSearchFocused)
                if !searchQuery.isEmpty {
                    Button {
                        searchQuery = ""
                        searchResults = nil
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundStyle(.tertiary)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)

            Divider()

            // Tag filter chips
            if !appState.tags.isEmpty && searchResults == nil {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        TagChip(label: "All", isSelected: selectedTag == nil) {
                            selectedTag = nil
                        }
                        ForEach(appState.tags, id: \.self) { tag in
                            TagChip(label: "#\(tag)", isSelected: selectedTag == tag) {
                                selectedTag = selectedTag == tag ? nil : tag
                            }
                        }
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                }
                Divider()
            }

            // Results
            if let hits = searchResults {
                if hits.isEmpty {
                    ContentUnavailableView.search(text: searchQuery)
                } else {
                    List(hits) { hit in
                        SearchResultRow(hit: hit)
                            .onTapGesture {
                                Task {
                                    if let page = try? await appState.api.getNote(id: hit.noteId) {
                                        appState.open(page)
                                    }
                                }
                            }
                    }
                    .listStyle(.plain)
                }
            } else {
                List(filteredPages) { page in
                    PageListRow(page: page)
                        .onTapGesture { appState.open(page) }
                        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
                            Button(role: .destructive) {
                                Task { await appState.deletePage(page) }
                            } label: {
                                Label("Delete", systemImage: "trash")
                            }
                        }
                }
                .listStyle(.plain)
            }
        }
        .onChange(of: searchQuery) { _, newQuery in
            debounceSearch(query: newQuery)
        }
        .onChange(of: appState.isSearchVisible) { _, visible in
            if visible { isSearchFocused = true }
        }
    }

    private func debounceSearch(query: String) {
        searchTask?.cancel()
        guard !query.isEmpty else {
            searchResults = nil
            return
        }
        searchTask = Task {
            try? await Task.sleep(for: .milliseconds(300))
            guard !Task.isCancelled else { return }
            let hits = (try? await appState.api.search(query: query)) ?? []
            searchResults = hits
        }
    }
}

// MARK: - SearchResultRow
private struct SearchResultRow: View {
    let hit: SearchHit

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(hit.title)
                .font(.body)
                .bold()
            if !hit.snippet.isEmpty {
                Text(hit.snippet)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }
            if !hit.tags.isEmpty {
                Text(hit.tags.prefix(3).map { "#\($0)" }.joined(separator: " "))
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            }
        }
        .padding(.vertical, 4)
    }
}

// MARK: - PageListRow
private struct PageListRow: View {
    let page: Page

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(page.title)
                .font(.body)
                .bold()
            HStack {
                Text(page.modifiedAt, style: .date)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                if !page.metadata.tags.isEmpty {
                    Text("·")
                        .foregroundStyle(.secondary)
                    Text(page.metadata.tags.prefix(3).map { "#\($0)" }.joined(separator: " "))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .padding(.vertical, 4)
    }
}

// MARK: - TagChip
private struct TagChip: View {
    let label: String
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(label)
                .font(.caption)
                .padding(.horizontal, 10)
                .padding(.vertical, 4)
                .background(isSelected ? Color.accentColor : Color.secondary.opacity(0.15),
                            in: Capsule())
                .foregroundStyle(isSelected ? .white : .primary)
        }
        .buttonStyle(.plain)
    }
}
