import SwiftUI
import UIKit

/// PageView — the page-renderer host. Mirrors the canvas's Tile-Page
/// screen with:
///   • Title chrome (kind badge · slug · title · meta)
///   • PageTagsChips strip directly under the title (decision #8)
///   • Page body (block list)
///   • Collapsible derived-only Peek segmented control (decision #7)
///
/// Pushed onto a NavigationStack from LibraryView or wiki-link taps.
struct PageView: View {
    let page: Page
    @ObservedObject var mosaic: MockMosaicService
    @ObservedObject var pageStack: PageStack
    @ObservedObject var syncState: SyncState

    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss
    @Environment(\.captureContext) private var captureContext
    @State private var tags: [String] = []
    @State private var peekOpen: Bool = false
    @State private var peekSegment: PeekSegment = .backlinks
    @State private var showProperties: Bool = false
    @State private var showOpenPages: Bool = false
    @State private var editingBlockId: String? = nil
    @State private var collapsedBlockIds: Set<String> = []

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                titleChrome
                PageTagsChips(
                    pageId: page.id,
                    tags: $tags,
                    knownTags: mosaic.tags.map { $0.title }
                )
                Divider().background(theme.lineSoft)
                pageBody
                peekSection
                Spacer().frame(height: 24)
            }
        }
        .background(theme.bg)
        .onChange(of: editingBlockId) { _, newValue in
            // Defer live remote refreshes while editing — see DailyView.
            mosaic.isEditingBlock = (newValue != nil)
            if let id = newValue,
               let block = mosaic.loadedPageBlocks[page.id]?.first(where: { $0.id == id })
            {
                captureContext.focusedBlock = CaptureBlockRef(
                    id: id,
                    preview: block.text,
                    pageSlug: page.id
                )
            } else {
                captureContext.focusedBlock = nil
            }
        }
        .onDisappear {
            captureContext.focusedBlock = nil
            mosaic.isEditingBlock = false
        }
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    showOpenPages = true
                } label: {
                    Image(systemName: "square.stack.3d.up")
                        .font(.system(size: 18))
                        .foregroundStyle(theme.fgMuted)
                }
                .accessibilityLabel("Open pages")
            }
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    showProperties = true
                } label: {
                    Icon(name: .more, size: 20)
                        .foregroundStyle(theme.fgMuted)
                }
            }
        }
        .sheet(isPresented: $showProperties) {
            PagePropertiesSheet(page: page, tags: $tags)
                .environment(\.theme, theme)
        }
        .sheet(isPresented: $showOpenPages) {
            OpenPagesOverlay(
                stack: pageStack,
                isPresented: $showOpenPages,
                onJump: { _ in /* Phase 15: navigate to a different page */ }
            )
            .environment(\.theme, theme)
        }
        .onAppear {
            // Mock: pre-populate with a couple of tags so the strip
            // demonstrates itself. Real data lands in Phase 15.
            if tags.isEmpty {
                tags = page.type == "note" ? ["prism", "tesela"] : []
            }
        }
    }

    // ── Title chrome ────────────────────────────────────────────────────

    private var titleChrome: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                KindBadge(kind: page.type)
                Text("notes/\(page.slug).md")
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgSubtle)
                Spacer()
                Button {
                    mosaic.togglePin(page: page)
                } label: {
                    Image(systemName: mosaic.isPinned(page.id) ? "star.fill" : "star")
                        .font(.system(size: 13))
                        .foregroundStyle(mosaic.isPinned(page.id) ? theme.accentPrimary : theme.fgSubtle)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(mosaic.isPinned(page.id) ? "Unpin page" : "Pin page")
            }
            HStack(spacing: 8) {
                if syncState.showsModifiedMarker {
                    Circle()
                        .fill(theme.typeTask)
                        .frame(width: 8, height: 8)
                        .accessibilityLabel("Local edits pending sync")
                }
                Text(page.title)
                    .font(.system(size: 26, weight: .semibold))
                    .tracking(-0.26)
                    .foregroundStyle(theme.fgDefault)
            }
            Text("\(page.blocks) blocks · \(page.refs) refs · edited \(page.edited)")
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
        }
        .padding(.horizontal, 18)
        .padding(.top, 12)
    }

    // ── Body ────────────────────────────────────────────────────────────

    /// Real page body — fetched via `mosaic.loadPage` on appear and
    /// rendered from `mosaic.loadedPageBlocks[page.id]`. Shows a
    /// loading shimmer while the request is in flight and a failure
    /// banner if the fetch errored.
    private var pageBody: some View {
        VStack(alignment: .leading, spacing: 0) {
            Spacer().frame(height: 12)
            switch mosaic.pageLoadStates[page.id] {
            case .loading:
                pageBodyLoading
            case .failed(let message):
                pageBodyError(message)
            case .ready, .idle, nil:
                renderedBlocks
            }
        }
        .task(id: page.id) {
            await mosaic.loadPage(id: page.id)
        }
    }

    @ViewBuilder
    private var renderedBlocks: some View {
        let blocks = mosaic.loadedPageBlocks[page.id] ?? []
        ForEach(BlockFold.visibleBlocks(in: blocks, collapsed: collapsedBlockIds)) { block in
            BlockRow(
                id: block.id,
                kind: block.kind,
                text: block.displayText,
                indent: block.indent,
                isDone: block.done,
                tags: block.tags,
                properties: block.properties,
                isEditing: editingBlockId == block.id,
                isFoldable: BlockFold.hasChildren(block: block, in: blocks),
                isCollapsed: collapsedBlockIds.contains(block.id),
                onToggleFold: { toggleFold(block.id) },
                onToggleTask: { togglePageTask(block.id) },
                onTap: { editingBlockId = block.id },
                onCommitEdit: { newText in
                    mosaic.editPageBlock(pageId: page.id, blockId: block.id, text: newText)
                    editingBlockId = nil
                },
                onTextChanged: { newText in
                    mosaic.editPageBlock(pageId: page.id, blockId: block.id, text: newText)
                },
                onMenuAction: { action in
                    handlePageAction(action, on: block)
                },
                onSetProperties: { updated in
                    mosaic.setBlockProperties(id: block.id, properties: updated)
                },
                onSkipRecurrence: {
                    Task { try? await mosaic.recurBump(blockId: block.id, mode: .skip) }
                }
            )
        }
        // "+ add block" affordance, always visible at the bottom.
        Button {
            let newId = mosaic.appendPageBlock(pageId: page.id)
            editingBlockId = newId
        } label: {
            HStack(spacing: 10) {
                Image(systemName: "plus.circle")
                    .font(.system(size: 14, weight: .regular))
                    .foregroundStyle(theme.fgFaint)
                    .frame(width: 14, alignment: .center)
                Text("Add block")
                    .font(.system(size: 13))
                    .foregroundStyle(theme.fgFaint)
                Spacer()
            }
            .padding(.leading, 18)
            .padding(.trailing, 18)
            .padding(.vertical, 8)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func toggleFold(_ blockId: String) {
        if collapsedBlockIds.contains(blockId) {
            collapsedBlockIds.remove(blockId)
        } else {
            collapsedBlockIds.insert(blockId)
        }
    }

    private var pageBodyLoading: some View {
        VStack(alignment: .leading, spacing: 10) {
            ForEach(0..<5, id: \.self) { _ in
                RoundedRectangle(cornerRadius: 3)
                    .fill(theme.bg2)
                    .frame(height: 14)
                    .frame(maxWidth: .infinity)
            }
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 12)
        .opacity(0.6)
    }

    private func pageBodyError(_ message: String) -> some View {
        HStack(spacing: 10) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(theme.typeTask)
            VStack(alignment: .leading, spacing: 2) {
                Text("Couldn't load this page")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(theme.fgDefault)
                Text(message)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }
            Spacer()
            Button {
                Task { await mosaic.loadPage(id: page.id, force: true) }
            } label: {
                Text("Retry")
                    .font(.system(size: 12, weight: .semibold, design: .monospaced))
                    .foregroundStyle(theme.accentPrimary)
            }
            .buttonStyle(.plain)
        }
        .padding(14)
        .background(theme.bg2)
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
    }

    /// Toggle a task block inside the loaded page body, then push the
    /// new blocks list back to the server.
    private func togglePageTask(_ blockId: String) {
        var blocks = mosaic.loadedPageBlocks[page.id] ?? []
        guard let idx = blocks.firstIndex(where: { $0.id == blockId }),
              blocks[idx].kind == .task
        else { return }
        blocks[idx].done.toggle()
        Task { await mosaic.pushPage(id: page.id, blocks: blocks) }
    }

    private func handlePageAction(_ action: BlockAction, on block: Block) {
        switch action {
        case .edit:
            editingBlockId = block.id
        case .delete, .archive:
            mosaic.deletePageBlock(pageId: page.id, blockId: block.id)
        case .yankLink:
            UIPasteboard.general.string = "tesela://block/\(block.id)"
        case .indent:
            mosaic.indentPageBlock(pageId: page.id, blockId: block.id, by: 1)
        case .promote, .convertToTag, .moveTo:
            break
        }
    }

    // ── Peek (derived-only) ─────────────────────────────────────────────

    private var peekSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Toggle row
            Button {
                withAnimation(.easeInOut(duration: 0.18)) { peekOpen.toggle() }
            } label: {
                HStack(spacing: 6) {
                    Icon(name: peekOpen ? .chevDown : .chevRight, size: 14)
                        .foregroundStyle(theme.fgSubtle)
                    Text("Peek")
                        .font(.system(size: 10.5, design: .monospaced))
                        .tracking(0.8)
                        .foregroundStyle(theme.fgSubtle)
                    Spacer()
                    Text("derived · 5 lenses")
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                }
                .padding(.horizontal, 18)
                .padding(.vertical, 10)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            if peekOpen {
                segmentedControl
                Divider().background(theme.lineSoft)
                segmentBody
            }
        }
        .background(
            peekOpen ? theme.bg2.opacity(0.5) : Color.clear
        )
        .overlay(alignment: .top) {
            Rectangle().fill(theme.lineSoft).frame(height: 1)
        }
        .padding(.top, 16)
    }

    private var segmentedControl: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 18) {
                ForEach(PeekSegment.allCases) { seg in
                    Button {
                        peekSegment = seg
                    } label: {
                        HStack(spacing: 4) {
                            Text(seg.label)
                            if let count = seg.count(mosaic, pageId: page.id) {
                                Text(String(count))
                                    .foregroundStyle(theme.fgFaint)
                            }
                        }
                        .font(.system(size: 11.5, weight: peekSegment == seg ? .semibold : .regular, design: .monospaced))
                        .foregroundStyle(peekSegment == seg ? theme.accentPrimary : theme.fgSubtle)
                        .padding(.vertical, 10)
                        .overlay(alignment: .bottom) {
                            Rectangle()
                                .fill(peekSegment == seg ? theme.accentPrimary : .clear)
                                .frame(height: 2)
                                .offset(y: 1)
                        }
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 18)
        }
    }

    @ViewBuilder
    private var segmentBody: some View {
        switch peekSegment {
        case .backlinks: BacklinksView(mosaic: mosaic, pageId: page.id)
        case .outline:   OutlineLensView(mosaic: mosaic, pageId: page.id)
        case .props:     PropsLensView(page: page, tags: tags)
        case .tasks:     TasksLensView(mosaic: mosaic, pageId: page.id)
        case .graph:     GraphLensView(mosaic: mosaic, pageId: page.id)
        }
    }
}

// MARK: - Peek segments

enum PeekSegment: String, CaseIterable, Identifiable {
    case backlinks, outline, props, tasks, graph

    var id: String { rawValue }
    var label: String { rawValue }

    @MainActor
    func count(_ mosaic: MockMosaicService, pageId: String) -> Int? {
        switch self {
        case .backlinks:
            return mosaic.loadedBacklinks[pageId]?.count
        case .outline:
            return mosaic.loadedPageBlocks[pageId].map {
                OutlineEntry.derive(from: $0).count
            }
        case .tasks:
            return mosaic.loadedPageBlocks[pageId].map { blocks in
                blocks.filter { $0.kind == .task }.count
            }
        case .graph:
            return mosaic.loadedLinks[pageId]?.count
        case .props:
            return nil
        }
    }
}

// MARK: - Derived lens views

struct BacklinksView: View {
    @ObservedObject var mosaic: MockMosaicService
    let pageId: String
    @Environment(\.theme) private var theme
    @Environment(\.openURL) private var openURL

    var body: some View {
        let backlinks = mosaic.loadedBacklinks[pageId] ?? []
        VStack(alignment: .leading, spacing: 0) {
            if backlinks.isEmpty {
                lensPlaceholder("No backlinks yet")
            } else {
                ForEach(backlinks) { b in
                    Button {
                        openPage(b.pageId, openURL)
                    } label: {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(b.from)
                                .font(.system(size: 11, design: .monospaced))
                                .foregroundStyle(theme.accentPrimary)
                            Text(b.snippet)
                                .font(.system(size: 13.5))
                                .foregroundStyle(theme.fgMuted)
                                .lineSpacing(2)
                        }
                        .padding(.horizontal, 18)
                        .padding(.vertical, 10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .contentShape(Rectangle())
                        .overlay(alignment: .bottom) {
                            Rectangle().fill(theme.lineSoft).frame(height: 1)
                        }
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }
}

struct OutlineLensView: View {
    @ObservedObject var mosaic: MockMosaicService
    let pageId: String
    @Environment(\.theme) private var theme

    var body: some View {
        let entries = OutlineEntry.derive(from: mosaic.loadedPageBlocks[pageId] ?? [])
        VStack(alignment: .leading, spacing: 0) {
            if entries.isEmpty {
                lensPlaceholder("No outline")
            } else {
                ForEach(entries) { o in
                    Text(o.text)
                        .font(.system(size: 14))
                        .foregroundStyle(o.depth == 0 ? theme.fgDefault : theme.fgMuted)
                        .lineLimit(1)
                        .padding(.leading, CGFloat(18 + o.depth * 18))
                        .padding(.trailing, 18)
                        .padding(.vertical, 8)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .overlay(alignment: .bottom) {
                            Rectangle().fill(theme.lineSoft).frame(height: 1)
                        }
                }
            }
        }
    }
}

/// Shared muted placeholder for a Peek lens that has nothing to show.
private func lensPlaceholder(_ text: String) -> some View {
    Text(text)
        .font(.system(size: 12, design: .monospaced))
        .foregroundStyle(Color.secondary)
        .padding(.horizontal, 18)
        .padding(.vertical, 14)
        .frame(maxWidth: .infinity, alignment: .leading)
}

/// Navigate to a page by id via the app's `tesela://page/<slug>` link
/// scheme — the enclosing NavigationStack's `openURL` handler routes it.
/// A no-op when the id is empty (unresolved link).
private func openPage(_ pageId: String, _ openURL: OpenURLAction) {
    guard !pageId.isEmpty, let url = URL(string: "tesela://page/\(pageId)") else { return }
    openURL(url)
}

struct PropsLensView: View {
    let page: Page
    let tags: [String]
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            propRow(key: "type",    value: page.type)
            propRow(key: "slug",    value: page.slug)
            propRow(key: "created", value: page.created.isEmpty ? "—" : page.created)
            propRow(key: "edited",  value: page.edited)
            HStack(alignment: .top, spacing: 14) {
                Text("tags")
                    .frame(width: 80, alignment: .leading)
                    .foregroundStyle(theme.fgFaint)
                if tags.isEmpty {
                    Text("—").foregroundStyle(theme.fgFaint)
                } else {
                    HStack(spacing: 4) {
                        ForEach(tags, id: \.self) { TagChip(value: $0) }
                    }
                }
                Spacer()
            }
            propRow(key: "refs", value: "\(page.refs) in")
        }
        .font(.system(size: 12, design: .monospaced))
        .padding(.horizontal, 18)
        .padding(.vertical, 14)
    }

    private func propRow(key: String, value: String) -> some View {
        HStack(alignment: .top, spacing: 14) {
            Text(key)
                .frame(width: 80, alignment: .leading)
                .foregroundStyle(theme.fgFaint)
            Text(value)
                .foregroundStyle(theme.fgDefault)
            Spacer()
        }
    }
}

struct TasksLensView: View {
    @ObservedObject var mosaic: MockMosaicService
    let pageId: String
    @Environment(\.theme) private var theme

    var body: some View {
        let tasks = (mosaic.loadedPageBlocks[pageId] ?? []).filter { $0.kind == .task }
        VStack(alignment: .leading, spacing: 0) {
            if tasks.isEmpty {
                lensPlaceholder("No tasks on this page")
            } else {
                ForEach(tasks) { task in
                    BlockRow(
                        id: task.id,
                        kind: .task,
                        text: task.displayText,
                        isDone: task.done,
                        tags: task.tags
                    )
                }
            }
        }
    }
}

/// The Peek "graph" lens — for now an outgoing-links list (the pages
/// this page links to). A real in-app graph render is a later roadmap
/// item; this gives the lens real, navigable data in the meantime.
struct GraphLensView: View {
    @ObservedObject var mosaic: MockMosaicService
    let pageId: String
    @Environment(\.theme) private var theme
    @Environment(\.openURL) private var openURL

    var body: some View {
        let links = mosaic.loadedLinks[pageId] ?? []
        VStack(alignment: .leading, spacing: 0) {
            if links.isEmpty {
                lensPlaceholder("No outgoing links")
            } else {
                ForEach(links) { link in
                    Button {
                        openPage(link.pageId, openURL)
                    } label: {
                        VStack(alignment: .leading, spacing: 4) {
                            HStack(spacing: 6) {
                                Image(systemName: "arrow.up.right")
                                    .font(.system(size: 10))
                                Text(link.from)
                                    .font(.system(size: 11, design: .monospaced))
                            }
                            .foregroundStyle(theme.accentPrimary)
                            Text(link.snippet)
                                .font(.system(size: 13.5))
                                .foregroundStyle(theme.fgMuted)
                                .lineSpacing(2)
                        }
                        .padding(.horizontal, 18)
                        .padding(.vertical, 10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .contentShape(Rectangle())
                        .overlay(alignment: .bottom) {
                            Rectangle().fill(theme.lineSoft).frame(height: 1)
                        }
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }
}
