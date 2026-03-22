import SwiftUI

// MARK: - SlashMenuView
// Notion/Logseq-style slash command menu. Appears in Insert mode when `/` is typed.

struct SlashMenuView: View {
    @Environment(AppState.self) private var appState
    @State private var query = ""
    @State private var selectedIndex = 0
    @FocusState private var isFocused: Bool
    var onCommand: ((String) -> Void)?

    private var results: [MenuCommand] {
        CommandRegistry.matching(query: query)
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                Text("/")
                    .font(.system(.body, design: .monospaced))
                    .foregroundStyle(.secondary)
                TextField("Type a command…", text: $query)
                    .textFieldStyle(.plain)
                    .font(.body)
                    .focused($isFocused)
                    .onKeyPress(.escape) {
                        dismiss()
                        return .handled
                    }
                    .onKeyPress(.upArrow) {
                        selectedIndex = max(0, selectedIndex - 1)
                        return .handled
                    }
                    .onKeyPress(.downArrow) {
                        selectedIndex = min(results.count - 1, selectedIndex + 1)
                        return .handled
                    }
                    .onKeyPress(.return) {
                        if selectedIndex < results.count {
                            execute(results[selectedIndex])
                        }
                        return .handled
                    }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)

            Divider()

            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(Array(results.enumerated()), id: \.element.id) { i, cmd in
                        HStack(spacing: 10) {
                            Text(cmd.icon)
                                .frame(width: 20)
                            Text(cmd.label)
                                .font(.body)
                            Spacer()
                            if let hint = cmd.shortcutHint {
                                Text(hint)
                                    .font(.caption)
                                    .foregroundStyle(.tertiary)
                            }
                        }
                        .padding(.horizontal, 12)
                        .padding(.vertical, 6)
                        .background(i == selectedIndex ? Color.accentColor.opacity(0.15) : .clear)
                        .onTapGesture { execute(cmd) }
                    }
                }
            }
            .frame(maxHeight: 250)
        }
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
        .shadow(color: .black.opacity(0.25), radius: 12, y: 4)
        .frame(width: 280)
        .onAppear {
            selectedIndex = 0
            isFocused = true
        }
        .onChange(of: query) { _, _ in
            selectedIndex = 0
        }
    }

    private func execute(_ cmd: MenuCommand) {
        dismiss()
        onCommand?(cmd.id)
    }

    private func dismiss() {
        appState.isSlashMenuVisible = false
    }
}
