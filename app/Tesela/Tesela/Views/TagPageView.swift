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
                    Text("# \(page.title)")
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

                // Table view of all tagged nodes
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        HStack(spacing: 4) {
                            Image(systemName: "tablecells")
                            Text("All")
                                .font(.headline)
                            Text("\(taggedBlocks.count)")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                    }
                    .padding(.horizontal, 24)
                    .padding(.top, 16)

                    if taggedBlocks.isEmpty {
                        Text("No \(page.title) blocks yet")
                            .font(.caption).foregroundStyle(.tertiary)
                            .padding(.horizontal, 24)
                    } else {
                        // Table header
                        let columns = resolvedType?.properties.prefix(5).map(\.name) ?? []
                        HStack(spacing: 0) {
                            Text("Name")
                                .font(.caption).bold().foregroundStyle(.secondary)
                                .frame(maxWidth: .infinity, alignment: .leading)
                            Text("Tags")
                                .font(.caption).bold().foregroundStyle(.secondary)
                                .frame(width: 80, alignment: .leading)
                            ForEach(columns, id: \.self) { col in
                                Text(col)
                                    .font(.caption).bold().foregroundStyle(.secondary)
                                    .frame(width: 100, alignment: .leading)
                            }
                        }
                        .padding(.horizontal, 24)
                        .padding(.vertical, 6)
                        .background(Color.secondary.opacity(0.08))

                        // Table rows — blocks with DB-indexed properties
                        ForEach(taggedBlocks) { block in
                            HStack(spacing: 0) {
                                Button(block.text.isEmpty ? "(empty)" : block.text) {
                                    // Navigate to the note containing this block
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
                                    Text(value)
                                        .font(.caption)
                                        .foregroundStyle(value == "Empty" ? .tertiary : .primary)
                                        .frame(width: 100, alignment: .leading)
                                }
                            }
                            .padding(.horizontal, 24)
                            .padding(.vertical, 6)

                            Divider().padding(.horizontal, 24)
                        }
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
        taggedBlocks = (try? await appState.api.getTypedBlocks(typeName: page.title)) ?? []
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
    private func saveTagProperties(_ properties: [String]) async {
        let propsStr = properties.map { "\"\($0)\"" }.joined(separator: ", ")

        var yaml = "---\n"
        yaml += "title: \"\(page.title)\"\n"
        yaml += "type: \"Tag\"\n"
        if let ext = extendsTag {
            yaml += "extends: \"\(ext)\"\n"
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
