import SwiftUI

// MARK: - TagPageView
// Special view for Tag pages (type: "Tag"). Shows tag properties,
// extends chain, and a table of all blocks tagged with this tag.

struct TagPageView: View {
    let page: Page
    @Environment(AppState.self) private var appState
    @State private var resolvedType: TypeDefinition?
    @State private var taggedNodes: [Page] = []

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
                            Text("\(taggedNodes.count)")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                    }
                    .padding(.horizontal, 24)
                    .padding(.top, 16)

                    if taggedNodes.isEmpty {
                        Text("No \(page.title) nodes yet")
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

                        // Table rows
                        ForEach(taggedNodes) { node in
                            HStack(spacing: 0) {
                                Button(node.title) {
                                    appState.open(node)
                                }
                                .buttonStyle(.plain)
                                .foregroundStyle(.primary)
                                .frame(maxWidth: .infinity, alignment: .leading)

                                Text("#\(page.title)")
                                    .font(.caption)
                                    .foregroundStyle(Color.accentColor)
                                    .frame(width: 80, alignment: .leading)

                                ForEach(columns, id: \.self) { col in
                                    let value = extractProperty(from: node, key: col)
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
        taggedNodes = (try? await appState.api.getTypedNodes(typeName: page.title)) ?? []
    }

    private func extractProperty(from note: Page, key: String) -> String {
        // Search the note body for `key:: value` patterns
        let pattern = "\(key.lowercased()):: "
        for line in note.body.components(separatedBy: "\n") {
            let trimmed = line.trimmingCharacters(in: .whitespaces).lowercased()
            if trimmed.hasPrefix(pattern) {
                let value = String(line.trimmingCharacters(in: .whitespaces).dropFirst(pattern.count))
                // Strip wiki-link brackets
                if value.hasPrefix("[[") && value.hasSuffix("]]") {
                    return String(value.dropFirst(2).dropLast(2))
                }
                return value
            }
        }
        return "Empty"
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
