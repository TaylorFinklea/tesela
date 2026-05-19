import SwiftUI

/// Bottom sheet shown when the user taps the mosaic chrome button in
/// any TopBar. Lists profiles, highlights the active one, and provides
/// add / edit actions.
struct MosaicSwitcherSheet: View {
    @ObservedObject var registry: MosaicRegistry
    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss

    @State private var showAdd: Bool = false
    @State private var editing: MosaicProfile? = nil

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    if registry.profiles.isEmpty {
                        ContentUnavailableView {
                            Label("No mosaics yet", systemImage: "circle.grid.3x3")
                        } description: {
                            Text("Add a mosaic to point Tesela at a server.")
                        } actions: {
                            Button("Add mosaic") { showAdd = true }
                                .buttonStyle(.borderedProminent)
                        }
                    } else {
                        ForEach(registry.profiles) { profile in
                            row(for: profile)
                        }
                    }
                }

                Section {
                    Button {
                        showAdd = true
                    } label: {
                        Label("Add mosaic", systemImage: "plus.circle.fill")
                    }
                }
            }
            .navigationTitle("Mosaics")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }
                }
            }
            .sheet(isPresented: $showAdd) {
                MosaicEditView(registry: registry, existing: nil)
                    .environment(\.theme, theme)
            }
            .sheet(item: $editing) { profile in
                MosaicEditView(registry: registry, existing: profile)
                    .environment(\.theme, theme)
            }
        }
    }

    private func row(for profile: MosaicProfile) -> some View {
        let isActive = registry.activeID == profile.id
        return Button {
            registry.setActive(profile.id)
            dismiss()
        } label: {
            HStack(spacing: 12) {
                Image(systemName: profile.iconSymbol)
                    .font(.system(size: 18, weight: .semibold))
                    .foregroundStyle(isActive ? theme.accentPrimary : theme.fgMuted)
                    .frame(width: 32, height: 32)
                VStack(alignment: .leading, spacing: 2) {
                    Text(profile.name)
                        .foregroundStyle(theme.fgDefault)
                    Text(profile.serverURL)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(theme.fgSubtle)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
                Spacer()
                if isActive {
                    Image(systemName: "checkmark")
                        .foregroundStyle(theme.accentPrimary)
                }
                Button {
                    editing = profile
                } label: {
                    Image(systemName: "ellipsis.circle")
                        .foregroundStyle(theme.fgMuted)
                }
                .buttonStyle(.plain)
            }
        }
        .buttonStyle(.plain)
        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
            if registry.profiles.count > 1 {
                Button(role: .destructive) {
                    registry.delete(profile.id)
                } label: {
                    Label("Remove", systemImage: "trash")
                }
            }
        }
    }
}

/// Add-or-edit form for a `MosaicProfile`. Used both as a fresh "add"
/// flow and an "edit existing" flow — `existing` distinguishes.
struct MosaicEditView: View {
    @ObservedObject var registry: MosaicRegistry
    /// If non-nil we're editing this profile; if nil we're adding new.
    let existing: MosaicProfile?

    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss

    @State private var name: String = ""
    @State private var serverURL: String = "http://"
    @State private var authToken: String = ""
    @State private var iconSymbol: String = "circle.grid.3x3"
    @State private var customIconText: String = ""

    var body: some View {
        NavigationStack {
            Form {
                Section("Name") {
                    TextField("Personal, Work, …", text: $name)
                        .submitLabel(.next)
                }

                Section("Server") {
                    TextField("http://127.0.0.1:7474", text: $serverURL)
                        .keyboardType(.URL)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .font(.system(size: 13, design: .monospaced))
                    TextField("Auth token (optional)", text: $authToken)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .font(.system(size: 13, design: .monospaced))
                }

                Section("Icon") {
                    LazyVGrid(columns: Array(repeating: GridItem(.flexible()), count: 6), spacing: 12) {
                        ForEach(mosaicIconPalette, id: \.self) { symbol in
                            Button {
                                iconSymbol = symbol
                                customIconText = ""
                            } label: {
                                Image(systemName: symbol)
                                    .font(.system(size: 18, weight: .semibold))
                                    .foregroundStyle(iconSymbol == symbol ? theme.accentPrimary : theme.fgMuted)
                                    .frame(width: 36, height: 36)
                                    .background(
                                        Circle().fill(
                                            iconSymbol == symbol
                                                ? theme.accentPrimary.opacity(0.18)
                                                : Color.clear
                                        )
                                    )
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(.vertical, 4)

                    TextField("Other SF Symbol name…", text: $customIconText)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .font(.system(size: 13, design: .monospaced))
                        .onSubmit {
                            let trimmed = customIconText.trimmingCharacters(in: .whitespaces)
                            if !trimmed.isEmpty {
                                iconSymbol = trimmed
                            }
                        }
                }

                if existing != nil {
                    Section {
                        Button(role: .destructive) {
                            if let id = existing?.id { registry.delete(id) }
                            dismiss()
                        } label: {
                            Label("Delete mosaic", systemImage: "trash")
                        }
                        .disabled(registry.profiles.count <= 1)
                    } footer: {
                        if registry.profiles.count <= 1 {
                            Text("You need at least one mosaic.")
                                .font(.caption2)
                        }
                    }
                }
            }
            .navigationTitle(existing == nil ? "Add mosaic" : "Edit mosaic")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Save") { save() }
                        .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty
                            || serverURL.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
            .onAppear {
                if let e = existing {
                    name = e.name
                    serverURL = e.serverURL
                    authToken = e.authToken ?? ""
                    iconSymbol = e.iconSymbol
                }
            }
        }
    }

    private func save() {
        let trimmedToken = authToken.trimmingCharacters(in: .whitespaces)
        let token: String? = trimmedToken.isEmpty ? nil : trimmedToken
        if var existing {
            existing.name = name.trimmingCharacters(in: .whitespaces)
            existing.serverURL = serverURL.trimmingCharacters(in: .whitespaces)
            existing.authToken = token
            existing.iconSymbol = iconSymbol
            registry.update(existing)
        } else {
            let new = MosaicProfile(
                name: name.trimmingCharacters(in: .whitespaces),
                serverURL: serverURL.trimmingCharacters(in: .whitespaces),
                authToken: token,
                iconSymbol: iconSymbol
            )
            registry.add(new, makeActive: true)
        }
        dismiss()
    }
}
