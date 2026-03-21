import SwiftUI

@main
struct TeselaApp: App {
    @State private var appState = AppState()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(appState)
                .frame(minWidth: 900, minHeight: 600)
        }
        .windowStyle(.automatic)
        .windowToolbarStyle(.unified)
        .commands {
            TeselaCommands(appState: appState)
        }
        .defaultSize(width: 1280, height: 800)

        Settings {
            SettingsView()
                .environment(appState)
        }
    }
}
