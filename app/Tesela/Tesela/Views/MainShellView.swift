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
        .sheet(isPresented: $state.isShowingNewPageSheet) {
            NewPageSheet()
        }
        .alert("Error", isPresented: Binding(
            get: { state.lastError != nil },
            set: { if !$0 { state.lastError = nil } }
        )) {
            Button("OK") { state.lastError = nil }
        } message: {
            Text(state.lastError ?? "")
        }
    }
}

// MARK: - NewPageSheet
private struct NewPageSheet: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss
    @State private var title = ""
    @FocusState private var isFocused: Bool

    var body: some View {
        VStack(spacing: 20) {
            Text("New Page")
                .font(.headline)

            TextField("Page title", text: $title)
                .textFieldStyle(.roundedBorder)
                .focused($isFocused)
                .onSubmit { create() }
                .frame(width: 300)

            HStack {
                Button("Cancel") { dismiss() }
                    .keyboardShortcut(.escape, modifiers: [])
                Button("Create") { create() }
                    .buttonStyle(.borderedProminent)
                    .disabled(title.trimmingCharacters(in: .whitespaces).isEmpty)
                    .keyboardShortcut(.return, modifiers: [])
            }
        }
        .padding(24)
        .onAppear { isFocused = true }
    }

    private func create() {
        let trimmed = title.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return }
        dismiss()
        Task { await appState.createPage(title: trimmed) }
    }
}
