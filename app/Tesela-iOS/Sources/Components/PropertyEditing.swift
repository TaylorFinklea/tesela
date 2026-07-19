import SwiftUI

struct PropertyListDelta: Equatable {
    let current: [String]
    let add: [String]
    let remove: [String]
}

struct PropertyEditTarget: Identifiable {
    let key: String
    let value: String
    let definition: PropertyDef?

    var id: String { key.lowercased() }
}

enum PropertyEditing {
    static func multiSelectValues(_ value: String) -> [String] {
        stableUnique(value.split(separator: ",").map(String.init))
    }

    static func multiSelectDelta(
        currentValue: String,
        selected: [String]
    ) -> PropertyListDelta {
        let current = multiSelectValues(currentValue)
        let next = stableUnique(selected)
        return PropertyListDelta(
            current: current,
            add: next.filter { !current.contains($0) },
            remove: current.filter { !next.contains($0) }
        )
    }

    static func isChecked(_ value: String) -> Bool {
        value.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() == "true"
    }

    static func toggledCheckboxValue(_ value: String) -> String {
        isChecked(value) ? "false" : "true"
    }

    static func linkURL(valueType: PropertyType?, value: String) -> URL? {
        let raw = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !raw.isEmpty else { return nil }
        switch valueType {
        case .url:
            if let url = URL(string: raw),
               let scheme = url.scheme?.lowercased() {
                return scheme == "http" || scheme == "https" ? url : nil
            }
            return URL(string: "https://\(raw)")
        case .email:
            let address = raw.replacingOccurrences(
                of: #"^mailto:"#,
                with: "",
                options: [.regularExpression, .caseInsensitive]
            )
            guard address.contains("@"),
                  address.rangeOfCharacter(from: .whitespacesAndNewlines) == nil else { return nil }
            return URL(string: "mailto:\(address)")
        case .phone:
            let withoutScheme = raw.replacingOccurrences(
                of: #"^tel:"#,
                with: "",
                options: [.regularExpression, .caseInsensitive]
            )
            let allowed = CharacterSet(charactersIn: "+0123456789*#,;")
            let number = String(withoutScheme.unicodeScalars.filter { allowed.contains($0) })
            guard number.contains(where: \Character.isNumber) else { return nil }
            return URL(string: "tel:\(number)")
        default:
            return nil
        }
    }

    private static func stableUnique(_ values: [String]) -> [String] {
        var output: [String] = []
        for raw in values {
            let value = raw.trimmingCharacters(in: .whitespacesAndNewlines)
            if !value.isEmpty, !output.contains(value) { output.append(value) }
        }
        return output
    }
}

struct PropertyEditSheet: View {
    let target: PropertyEditTarget
    let onSaveScalar: (String) -> Void
    let onSaveList: (PropertyListDelta) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var draft: String
    @State private var selected: Set<String>

    init(
        target: PropertyEditTarget,
        onSaveScalar: @escaping (String) -> Void,
        onSaveList: @escaping (PropertyListDelta) -> Void
    ) {
        self.target = target
        self.onSaveScalar = onSaveScalar
        self.onSaveList = onSaveList
        _draft = State(initialValue: target.value)
        _selected = State(initialValue: Set(PropertyEditing.multiSelectValues(target.value)))
    }

    private var valueType: PropertyType { target.definition?.valueType ?? .text }
    private var choices: [String] { target.definition?.choices ?? [] }

    var body: some View {
        NavigationStack {
            Form {
                if let url = PropertyEditing.linkURL(valueType: valueType, value: draft) {
                    Section {
                        Link(destination: url) {
                            Label("Open current value", systemImage: "arrow.up.right.square")
                        }
                    }
                }

                Section {
                    editor
                } header: {
                    Text(PROPERTY_TYPE_LABELS[valueType] ?? "Value")
                }
            }
            .navigationTitle(target.definition?.name ?? target.key)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { save() }
                }
            }
        }
        .presentationDetents([.medium, .large])
    }

    @ViewBuilder
    private var editor: some View {
        switch valueType {
        case .select:
            ForEach(choices, id: \.self) { choice in
                Button {
                    draft = choice
                } label: {
                    HStack {
                        Text(choice).foregroundStyle(.primary)
                        Spacer()
                        if draft == choice { Image(systemName: "checkmark") }
                    }
                }
            }
        case .multiSelect:
            ForEach(choices, id: \.self) { choice in
                Button {
                    if selected.contains(choice) {
                        selected.remove(choice)
                    } else {
                        selected.insert(choice)
                    }
                } label: {
                    HStack {
                        Text(choice).foregroundStyle(.primary)
                        Spacer()
                        Image(systemName: selected.contains(choice) ? "checkmark.square.fill" : "square")
                    }
                }
            }
        case .checkbox:
            Toggle("Checked", isOn: Binding(
                get: { PropertyEditing.isChecked(draft) },
                set: { draft = $0 ? "true" : "false" }
            ))
        case .number:
            TextField("Value", text: $draft)
                .keyboardType(.decimalPad)
        case .url:
            TextField("https://example.com", text: $draft)
                .textInputAutocapitalization(.never)
                .keyboardType(.URL)
        case .email:
            TextField("name@example.com", text: $draft)
                .textInputAutocapitalization(.never)
                .keyboardType(.emailAddress)
        case .phone:
            TextField("Phone number", text: $draft)
                .keyboardType(.phonePad)
        case .date:
            TextField("YYYY-MM-DD", text: $draft)
                .textInputAutocapitalization(.never)
        case .dateTime:
            TextField("YYYY-MM-DD HH:MM", text: $draft)
                .textInputAutocapitalization(.never)
        default:
            TextField("Value", text: $draft, axis: .vertical)
                .lineLimit(1...4)
        }
    }

    private func save() {
        if valueType == .multiSelect {
            let ordered = choices.filter(selected.contains)
                + selected.filter { !choices.contains($0) }.sorted()
            onSaveList(PropertyEditing.multiSelectDelta(
                currentValue: target.value,
                selected: ordered
            ))
        } else {
            onSaveScalar(draft.trimmingCharacters(in: .whitespacesAndNewlines))
        }
        dismiss()
    }
}
