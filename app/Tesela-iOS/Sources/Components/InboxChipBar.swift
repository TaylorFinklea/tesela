import SwiftUI

/// Chip toolbar that sits above the Inbox list. Renders the current
/// chip state (static toggleable chips, active type pills, exclusion
/// pills, read-only unknown clauses) and reports user toggles back to
/// the parent via callbacks. The parent (`InboxView`) is responsible
/// for the actual DSL write-back + reload — this view is purely
/// presentational over a `ChipState` value.
///
/// Mirrors `web/src/lib/ambients/inbox/ChipBar.svelte`. Stage 4 will
/// add the saved-filter dropdown + "+ Save as…" button; stage 5 will
/// add the "Edit raw query" disclosure.
struct InboxChipBar: View {
    let state: ChipState
    var onToggleStatic: (String) -> Void
    var onRemoveType: (String) -> Void
    var onUnhidePage: (String) -> Void
    var onUnhideBlock: (String) -> Void

    @Environment(\.theme) private var theme

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            staticRow
            if !state.activeTypes.isEmpty
                || !state.hiddenPages.isEmpty
                || !state.hiddenBlocks.isEmpty
                || !state.unknownClauses.isEmpty
            {
                dynamicRow
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(theme.bg)
    }

    // MARK: - Static chips row

    private var staticRow: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(chipRegistry, id: \.id) { chip in
                    let isActive = state.active[chip.id] == true
                    Button {
                        onToggleStatic(chip.id)
                    } label: {
                        Label {
                            Text(chip.label)
                                .font(.system(size: 12, weight: .medium))
                        } icon: {
                            Text(chip.glyph)
                                .font(.system(size: 12))
                        }
                        .padding(.horizontal, 9)
                        .padding(.vertical, 5)
                        .background(
                            isActive ? theme.accentPrimary.opacity(0.18) : Color.clear
                        )
                        .overlay(
                            Capsule()
                                .stroke(
                                    isActive ? theme.accentPrimary : theme.lineSoft,
                                    lineWidth: 1
                                )
                        )
                        .clipShape(Capsule())
                        .foregroundStyle(
                            isActive ? theme.accentPrimary : theme.fgDefault
                        )
                    }
                    .buttonStyle(.plain)
                    .accessibilityHint(chip.hint)
                }
            }
            .padding(.horizontal, 2)
        }
    }

    // MARK: - Dynamic-content row (types + exclusions + unknown)

    private var dynamicRow: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                if !state.activeTypes.isEmpty {
                    Text("TYPES:")
                        .font(.system(size: 9, weight: .semibold, design: .monospaced))
                        .tracking(0.8)
                        .foregroundStyle(theme.fgFaint)
                    ForEach(state.activeTypes, id: \.self) { name in
                        dismissPill(name, hint: "Remove \(name)") { onRemoveType(name) }
                    }
                }
                if !state.hiddenPages.isEmpty || !state.hiddenBlocks.isEmpty {
                    Text("HIDDEN:")
                        .font(.system(size: 9, weight: .semibold, design: .monospaced))
                        .tracking(0.8)
                        .foregroundStyle(theme.fgFaint)
                    ForEach(state.hiddenPages, id: \.self) { id in
                        dismissPill("page:\(id)", hint: "Unhide \(id)") { onUnhidePage(id) }
                    }
                    ForEach(state.hiddenBlocks, id: \.self) { id in
                        dismissPill("block:\(id)", hint: "Unhide \(id)") { onUnhideBlock(id) }
                    }
                }
                if !state.unknownClauses.isEmpty {
                    Text("RAW:")
                        .font(.system(size: 9, weight: .semibold, design: .monospaced))
                        .tracking(0.8)
                        .foregroundStyle(theme.fgFaint)
                    ForEach(state.unknownClauses, id: \.self) { clause in
                        unknownPill(clause)
                    }
                }
            }
            .padding(.horizontal, 2)
        }
    }

    // MARK: - Pill builders

    private func dismissPill(_ text: String, hint: String, onRemove: @escaping () -> Void) -> some View {
        Button(action: onRemove) {
            HStack(spacing: 4) {
                Text(text)
                    .font(.system(size: 11, design: .monospaced))
                Text("×")
                    .font(.system(size: 11, weight: .bold))
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .overlay(
                Capsule().stroke(theme.lineSoft, lineWidth: 1)
            )
            .clipShape(Capsule())
            .foregroundStyle(theme.fgDefault)
        }
        .buttonStyle(.plain)
        .accessibilityHint(hint)
    }

    private func unknownPill(_ clause: String) -> some View {
        Text(clause)
            .font(.system(size: 11, design: .monospaced))
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .overlay(
                Capsule()
                    .strokeBorder(theme.fgFaint, style: StrokeStyle(lineWidth: 1, dash: [3, 2]))
            )
            .clipShape(Capsule())
            .foregroundStyle(theme.fgFaint)
            .accessibilityHint("Read-only clause — edit via raw query")
    }
}
