import SwiftUI

// MARK: - BlockZoomView
// Logseq-style block drill-in: shows only a block and its children.
// Breadcrumb header for navigation back to the full page.

struct BlockZoomView: View {
    let page: Page
    let blockId: UUID
    @Environment(AppState.self) private var appState
    @State private var blocks: [Block] = []
    @State private var zoomedBlockText: String = ""
    @State private var saveTask: Task<Void, Never>?
    @State private var vimMode: VimMode = .normal

    /// Find the target block in the parsed tree and extract it + children
    private func findBlock(id: UUID, in tree: [Block]) -> Block? {
        for block in tree {
            if block.id == id { return block }
            if let found = findBlock(id: id, in: block.children) { return found }
        }
        return nil
    }

    /// The line range in the original body where this block's subtree lives
    private func findLineRange(for blockId: UUID, in body: String) -> Range<String.Index>? {
        let tree = BlockParser.parse(markdown: body)
        let flat = BlockParser.flatten(blocks: tree)
        guard let idx = flat.firstIndex(where: { $0.id == blockId }) else { return nil }
        // Find the serialized range by serializing up to and including this block's subtree
        // This is approximate — for now, just re-serialize the whole page from modified tree
        return nil
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Breadcrumb header
            HStack {
                // Back / Forward
                HStack(spacing: 2) {
                    Button { appState.goBack() } label: {
                        Image(systemName: "chevron.left")
                    }
                    .disabled(!appState.canGoBack)
                    .buttonStyle(.borderless)

                    Button { appState.goForward() } label: {
                        Image(systemName: "chevron.right")
                    }
                    .disabled(!appState.canGoForward)
                    .buttonStyle(.borderless)
                }
                .foregroundStyle(.secondary)

                // Breadcrumb: Page Title > Block text
                Button(page.title) {
                    appState.exitBlockZoom()
                }
                .buttonStyle(.plain)
                .foregroundStyle(Color.accentColor)
                .font(.caption)

                Image(systemName: "chevron.right")
                    .font(.caption2)
                    .foregroundStyle(.tertiary)

                Text(zoomedBlockText)
                    .font(.title3)
                    .bold()
                    .lineLimit(1)

                Spacer()
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 12)

            Divider()

            // Outliner showing only the zoomed block + children
            OutlinerCoordinator(
                blocks: $blocks,
                onContentChanged: { updatedBlocks in
                    blocks = updatedBlocks
                    scheduleAutoSave(updatedBlocks: updatedBlocks)
                },
                onWikiLinkClicked: { target in
                    if let linked = appState.pages.first(where: {
                        $0.title.lowercased() == target.lowercased()
                    }) {
                        appState.open(linked)
                    }
                },
                onModeChanged: { mode in
                    vimMode = mode
                },
                onCommandPalette: {
                    appState.isCommandPaletteVisible = true
                },
                onSlashMenu: {
                    appState.isSlashMenuVisible = true
                },
                onSpaceMenu: {
                    appState.isSpaceMenuVisible = true
                },
                isMenuVisible: {
                    appState.isSlashMenuVisible || appState.isSpaceMenuVisible
                },
                onDismissMenu: {
                    appState.isSlashMenuVisible = false
                    appState.isSpaceMenuVisible = false
                },
                onBlockZoom: { childBlockId in
                    appState.openBlockZoom(blockId: childBlockId)
                },
                apiClient: appState.api,
                typeRegistry: appState.typeRegistry,
                propertyRegistry: appState.propertyRegistry,
                allTags: appState.tags,
                allPageTitles: appState.pages.map(\.title)
            )
            .padding(.horizontal, 8)
        }
        .onAppear { loadZoomedBlocks() }
        .onChange(of: blockId) { _, _ in loadZoomedBlocks() }
    }

    private func loadZoomedBlocks() {
        // Parse the full page tree
        let tree = BlockParser.parse(markdown: page.body)
        // Find the target block
        guard let target = findBlock(id: blockId, in: tree) else {
            blocks = [Block(text: "(block not found)")]
            return
        }
        zoomedBlockText = target.displayText
        // Flatten the target + its children for the outliner
        blocks = BlockParser.flatten(blocks: [target])
    }

    private func scheduleAutoSave(updatedBlocks: [Block]) {
        saveTask?.cancel()
        saveTask = Task {
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled else { return }

            // Re-parse the full page, find the zoomed block, replace its subtree
            let fullTree = BlockParser.parse(markdown: page.body)
            guard replaceBlock(id: blockId, in: fullTree, with: updatedBlocks) else { return }

            // Serialize full tree back to markdown
            let newBody = BlockParser.serialize(blocks: fullTree)
            await appState.updatePage(id: page.id, newBody: newBody)
        }
    }

    /// Replace a block's content and children in the tree. Returns true if found.
    @discardableResult
    private func replaceBlock(id: UUID, in tree: [Block], with flat: [Block]) -> Bool {
        for block in tree {
            if block.id == id {
                // Replace this block's text and children from the flat list
                if let first = flat.first {
                    block.text = first.text
                    block.tags = first.tags
                    block.properties = first.properties
                    block.priority = first.priority
                    block.deadline = first.deadline
                    block.scheduled = first.scheduled
                    block.effort = first.effort
                }
                // Rebuild children from flat list (skip first which is the block itself)
                let childFlat = Array(flat.dropFirst())
                block.children = BlockParser.parse(markdown: BlockParser.serializeFlat(blocks: childFlat))
                return true
            }
            if replaceBlock(id: id, in: block.children, with: flat) { return true }
        }
        return false
    }
}
