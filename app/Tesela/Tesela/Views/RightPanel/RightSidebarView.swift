import SwiftUI

// MARK: - RightSidebarView
// Trailing column: page properties, backlinks (grouped by source), forward links

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

    private var groupedBacklinks: [(source: String, links: [Link])] {
        var groups: [String: [Link]] = [:]
        for link in backlinks {
            // Group by link source (the page that links TO this page)
            // For backlinks, `target` is actually the source page's ID
            groups[link.target, default: []].append(link)
        }
        return groups.map { (source: $0.key, links: $0.value) }
            .sorted { $0.source < $1.source }
    }

    var body: some View {
        List {
            // Page properties
            Section("Page Info") {
                LabeledContent("Type") {
                    Text(page.metadata.noteType ?? "Page")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .font(.caption)

                if !page.metadata.tags.isEmpty {
                    LabeledContent("Tags") {
                        HStack(spacing: 4) {
                            ForEach(page.metadata.tags, id: \.self) { tag in
                                Text("#\(tag)")
                                    .font(.caption2)
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(Color.accentColor.opacity(0.12), in: Capsule())
                                    .foregroundStyle(Color.accentColor)
                            }
                        }
                    }
                    .font(.caption)
                }

                LabeledContent("Created") {
                    Text(page.createdAt, style: .date)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .font(.caption)

                LabeledContent("Modified") {
                    Text(page.modifiedAt, style: .relative)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .font(.caption)
            }

            // Backlinks — grouped by source page
            Section("Linked References (\(backlinks.count))") {
                if backlinks.isEmpty {
                    Text("No backlinks")
                        .foregroundStyle(.tertiary)
                        .font(.caption)
                } else {
                    ForEach(groupedBacklinks, id: \.source) { group in
                        VStack(alignment: .leading, spacing: 4) {
                            // Source page header
                            Button {
                                navigateTo(noteId: group.source)
                            } label: {
                                HStack(spacing: 4) {
                                    Image(systemName: "doc.text")
                                        .font(.caption2)
                                    Text(pageTitle(for: group.source))
                                        .font(.caption).bold()
                                        .foregroundStyle(Color.accentColor)
                                }
                            }
                            .buttonStyle(.plain)

                            // Context lines from that page
                            ForEach(group.links) { link in
                                if let text = link.text, !text.isEmpty {
                                    Text(text)
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                        .lineLimit(2)
                                        .padding(.leading, 16)
                                }
                            }
                        }
                        .padding(.vertical, 2)
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
                        Button {
                            navigateTo(noteId: link.target)
                        } label: {
                            HStack(spacing: 6) {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(pageTitle(for: link.target))
                                        .font(.caption).bold()
                                        .foregroundStyle(Color.accentColor)
                                    if let text = link.text, !text.isEmpty {
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
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 2)
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

    private func pageTitle(for id: String) -> String {
        appState.pages.first { $0.id == id }?.title ?? id
    }
}
