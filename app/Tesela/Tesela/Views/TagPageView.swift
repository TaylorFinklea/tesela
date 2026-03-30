import SwiftUI

// MARK: - TagPageView
// Special view for Tag pages (type: "Tag"). Shows tag properties,
// extends chain, and a table of all blocks tagged with this tag.

struct TagPageView: View {
    let page: Page
    @Environment(AppState.self) private var appState
    @State private var resolvedType: TypeDefinition?
    @State private var taggedBlocks: [TypedBlock] = []
    @State private var isAddingProperty = false
    @State private var propertySearchText = ""
    @State private var isPickingIcon = false

    // Filtering & View Mode
    @State private var activeFilters: [String: String] = [:]  // property -> value
    @State private var sortProperty: String?
    @State private var sortAscending = true
    @State private var viewMode: ViewMode = .table
    @State private var kanbanProperty: String?

    private enum ViewMode { case table, kanban }

    /// The tag_properties from this tag's own frontmatter (not inherited)
    private var ownPropertyNames: [String] {
        if let tp = page.metadata.custom["tag_properties"], case .array(let arr) = tp {
            return arr.compactMap { if case .string(let s) = $0 { return s } else { return nil } }
        }
        return []
    }

    private var extendsTag: String? {
        if let ext = page.metadata.custom["extends"] {
            if case .string(let s) = ext { return s }
        }
        return nil
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                // Header
                HStack {
                    // Type icon — click to change
                    Button {
                        isPickingIcon = true
                    } label: {
                        if let icon = resolvedType?.icon, !icon.isEmpty,
                           let _ = NSImage(systemSymbolName: icon, accessibilityDescription: nil) {
                            Image(systemName: icon)
                                .font(.title)
                                .foregroundStyle(.secondary)
                        } else {
                            Image(systemName: "square.dashed")
                                .font(.title)
                                .foregroundStyle(.tertiary)
                        }
                    }
                    .buttonStyle(.plain)
                    .help("Change icon")
                    .popover(isPresented: $isPickingIcon) {
                        IconPickerView { selectedSymbol in
                            isPickingIcon = false
                            Task { await saveIcon(selectedSymbol) }
                        }
                    }

                    Text(page.title)
                        .font(.largeTitle)
                        .bold()
                    Spacer()
                    Text("#Tag")
                        .font(.caption)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 4)
                        .foregroundStyle(Color.accentColor)
                        .background(Color.accentColor.opacity(0.12), in: Capsule())
                }
                .padding(.horizontal, 24)
                .padding(.top, 24)
                .padding(.bottom, 16)

                // Extends
                if let parent = extendsTag {
                    HStack(spacing: 8) {
                        Text("T")
                            .font(.caption).foregroundStyle(.secondary).frame(width: 20)
                        Text("Extends")
                            .foregroundStyle(.secondary)
                        Text("•").foregroundStyle(.tertiary)
                        Button(parent) {
                            if let linked = appState.pages.first(where: {
                                $0.title.lowercased() == parent.lowercased()
                            }) { appState.open(linked) }
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(Color.accentColor)
                    }
                    .padding(.horizontal, 24)
                    .padding(.bottom, 8)
                }

                // Tag Properties
                VStack(alignment: .leading, spacing: 4) {
                    HStack(spacing: 8) {
                        Text("P").font(.caption).foregroundStyle(.secondary).frame(width: 20)
                        Text("Tag Properties").font(.headline)
                    }
                    .padding(.horizontal, 24)

                    Text("Tag properties are inherited by all nodes using the tag.")
                        .font(.caption).foregroundStyle(.tertiary)
                        .padding(.horizontal, 52).padding(.bottom, 4)

                    if let resolved = resolvedType {
                        ForEach(resolved.properties) { prop in
                            let isOwn = ownPropertyNames.contains(prop.name)
                            HStack(spacing: 8) {
                                Text(propertyTypeIcon(prop.valueType))
                                    .font(.caption).foregroundStyle(.secondary).frame(width: 20)
                                Button(prop.name) {
                                    if let linked = appState.pages.first(where: {
                                        $0.title.lowercased() == prop.name.lowercased()
                                    }) {
                                        appState.open(linked)
                                    }
                                }
                                .buttonStyle(.plain)
                                .bold()
                                .foregroundStyle(Color.accentColor)
                                Text("•").foregroundStyle(.tertiary)
                                if let vals = prop.values, !vals.isEmpty {
                                    Text(vals.joined(separator: ", "))
                                        .font(.caption).foregroundStyle(.tertiary).lineLimit(1)
                                } else {
                                    Text(prop.valueType)
                                        .font(.caption).foregroundStyle(.tertiary)
                                }
                                if !isOwn {
                                    Text("inherited")
                                        .font(.caption2).foregroundStyle(.tertiary)
                                        .padding(.horizontal, 6).padding(.vertical, 1)
                                        .background(Color.secondary.opacity(0.08), in: Capsule())
                                }
                                Spacer()
                                if isOwn {
                                    Button {
                                        Task { await removeProperty(prop.name) }
                                    } label: {
                                        Image(systemName: "xmark.circle.fill")
                                            .foregroundStyle(.tertiary)
                                    }
                                    .buttonStyle(.plain)
                                }
                            }
                            .padding(.horizontal, 52).padding(.vertical, 2)
                        }
                    }

                    Button { isAddingProperty.toggle() } label: {
                        HStack(spacing: 4) { Text("+"); Text("Add property") }
                            .foregroundStyle(.secondary)
                    }
                    .buttonStyle(.plain)
                    .padding(.horizontal, 52).padding(.top, 4)
                    .popover(isPresented: $isAddingProperty, arrowEdge: .bottom) {
                        PropertyPicker(
                            existingNames: ownPropertyNames,
                            allProperties: appState.propertyRegistry,
                            onSelect: { propName in
                                isAddingProperty = false
                                Task { await addProperty(propName) }
                            },
                            onCreateNew: { propName in
                                isAddingProperty = false
                                Task { await createAndAddProperty(propName) }
                            }
                        )
                    }
                }
                .padding(.bottom, 24)

                Divider().padding(.horizontal, 24)

                // View mode header
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        HStack(spacing: 4) {
                            Image(systemName: viewMode == .table ? "tablecells" : "rectangle.split.3x1")
                            Text("All")
                                .font(.headline)
                            Text("\(taggedBlocks.count)")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        // View mode toggle
                        Picker("", selection: $viewMode) {
                            Image(systemName: "tablecells").tag(ViewMode.table)
                            Image(systemName: "rectangle.split.3x1").tag(ViewMode.kanban)
                        }
                        .pickerStyle(.segmented)
                        .frame(width: 80)
                    }
                    .padding(.horizontal, 24)
                    .padding(.top, 16)

                    // Filter chips
                    if let resolved = resolvedType, !resolved.properties.isEmpty {
                        ScrollView(.horizontal, showsIndicators: false) {
                            HStack(spacing: 6) {
                                ForEach(resolved.properties) { prop in
                                    FilterChip(
                                        property: prop,
                                        isActive: activeFilters[prop.name] != nil,
                                        activeValue: activeFilters[prop.name],
                                        onSelect: { value in
                                            activeFilters[prop.name] = value
                                            Task { await loadData() }
                                        },
                                        onClear: {
                                            activeFilters.removeValue(forKey: prop.name)
                                            Task { await loadData() }
                                        }
                                    )
                                }
                            }
                            .padding(.horizontal, 24)
                        }
                    }

                    if taggedBlocks.isEmpty {
                        Text("No \(page.title) blocks yet")
                            .font(.caption).foregroundStyle(.tertiary)
                            .padding(.horizontal, 24)
                    } else if viewMode == .table {
                        // Table header — click to sort
                        let columns = resolvedType?.properties.prefix(5).map(\.name) ?? []
                        HStack(spacing: 0) {
                            Text("Name")
                                .font(.caption).bold().foregroundStyle(.secondary)
                                .frame(maxWidth: .infinity, alignment: .leading)
                            Text("Tags")
                                .font(.caption).bold().foregroundStyle(.secondary)
                                .frame(width: 80, alignment: .leading)
                            ForEach(columns, id: \.self) { col in
                                Button {
                                    if sortProperty == col {
                                        sortAscending.toggle()
                                    } else {
                                        sortProperty = col
                                        sortAscending = true
                                    }
                                    Task { await loadData() }
                                } label: {
                                    HStack(spacing: 2) {
                                        Text(col)
                                            .font(.caption).bold()
                                        if sortProperty == col {
                                            Image(systemName: sortAscending ? "chevron.up" : "chevron.down")
                                                .font(.caption2)
                                        }
                                    }
                                    .foregroundStyle(sortProperty == col ? Color.accentColor : .secondary)
                                }
                                .buttonStyle(.plain)
                                .frame(width: 100, alignment: .leading)
                            }
                        }
                        .padding(.horizontal, 24)
                        .padding(.vertical, 6)
                        .background(Color.secondary.opacity(0.08))

                        // Table rows
                        ForEach(taggedBlocks) { block in
                            HStack(spacing: 0) {
                                Button(block.text.isEmpty ? "(empty)" : block.text) {
                                    if let note = appState.pages.first(where: { $0.id == block.noteId }) {
                                        appState.open(note)
                                    }
                                }
                                .buttonStyle(.plain)
                                .foregroundStyle(.primary)
                                .frame(maxWidth: .infinity, alignment: .leading)

                                Text("#\(page.title)")
                                    .font(.caption)
                                    .foregroundStyle(Color.accentColor)
                                    .frame(width: 80, alignment: .leading)

                                ForEach(columns, id: \.self) { col in
                                    let value = block.properties[col] ?? block.properties[col.lowercased()] ?? "Empty"
                                    let stripped = BlockParser.stripWikiLink(value)
                                    let isLink = value.hasPrefix("[[") && value.hasSuffix("]]")
                                    if isLink {
                                        Button(stripped) {
                                            if let linked = appState.pages.first(where: {
                                                $0.title.lowercased() == stripped.lowercased()
                                            }) {
                                                appState.open(linked)
                                            }
                                        }
                                        .buttonStyle(.plain)
                                        .font(.caption)
                                        .foregroundStyle(Color.accentColor)
                                        .frame(width: 100, alignment: .leading)
                                    } else {
                                        Text(value)
                                            .font(.caption)
                                            .foregroundStyle(value == "Empty" ? .tertiary : .primary)
                                            .frame(width: 100, alignment: .leading)
                                    }
                                }
                            }
                            .padding(.horizontal, 24)
                            .padding(.vertical, 6)

                            Divider().padding(.horizontal, 24)
                        }
                    } else {
                        // Kanban view — columns by first select property
                        KanbanBoard(
                            blocks: taggedBlocks,
                            resolvedType: resolvedType,
                            kanbanProperty: kanbanProperty,
                            onSelectProperty: { kanbanProperty = $0 },
                            onNavigate: { noteId in
                                if let note = appState.pages.first(where: { $0.id == noteId }) {
                                    appState.open(note)
                                }
                            },
                            onMoveBlock: { block, newValue in
                                Task { await moveBlockProperty(block: block, newValue: newValue) }
                            }
                        )
                    }
                }
                .padding(.bottom, 24)

                Spacer()
            }
        }
        .task(id: page.id) {
            await loadData()
        }
    }

    private func loadData() async {
        resolvedType = try? await appState.api.getResolvedType(name: page.title)
        let filters = activeFilters.map { APIClient.PropertyFilter(property: $0.key, value: $0.value) }
        taggedBlocks = (try? await appState.api.getTypedBlocks(
            typeName: page.title,
            filters: filters,
            sortBy: sortProperty,
            sortDir: sortProperty != nil ? (sortAscending ? "asc" : "desc") : nil
        )) ?? []
    }

    /// Update a block's property value when dragged between kanban columns.
    /// Reads the source note, finds the block by line, updates the property, saves.
    private func moveBlockProperty(block: TypedBlock, newValue: String) async {
        guard let propName = kanbanProperty ?? resolvedType?.properties.first(where: { $0.valueType == "select" })?.name else { return }
        do {
            let note = try await appState.api.getNote(id: block.noteId)
            var lines = note.body.components(separatedBy: "\n")
            // block.id is "noteId:lineNumber"
            let parts = block.id.split(separator: ":")
            guard let lineNum = parts.last.flatMap({ Int($0) }), lineNum < lines.count else { return }

            let blockLine = lines[lineNum]
            let propKey = propName.lowercased()
            let propPattern = "\(propKey):: "

            // Check if the block already has this property as a continuation line
            var propLineIndex: Int? = nil
            for i in (lineNum + 1)..<lines.count {
                let trimmed = lines[i].trimmingCharacters(in: .whitespaces)
                if trimmed.hasPrefix(propPattern) {
                    propLineIndex = i
                    break
                }
                // Stop if we hit a non-continuation line (not indented enough or a new block)
                if !trimmed.contains(":: ") { break }
            }

            if let idx = propLineIndex {
                // Update existing property line
                let indent = String(lines[idx].prefix(while: { $0 == " " }))
                lines[idx] = "\(indent)\(propKey):: \(newValue)"
            } else {
                // Add property as a continuation line after the block
                let indent = String(repeating: " ", count: (block.indentLevel + 1) * 2)
                lines.insert("\(indent)\(propKey):: \(newValue)", at: lineNum + 1)
            }

            let newBody = lines.joined(separator: "\n")
            await appState.updatePage(id: block.noteId, newBody: newBody)
            await loadData()
        } catch {
            print("[TagPageView] moveBlockProperty failed: \(error)")
        }
    }

    private func addProperty(_ name: String) async {
        var props = ownPropertyNames
        guard !props.contains(name) else { return }
        props.append(name)
        await saveTagProperties(props)
    }

    private func removeProperty(_ name: String) async {
        var props = ownPropertyNames
        props.removeAll { $0 == name }
        await saveTagProperties(props)
    }

    private func createAndAddProperty(_ name: String) async {
        // Create a new Property page, then add to this tag
        let content = "---\ntitle: \"\(name)\"\ntype: \"Property\"\nvalue_type: \"text\"\ntags: []\n---\n- \(name) property.\n"
        _ = try? await appState.api.createNote(title: name, content: content, tags: [])
        await appState.refreshPages()
        // Refresh property registry
        appState.propertyRegistry = (try? await appState.api.getProperties()) ?? appState.propertyRegistry
        await addProperty(name)
    }

    /// Reconstructs frontmatter with updated tag_properties and saves
    private func saveIcon(_ symbolName: String) async {
        await saveTagFrontmatter(icon: symbolName)
    }

    private func saveTagProperties(_ properties: [String]) async {
        await saveTagFrontmatter(tagProperties: properties)
    }

    /// Rebuild and save the tag page frontmatter, preserving all fields
    private func saveTagFrontmatter(icon: String? = nil, tagProperties: [String]? = nil) async {
        let props = tagProperties ?? ownPropertyNames
        let propsStr = props.map { "\"\($0)\"" }.joined(separator: ", ")
        let iconValue = icon ?? resolvedType?.icon ?? ""

        var yaml = "---\n"
        yaml += "title: \"\(page.title)\"\n"
        yaml += "type: \"Tag\"\n"
        if let ext = extendsTag {
            yaml += "extends: \"\(ext)\"\n"
        }
        if !iconValue.isEmpty {
            yaml += "icon: \"\(iconValue)\"\n"
        }
        yaml += "tag_properties: [\(propsStr)]\n"
        yaml += "tags: []\n"
        yaml += "---\n"

        let fullContent = yaml + page.body
        await appState.updatePageContent(id: page.id, fullContent: fullContent)
        await loadData()
    }

    private func propertyTypeIcon(_ valueType: String) -> String {
        switch valueType {
        case "text", "select": return "T"
        case "number": return "N°"
        case "date", "datetime": return "📅"
        case "checkbox": return "☑"
        case "url": return "🔗"
        case "node": return "→"
        default: return "T"
        }
    }
}

// MARK: - KanbanBoard
// Horizontal columns grouped by a select property's values.
private struct KanbanBoard: View {
    let blocks: [TypedBlock]
    let resolvedType: TypeDefinition?
    let kanbanProperty: String?
    let onSelectProperty: (String) -> Void
    let onNavigate: (String) -> Void
    let onMoveBlock: (TypedBlock, String) -> Void  // (block, newValue)

    /// First select-type property, or user-chosen
    private var groupByProperty: PropertyDef? {
        guard let resolved = resolvedType else { return nil }
        if let chosen = kanbanProperty {
            return resolved.properties.first { $0.name == chosen }
        }
        return resolved.properties.first { $0.valueType == "select" }
    }

    private var columnValues: [String] {
        groupByProperty?.values ?? []
    }

    private func blocksForColumn(_ value: String) -> [TypedBlock] {
        guard let prop = groupByProperty else { return [] }
        let key = prop.name.lowercased()
        return blocks.filter { block in
            let v = block.properties[prop.name] ?? block.properties[key] ?? ""
            return v.lowercased() == value.lowercased()
        }
    }

    private var uncategorizedBlocks: [TypedBlock] {
        guard let prop = groupByProperty else { return blocks }
        let key = prop.name.lowercased()
        let validValues = Set(columnValues.map { $0.lowercased() })
        return blocks.filter { block in
            let v = (block.properties[prop.name] ?? block.properties[key] ?? "").lowercased()
            return v.isEmpty || !validValues.contains(v)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Property selector (if multiple select properties)
            if let resolved = resolvedType {
                let selectProps = resolved.properties.filter { $0.valueType == "select" }
                if selectProps.count > 1 {
                    HStack(spacing: 8) {
                        Text("Group by:")
                            .font(.caption).foregroundStyle(.secondary)
                        ForEach(selectProps) { prop in
                            Button(prop.name) {
                                onSelectProperty(prop.name)
                            }
                            .buttonStyle(.bordered)
                            .controlSize(.small)
                            .tint(groupByProperty?.name == prop.name ? .accentColor : .secondary)
                        }
                    }
                    .padding(.horizontal, 24)
                }
            }

            if groupByProperty == nil {
                Text("No select property to group by")
                    .font(.caption).foregroundStyle(.tertiary)
                    .padding(.horizontal, 24)
            } else {
                ScrollView(.horizontal, showsIndicators: true) {
                    HStack(alignment: .top, spacing: 12) {
                        ForEach(columnValues, id: \.self) { value in
                            KanbanColumn(
                                title: value,
                                blocks: blocksForColumn(value),
                                allBlocks: blocks,
                                onNavigate: onNavigate,
                                onDrop: { block in onMoveBlock(block, value) }
                            )
                        }
                        // Uncategorized column
                        let uncat = uncategorizedBlocks
                        if !uncat.isEmpty {
                            KanbanColumn(
                                title: "Unset",
                                blocks: uncat,
                                allBlocks: blocks,
                                onNavigate: onNavigate,
                                onDrop: { _ in }
                            )
                        }
                    }
                    .padding(.horizontal, 24)
                    .padding(.vertical, 4)
                }
            }
        }
    }
}

// MARK: - KanbanColumn
private struct KanbanColumn: View {
    let title: String
    let blocks: [TypedBlock]
    let allBlocks: [TypedBlock]  // full list for drop lookup
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
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color.secondary.opacity(0.08))

            // Cards
            ScrollView {
                LazyVStack(spacing: 4) {
                    ForEach(blocks) { block in
                        KanbanCard(block: block, onNavigate: onNavigate)
                            .draggable(block.id)
                    }
                }
                .padding(.horizontal, 4)
                .padding(.vertical, 6)
            }
        }
        .frame(width: 200)
        .background(isTargeted ? Color.accentColor.opacity(0.08) : Color.secondary.opacity(0.03), in: RoundedRectangle(cornerRadius: 8))
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

// MARK: - KanbanCard
private struct KanbanCard: View {
    let block: TypedBlock
    let onNavigate: (String) -> Void

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
                if !block.tags.isEmpty {
                    HStack(spacing: 4) {
                        ForEach(block.tags, id: \.self) { tag in
                            Text("#\(tag)")
                                .font(.caption2)
                                .foregroundStyle(Color.accentColor)
                        }
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

// MARK: - PropertyPicker
// Popover for searching/adding properties to a tag
private struct PropertyPicker: View {
    let existingNames: [String]
    let allProperties: [PropertyDef]
    let onSelect: (String) -> Void
    let onCreateNew: (String) -> Void
    @State private var searchText = ""

    private var filteredProperties: [PropertyDef] {
        let available = allProperties.filter { !existingNames.contains($0.name) }
        if searchText.isEmpty { return available }
        return available.filter { $0.name.localizedCaseInsensitiveContains(searchText) }
    }

    private var canCreateNew: Bool {
        let trimmed = searchText.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return false }
        return !allProperties.contains { $0.name.lowercased() == trimmed.lowercased() }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            TextField("Search properties…", text: $searchText)
                .textFieldStyle(.roundedBorder)

            if filteredProperties.isEmpty && !canCreateNew {
                Text("No properties available")
                    .font(.caption).foregroundStyle(.tertiary)
            }

            ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(filteredProperties) { prop in
                        Button {
                            onSelect(prop.name)
                        } label: {
                            HStack(spacing: 6) {
                                Text(propIcon(prop.valueType))
                                    .font(.caption).frame(width: 20)
                                Text(prop.name)
                                Text("·").foregroundStyle(.tertiary)
                                Text(prop.valueType)
                                    .font(.caption).foregroundStyle(.secondary)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.vertical, 4)
                            .padding(.horizontal, 6)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .frame(maxHeight: 200)

            if canCreateNew {
                Divider()
                Button {
                    onCreateNew(searchText.trimmingCharacters(in: .whitespaces))
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "plus.circle.fill")
                            .foregroundStyle(.green)
                        Text("Create \"\(searchText.trimmingCharacters(in: .whitespaces))\"")
                            .bold()
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.vertical, 4)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(12)
        .frame(width: 280)
    }

    private func propIcon(_ valueType: String) -> String {
        switch valueType {
        case "text", "select": return "T"
        case "number": return "N°"
        case "date": return "📅"
        case "checkbox": return "☑"
        default: return "T"
        }
    }
}

// MARK: - FilterChip
// Clickable filter chip for a property. Shows a popover with values to filter by.
private struct FilterChip: View {
    let property: PropertyDef
    let isActive: Bool
    let activeValue: String?
    let onSelect: (String) -> Void
    let onClear: () -> Void
    @State private var isShowingPopover = false
    @State private var textFilterValue = ""

    var body: some View {
        Button {
            if isActive {
                onClear()
            } else {
                isShowingPopover = true
            }
        } label: {
            HStack(spacing: 4) {
                Text(property.name)
                    .font(.caption)
                if let val = activeValue {
                    Text("= \(val)")
                        .font(.caption).bold()
                    Image(systemName: "xmark.circle.fill")
                        .font(.caption2)
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 4)
            .foregroundStyle(isActive ? .white : .secondary)
            .background(isActive ? Color.accentColor : Color.secondary.opacity(0.12), in: Capsule())
        }
        .buttonStyle(.plain)
        .popover(isPresented: $isShowingPopover, arrowEdge: .bottom) {
            filterOptions
        }
    }

    @ViewBuilder
    private var filterOptions: some View {
        VStack(alignment: .leading, spacing: 4) {
            if let choices = property.values, !choices.isEmpty {
                // Select property — show choices
                ForEach(choices, id: \.self) { choice in
                    Button {
                        isShowingPopover = false
                        onSelect(choice)
                    } label: {
                        Text(choice)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.vertical, 4)
                            .padding(.horizontal, 8)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            } else {
                // Text property — freeform input
                TextField("Filter value…", text: $textFilterValue)
                    .textFieldStyle(.roundedBorder)
                    .onSubmit {
                        let trimmed = textFilterValue.trimmingCharacters(in: .whitespaces)
                        guard !trimmed.isEmpty else { return }
                        isShowingPopover = false
                        onSelect(trimmed)
                    }
            }
        }
        .padding(8)
        .frame(width: 180)
    }
}
