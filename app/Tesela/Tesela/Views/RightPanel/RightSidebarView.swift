import SwiftUI

// MARK: - RightSidebarView
// Trailing column: backlinks, forward links, TOC for current page

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
            // Backlinks
            Section("Linked References (\(backlinks.count))") {
                if backlinks.isEmpty {
                    Text("No backlinks")
                        .foregroundStyle(.tertiary)
                        .font(.caption)
                } else {
                    ForEach(backlinks) { link in
                        LinkRowView(link: link)
                    }
                }
            }

            // Forward links
            Section("Forward Links (\(forwardLinks.count))") {
                if forwardLinks.isEmpty {
                    Text("No outgoing links")
                        .foregroundStyle(.tertiary)
                        .font(.caption)
                } else {
                    ForEach(forwardLinks) { link in
                        LinkRowView(link: link)
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
}

// MARK: - LinkRowView
private struct LinkRowView: View {
    let link: Link

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(link.target)
                .font(.caption)
                .bold()
            if let text = link.text {
                Text(text)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }
        }
        .padding(.vertical, 2)
    }
}
