import SwiftUI

// MARK: - BlockZoomView
// Logseq-style block drill-in: shows only a block and its children.
// Breadcrumb header for navigation back to the full page.

struct BlockZoomView: View {
    let page: Page
    let blockIndex: Int  // flat index into the page's block list
    @Environment(AppState.self) private var appState
    @State private var blocks: [Block] = []
    @State private var zoomedBlockText: String = ""
    @State private var saveTask: Task<Void, Never>?
    @State private var vimMode: VimMode = .normal

    /// Walk the tree depth-first to find the node at the given flat index.
    private func treeNodeAtFlatIndex(_ target: Int, in tree: [Block]) -> Block? {
        var counter = 0
        func walk(_ nodes: [Block]) -> Block? {
            for node in nodes {
                if counter == target { return node }
                counter += 1
                if let found = walk(node.children) { return found }
            }
            return nil
        }
        return walk(tree)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Breadcrumb header
            HStack {
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
                onModeChanged: { mode in vimMode = mode },
                onCommandPalette: { appState.isCommandPaletteVisible = true },
                onSlashMenu: { appState.isSlashMenuVisible = true },
                onSpaceMenu: { appState.isSpaceMenuVisible = true },
                isMenuVisible: { appState.isSlashMenuVisible || appState.isSpaceMenuVisible },
                onDismissMenu: {
                    appState.isSlashMenuVisible = false
                    appState.isSpaceMenuVisible = false
                },
                onBlockZoom: { localIndex in
                    // Convert local index within zoomed view to full-page flat index
                    appState.openBlockZoom(blockIndex: blockIndex + localIndex)
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
        .onChange(of: blockIndex) { _, _ in loadZoomedBlocks() }
    }

    private func loadZoomedBlocks() {
        let tree = BlockParser.parse(markdown: page.body)
        guard let target = treeNodeAtFlatIndex(blockIndex, in: tree) else {
            blocks = [Block(text: "(block not found)")]
            zoomedBlockText = "?"
            return
        }
        zoomedBlockText = target.displayText
        // Flatten the target + its children, resetting indent levels to start at 0
        blocks = BlockParser.flatten(blocks: [target])
    }

    private func scheduleAutoSave(updatedBlocks: [Block]) {
        saveTask?.cancel()
        saveTask = Task {
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled else { return }

            // Rebuild the full page: parse tree, replace the zoomed subtree, serialize
            let fullTree = BlockParser.parse(markdown: page.body)
            guard let target = treeNodeAtFlatIndex(blockIndex, in: fullTree) else { return }

            // Update the target block's text and children from edited blocks
            if let first = updatedBlocks.first {
                target.text = first.text
                target.tags = first.tags
                target.properties = first.properties
                target.priority = first.priority
                target.deadline = first.deadline
                target.scheduled = first.scheduled
                target.effort = first.effort
            }
            // Rebuild children from the edited flat list
            let childBlocks = Array(updatedBlocks.dropFirst())
            if !childBlocks.isEmpty {
                let markdown = BlockParser.serializeFlat(blocks: childBlocks)
                target.children = BlockParser.parse(markdown: markdown)
            } else {
                target.children = []
            }

            let newBody = BlockParser.serialize(blocks: fullTree)
            await appState.updatePage(id: page.id, newBody: newBody)
        }
    }
}
