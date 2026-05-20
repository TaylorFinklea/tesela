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
                AddMosaicView(registry: registry)
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
            .contentShape(Rectangle())
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

                Section {
                    TextField("http://192.168.1.42:7474", text: $serverURL)
                        .keyboardType(.URL)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .font(.system(size: 13, design: .monospaced))
                    TextField("Auth token (optional)", text: $authToken)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .font(.system(size: 13, design: .monospaced))
                } header: {
                    Text("Server")
                } footer: {
                    Text("URL of the `tesela-server` instance hosting this mosaic.")
                        .font(.caption2)
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

/// Discovery-driven "Add mosaic" flow. Instead of typing a server URL
/// per mosaic, the user points at one server and picks from the
/// mosaics it reports — or creates a new one on it. Added profiles are
/// not made active (switching would restart the server); the user taps
/// one in the switcher to actually switch.
struct AddMosaicView: View {
    @ObservedObject var registry: MosaicRegistry

    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss

    @State private var serverURL: String
    @State private var discovered: [MosaicServerClient.DiscoveredMosaic] = []
    @State private var loading = false
    @State private var error: String?
    @State private var didSearch = false
    @State private var showCreate = false
    @State private var newName = ""

    init(registry: MosaicRegistry) {
        self.registry = registry
        // Pre-fill with an existing server URL — most users add another
        // mosaic on the same Mac they already paired with.
        _serverURL = State(initialValue: registry.profiles.first?.serverURL ?? "http://")
    }

    private var trimmedURL: String {
        serverURL.trimmingCharacters(in: .whitespaces)
    }

    var body: some View {
        NavigationStack {
            Form {
                serverSection
                if let error {
                    Section {
                        Text(error)
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundStyle(theme.typeTask)
                    }
                }
                if didSearch {
                    discoveredSection
                }
            }
            .navigationTitle("Add mosaic")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Done") { dismiss() }
                }
            }
            .alert("New mosaic", isPresented: $showCreate) {
                TextField("Name", text: $newName)
                Button("Cancel", role: .cancel) { newName = "" }
                Button("Create") { Task { await createMosaic() } }
            } message: {
                Text("Creates an empty mosaic on the server.")
            }
        }
    }

    private var serverSection: some View {
        Section {
            TextField("http://192.168.1.42:7474", text: $serverURL)
                .keyboardType(.URL)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .font(.system(size: 13, design: .monospaced))
            Button {
                Task { await find() }
            } label: {
                HStack {
                    if loading { ProgressView() }
                    Text(loading ? "Looking…" : "Find mosaics")
                }
            }
            .disabled(loading || trimmedURL.isEmpty)
        } header: {
            Text("Server")
        } footer: {
            Text("Point at a `tesela-server`; it reports every mosaic it can host.")
                .font(.caption2)
        }
    }

    private var discoveredSection: some View {
        Section {
            if discovered.isEmpty && !loading {
                Text("No mosaics found on this server.")
                    .font(.system(size: 12))
                    .foregroundStyle(theme.fgFaint)
            }
            ForEach(discovered) { mosaic in
                discoveredRow(mosaic)
            }
            Button {
                showCreate = true
            } label: {
                Label("Create new mosaic", systemImage: "plus.circle.fill")
            }
        } header: {
            Text("Mosaics on this server")
        }
    }

    private func discoveredRow(_ mosaic: MosaicServerClient.DiscoveredMosaic) -> some View {
        let added = registry.profiles.contains {
            $0.serverURL == trimmedURL && $0.mosaicPath == mosaic.path
        }
        return Button {
            guard !added else { return }
            registry.add(
                MosaicProfile(name: mosaic.name, serverURL: trimmedURL, mosaicPath: mosaic.path),
                makeActive: false
            )
        } label: {
            HStack(spacing: 12) {
                Image(systemName: "circle.grid.3x3")
                    .foregroundStyle(theme.fgMuted)
                VStack(alignment: .leading, spacing: 2) {
                    Text(mosaic.name)
                        .foregroundStyle(theme.fgDefault)
                    Text("\(mosaic.note_count) notes")
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(theme.fgSubtle)
                }
                Spacer()
                Image(systemName: added ? "checkmark.circle.fill" : "plus.circle")
                    .foregroundStyle(added ? theme.typeQuery : theme.accentPrimary)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(added)
    }

    @MainActor
    private func find() async {
        loading = true
        error = nil
        defer {
            loading = false
            didSearch = true
        }
        do {
            discovered = try await MosaicServerClient.discovered(serverURL: trimmedURL)
        } catch {
            discovered = []
            self.error = describe(error)
        }
    }

    @MainActor
    private func createMosaic() async {
        let name = newName.trimmingCharacters(in: .whitespaces)
        newName = ""
        guard !name.isEmpty else { return }
        error = nil
        do {
            let path = try await MosaicServerClient.createMosaic(serverURL: trimmedURL, name: name)
            registry.add(
                MosaicProfile(name: name, serverURL: trimmedURL, mosaicPath: path),
                makeActive: false
            )
            await find()
        } catch {
            self.error = describe(error)
        }
    }

    private func describe(_ error: Error) -> String {
        (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
    }
}
