import SwiftUI

// MARK: - SidebarView
// Left sidebar with navigation sections, favorites, and recents

struct SidebarView: View {
    @Environment(AppState.self) private var appState
    @State private var isFavoritesExpanded = true
    @State private var isRecentsExpanded = true
    @State private var sidebarFilter = ""

    private func matchesFilter(_ page: Page) -> Bool {
        sidebarFilter.isEmpty || page.title.localizedCaseInsensitiveContains(sidebarFilter)
    }

    var body: some View {
        List(selection: Binding(
            get: { appState.selectedNavItem },
            set: { if let item = $0 { appState.selectedNavItem = item } }
        )) {
            // MARK: Navigation
            Section {
                ForEach(NavItem.allCases, id: \.self) { item in
                    Label(item.label, systemImage: item.systemImage)
                        .tag(item)
                        .onTapGesture {
                            appState.selectedNavItem = item
                            appState.currentPage = nil
                        }
                }
            } header: {
                SidebarSectionHeader(title: "Navigation", systemImage: "square.grid.2x2")
            }

            // MARK: Favorites
            if !appState.favoritePages.isEmpty {
                Section(isExpanded: $isFavoritesExpanded) {
                    ForEach(appState.favoritePages.filter(matchesFilter)) { page in
                        PageRowView(page: page)
                    }
                } header: {
                    SidebarSectionHeader(title: "Favorites", systemImage: "star.fill")
                }
            }

            // MARK: Recent
            if !appState.recentPages.isEmpty {
                Section(isExpanded: $isRecentsExpanded) {
                    ForEach(appState.recentPages.filter(matchesFilter)) { page in
                        PageRowView(page: page)
                    }
                } header: {
                    SidebarSectionHeader(title: "Recent", systemImage: "clock")
                }
            }
        }
        .listStyle(.sidebar)
        .searchable(text: $sidebarFilter, placement: .sidebar, prompt: "Filter")
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
        case .tiles:
            appState.currentPage = nil
        case .pages:
            appState.currentPage = nil
        case .graph:
            appState.currentPage = nil
        }
    }
}

// MARK: - PageRowView
private struct PageRowView: View {
    let page: Page
    @Environment(AppState.self) private var appState

    private var pageIcon: String {
        if page.metadata.tags.contains("daily") {
            return "calendar"
        }
        switch page.metadata.noteType {
        case "Tag":
            return "tag"
        case "Property":
            return "slider.horizontal.3"
        default:
            return "doc.text"
        }
    }

    var body: some View {
        Button {
            appState.open(page)
        } label: {
            HStack(alignment: .top, spacing: 10) {
                Image(systemName: pageIcon)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(.secondary)
                    .frame(width: 14, alignment: .center)
                    .padding(.top, 2)

                VStack(alignment: .leading, spacing: 3) {
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
                Spacer(minLength: 0)
            }
            .padding(.vertical, 2)
        }
        .buttonStyle(.plain)
    }
}

private struct SidebarSectionHeader: View {
    let title: String
    let systemImage: String

    var body: some View {
        Label(title, systemImage: systemImage)
            .font(.caption.weight(.semibold))
            .foregroundStyle(.secondary)
            .textCase(.uppercase)
            .tracking(0.4)
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
