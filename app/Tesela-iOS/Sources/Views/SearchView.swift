import SwiftUI

/// Search tab content. Hosted under a `Tab(role: .search)` so iOS 26
/// renders the search chip on the right side of the Liquid Glass tab
/// bar. Per decision #6, verbs do NOT appear here — they live in the
/// capture sheet's palette mode triggered by typing `:`.
struct SearchView: View {
    @ObservedObject var mosaic: MockMosaicService
    @ObservedObject var pageStack: PageStack
    @ObservedObject var syncState: SyncState

    @Environment(\.theme) private var theme
    @State private var query: String = ""

    var body: some View {
        NavigationStack {
            content
                .navigationTitle("Search")
                .navigationBarTitleDisplayMode(.large)
                .searchable(text: $query, placement: .navigationBarDrawer(displayMode: .always), prompt: "Search pages, blocks, tags…")
                .navigationDestination(for: Page.self) { page in
                    PageView(page: page, mosaic: mosaic, pageStack: pageStack, syncState: syncState)
                        .environment(\.theme, theme)
                        .onAppear { pageStack.open(page) }
                }
        }
    }

    @ViewBuilder
    private var content: some View {
        let results = mosaic.search(query)
        let grouped = Dictionary(grouping: results, by: \.kind)

        if results.isEmpty && !query.isEmpty {
            ContentUnavailableView(
                "No results",
                systemImage: "doc.text.magnifyingglass",
                description: Text("Nothing in the mosaic matches **\(query)**")
            )
            .background(theme.bg)
        } else {
            List {
                ForEach([SearchResult.Kind.page, .block, .tag], id: \.self) { kind in
                    if let rows = grouped[kind], !rows.isEmpty {
                        Section {
                            ForEach(rows) { result in
                                row(for: result)
                                    .listRowBackground(theme.bg2)
                            }
                        } header: {
                            Text("\(kindLabel(kind)) · \(rows.count)")
                                .font(.system(size: 10, design: .monospaced))
                                .tracking(1.2)
                                .foregroundStyle(theme.fgFaint)
                        }
                    }
                }
            }
            .listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
            .background(theme.bg)
        }
    }

    private func kindLabel(_ k: SearchResult.Kind) -> String {
        switch k {
        case .page: return "Pages"
        case .block: return "Blocks"
        case .tag: return "Tags"
        }
    }

    @ViewBuilder
    private func row(for r: SearchResult) -> some View {
        if r.kind == .page, let page = mosaic.pages.first(where: { $0.id == r.id }) {
            NavigationLink(value: page) {
                rowBody(for: r)
            }
        } else {
            rowBody(for: r)
        }
    }

    private func rowBody(for r: SearchResult) -> some View {
        HStack(alignment: .top, spacing: 12) {
            VStack(alignment: .leading, spacing: 3) {
                Text(r.title)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(theme.fgDefault)
                    .lineLimit(1)
                // Render snippet with **highlight** spans tinted by accent.
                snippetText(r.snippet)
                    .font(.system(size: 13))
                    .foregroundStyle(theme.fgMuted)
                    .lineSpacing(2)
            }
            Spacer()
            KindBadge(kind: r.kind == .page ? "note" : r.kind.rawValue)
        }
        .padding(.vertical, 4)
    }

    /// Parses `**bold**` spans in a search snippet and tints them with the
    /// accent color. Mirrors the canvas's snippet highlighting.
    private func snippetText(_ snippet: String) -> Text {
        let pattern = try? NSRegularExpression(pattern: #"\*\*([^*]+)\*\*"#)
        guard let re = pattern else { return Text(snippet) }
        let ns = snippet as NSString
        let matches = re.matches(in: snippet, range: NSRange(location: 0, length: ns.length))
        var out = Text("")
        var cursor = 0
        for m in matches {
            if m.range.location > cursor {
                out = out + Text(ns.substring(with: NSRange(location: cursor, length: m.range.location - cursor)))
            }
            let bold = ns.substring(with: NSRange(location: m.range.location + 2, length: m.range.length - 4))
            out = out + Text(bold).bold().foregroundColor(theme.accentPrimary)
            cursor = m.range.location + m.range.length
        }
        if cursor < ns.length {
            out = out + Text(ns.substring(from: cursor))
        }
        return out
    }
}
