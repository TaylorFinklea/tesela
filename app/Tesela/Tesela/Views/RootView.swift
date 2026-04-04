import SwiftUI

// MARK: - RootView
// Entry point view — shows connection status on launch, then main shell

struct RootView: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        Group {
            switch appState.connectionStatus {
            case .disconnected, .connecting:
                ConnectingView()
            case .connected:
                MainShellView()
            case .error(let message):
                ConnectionErrorView(message: message)
            }
        }
        .task {
            // Start server automatically if not already running
            let _ = await ServerManager.shared.ensureRunning()
            await appState.launch()
        }
    }
}

// MARK: - ConnectingView
private struct ConnectingView: View {
    var body: some View {
        VStack(spacing: 16) {
            ProgressView()
                .scaleEffect(1.5)
            Text("Connecting to Tesela server…")
                .font(.title3)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - ConnectionErrorView
struct ConnectionErrorView: View {
    let message: String
    @Environment(AppState.self) private var appState

    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 56))
                .foregroundStyle(.orange)

            Text("Server Not Running")
                .font(.title)
                .bold()

            Text("Could not connect to tesela-server at localhost:7474")
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)

            Text(message)
                .font(.caption)
                .foregroundStyle(.tertiary)
                .padding(.horizontal)

            Divider()
                .frame(maxWidth: 320)

            Text("The app tried to start tesela-server automatically but couldn't connect.")
                .foregroundStyle(.secondary)
                .font(.caption)
                .multilineTextAlignment(.center)

            Text("Make sure tesela-server is installed:\ncargo install --path crates/tesela-server")
                .font(.system(.caption, design: .monospaced))
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .background(.fill.secondary, in: RoundedRectangle(cornerRadius: 8))

            Button("Retry") {
                Task {
                    let _ = await ServerManager.shared.ensureRunning()
                    await appState.launch()
                }
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(40)
    }
}
