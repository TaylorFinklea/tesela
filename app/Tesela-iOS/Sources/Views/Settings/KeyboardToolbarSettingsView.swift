import SwiftUI

/// Settings surface that lets the user enable / disable / reorder the
/// buttons on the keyboard accessory toolbar shown while editing a
/// block. Storage is a single comma-separated `@AppStorage` string —
/// flat enough to round-trip easily on disk.
struct KeyboardToolbarSettingsView: View {
    @AppStorage("keyboardToolbarItems") private var itemsRaw: String = defaultKeyboardToolbarItemsRaw

    @Environment(\.theme) private var theme

    private var enabled: [KeyboardToolbarItem] {
        decodeKeyboardToolbarItems(itemsRaw).filter { $0 != .hideKeyboard }
    }

    private var available: [KeyboardToolbarItem] {
        let on = Set(enabled)
        // Hide-keyboard is always pinned to the right of the toolbar
        // and is not user-configurable, so it doesn't appear here.
        return KeyboardToolbarItem.allCases.filter {
            $0 != .hideKeyboard && !on.contains($0)
        }
    }

    var body: some View {
        List {
            Section {
                if enabled.isEmpty {
                    Text("No buttons enabled — your keyboard toolbar will be empty.")
                        .font(.footnote)
                        .foregroundStyle(theme.fgMuted)
                } else {
                    ForEach(enabled) { item in
                        HStack(spacing: 12) {
                            Image(systemName: item.systemImage)
                                .frame(width: 24)
                            Text(item.label)
                            Spacer()
                            Button {
                                disable(item)
                            } label: {
                                Image(systemName: "minus.circle.fill")
                                    .foregroundStyle(theme.typeTask)
                            }
                            .buttonStyle(.plain)
                            .accessibilityLabel("Remove \(item.label)")
                        }
                    }
                    .onMove(perform: move)
                }
            } header: {
                Text("Enabled")
            } footer: {
                Text("Drag the handle on the right to reorder. The first item appears on the left of the scrollable toolbar; the Hide-keyboard button is always pinned to the right edge.")
                    .font(.caption2)
            }

            if !available.isEmpty {
                Section("Available") {
                    ForEach(available) { item in
                        Button {
                            enable(item)
                        } label: {
                            HStack(spacing: 12) {
                                Image(systemName: item.systemImage)
                                    .frame(width: 24)
                                Text(item.label)
                                    .foregroundStyle(theme.fgDefault)
                                Spacer()
                                Image(systemName: "plus.circle.fill")
                                    .foregroundStyle(theme.accentPrimary)
                            }
                        }
                        .buttonStyle(.plain)
                    }
                }
            }

            Section {
                Button {
                    itemsRaw = defaultKeyboardToolbarItemsRaw
                } label: {
                    Text("Reset to defaults")
                        .foregroundStyle(theme.accentPrimary)
                }
            }
        }
        .navigationTitle("Keyboard toolbar")
        .toolbar { EditButton() }
        .environment(\.editMode, .constant(.active))
    }

    // MARK: - Mutations

    private func enable(_ item: KeyboardToolbarItem) {
        var next = enabled
        guard !next.contains(item) else { return }
        next.append(item)
        itemsRaw = encodeKeyboardToolbarItems(next)
    }

    private func disable(_ item: KeyboardToolbarItem) {
        var next = enabled
        next.removeAll { $0 == item }
        itemsRaw = encodeKeyboardToolbarItems(next)
    }

    private func move(from source: IndexSet, to destination: Int) {
        var next = enabled
        next.move(fromOffsets: source, toOffset: destination)
        itemsRaw = encodeKeyboardToolbarItems(next)
    }
}
