import SwiftUI

// MARK: - TagPageView
// Special view for Tag pages (type: "Tag"). Shows tag properties,
// extends chain, and a table of all blocks tagged with this tag.

struct TagPageView: View {
    let page: Page
    @Environment(AppState.self) private var appState
    @State private var resolvedType: TypeDefinition?
    @State private var taggedBlocks: [TypedBlock] = []

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
                            HStack(spacing: 8) {
                                Text(propertyTypeIcon(prop.valueType))
                                    .font(.caption).foregroundStyle(.secondary).frame(width: 20)
                                Text(prop.name).bold()
                                Text("•").foregroundStyle(.tertiary)
                                if let vals = prop.values, !vals.isEmpty {
                                    Text(vals.joined(separator: ", "))
                                        .font(.caption).foregroundStyle(.tertiary).lineLimit(1)
                                } else {
                                    Text(prop.valueType)
                                        .font(.caption).foregroundStyle(.tertiary)
                                }
                            }
                            .padding(.horizontal, 52).padding(.vertical, 2)
                        }
                    }

                    Button { } label: {
                        HStack(spacing: 4) { Text("+"); Text("Add property") }
                            .foregroundStyle(.secondary)
                    }
                    .buttonStyle(.plain)
                    .padding(.horizontal, 52).padding(.top, 4)
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
