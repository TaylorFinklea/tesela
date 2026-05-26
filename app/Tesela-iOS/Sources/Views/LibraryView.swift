import SwiftUI

/// Library tab — one flat list of every page with a horizontal type-filter
/// chip strip across the top. Per decision #2 in the iOS Tile follow-up:
/// "tags are pages, so don't subordinate them as a Library sub-segment."
/// All page types appear in one stream; the strip chooses which subset.
struct LibraryView: View {
    @ObservedObject var mosaic: MockMosaicService
    @ObservedObject var appearance: AppearanceController
    @ObservedObject var pageStack: PageStack
    @ObservedObject var syncState: SyncState
    @ObservedObject var backend: BackendSettings
    var relayTicker: RelayTicker? = nil
    var transcription: TranscriptionStore? = nil

    @Environment(\.theme) private var theme
    @Environment(\.captureContext) private var captureContext
    @EnvironmentObject private var mosaicRegistry: MosaicRegistry
    @State private var activeFilter: LibraryFilter = .all
    @State private var navigationPath = NavigationPath()
    @State private var showSettings: Bool = false
    @State private var showMosaicSwitcher: Bool = false

    var body: some View {
        NavigationStack(path: $navigationPath) {
            VStack(spacing: 0) {
                TabHeader(
                    title: "Library",
                    syncStatus: TabHeader.SyncDotState(mosaic.connection),
                    onTapSettings: { showSettings = true },
                    onTapMosaic: { showMosaicSwitcher = true }
                )
                ConnectionBanner(connection: mosaic.connection) {
                    Task { await mosaic.refresh(from: backend.backend, userInitiated: true) }
                }
                filterStrip
                content
            }
            .background(theme.bg)
            .sheet(isPresented: $showMosaicSwitcher) {
                MosaicSwitcherSheet(registry: mosaicRegistry)
                    .environment(\.theme, theme)
            }
            .sheet(isPresented: $showSettings) {
                if let relayTicker {
                    SettingsView(
                        appearance: appearance,
                        mosaic: mosaic,
                        syncState: syncState,
                        backend: backend,
                        relayTicker: relayTicker,
                        transcription: transcription
                    )
                    .environment(\.theme, theme)
                    .environment(\.density, appearance.density)
                }
            }
            .navigationDestination(for: Page.self) { page in
                PageView(page: page, mosaic: mosaic, pageStack: pageStack, syncState: syncState)
                    .environment(\.theme, theme)
                    .onAppear {
                        pageStack.open(page)
                        captureContext.currentPage = CapturePageRef(slug: page.slug, title: page.title)
                    }
                    .onDisappear { captureContext.currentPage = nil }
            }
            .navigationDestination(for: DailyPageRoute.self) { route in
                if let page = route.resolvedPage(mosaic) {
                    PageView(page: page, mosaic: mosaic, pageStack: pageStack, syncState: syncState)
                        .environment(\.theme, theme)
                        .onAppear {
                            pageStack.open(page)
                            captureContext.currentPage = CapturePageRef(slug: page.slug, title: page.title)
                        }
                        .onDisappear { captureContext.currentPage = nil }
                }
            }
            .environment(\.openURL, OpenURLAction { url in
                if let slug = TeselaLink.pageSlug(from: url) {
                    navigationPath.append(DailyPageRoute(slug: slug))
                    return .handled
                }
                return .systemAction
            })
            .navigationDestination(for: Tag.self) { tag in
                TagViewPlaceholder(tag: tag)
                    .environment(\.theme, theme)
            }
        }
    }

    // ── Type-filter strip ───────────────────────────────────────────────

    private var filterStrip: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(LibraryFilter.allCases) { f in
                    chip(for: f)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
        }
        .scrollClipDisabled()
        .background(theme.bg)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(theme.lineSoft)
                .frame(height: 1)
        }
    }

    private func chip(for f: LibraryFilter) -> some View {
        let on = (activeFilter == f)
        return Button {
            activeFilter = f
        } label: {
            HStack(spacing: 4) {
                Text(f.label)
                if let count = f.count(mosaic) {
                    Text(String(count))
                        .foregroundStyle(on ? theme.bg.opacity(0.7) : theme.fgFaint)
                }
            }
            .font(.system(size: 11.5, weight: on ? .semibold : .regular, design: .monospaced))
            .foregroundStyle(on ? theme.bg : theme.fgMuted)
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(
                Capsule()
                    .fill(on ? theme.accentPrimary : Color.clear)
            )
            .overlay(
                Capsule()
                    .stroke(on ? Color.clear : theme.line, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
    }

    // ── List content ────────────────────────────────────────────────────

    @ViewBuilder
    private var content: some View {
        switch activeFilter {
        case .workspace:
            workspacePlaceholder
        case .scratch:
            scratchList
        case .tags:
            tagList
        default:
            pageList
        }
    }

    private var pageList: some View {
        List {
            // Recent eyebrow only when no filter narrows the list.
            if activeFilter == .all {
                Section {
                    ForEach(mosaic.recent) { entry in
                        Button {
                            if let page = mosaic.pages.first(where: { $0.id == entry.id }) {
                                navigationPath.append(page)
                            }
                        } label: {
                            HStack {
                                Text(entry.title)
                                    .foregroundStyle(theme.fgDefault)
                                Spacer()
                                Text(entry.at)
                                    .font(.system(size: 10.5, design: .monospaced))
                                    .foregroundStyle(theme.fgFaint)
                            }
                        }
                        .buttonStyle(.plain)
                        .listRowBackground(theme.bg2)
                    }
                } header: {
                    Text("Recent")
                        .font(.system(size: 10, design: .monospaced))
                        .tracking(1.2)
                        .foregroundStyle(theme.fgFaint)
                }
            }

            Section {
                ForEach(filteredPages) { page in
                    NavigationLink(value: page) {
                        PageRow(page: page)
                    }
                    .listRowBackground(theme.bg2)
                }
            } header: {
                Text(activeFilter == .all ? "All pages" : activeFilter.label)
                    .font(.system(size: 10, design: .monospaced))
                    .tracking(1.2)
                    .foregroundStyle(theme.fgFaint)
            }
        }
        .listStyle(.insetGrouped)
        .scrollContentBackground(.hidden)
        .background(theme.bg)
    }

    private var tagList: some View {
        List {
            ForEach(mosaic.tags) { tag in
                NavigationLink(value: tag) {
                    TagRow(tag: tag)
                }
                .listRowBackground(theme.bg2)
            }
        }
        .listStyle(.insetGrouped)
        .scrollContentBackground(.hidden)
        .background(theme.bg)
    }

    private var scratchList: some View {
        let scratches = mosaic.pages.filter { $0.type == "scratch" }
        return Group {
            if scratches.isEmpty {
                emptyState(
                    title: "No scratches yet",
                    hint: "Type `:scratch` in the capture sheet to start one."
                )
            } else {
                List {
                    ForEach(scratches) { page in
                        NavigationLink(value: page) {
                            PageRow(page: page)
                        }
                        .listRowBackground(theme.bg2)
                    }
                }
                .listStyle(.insetGrouped)
                .scrollContentBackground(.hidden)
                .background(theme.bg)
            }
        }
    }

    private var workspacePlaceholder: some View {
        WorkspaceGridView(mosaic: mosaic)
    }

    private func emptyState(title: String, hint: String) -> some View {
        VStack(spacing: 10) {
            Text(title)
                .font(.system(size: 18, weight: .semibold))
                .foregroundStyle(theme.fgDefault)
            Text(hint)
                .font(.system(size: 11.5, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(theme.bg)
    }

    private var filteredPages: [Page] {
        switch activeFilter {
        case .all:
            return mosaic.pages.filter { !$0.hidden }
        case .pages:
            return mosaic.pages.filter { $0.type == "note" && !$0.hidden }
        case .daily:
            return mosaic.pages.filter { $0.type == "daily" && !$0.hidden }
        case .project:
            return mosaic.pages.filter { $0.type == "project" && !$0.hidden }
        case .person:
            return mosaic.pages.filter { $0.type == "person" && !$0.hidden }
        case .query:
            return mosaic.pages.filter { $0.type == "query" && !$0.hidden }
        case .scratch, .tags, .workspace:
            return [] // handled by their own list bodies
        }
    }
}

// MARK: - Filter chips

enum LibraryFilter: String, CaseIterable, Identifiable, Hashable {
    case all, pages, tags, daily, project, person, query, workspace, scratch

    var id: String { rawValue }

    var label: String {
        switch self {
        case .all:       return "All"
        case .pages:     return "Pages"
        case .tags:      return "Tags"
        case .daily:     return "Daily"
        case .project:   return "Projects"
        case .person:    return "People"
        case .query:     return "Queries"
        case .workspace: return "Workspace"
        case .scratch:   return "Scratch"
        }
    }

    /// Count to render after the chip label. Nil for filters where the
    /// number adds no useful information (workspace, all).
    @MainActor
    func count(_ mosaic: MockMosaicService) -> Int? {
        switch self {
        case .all, .workspace:
            return nil
        case .pages:
            return mosaic.pages.filter { $0.type == "note" && !$0.hidden }.count
        case .tags:
            return mosaic.tags.count
        case .daily:
            return mosaic.pages.filter { $0.type == "daily" }.count
        case .project:
            return mosaic.pages.filter { $0.type == "project" }.count
        case .person:
            return mosaic.pages.filter { $0.type == "person" }.count
        case .query:
            return mosaic.pages.filter { $0.type == "query" }.count
        case .scratch:
            return mosaic.pages.filter { $0.type == "scratch" }.count
        }
    }
}

// MARK: - Row views

struct PageRow: View {
    let page: Page

    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(page.title)
                    .font(.system(size: 14.5))
                    .foregroundStyle(theme.fgDefault)
                    .lineLimit(1)
                Text("notes/\(page.slug).md")
                    .font(.system(size: 10.5, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
                    .lineLimit(1)
            }
            Spacer()
            VStack(alignment: .trailing, spacing: 4) {
                KindBadge(kind: page.type)
                Text(page.edited)
                    .font(.system(size: 10.5, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }
        }
        .padding(.vertical, 4)
    }
}

struct TagRow: View {
    let tag: Tag

    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 0) {
                    Text("#")
                        .foregroundStyle(theme.fgFaint)
                    if let parent = tag.parent {
                        Text("\(parent)/")
                            .foregroundStyle(theme.fgFaint)
                    }
                    Text(tag.title)
                        .foregroundStyle(theme.fgDefault)
                }
                .font(.system(size: 13.5, design: .monospaced))
                Text("tags/\(tag.slug).md")
                    .font(.system(size: 10.5, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
                    .lineLimit(1)
            }
            Spacer()
            VStack(alignment: .trailing, spacing: 4) {
                KindBadge(kind: "tag")
                Text("\(tag.count) refs")
                    .font(.system(size: 10.5, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }
        }
        .padding(.vertical, 4)
    }
}

// MARK: - Tag destination placeholder (Phase 6 tag composite TBD)

struct TagViewPlaceholder: View {
    let tag: Tag
    @Environment(\.theme) private var theme
    var body: some View {
        VStack(spacing: 10) {
            Text("#\(tag.title)")
                .font(.system(size: 22, weight: .semibold, design: .monospaced))
                .foregroundStyle(theme.fgDefault)
            Text("Phase 6 — composite tag page renderer")
                .font(.system(size: 11.5, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(theme.bg)
    }
}
