import SwiftUI

// MARK: - SettingsView
struct SettingsView: View {
    @Environment(AppState.self) private var appState
    @Environment(ThemeManager.self) private var theme
    @State private var serverURL = Persistence.loadServerURL()

    var body: some View {
        @Bindable var theme = theme

        Form {
            Section("Appearance") {
                Picker("Color Scheme", selection: $theme.colorScheme) {
                    ForEach(AppColorScheme.allCases, id: \.self) { scheme in
                        Label(scheme.label, systemImage: scheme.icon).tag(scheme)
                    }
                }
                .pickerStyle(.segmented)

                LabeledContent("Accent Color") {
                    HStack(spacing: 6) {
                        ForEach(AppAccentColor.allCases, id: \.self) { color in
                            Button {
                                theme.accentColor = color
                            } label: {
                                Circle()
                                    .fill(color.color)
                                    .frame(width: 20, height: 20)
                                    .overlay {
                                        if theme.accentColor == color {
                                            Image(systemName: "checkmark")
                                                .font(.caption2).bold()
                                                .foregroundStyle(.white)
                                        }
                                    }
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
            }

            Section("Server") {
                LabeledContent("Server URL") {
                    TextField("http://localhost:7474", text: $serverURL)
                        .frame(width: 240)
                }
                Text("The tesela-server REST API endpoint.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
        .padding()
        .frame(width: 520, height: 300)
        .navigationTitle("Settings")
        .onChange(of: serverURL) { _, url in
            Persistence.saveServerURL(url)
        }
    }
}
