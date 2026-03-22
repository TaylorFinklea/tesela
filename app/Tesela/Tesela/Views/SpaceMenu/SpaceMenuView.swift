import SwiftUI

// MARK: - SpaceMenuView
// Neovim which-key-style leader menu. Appears in Normal mode when Space is pressed.
// Two-level: categories → commands. Press a key at each level.

struct SpaceMenuView: View {
    @Environment(AppState.self) private var appState
    @State private var activeCategory: String?
    var onCommand: ((String) -> Void)?

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text(activeCategory == nil ? "Leader" : categoryLabel)
                    .font(.headline)
                Spacer()
                Text("ESC to close")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)

            Divider()

            // Content: categories or commands
            if let cat = activeCategory {
                // Show commands for this category
                let commands = CommandRegistry.commandsForCategory(cat)
                VStack(spacing: 0) {
                    ForEach(commands, id: \.command.id) { key, cmd in
                        SpaceMenuRow(key: key, label: cmd.label, icon: cmd.icon)
                    }
                }
                .padding(.vertical, 4)
            } else {
                // Show top-level categories
                VStack(spacing: 0) {
                    ForEach(CommandRegistry.spaceCategories, id: \.key) { cat in
                        SpaceMenuRow(key: cat.key, label: cat.label, icon: cat.icon)
                    }

                    Divider().padding(.vertical, 4)

                    // Direct actions
                    SpaceMenuRow(key: ":", label: "Command Palette", icon: "⌘")
                    SpaceMenuRow(key: "/", label: "Search Pages", icon: "🔍")
                }
                .padding(.vertical, 4)
            }
        }
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 10))
        .shadow(color: .black.opacity(0.3), radius: 16, y: 6)
        .frame(width: 260)
        .onKeyPress { press in
            handleKey(press.characters)
            return .handled
        }
        .onKeyPress(.escape) {
            if activeCategory != nil {
                activeCategory = nil
            } else {
                dismiss()
            }
            return .handled
        }
    }

    private var categoryLabel: String {
        CommandRegistry.spaceCategories.first { $0.key == activeCategory }?.label ?? ""
    }

    private func handleKey(_ chars: String) {
        if let cat = activeCategory {
            // In a category — look for a command match
            let commands = CommandRegistry.commandsForCategory(cat)
            if let match = commands.first(where: { $0.key == chars }) {
                dismiss()
                onCommand?(match.command.id)
            }
        } else {
            // Top level — look for a category or direct action
            if CommandRegistry.spaceCategories.contains(where: { $0.key == chars }) {
                activeCategory = chars
            } else if chars == ":" {
                dismiss()
                appState.isCommandPaletteVisible = true
            } else if chars == "/" {
                dismiss()
                appState.isCommandPaletteVisible = true
            }
        }
    }

    private func dismiss() {
        activeCategory = nil
        appState.isSpaceMenuVisible = false
    }
}

// MARK: - SpaceMenuRow
private struct SpaceMenuRow: View {
    let key: String
    let label: String
    let icon: String

    var body: some View {
        HStack(spacing: 10) {
            Text(key)
                .font(.system(.body, design: .monospaced))
                .foregroundStyle(Color.accentColor)
                .frame(width: 20, alignment: .center)
            Text(icon)
                .frame(width: 20)
            Text(label)
                .font(.body)
            Spacer()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 5)
    }
}
