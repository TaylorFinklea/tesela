import SwiftUI

/// Graphite Search — native search chrome over the same live search state
/// the legacy `SearchView` uses (`runSearch`, `searchHits`,
/// `searchInFlight`, `searchError`). Pages and tags navigate to their
/// Graphite page route; block matches are shown as non-tappable context rows.
struct GrSearchView: View {
    @ObservedObject var mosaic: MockMosaicService
    var backend: BackendSettings? = nil

    @Environment(\.theme) private var theme

    @State private var query: String = ""
    @State private var navigationPath = NavigationPath()

    struct SearchSection: Identifiable, Equatable {
        let kind: SearchResult.Kind
        let title: String
        let results: [SearchResult]

        var id: SearchResult.Kind { kind }
        var count: Int { results.count }
    }

    var body: some View {
        NavigationStack(path: $navigationPath) {
            VStack(spacing: 0) {
                GrHeader(title: "Search", subtitle: "FIND")
                content
            }
            .background(theme.bg)
            .navigationDestination(for: GrPageRoute.self) { route in
                GrPageView(slug: route.slug, mosaic: mosaic, path: $navigationPath)
                    .environment(\.theme, theme)
            }
        }
        .searchable(text: $query, prompt: "Search pages, blocks, tags…")
        .task(id: query) {
            try? await Task.sleep(nanoseconds: 250_000_000)
            if !Task.isCancelled {
                await mosaic.runSearch(query)
            }
        }
    }

    @ViewBuilder
    private var content: some View {
        let results = mosaic.searchHits
        let sections = Self.sections(for: results)

        if query.isEmpty {
            ContentUnavailableView {
                Label("Search the mosaic", systemImage: "magnifyingglass")
            } description: {
                Text("Search for pages, daily entries, or block content.")
            }
            .background(theme.bg)
        } else if let err = mosaic.searchError {
            ContentUnavailableView {
                Label("Search failed", systemImage: "exclamationmark.triangle")
            } description: {
                Text(err)
            }
            .background(theme.bg)
        } else if mosaic.searchInFlight && results.isEmpty {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(theme.bg)
        } else if results.isEmpty {
            ContentUnavailableView(
                "No results",
                systemImage: "doc.text.magnifyingglass",
                description: Text("Nothing in the mosaic matches **\(query)**")
            )
            .background(theme.bg)
        } else {
            List {
                ForEach(sections) { section in
                    Section {
                        ForEach(section.results) { result in
                            row(for: result)
                                .listRowBackground(theme.bg2)
                        }
                    } header: {
                        Text("\(section.title) · \(section.count)")
                            .font(.system(size: 10, design: .monospaced))
                            .tracking(1.2)
                            .foregroundStyle(theme.fgFaint)
                    }
                }
            }
            .listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
            .background(theme.bg)
        }
    }

    static func sections(for results: [SearchResult]) -> [SearchSection] {
        let grouped = Dictionary(grouping: results, by: \.kind)
        return [SearchResult.Kind.page, .block, .tag].compactMap { kind in
            guard let rows = grouped[kind], !rows.isEmpty else { return nil }
            return SearchSection(kind: kind, title: kindLabel(kind), results: rows)
        }
    }

    private static func kindLabel(_ k: SearchResult.Kind) -> String {
        switch k {
        case .page: return "Pages"
        case .block: return "Blocks"
        case .tag: return "Tags"
        }
    }

    @ViewBuilder
    private func row(for r: SearchResult) -> some View {
        if r.kind == .page || r.kind == .tag {
            NavigationLink(value: GrPageRoute(slug: r.id)) {
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
