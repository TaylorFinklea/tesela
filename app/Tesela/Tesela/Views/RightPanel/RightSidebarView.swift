import SwiftUI

// MARK: - RightSidebarView
// Trailing column: backlinks, forward links for current page

struct RightSidebarView: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        Group {
            if let page = appState.currentPage {
                RightSidebarContent(page: page)
            } else {
                ContentUnavailableView(
                    "No Page Open",
                    systemImage: "sidebar.right",
                    description: Text("Open a page to see its links and outline")
                )
            }
        }
        .frame(minWidth: 260, idealWidth: 280)
    }
}

// MARK: - RightSidebarContent
private struct RightSidebarContent: View {
    let page: Page
    @State private var backlinks: [Link] = []
    @State private var forwardLinks: [Link] = []
    @Environment(AppState.self) private var appState

    var body: some View {
        List {
            Section("Linked References (\(backlinks.count))") {
                if backlinks.isEmpty {
                    Text("No backlinks")
                        .foregroundStyle(.tertiary)
                        .font(.caption)
                } else {
                    ForEach(backlinks) { link in
                        LinkRowView(link: link)
                            .onTapGesture { navigateTo(noteId: link.target) }
                    }
                }
            }

            Section("Forward Links (\(forwardLinks.count))") {
                if forwardLinks.isEmpty {
                    Text("No outgoing links")
                        .foregroundStyle(.tertiary)
                        .font(.caption)
                } else {
                    ForEach(forwardLinks) { link in
                        LinkRowView(link: link)
                            .onTapGesture { navigateTo(noteId: link.target) }
                    }
                }
            }
        }
        .listStyle(.sidebar)
        .task(id: page.id) {
            await loadLinks()
        }
    }

    private func loadLinks() async {
        async let backlinksTask = appState.api.getBacklinks(id: page.id)
        async let forwardLinksTask = appState.api.getLinks(id: page.id)
        backlinks = (try? await backlinksTask) ?? []
        forwardLinks = (try? await forwardLinksTask) ?? []
    }

    private func navigateTo(noteId: String) {
        Task {
            if let page = try? await appState.api.getNote(id: noteId) {
                appState.open(page)
            }
        }
    }
}

// MARK: - LinkRowView
private struct LinkRowView: View {
    let link: Link

    var body: some View {
        HStack(spacing: 6) {
            VStack(alignment: .leading, spacing: 2) {
                Text(link.target)
                    .font(.caption)
                    .bold()
                    .foregroundStyle(Color.accentColor)
                if let text = link.text {
                    Text(text)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                }
            }
            Spacer()
            Image(systemName: "chevron.right")
                .font(.caption2)
                .foregroundStyle(.tertiary)
        }
        .padding(.vertical, 2)
        .contentShape(Rectangle())
    }
}
