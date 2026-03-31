import SwiftUI

// MARK: - BoardView
// Life OS kanban board: columns by status, swimlanes by domain.
// Supports type switching (Task, LifeProject, Issue), domain filtering, drag-and-drop.

struct BoardView: View {
    @Environment(AppState.self) private var appState
    @State private var allBlocks: [TypedBlock] = []
    @State private var selectedType: String = "Task"
    @State private var selectedDomain: String? = nil  // nil = all domains
    @State private var resolvedType: TypeDefinition?

    private let typeChoices = ["Task", "LifeProject", "Issue"]

    // Status columns based on the selected type
    private var statusChoices: [String] {
        if selectedType == "Issue" {
            return ["inbox", "open", "thinking", "resolved"]
        }
        return ["backlog", "todo", "doing", "in-review", "done"]
    }

    private var statusPropertyName: String {
        selectedType == "Issue" ? "IssueStatus" : "Status"
    }

    // Extract unique domains from blocks
    private var availableDomains: [String] {
        let refs = allBlocks.compactMap { block -> String? in
            let val = block.properties["DomainRef"] ?? block.properties["domainref"] ?? ""
            return val.isEmpty ? nil : BlockParser.strippedWikiLink(val)
        }
        return Array(Set(refs)).sorted()
    }

    // Filter blocks by selected domain
    private var filteredBlocks: [TypedBlock] {
        guard let domain = selectedDomain else { return allBlocks }
        return allBlocks.filter { block in
            let val = BlockParser.strippedWikiLink(block.properties["DomainRef"] ?? block.properties["domainref"] ?? "")
            return val.lowercased() == domain.lowercased()
        }
    }

    // Group filtered blocks by domain for swimlanes
    private var swimlanes: [(domain: String, blocks: [TypedBlock])] {
        var groups: [String: [TypedBlock]] = [:]
        for block in filteredBlocks {
            let domain = BlockParser.strippedWikiLink(block.properties["DomainRef"] ?? block.properties["domainref"] ?? "")
            let key = domain.isEmpty ? "Uncategorized" : domain
            groups[key, default: []].append(block)
        }
        return groups.map { (domain: $0.key, blocks: $0.value) }
            .sorted { $0.domain < $1.domain }
    }

    func blocksForColumn(_ status: String, in blocks: [TypedBlock]) -> [TypedBlock] {
        let propName = statusPropertyName.lowercased()
        return blocks.filter { block in
            let val = (block.properties[statusPropertyName] ?? block.properties[propName] ?? "").lowercased()
            return val == status.lowercased()
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            // Toolbar
            HStack(spacing: 12) {
                // Type picker
                Picker("Type", selection: $selectedType) {
                    ForEach(typeChoices, id: \.self) { type in
                        Text(type).tag(type)
                    }
                }
                .pickerStyle(.segmented)
                .frame(width: 260)

                Divider().frame(height: 20)

                // Domain filter
                Picker("Domain", selection: Binding(
                    get: { selectedDomain ?? "All" },
                    set: { selectedDomain = $0 == "All" ? nil : $0 }
                )) {
                    Text("All Domains").tag("All")
                    ForEach(availableDomains, id: \.self) { domain in
                        Text(domain).tag(domain)
                    }
                }
                .frame(width: 160)

                Spacer()

                Text("\(filteredBlocks.count) items")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)

            Divider()

            // Board content
            if allBlocks.isEmpty {
                ContentUnavailableView(
                    "No \(selectedType) items",
                    systemImage: "rectangle.split.3x1",
                    description: Text("Create blocks tagged with #\(selectedType) to see them here")
                )
            } else {
                ScrollView(.horizontal) {
                    ScrollView(.vertical) {
                        VStack(alignment: .leading, spacing: 16) {
                            ForEach(swimlanes, id: \.domain) { lane in
                                BoardSwimlane(
                                    domain: lane.domain,
                                    statusColumns: statusChoices,
                                    blocks: lane.blocks,
                                    statusPropertyName: statusPropertyName,
                                    blocksForColumn: blocksForColumn,
                                    allBlocks: allBlocks,
                                    onNavigate: { noteId in
                                        if let note = appState.pages.first(where: { $0.id == noteId }) {
                                            appState.open(note)
                                        }
                                    },
                                    onMoveBlock: { block, newStatus in
                                        Task { await moveBlockStatus(block: block, newStatus: newStatus) }
                                    }
                                )
                            }
                        }
                        .padding(16)
                    }
                }
            }
        }
        .task { await loadData() }
        .onChange(of: selectedType) { _, _ in Task { await loadData() } }
    }

    private func loadData() async {
        resolvedType = try? await appState.api.getResolvedType(name: selectedType)
        allBlocks = (try? await appState.api.getTypedBlocks(typeName: selectedType)) ?? []
    }

    private func moveBlockStatus(block: TypedBlock, newStatus: String) async {
        let propName = statusPropertyName.lowercased()
        do {
            let note = try await appState.api.getNote(id: block.noteId)
            var lines = note.body.components(separatedBy: "\n")
            let parts = block.id.split(separator: ":")
            guard let lineNum = parts.last.flatMap({ Int($0) }), lineNum < lines.count else { return }

            // Find or insert the status property line
            var propLineIndex: Int? = nil
            for i in (lineNum + 1)..<lines.count {
                let trimmed = lines[i].trimmingCharacters(in: .whitespaces)
                if trimmed.lowercased().hasPrefix("\(propName):: ") {
                    propLineIndex = i
                    break
                }
                if !trimmed.contains(":: ") { break }
            }

            if let idx = propLineIndex {
                let indent = String(lines[idx].prefix(while: { $0 == " " }))
                lines[idx] = "\(indent)\(propName):: \(newStatus)"
            } else {
                let indent = String(repeating: " ", count: (block.indentLevel + 1) * 2)
                lines.insert("\(indent)\(propName):: \(newStatus)", at: lineNum + 1)
            }

            let newBody = lines.joined(separator: "\n")
            await appState.updatePage(id: block.noteId, newBody: newBody)
            await loadData()
        } catch {
            print("[BoardView] moveBlockStatus failed: \(error)")
        }
    }
}

// MARK: - BoardSwimlane
// A horizontal row representing one domain, containing columns for each status
private struct BoardSwimlane: View {
    let domain: String
    let statusColumns: [String]
    let blocks: [TypedBlock]
    let statusPropertyName: String
    let blocksForColumn: (String, [TypedBlock]) -> [TypedBlock]
    let allBlocks: [TypedBlock]
    let onNavigate: (String) -> Void
    let onMoveBlock: (TypedBlock, String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            // Swimlane header
            HStack {
                Image(systemName: "globe")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Text(domain)
                    .font(.headline)
                Text("\(blocks.count)")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }
            .padding(.horizontal, 4)

            // Columns
            HStack(alignment: .top, spacing: 8) {
                ForEach(statusColumns, id: \.self) { status in
                    BoardColumn(
                        title: status,
                        blocks: blocksForColumn(status, blocks),
                        allBlocks: allBlocks,
                        onNavigate: onNavigate,
                        onDrop: { block in onMoveBlock(block, status) }
                    )
                }
            }
        }
        .padding(8)
        .background(Color.secondary.opacity(0.03), in: RoundedRectangle(cornerRadius: 8))
    }
}

// MARK: - BoardColumn
// A single status column within a swimlane
private struct BoardColumn: View {
    let title: String
    let blocks: [TypedBlock]
    let allBlocks: [TypedBlock]
    let onNavigate: (String) -> Void
    let onDrop: (TypedBlock) -> Void
    @State private var isTargeted = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Column header
            HStack {
                Text(title.capitalized)
                    .font(.caption).bold()
                Text("\(blocks.count)")
                    .font(.caption2).foregroundStyle(.tertiary)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color.secondary.opacity(0.08))

            // Cards
            VStack(spacing: 4) {
                ForEach(blocks) { block in
                    BoardCard(block: block, onNavigate: onNavigate)
                        .draggable(block.id)
                }

                if blocks.isEmpty {
                    Text("—")
                        .font(.caption2)
                        .foregroundStyle(.quaternary)
                        .frame(maxWidth: .infinity, minHeight: 30)
                }
            }
            .padding(4)
        }
        .frame(width: 180, alignment: .top)
        .background(isTargeted ? Color.accentColor.opacity(0.08) : Color.secondary.opacity(0.02), in: RoundedRectangle(cornerRadius: 6))
        .dropDestination(for: String.self) { items, _ in
            guard let droppedId = items.first,
                  let block = allBlocks.first(where: { $0.id == droppedId }) else { return false }
            onDrop(block)
            return true
        } isTargeted: { targeted in
            isTargeted = targeted
        }
    }
}

// MARK: - BoardCard
// A single item card on the board
private struct BoardCard: View {
    let block: TypedBlock
    let onNavigate: (String) -> Void

    private var priorityColor: Color {
        switch (block.properties["Priority"] ?? block.properties["priority"] ?? "").lowercased() {
        case "critical": .red
        case "high": .orange
        case "medium": .secondary
        case "low": .blue
        default: .clear
        }
    }

    private var deadline: String? {
        block.properties["Deadline"] ?? block.properties["deadline"]
    }

    private var domainRef: String? {
        let val = block.properties["DomainRef"] ?? block.properties["domainref"] ?? ""
        return val.isEmpty ? nil : BlockParser.strippedWikiLink(val)
    }

    var body: some View {
        Button {
            onNavigate(block.noteId)
        } label: {
            VStack(alignment: .leading, spacing: 4) {
                Text(block.text.isEmpty ? "(empty)" : block.text)
                    .font(.caption)
                    .foregroundStyle(.primary)
                    .lineLimit(3)
                    .multilineTextAlignment(.leading)

                HStack(spacing: 4) {
                    // Priority dot
                    if priorityColor != .clear {
                        Circle()
                            .fill(priorityColor)
                            .frame(width: 6, height: 6)
                    }

                    // Deadline badge
                    if let dl = deadline {
                        Text(dl)
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }

                    Spacer()

                    // Tags
                    ForEach(block.tags.prefix(2), id: \.self) { tag in
                        Text("#\(tag)")
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(8)
            .background(Color.secondary.opacity(0.06), in: RoundedRectangle(cornerRadius: 6))
        }
        .buttonStyle(.plain)
    }
}
