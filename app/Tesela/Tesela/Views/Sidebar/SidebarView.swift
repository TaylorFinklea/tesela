import SwiftUI

// MARK: - SidebarView
// Left sidebar with navigation sections, favorites, and recents

struct SidebarView: View {
    @Environment(AppState.self) private var appState
    @State private var isFavoritesExpanded = true
    @State private var isRecentsExpanded = true

    var body: some View {
        List(selection: Binding(
            get: { appState.selectedNavItem },
            set: { if let item = $0 { appState.selectedNavItem = item } }
        )) {
            // MARK: Navigation
            Section("Navigation") {
                ForEach(NavItem.allCases, id: \.self) { item in
                    Label(item.label, systemImage: item.systemImage)
                        .tag(item)
                }
            }

            // MARK: Favorites
            if !appState.favoritePages.isEmpty {
                Section(isExpanded: $isFavoritesExpanded) {
                    ForEach(appState.favoritePages) { page in
                        PageRowView(page: page)
                    }
                } header: {
                    Text("Favorites")
                }
            }

            // MARK: Recent
            if !appState.recentPages.isEmpty {
                Section(isExpanded: $isRecentsExpanded) {
                    ForEach(appState.recentPages) { page in
                        PageRowView(page: page)
                    }
                } header: {
                    Text("Recent")
                }
            }
        }
        .listStyle(.sidebar)
        .frame(minWidth: 200, idealWidth: 220)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    appState.isShowingNewPageSheet = true
                } label: {
                    Image(systemName: "square.and.pencil")
                }
                .help("New page (⌘N)")
            }
        }
        .onChange(of: appState.selectedNavItem) { _, newItem in
            handleNavChange(to: newItem)
        }
        .safeAreaInset(edge: .bottom) {
            SidebarBottomBar()
        }
    }

    private func handleNavChange(to item: NavItem) {
        switch item {
        case .journals:
            Task {
                if let page = try? await appState.api.getDailyNote() {
                    appState.open(page)
                }
            }
        case .pages:
            appState.currentPage = nil  // Show page list in content area
        case .graph:
            appState.currentPage = nil  // Show graph in content area
        }
    }
}

// MARK: - PageRowView
private struct PageRowView: View {
    let page: Page
    @Environment(AppState.self) private var appState

    var body: some View {
        Button {
            appState.open(page)
        } label: {
            VStack(alignment: .leading, spacing: 2) {
                Text(page.title)
                    .font(.body)
                    .lineLimit(1)
                if !page.metadata.tags.isEmpty {
                    Text(page.metadata.tags.map { "#\($0)" }.joined(separator: " "))
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
            }
        }
        .buttonStyle(.plain)
    }
}

// MARK: - SidebarBottomBar
private struct SidebarBottomBar: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        HStack {
            Circle()
                .fill(appState.connectionStatus.color)
                .frame(width: 8, height: 8)

            Text(appState.connectionStatus.displayLabel)
                .font(.caption)
                .foregroundStyle(.secondary)

            Spacer()
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(.bar)
    }
}
