import SwiftUI
import UIKit

/// Graphite Page — the page/project outliner, re-themed over the SAME
/// `MockMosaicService` page-load + `BlockRow` editor the legacy
/// `PageView` uses. Bodies bind to `mosaic.loadedPageBlocks[slug]`
/// (filled by `mosaic.loadPage(id:)`), edits route through
/// `editPageBlock` / `appendPageBlock` / `deletePageBlock` /
/// `pushPage`, and linked references read `mosaic.loadedBacklinks[slug]`.
///
/// Pushed onto a Graphite `NavigationStack` (from `GrDailyView` /
/// `GrLibraryView`). Wiki-link taps push further `GrPageView`s by
/// appending to the shared `path` binding, so the back-stack stays one
/// NavigationStack deep (mirrors `PageStack`'s host-agnostic intent).
struct GrPageView: View {
    let slug: String
    @ObservedObject var mosaic: MockMosaicService
    /// Shared stack binding so wiki-link taps push siblings rather than
    /// nesting a second NavigationStack.
    @Binding var path: NavigationPath

    @Environment(\.theme) private var theme
    @Environment(\.captureContext) private var captureContext

    @State private var editingBlockId: String? = nil
    @State private var collapsedBlockIds: Set<String> = []

    /// The resolved Page record (for title / type / meta). Falls back to
    /// a synthesized minimal Page when the slug isn't in `mosaic.pages`
    /// yet — `loadPage` fills the body regardless.
    private var page: Page {
        mosaic.pages.first(where: { $0.id == slug })
            ?? Page(id: slug, title: slug, slug: slug, type: "note",
                    edited: "", blocks: 0, refs: 0)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                pageHead
                Rectangle().fill(theme.lineSoft).frame(height: 1)
                pageBody
                taggedBlocks
                linkedRefs
                Spacer().frame(height: 96)
            }
        }
        .background(theme.bg)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    mosaic.togglePin(page: page)
                } label: {
                    GrIcon(name: "pin", size: 17)
                        .foregroundStyle(mosaic.isPinned(slug) ? theme.accentPrimary : theme.fgSubtle)
                }
                .accessibilityLabel(mosaic.isPinned(slug) ? "Unpin page" : "Pin page")
            }
        }
        .onChange(of: editingBlockId) { _, newValue in
            mosaic.isEditingBlock = (newValue != nil)
            if let id = newValue,
               let block = mosaic.loadedPageBlocks[slug]?.first(where: { $0.id == id })
            {
                captureContext.focusedBlock = CaptureBlockRef(
                    id: id, preview: block.text, pageSlug: slug
                )
            } else {
                captureContext.focusedBlock = nil
            }
        }
        .onDisappear {
            captureContext.focusedBlock = nil
            mosaic.isEditingBlock = false
        }
        .environment(\.openURL, OpenURLAction { url in
            if let target = TeselaLink.pageSlug(from: url) {
                path.append(GrPageRoute(slug: target))
                return .handled
            }
            return .systemAction
        })
        .task(id: slug) {
            await mosaic.loadPage(id: slug)
        }
    }

    // ── Page head (.grm-pagehead) ───────────────────────────────────────

    private var pageHead: some View {
        VStack(alignment: .leading, spacing: 9) {
            Text(page.title)
                .font(.system(size: 22, weight: .semibold))
                .tracking(-0.4)
                .foregroundStyle(theme.fgDefault)
                .lineSpacing(2)
            HStack(spacing: 9) {
                GrTypeTag(kind: page.type)
                Text("notes/\(page.slug).md")
                    .font(.system(size: 10.5, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
                    .lineLimit(1)
            }
            if page.blocks > 0 || page.refs > 0 || !page.edited.isEmpty {
                Text(metaLine)
                    .font(.system(size: 10.5, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.leading, 18)
        .padding(.trailing, 14)
        .padding(.top, 8)
        .padding(.bottom, 14)
    }

    private var metaLine: String {
        var parts: [String] = []
        if page.blocks > 0 { parts.append("\(page.blocks) blocks") }
        if page.refs > 0 { parts.append("\(page.refs) refs") }
        if !page.edited.isEmpty { parts.append("edited \(page.edited)") }
        return parts.joined(separator: " · ")
    }

    // ── Body (reuses BlockRow + page-load) ──────────────────────────────

    private var pageBody: some View {
        VStack(alignment: .leading, spacing: 0) {
            Spacer().frame(height: 10)
            switch mosaic.pageLoadStates[slug] {
            case .loading:
                bodyLoading
            case .failed(let message):
                bodyError(message)
            case .ready, .idle, .none:
                renderedBlocks
            }
        }
        .padding(.horizontal, 8)
    }

    /// Tag pages get a "Blocks tagged #X" section — the iOS parity for the
    /// web's `backlinks-of-tag` renderer (which lists `getTypedBlocks(tag)`).
    /// Runs `tag:<name>` through the same query engine as the inline widget.
    @ViewBuilder
    private var taggedBlocks: some View {
        if page.type == "tag" {
            VStack(alignment: .leading, spacing: 0) {
                Spacer().frame(height: 14)
                GrQueryWidget(
                    dsl: "tag:\(page.title)",
                    title: "Blocks tagged #\(page.title)",
                    mosaic: mosaic,
                    path: $path
                )
            }
            .padding(.horizontal, 8)
        }
    }

    @ViewBuilder
    private var renderedBlocks: some View {
        let blocks = mosaic.loadedPageBlocks[slug] ?? []
        ForEach(BlockFold.visibleBlocks(in: blocks, collapsed: collapsedBlockIds)) { block in
            let query = queryInfo(for: block)
            VStack(alignment: .leading, spacing: 6) {
            if !(query?.hideRow ?? false) {
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
                    mosaic.editPageBlock(pageId: slug, blockId: block.id, text: newText)
                    editingBlockId = nil
                },
                onTextChanged: { newText in
                    mosaic.editPageBlock(pageId: slug, blockId: block.id, text: newText)
                },
                onMenuAction: { action in handlePageAction(action, on: block) },
                onSplitToNewBlock: { committedText in
                    mosaic.editPageBlock(pageId: slug, blockId: block.id, text: committedText)
                    // Logseq Enter: inherit the current block's indent; an
                    // empty indented block outdents one level instead.
                    let isEmpty = committedText
                        .trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    if isEmpty && block.indent > 0 {
                        mosaic.indentPageBlock(pageId: slug, blockId: block.id, by: -1)
                        editingBlockId = block.id
                    } else {
                        let newId = mosaic.appendPageBlock(pageId: slug, kind: .note, indent: block.indent)
                        editingBlockId = newId
                    }
                },
                onIndent: { delta in
                    mosaic.indentPageBlock(pageId: slug, blockId: block.id, by: delta)
                },
                onCycleStatus: { mosaic.cycleBlockStatus(id: block.id, pageSlug: slug) },
                onSetProperties: { updated in
                    mosaic.setBlockProperties(id: block.id, properties: updated)
                },
                onSkipRecurrence: {
                    Task { try? await mosaic.recurBump(blockId: block.id, mode: .skip) }
                }
            )
            }
            if let query {
                GrQueryWidget(dsl: query.dsl, mosaic: mosaic, path: $path)
                    .padding(.leading, CGFloat(18 + block.indent * 18))
                    .padding(.trailing, 18)
            }
            }
        }
        addBlockRow
    }

    /// The query DSL for a block when it is a query block, plus whether the
    /// raw block row should be hidden. Two source forms, matching the web:
    ///  - a `query::` sub-line → `block.properties["query"]` (the row text is a
    ///    label/title, so keep it and render the widget beneath);
    ///  - a main-line `- query:: <dsl>` → the DSL is the block's own text, so
    ///    hide the raw row and let the widget replace it.
    private func queryInfo(for block: Block) -> (dsl: String, hideRow: Bool)? {
        if let prop = block.properties.first(where: { $0.key.lowercased() == "query" }) {
            let dsl = prop.value.trimmingCharacters(in: .whitespaces)
            if !dsl.isEmpty { return (dsl, false) }
        }
        let text = block.displayText.trimmingCharacters(in: .whitespaces)
        if text.lowercased().hasPrefix("query::") {
            let dsl = String(text.dropFirst("query::".count)).trimmingCharacters(in: .whitespaces)
            if !dsl.isEmpty { return (dsl, true) }
        }
        return nil
    }

    private func toggleFold(_ blockId: String) {
        if collapsedBlockIds.contains(blockId) {
            collapsedBlockIds.remove(blockId)
        } else {
            collapsedBlockIds.insert(blockId)
        }
    }

    private var addBlockRow: some View {
        Button {
            let newId = mosaic.appendPageBlock(pageId: slug)
            editingBlockId = newId
        } label: {
            HStack(spacing: 10) {
                GrIcon(name: "plus", size: 13)
                    .foregroundStyle(theme.fgFaint)
                    .frame(width: 14)
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

    private var bodyLoading: some View {
        VStack(alignment: .leading, spacing: 10) {
            ForEach(0..<5, id: \.self) { _ in
                RoundedRectangle(cornerRadius: 3)
                    .fill(theme.bg3)
                    .frame(height: 14)
                    .frame(maxWidth: .infinity)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 12)
        .opacity(0.6)
    }

    private func bodyError(_ message: String) -> some View {
        HStack(spacing: 10) {
            GrIcon(name: "bolt", size: 16)
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
                Task { await mosaic.loadPage(id: slug, force: true) }
            } label: {
                Text("Retry")
                    .font(.system(size: 12, weight: .semibold, design: .monospaced))
                    .foregroundStyle(theme.accentPrimary)
            }
            .buttonStyle(.plain)
        }
        .padding(14)
        .background(theme.bg2)
        .clipShape(RoundedRectangle(cornerRadius: 10))
        .padding(.horizontal, 6)
        .padding(.vertical, 8)
    }

    private func togglePageTask(_ blockId: String) {
        var blocks = mosaic.loadedPageBlocks[slug] ?? []
        guard let idx = blocks.firstIndex(where: { $0.id == blockId }),
              blocks[idx].kind == .task
        else { return }
        blocks[idx].done.toggle()
        Task { await mosaic.pushPage(id: slug, blocks: blocks) }
    }

    private func handlePageAction(_ action: BlockAction, on block: Block) {
        switch action {
        case .edit:            editingBlockId = block.id
        case .delete, .archive: mosaic.deletePageBlock(pageId: slug, blockId: block.id)
        case .yankLink:        UIPasteboard.general.string = "tesela://block/\(block.id)"
        case .indent:          mosaic.indentPageBlock(pageId: slug, blockId: block.id, by: 1)
        case .promote, .convertToTag, .moveTo: break
        }
    }

    // ── Linked references (.grm-refcard over loadedBacklinks) ───────────

    @ViewBuilder
    private var linkedRefs: some View {
        let backlinks = mosaic.loadedBacklinks[slug] ?? []
        if !backlinks.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text("LINKED REFERENCES")
                    .font(.system(size: 9.5, design: .monospaced))
                    .tracking(1.0)
                    .foregroundStyle(theme.fgFaint)
                    .padding(.horizontal, 8)
                    .padding(.top, 18)
                    .padding(.bottom, 2)
                ForEach(backlinks) { ref in
                    Button {
                        guard !ref.pageId.isEmpty else { return }
                        path.append(GrPageRoute(slug: ref.pageId))
                    } label: {
                        refCard(ref)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 10)
        }
    }

    private func refCard(_ ref: Backlink) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 7) {
                GrTypeDot(kind: "note")
                Text(ref.from)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgMuted)
                    .lineLimit(1)
            }
            Text(ref.snippet)
                .font(.system(size: 13))
                .foregroundStyle(theme.fgSubtle)
                .lineSpacing(2)
                .multilineTextAlignment(.leading)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 13)
        .padding(.vertical, 12)
        .background(theme.bg2)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(theme.lineSoft, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }
}
