import SwiftUI

// MARK: - TeselaCommands
// App-level menu bar commands

struct TeselaCommands: Commands {
    let appState: AppState

    var body: some Commands {
        CommandGroup(after: .newItem) {
            Button("New Page") {
                // Handled via AppState
            }
            .keyboardShortcut("n", modifiers: [.command, .shift])

            Button("Open Today's Journal") {
                Task { @MainActor in
                    if let page = try? await appState.api.getDailyNote() {
                        appState.open(page)
                    }
                }
            }
            .keyboardShortcut("j", modifiers: [.command])
        }

        CommandGroup(after: .sidebar) {
            Button(appState.isLeftSidebarVisible ? "Hide Left Sidebar" : "Show Left Sidebar") {
                appState.isLeftSidebarVisible.toggle()
            }
            .keyboardShortcut("[", modifiers: [.command])

            Button(appState.isRightSidebarVisible ? "Hide Right Sidebar" : "Show Right Sidebar") {
                appState.isRightSidebarVisible.toggle()
            }
            .keyboardShortcut("]", modifiers: [.command])
        }

        CommandMenu("View") {
            Button("Command Palette") {
                appState.isCommandPaletteVisible.toggle()
            }
            .keyboardShortcut("k", modifiers: [.command])

            Button("Find in Pages") {
                appState.selectedNavItem = .pages
                appState.currentPage = nil
                appState.isSearchVisible = true
            }
            .keyboardShortcut("f", modifiers: [.command])
        }
    }
}
