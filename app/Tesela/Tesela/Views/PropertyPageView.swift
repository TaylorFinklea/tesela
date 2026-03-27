import SwiftUI

// MARK: - PropertyPageView
// Special view for Property pages (type: "Property"). Shows editable schema:
// value type selector, choices editor (for selects), default value.

struct PropertyPageView: View {
    let page: Page
    @Environment(AppState.self) private var appState
    @State private var valueType: String = "text"
    @State private var choices: [String] = []
    @State private var defaultValue: String = ""
    @State private var newChoice: String = ""
    @State private var isDirty = false

    private let valueTypes = ["text", "number", "date", "select", "checkbox", "url"]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                // Header
                HStack {
                    Text(page.title)
                        .font(.largeTitle)
                        .bold()
                    Spacer()
                    Text("Property")
                        .font(.caption)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 4)
                        .foregroundStyle(.orange)
                        .background(Color.orange.opacity(0.12), in: Capsule())
                }
                .padding(.horizontal, 24)
                .padding(.top, 24)
                .padding(.bottom, 16)

                Divider().padding(.horizontal, 24)

                // Schema section
                VStack(alignment: .leading, spacing: 16) {
                    // Value Type
                    HStack(spacing: 12) {
                        Text(propertyTypeIcon(valueType))
                            .font(.title3)
                            .frame(width: 28)
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Value Type")
                                .font(.caption).foregroundStyle(.secondary)
                            Picker("", selection: $valueType) {
                                ForEach(valueTypes, id: \.self) { type in
                                    Text(type).tag(type)
                                }
                            }
                            .pickerStyle(.segmented)
                            .onChange(of: valueType) { _, _ in isDirty = true }
                        }
                    }

                    // Choices (only for select type)
                    if valueType == "select" {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Choices")
                                .font(.caption).foregroundStyle(.secondary)
                                .padding(.leading, 40)

                            ForEach(Array(choices.enumerated()), id: \.offset) { index, choice in
                                HStack(spacing: 8) {
                                    Image(systemName: "line.3.horizontal")
                                        .foregroundStyle(.tertiary)
                                        .frame(width: 28)
                                    Text(choice)
                                        .padding(.horizontal, 10)
                                        .padding(.vertical, 4)
                                        .background(Color.secondary.opacity(0.08), in: RoundedRectangle(cornerRadius: 4))
                                    Spacer()
                                    Button {
                                        choices.remove(at: index)
                                        isDirty = true
                                    } label: {
                                        Image(systemName: "xmark.circle.fill")
                                            .foregroundStyle(.tertiary)
                                    }
                                    .buttonStyle(.plain)
                                }
                                .padding(.leading, 40)
                            }

                            // Add new choice
                            HStack(spacing: 8) {
                                Image(systemName: "plus.circle")
                                    .foregroundStyle(.secondary)
                                    .frame(width: 28)
                                TextField("New choice", text: $newChoice)
                                    .textFieldStyle(.roundedBorder)
                                    .frame(maxWidth: 200)
                                    .onSubmit {
                                        addChoice()
                                    }
                                Button("Add") {
                                    addChoice()
                                }
                                .buttonStyle(.bordered)
                                .disabled(newChoice.trimmingCharacters(in: .whitespaces).isEmpty)
                            }
                            .padding(.leading, 40)
                        }
                    }

                    // Default Value
                    HStack(spacing: 12) {
                        Image(systemName: "arrow.uturn.backward")
                            .foregroundStyle(.secondary)
                            .frame(width: 28)
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Default Value")
                                .font(.caption).foregroundStyle(.secondary)
                            if valueType == "select" && !choices.isEmpty {
                                Picker("", selection: $defaultValue) {
                                    Text("(none)").tag("")
                                    ForEach(choices, id: \.self) { choice in
                                        Text(choice).tag(choice)
                                    }
                                }
                                .pickerStyle(.menu)
                                .onChange(of: defaultValue) { _, _ in isDirty = true }
                            } else if valueType == "checkbox" {
                                Toggle("", isOn: Binding(
                                    get: { defaultValue == "true" },
                                    set: { defaultValue = $0 ? "true" : "false"; isDirty = true }
                                ))
                                .labelsHidden()
                            } else {
                                TextField("Default", text: $defaultValue)
                                    .textFieldStyle(.roundedBorder)
                                    .frame(maxWidth: 200)
                                    .onChange(of: defaultValue) { _, _ in isDirty = true }
                            }
                        }
                    }

                    // Save button
                    if isDirty {
                        Button {
                            Task { await save() }
                        } label: {
                            HStack {
                                Image(systemName: "checkmark.circle.fill")
                                Text("Save Changes")
                            }
                        }
                        .buttonStyle(.borderedProminent)
                        .padding(.leading, 40)
                    }
                }
                .padding(.horizontal, 24)
                .padding(.top, 20)

                Divider().padding(.horizontal, 24).padding(.top, 24)

                // Usage section: which tags use this property
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 4) {
                        Image(systemName: "tag")
                        Text("Used By Tags")
                            .font(.headline)
                    }
                    .padding(.horizontal, 24)
                    .padding(.top, 16)

                    let usingTags = appState.typeRegistry.filter { tagDef in
                        tagDef.properties.contains { $0.name == page.title }
                    }

                    if usingTags.isEmpty {
                        Text("No tags use this property yet.")
                            .font(.caption).foregroundStyle(.tertiary)
                            .padding(.horizontal, 24)
                    } else {
                        ForEach(usingTags) { tagDef in
                            Button {
                                if let linked = appState.pages.first(where: {
                                    $0.title.lowercased() == tagDef.name.lowercased()
                                }) {
                                    appState.open(linked)
                                }
                            } label: {
                                HStack(spacing: 6) {
                                    Text(tagDef.icon.isEmpty ? "#" : tagDef.icon)
                                        .font(.caption)
                                    Text(tagDef.name)
                                        .foregroundStyle(Color.accentColor)
                                }
                            }
                            .buttonStyle(.plain)
                            .padding(.horizontal, 24)
                            .padding(.vertical, 2)
                        }
                    }
                }
                .padding(.bottom, 24)

                Spacer()
            }
        }
        .onAppear { parsePageSchema() }
        .onChange(of: page.id) { _, _ in parsePageSchema() }
    }

    // MARK: - Helpers

    private func parsePageSchema() {
        // Extract fields from page.metadata.custom (server parses YAML for us)
        if let vt = page.metadata.custom["value_type"], case .string(let s) = vt {
            valueType = s
        }
        if let ch = page.metadata.custom["choices"], case .array(let arr) = ch {
            choices = arr.compactMap { if case .string(let s) = $0 { return s } else { return nil } }
        }
        if let def = page.metadata.custom["default"], case .string(let s) = def {
            defaultValue = s
        }
        isDirty = false
    }

    private func addChoice() {
        let trimmed = newChoice.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty, !choices.contains(trimmed) else { return }
        choices.append(trimmed)
        newChoice = ""
        isDirty = true
    }

    private func save() async {
        // Reconstruct frontmatter YAML
        var yaml = "---\n"
        yaml += "title: \"\(page.title)\"\n"
        yaml += "type: \"Property\"\n"
        yaml += "value_type: \"\(valueType)\"\n"
        if valueType == "select" && !choices.isEmpty {
            let choicesStr = choices.map { "\"\($0)\"" }.joined(separator: ", ")
            yaml += "choices: [\(choicesStr)]\n"
        }
        if !defaultValue.isEmpty {
            yaml += "default: \"\(defaultValue)\"\n"
        }
        yaml += "tags: []\n"
        yaml += "---\n"

        // Preserve the body
        let fullContent = yaml + page.body
        await appState.updatePageContent(id: page.id, fullContent: fullContent)
        isDirty = false
    }

    private func propertyTypeIcon(_ valueType: String) -> String {
        switch valueType {
        case "text": return "T"
        case "select": return "☰"
        case "number": return "N°"
        case "date", "datetime": return "📅"
        case "checkbox": return "☑"
        case "url": return "🔗"
        default: return "T"
        }
    }
}
