import SwiftUI

/// The iOS command palette sheet — a search field over `commands`, the
/// manifest-derived list `GrAppShell` builds via `GrCommand.palette(from:)`
/// (tesela-cib / ADR-4). Opened from the keyboard toolbar's Commands button
/// (the `:`/leader stand-in); `onRun` hands the chosen command back to
/// `GrAppShell` to execute, then the sheet dismisses.
struct GrCommandPalette: View {
    let commands: [GrCommand]
    let onRun: (GrCommand) -> Void

    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss
    @State private var query: String = ""
    @FocusState private var fieldFocused: Bool

    private var results: [GrCommand] { GrCommand.matching(query, in: commands) }

    var body: some View {
        VStack(spacing: 0) {
            field
            Rectangle().fill(theme.line).frame(height: 1)
            list
        }
        .background(theme.bg)
        .presentationDetents([.medium, .large])
        .presentationDragIndicator(.visible)
        .onAppear { fieldFocused = true }
    }

    private var field: some View {
        HStack(spacing: 9) {
            Image(systemName: "command")
                .font(.system(size: 14, weight: .medium))
                .foregroundStyle(theme.fgFaint)
            TextField("Run a command…", text: $query)
                .focused($fieldFocused)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .submitLabel(.go)
                .onSubmit { if let first = results.first { run(first) } }
                .font(.system(size: 16))
                .foregroundStyle(theme.fgDefault)
                .tint(theme.accentPrimary)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 15)
    }

    @ViewBuilder
    private var list: some View {
        if results.isEmpty {
            ContentUnavailableView(
                "No commands",
                systemImage: "command",
                description: Text("Nothing matches **\(query)**")
            )
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(theme.bg)
        } else {
            List {
                ForEach(results) { cmd in
                    Button { run(cmd) } label: { row(cmd) }
                        .listRowBackground(theme.bg2)
                }
            }
            .listStyle(.plain)
            .scrollContentBackground(.hidden)
            .background(theme.bg)
        }
    }

    private func row(_ cmd: GrCommand) -> some View {
        HStack(spacing: 12) {
            Text(cmd.glyph)
                .font(.system(size: 15))
                .foregroundStyle(theme.accentPrimary)
                .frame(width: 24)
            VStack(alignment: .leading, spacing: 2) {
                Text(cmd.label)
                    .font(.system(size: 15, weight: .medium))
                    .foregroundStyle(theme.fgDefault)
                Text(cmd.hint)
                    .font(.system(size: 12))
                    .foregroundStyle(theme.fgMuted)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
        }
        .padding(.vertical, 4)
        .contentShape(Rectangle())
    }

    private func run(_ cmd: GrCommand) {
        dismiss()
        onRun(cmd)
    }
}
