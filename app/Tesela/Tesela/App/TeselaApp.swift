import SwiftUI

@main
struct TeselaApp: App {
    @State private var appState = AppState()
    @State private var theme = ThemeManager.shared

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(appState)
                .environment(theme)
                .frame(minWidth: 900, minHeight: 600)
                .preferredColorScheme(theme.preferredColorScheme)
                .tint(theme.tintColor)
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
                .environment(theme)
        }
    }
}
