import SwiftUI

// MARK: - TagPageView
// Special view for Tag pages (type: "Tag"). Shows tag properties,
// extends chain, and a table of all blocks tagged with this tag.

struct TagPageView: View {
    let page: Page
    @Environment(AppState.self) private var appState

    private var tagProperties: [String] {
        // Extract tag_properties from the page's custom metadata
        if let props = page.metadata.custom["tag_properties"] {
            switch props {
            case .array(let arr):
                return arr.compactMap { if case .string(let s) = $0 { return s } else { return nil } }
            default:
                return []
            }
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
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .frame(width: 16)
                        Text("Extends")
                            .foregroundStyle(.secondary)
                        Text("•")
                            .foregroundStyle(.tertiary)
                        Button(parent) {
                            if let linked = appState.pages.first(where: {
                                $0.title.lowercased() == parent.lowercased()
                            }) {
                                appState.open(linked)
                            }
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
                        Text("P")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .frame(width: 16)
                        Text("Tag Properties")
                            .font(.headline)
                    }
                    .padding(.horizontal, 24)

                    Text("Tag properties are inherited by all nodes using the tag.")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                        .padding(.horizontal, 48)
                        .padding(.bottom, 4)

                    ForEach(tagProperties, id: \.self) { propName in
                        let propDef = appState.propertyRegistry.first { $0.name == propName }
                        HStack(spacing: 8) {
                            // Type icon
                            Text(propertyTypeIcon(propDef?.valueType ?? "text"))
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .frame(width: 16)
                            Text(propName)
                                .bold()
                            Text("•")
                                .foregroundStyle(.tertiary)
                            Text(propDef != nil ? "Add description" : "Unknown property")
                                .foregroundStyle(.tertiary)
                                .italic()
                        }
                        .padding(.horizontal, 48)
                        .padding(.vertical, 2)
                    }

                    // + Add property
                    Button {
                        // TODO: add property picker
                    } label: {
                        HStack(spacing: 4) {
                            Text("+")
                            Text("Add property")
                        }
                        .foregroundStyle(.secondary)
                    }
                    .buttonStyle(.plain)
                    .padding(.horizontal, 48)
                    .padding(.top, 4)
                }
                .padding(.bottom, 24)

                Divider()
                    .padding(.horizontal, 24)

                // Table view placeholder
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Text("All")
                            .font(.headline)
                        Spacer()
                    }
                    .padding(.horizontal, 24)
                    .padding(.top, 16)

                    Text("Table view of all \(page.title) nodes — coming soon")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                        .padding(.horizontal, 24)
                }
                .padding(.bottom, 24)

                Spacer()
            }
        }
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
