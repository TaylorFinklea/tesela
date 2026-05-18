import SwiftUI

/// The editable frontmatter `tags:` strip rendered directly under the
/// page title. Mirrors the web's `PageTagsChips.svelte` (decision #8):
/// each tag is a chip with an × to remove, plus a trailing + that opens
/// a picker for adding existing or new tags.
struct PageTagsChips: View {
    let pageId: String
    @Binding var tags: [String]
    /// All existing tag names from the mosaic — feeds the picker's
    /// suggestion list.
    let knownTags: [String]

    @Environment(\.theme) private var theme
    @State private var pickerOpen: Bool = false
    @State private var pickerFilter: String = ""
    @FocusState private var pickerFocused: Bool

    var body: some View {
        HStack(alignment: .center, spacing: 6) {
            ForEach(tags, id: \.self) { name in
                chip(for: name)
            }
            addButton
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 8)
        .sheet(isPresented: $pickerOpen) {
            picker
                .presentationDetents([.medium])
                .presentationDragIndicator(.visible)
        }
    }

    // ── Chip ────────────────────────────────────────────────────────────

    private func chip(for name: String) -> some View {
        HStack(spacing: 4) {
            HStack(spacing: 0) {
                Text("#").foregroundStyle(theme.fgFaint)
                Text(name).foregroundStyle(theme.accentPrimary)
            }
            .font(.system(size: 11.5, design: .monospaced))

            Button {
                tags.removeAll { $0.lowercased() == name.lowercased() }
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 9, weight: .semibold))
                    .foregroundStyle(theme.accentPrimary.opacity(0.5))
                    .frame(width: 14, height: 14)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Remove \(name)")
        }
        .padding(.leading, 8)
        .padding(.trailing, 4)
        .padding(.vertical, 2)
        .background(theme.accentPrimary.opacity(0.12))
        .overlay(
            Capsule()
                .stroke(theme.accentPrimary.opacity(0.30), lineWidth: 1)
        )
        .clipShape(Capsule())
    }

    private var addButton: some View {
        Button {
            pickerFilter = ""
            pickerOpen = true
        } label: {
            Text("+")
                .font(.system(size: 13))
                .foregroundStyle(theme.fgSubtle)
                .padding(.horizontal, 10)
                .padding(.vertical, 1)
                .background(
                    Capsule()
                        .stroke(theme.line, style: StrokeStyle(lineWidth: 1, dash: [3, 2]))
                )
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Add tag")
    }

    // ── Picker (modal sheet) ────────────────────────────────────────────

    private var picker: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 0) {
                TextField("filter or create…", text: $pickerFilter)
                    .font(.system(size: 14, design: .monospaced))
                    .padding(.horizontal, 14)
                    .padding(.vertical, 10)
                    .focused($pickerFocused)

                Divider()

                List {
                    ForEach(suggestions, id: \.self) { name in
                        Button {
                            addTag(name)
                        } label: {
                            HStack {
                                Text("#\(name)")
                                    .font(.system(size: 13, design: .monospaced))
                                    .foregroundStyle(theme.fgDefault)
                                Spacer()
                            }
                        }
                        .buttonStyle(.plain)
                    }
                    if showCreateRow {
                        Button {
                            addTag(pickerFilter.trimmingCharacters(in: .whitespaces))
                        } label: {
                            HStack {
                                Text(#"+ create "\#(pickerFilter.trimmingCharacters(in: .whitespaces))""#)
                                    .font(.system(size: 13, design: .monospaced))
                                    .foregroundStyle(theme.accentPrimary)
                                Spacer()
                            }
                        }
                        .buttonStyle(.plain)
                    }
                }
                .listStyle(.plain)
            }
            .background(theme.bg2)
            .navigationTitle("Add tag")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") { pickerOpen = false }
                }
            }
        }
        .onAppear { pickerFocused = true }
    }

    private var suggestions: [String] {
        let f = pickerFilter.trimmingCharacters(in: .whitespaces).lowercased()
        let active = Set(tags.map { $0.lowercased() })
        return knownTags
            .filter { !active.contains($0.lowercased()) }
            .filter { f.isEmpty || $0.lowercased().contains(f) }
            .prefix(20)
            .map { $0 }
    }

    private var showCreateRow: Bool {
        let f = pickerFilter.trimmingCharacters(in: .whitespaces).lowercased()
        guard !f.isEmpty else { return false }
        return !knownTags.contains { $0.lowercased() == f }
    }

    private func addTag(_ raw: String) {
        let clean = raw.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !clean.isEmpty else { return }
        guard !tags.contains(where: { $0.lowercased() == clean }) else {
            pickerOpen = false
            return
        }
        tags.append(clean)
        pickerOpen = false
    }
}
