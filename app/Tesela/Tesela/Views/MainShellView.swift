import SwiftUI

// MARK: - MainShellView
// Three-pane NavigationSplitView: Sidebar | Content | Right Panel

struct MainShellView: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        @Bindable var state = appState

        NavigationSplitView {
            SidebarView()
        } content: {
            ContentArea()
        } detail: {
            if state.isRightSidebarVisible {
                RightSidebarView()
            } else {
                Color.clear
            }
        }
        .overlay(alignment: .center) {
            if state.isCommandPaletteVisible {
                CommandPaletteView()
                    .transition(.scale(scale: 0.95).combined(with: .opacity))
            }
        }
        .animation(.spring(duration: 0.2), value: state.isCommandPaletteVisible)
    }
}
