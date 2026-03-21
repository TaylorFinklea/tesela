import SwiftUI

// MARK: - SettingsView
struct SettingsView: View {
    @Environment(AppState.self) private var appState
    @State private var serverURL = Persistence.loadServerURL()

    var body: some View {
        Form {
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
        .frame(width: 480, height: 200)
        .navigationTitle("Settings")
        .onChange(of: serverURL) { _, url in
            Persistence.saveServerURL(url)
        }
    }
}
