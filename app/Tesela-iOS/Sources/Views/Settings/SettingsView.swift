import SwiftUI

/// Root Settings view — native Form-based settings tree. Reachable
/// from Library's top-bar gear button. Each section mirrors the
/// canvas's T-S1 main settings layout, but uses native iOS controls
/// (Toggle, Picker, NavigationLink) per the user's "use native buttons
/// wherever possible" directive.
struct SettingsView: View {
    @ObservedObject var appearance: AppearanceController
    @ObservedObject var mosaic: MockMosaicService
    @ObservedObject var syncState: SyncState
    @ObservedObject var backend: BackendSettings
    var transcription: TranscriptionStore? = nil

    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss
    @EnvironmentObject private var mosaicRegistry: MosaicRegistry

    @AppStorage("captureDefaultTarget") private var captureDefault: CaptureDefault = .contextAware
    @AppStorage("bareDateField") private var bareDateField: String = "scheduled"
    @State private var showMosaicSwitcher: Bool = false

    var body: some View {
        NavigationStack {
            Form {
                // Mosaic — list, switch, add/edit (replaces the old
                // placeholder workspace/device chips).
                Section("Mosaics") {
                    Button {
                        showMosaicSwitcher = true
                    } label: {
                        HStack(spacing: 10) {
                            if let active = mosaicRegistry.activeProfile {
                                Image(systemName: active.iconSymbol)
                                    .frame(width: 24)
                                    .foregroundStyle(theme.accentPrimary)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(active.name)
                                        .foregroundStyle(theme.fgDefault)
                                    Text("\(mosaicRegistry.profiles.count) \(mosaicRegistry.profiles.count == 1 ? "mosaic" : "mosaics")")
                                        .font(.caption2)
                                        .foregroundStyle(theme.fgSubtle)
                                }
                            } else {
                                Image(systemName: "circle.grid.3x3")
                                    .frame(width: 24)
                                    .foregroundStyle(theme.fgMuted)
                                Text("No mosaic selected")
                                    .foregroundStyle(theme.fgMuted)
                            }
                            Spacer()
                            Image(systemName: "chevron.right")
                                .font(.caption)
                                .foregroundStyle(theme.fgFaint)
                        }
                    }
                    .buttonStyle(.plain)
                }

                Section("Workspace") {
                    LabeledContent("Pages",    value: "\(mosaic.pages.filter { !$0.hidden }.count)")
                    LabeledContent("Tags",     value: "\(mosaic.tags.count)")
                    LabeledContent("Pinned",   value: "\(mosaic.pinned.count)")
                    LabeledContent("Scratch",  value: "\(mosaic.pages.filter { $0.type == "scratch" }.count)")
                    LabeledContent("Archived", value: "—")
                }

                // Appearance — theme + density
                Section("Appearance") {
                    NavigationLink {
                        ThemePickerView(appearance: appearance)
                    } label: {
                        LabeledContent("Theme", value: appearance.themeID.displayName)
                    }
                    NavigationLink {
                        DensityPickerView(appearance: appearance)
                    } label: {
                        LabeledContent("Density", value: appearance.density.displayName)
                    }
                    LabeledContent("Color scheme", value: "Always dark")
                }

                // Sync
                Section("Sync") {
                    NavigationLink {
                        SyncSettingsView(syncState: syncState, mosaic: mosaic)
                    } label: {
                        LabeledContent("Sync") {
                            HStack(spacing: 6) {
                                Circle().fill(theme.typeQuery).frame(width: 6, height: 6)
                                Text("on")
                            }
                        }
                    }
                    NavigationLink {
                        PairDeviceView(backend: backend, mosaic: mosaic, registry: mosaicRegistry)
                    } label: {
                        LabeledContent("Pair a new device", value: "")
                    }
                }

                // Bridges (cross-app integrations only — voice has its
                // own section per decision #12)
                Section("Bridges") {
                    NavigationLink("Apple Calendar")  { Text("placeholder").padding() }
                    NavigationLink("Apple Reminders") { Text("placeholder").padding() }
                    NavigationLink("Shortcuts")        { Text("placeholder").padding() }
                    NavigationLink("Share extension")  { Text("placeholder").padding() }
                    NavigationLink("Files")            { Text("placeholder").padding() }
                    NavigationLink("API webhooks")     { Text("placeholder").padding() }
                }

                Section("Capture") {
                    Picker("Default target", selection: $captureDefault) {
                        ForEach(CaptureDefault.allCases, id: \.self) { option in
                            Text(option.label).tag(option)
                        }
                    }
                    Picker("Default date field", selection: $bareDateField) {
                        Text("Scheduled").tag("scheduled")
                        Text("Deadline").tag("deadline")
                    }
                    NavigationLink("Keyboard toolbar") {
                        KeyboardToolbarSettingsView()
                            .environment(\.theme, theme)
                    }
                }

                // Voice (top-level — decision #12)
                Section("Voice") {
                    NavigationLink {
                        VoiceSettingsView(transcription: transcription)
                    } label: {
                        LabeledContent("Transcription", value: voiceModelLabel)
                    }
                    if let transcription {
                        NavigationLink {
                            TranscriptionModelsView(store: transcription)
                        } label: {
                            LabeledContent("Manage models") {
                                Text(modelsCountLabel(transcription))
                                    .font(.system(size: 11, design: .monospaced))
                                    .foregroundStyle(theme.fgSubtle)
                            }
                        }
                    }
                }

                Section("Advanced") {
                    NavigationLink {
                        BackendSettingsView(backend: backend, mosaic: mosaic)
                    } label: {
                        LabeledContent("Backend") {
                            Text(backendLabel)
                                .font(.system(size: 11, design: .monospaced))
                                .foregroundStyle(theme.fgSubtle)
                        }
                    }
                    NavigationLink("Export mosaic")     { Text("placeholder").padding() }
                    NavigationLink("Diagnostics")        { Text("placeholder").padding() }
                    Button(role: .destructive) {
                        // Reset placeholder
                    } label: {
                        Text("Reset this device")
                    }
                }

                Section {
                    Text("Tesela for iPhone · v0.4.1 · tesela-core 0.9.2")
                        .font(.system(size: 10.5, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                        .frame(maxWidth: .infinity, alignment: .center)
                }
            }
            .scrollContentBackground(.hidden)
            .background(theme.bg)
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.large)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") { dismiss() }
                }
            }
            .sheet(isPresented: $showMosaicSwitcher) {
                MosaicSwitcherSheet(registry: mosaicRegistry)
                    .environment(\.theme, theme)
            }
        }
    }

    private var backendLabel: String {
        switch backend.mode {
        case .mock: return "Mock"
        case .http: return backend.serverURL
        }
    }

    private var voiceModelLabel: String {
        guard let transcription, !transcription.activeModelId.isEmpty else {
            return "no model"
        }
        return TranscriptionCatalog.find(transcription.activeModelId)?.displayName ?? transcription.activeModelId
    }

    private func modelsCountLabel(_ store: TranscriptionStore) -> String {
        let downloaded = store.states.values.filter {
            if case .downloaded = $0 { return true } else { return false }
        }.count
        return "\(downloaded)/\(TranscriptionCatalog.all.count) downloaded"
    }
}
