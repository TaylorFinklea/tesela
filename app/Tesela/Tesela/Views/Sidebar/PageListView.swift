import SwiftUI

// MARK: - PageListView
// Shows all pages in the content area when "Pages" is selected in sidebar

struct PageListView: View {
    @Environment(AppState.self) private var appState
    @State private var selectedTag: String?

    var filteredPages: [Page] {
        guard let tag = selectedTag else { return appState.pages }
        return appState.pages.filter { $0.metadata.tags.contains(tag) }
    }

    var body: some View {
        VStack(spacing: 0) {
            // Tag filter chips
            if !appState.tags.isEmpty {
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

            // Page list
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
