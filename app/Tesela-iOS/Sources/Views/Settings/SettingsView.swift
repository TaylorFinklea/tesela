import SwiftUI

/// Root Settings view — native Form-based settings tree. Reachable
/// from Library's top-bar gear button. Each section mirrors the
/// canvas's T-S1 main settings layout, but uses native iOS controls
/// (Toggle, Picker, NavigationLink) per the user's "use native buttons
/// wherever possible" directive.
struct SettingsView: View {
    @ObservedObject var appearance: AppearanceController
    @ObservedObject var mosaic: MockMosaicService

    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Form {
                // Mosaic identity
                Section("Mosaic") {
                    LabeledContent("Workspace") {
                        Text("design-mosaic")
                            .font(.system(.body, design: .monospaced))
                            .foregroundStyle(theme.fgMuted)
                    }
                    LabeledContent("This device") {
                        Text("tesela-ios-7f3")
                            .font(.system(.body, design: .monospaced))
                            .foregroundStyle(theme.fgMuted)
                    }
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
                        SyncSettingsView()
                    } label: {
                        LabeledContent("Sync") {
                            HStack(spacing: 6) {
                                Circle().fill(theme.typeQuery).frame(width: 6, height: 6)
                                Text("on")
                            }
                        }
                    }
                    NavigationLink {
                        PairDeviceView()
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

                // Voice (top-level — decision #12)
                Section("Voice") {
                    NavigationLink {
                        VoiceSettingsView()
                    } label: {
                        LabeledContent("Parakeet v3", value: "ready")
                    }
                }

                Section("Advanced") {
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
        }
    }
}
