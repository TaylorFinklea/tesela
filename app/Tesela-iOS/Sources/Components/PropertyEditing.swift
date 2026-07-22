import SwiftUI

struct PropertyListDelta: Equatable {
    let current: [String]
    let add: [String]
    let remove: [String]
}

struct NodePageCandidate: Identifiable, Equatable {
    let pageId: String
    let slug: String
    let title: String
    let aliases: [String]

    init(pageId: String, slug: String, title: String, aliases: [String] = []) {
        self.pageId = pageId
        self.slug = slug
        self.title = title
        self.aliases = aliases
    }

    var id: String { pageId }
}

enum NodePageResolution: Equatable {
    case resolved(title: String, slug: String)
    case unresolved
    case deleted
    case conflict

    var isResolved: Bool {
        if case .resolved = self { return true }
        return false
    }

    func label(for rawValue: String) -> String {
        switch self {
        case .resolved(let title, _): title
        case .unresolved: "Unresolved: \(rawValue)"
        case .deleted: "Deleted: \(rawValue)"
        case .conflict: "Conflict: \(rawValue)"
        }
    }
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

    /// Reuse the page autocomplete scorer for the canonical candidates behind
    /// a Node property. The value written by the picker stays the PageId; aliases
    /// only participate in selection ranking.
    static func rankNodeCandidates(
        _ candidates: [NodePageCandidate],
        query: String,
        limit: Int
    ) -> [NodePageCandidate] {
        let needle = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !needle.isEmpty else { return Array(candidates.prefix(limit)) }

        var scored: [(candidate: NodePageCandidate, score: Int)] = []
        scored.reserveCapacity(candidates.count)
        for candidate in candidates {
            var score = 0
            for label in [candidate.title, candidate.slug] + candidate.aliases {
                score = max(score, LinkSuggest.score(label.lowercased(), needle))
            }
            if score > 0 {
                scored.append((candidate: candidate, score: score))
            }
        }
        scored.sort {
            $0.score != $1.score
                ? $0.score > $1.score
                : $0.candidate.title.count != $1.candidate.title.count
                    ? $0.candidate.title.count < $1.candidate.title.count
                    : $0.candidate.pageId < $1.candidate.pageId
        }
        return Array(scored.prefix(limit).map(\.candidate))
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
    let nodeCandidates: [NodePageCandidate]
    let nodeSearch: ((String) -> [NodePageCandidate])?

    @Environment(\.dismiss) private var dismiss
    @State private var draft: String
    @State private var selected: Set<String>
    @State private var nodeFilter = ""

    init(
        target: PropertyEditTarget,
        onSaveScalar: @escaping (String) -> Void,
        onSaveList: @escaping (PropertyListDelta) -> Void,
        nodeCandidates: [NodePageCandidate] = [],
        nodeSearch: ((String) -> [NodePageCandidate])? = nil
    ) {
        self.target = target
        self.onSaveScalar = onSaveScalar
        self.onSaveList = onSaveList
        self.nodeCandidates = nodeCandidates
        self.nodeSearch = nodeSearch
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
                        .keyboardShortcut(.cancelAction)
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
        case .node:
            TextField("Search pages", text: $nodeFilter)
                .textInputAutocapitalization(.never)
            ForEach(filteredNodeCandidates) { page in
                Button {
                    draft = page.pageId
                    nodeFilter = page.title
                } label: {
                    HStack {
                        VStack(alignment: .leading) {
                            Text(page.title).foregroundStyle(.primary)
                            Text(page.slug).font(.caption).foregroundStyle(.secondary)
                        }
                        Spacer()
                        if draft == page.pageId { Image(systemName: "checkmark") }
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

    private var filteredNodeCandidates: [NodePageCandidate] {
        if let nodeSearch {
            return nodeSearch(nodeFilter)
        }
        return PropertyEditing.rankNodeCandidates(
            nodeCandidates,
            query: nodeFilter,
            limit: nodeCandidates.count
        )
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
