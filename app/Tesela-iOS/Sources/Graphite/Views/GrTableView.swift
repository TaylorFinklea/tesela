import SwiftUI

/// Native compact columnar table for the saved-views surface (tesela-ya4.6,
/// spec decision 5 + gap G6): a PINNED first (block-text) column plus
/// horizontally-scrollable typed property columns, readable at iPhone
/// width — no vim keys, no cell editing (touch-first, mirrors Kanban's
/// no-drag-and-drop posture). Column resolution, config application
/// (hide/reorder), cell values, and sort come from `TableLogic`; this view
/// is presentation only.
///
/// Layout: the outer `ScrollView(.vertical)` scrolls the pinned column and
/// the property-columns region TOGETHER (they're both its direct
/// children); the inner `ScrollView(.horizontal)` scrolls ONLY the
/// property-columns region, independent of the pinned column — the
/// standard SwiftUI frozen-first-column pattern. Every cell shares a fixed
/// `rowHeight` so the two regions' rows stay aligned regardless of text
/// length (text is line-limited, not dynamically sized).
struct GrTableView: View {
    let items: [QueryItem]
    /// Already hide/reorder-projected via `TableLogic.applyConfig` — the
    /// caller resolves + applies the stored config; this view never
    /// re-derives it.
    let columns: [PropertyDef]
    /// The saved view's stored sort (`SavedViewTableConfig.sortBy/sortDir`),
    /// when it names a currently-visible column — seeds the INITIAL local
    /// sort only. Tapping a header afterward changes SESSION-LOCAL state
    /// below and never writes back: persisting hide/reorder/sort from iOS
    /// is out of scope (the stored config is owned by the web editor).
    let storedSort: (column: String, direction: TableSortDirection)?
    /// Navigate to the row's source page — same semantics as the kanban
    /// board's `onOpen` / the list surface's row tap.
    var onOpen: (QueryItem) -> Void

    @Environment(\.theme) private var theme
    @State private var localSort: (column: String, direction: TableSortDirection)?

    private let pinnedWidth: CGFloat = 160
    private let columnWidth: CGFloat = 128
    private let rowHeight: CGFloat = 50

    private var effectiveSort: (column: String, direction: TableSortDirection)? {
        localSort ?? storedSort
    }

    private var sortedItems: [QueryItem] {
        guard let sort = effectiveSort,
              let col = columns.first(where: { $0.name == sort.column }) else {
            return items
        }
        return TableLogic.sortRows(items, column: col, direction: sort.direction)
    }

    var body: some View {
        ScrollView(.vertical, showsIndicators: true) {
            HStack(alignment: .top, spacing: 0) {
                pinnedColumn
                Rectangle().fill(theme.lineSoft).frame(width: 1)
                ScrollView(.horizontal, showsIndicators: true) {
                    scrollableColumns
                }
            }
        }
        .background(theme.bg)
    }

    // MARK: - Pinned "Block" column

    private var pinnedColumn: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Block")
                .font(.system(size: 10.5, weight: .semibold, design: .monospaced))
                .foregroundStyle(theme.fgSubtle)
                .frame(width: pinnedWidth, height: rowHeight, alignment: .leading)
                .padding(.horizontal, 10)
                .background(theme.bg2)
                .overlay(alignment: .bottom) {
                    Rectangle().fill(theme.lineSoft).frame(height: 1)
                }
            ForEach(sortedItems) { item in
                Text(item.text.isEmpty ? "(empty block)" : item.text)
                    .font(.system(size: 12.5))
                    .foregroundStyle(theme.fgDefault)
                    .lineLimit(2)
                    .multilineTextAlignment(.leading)
                    .frame(width: pinnedWidth, height: rowHeight, alignment: .leading)
                    .padding(.horizontal, 10)
                    .contentShape(Rectangle())
                    .onTapGesture { onOpen(item) }
                    .overlay(alignment: .bottom) {
                        Rectangle().fill(theme.lineSoft.opacity(0.5)).frame(height: 1)
                    }
            }
        }
    }

    // MARK: - Scrollable property columns

    private var scrollableColumns: some View {
        HStack(alignment: .top, spacing: 0) {
            ForEach(columns, id: \.name) { column in
                VStack(alignment: .leading, spacing: 0) {
                    columnHeader(column)
                    ForEach(sortedItems) { item in
                        cell(item, column: column)
                    }
                }
            }
        }
    }

    private func columnHeader(_ column: PropertyDef) -> some View {
        HStack(spacing: 3) {
            Text(column.name)
                .font(.system(size: 10.5, weight: .semibold, design: .monospaced))
                .foregroundStyle(theme.fgSubtle)
                .lineLimit(1)
            if effectiveSort?.column == column.name {
                Image(systemName: effectiveSort?.direction == .desc ? "chevron.down" : "chevron.up")
                    .font(.system(size: 9, weight: .bold))
                    .foregroundStyle(theme.accentPrimary)
            }
        }
        .frame(width: columnWidth, height: rowHeight, alignment: .leading)
        .padding(.horizontal, 10)
        .background(theme.bg2)
        .contentShape(Rectangle())
        .onTapGesture { toggleSort(column) }
        .overlay(alignment: .bottom) {
            Rectangle().fill(theme.lineSoft).frame(height: 1)
        }
        .accessibilityLabel("\(column.name) column, tap to sort")
    }

    private func cell(_ item: QueryItem, column: PropertyDef) -> some View {
        let raw = TableLogic.rawValue(for: item, column: column)
        return HStack(spacing: 0) {
            if raw.trimmingCharacters(in: .whitespaces).isEmpty {
                Text("—")
                    .font(.system(size: 12.5))
                    .foregroundStyle(theme.fgFaint.opacity(0.5))
            } else {
                PropertyChip(key: column.name, value: raw, def: column, tint: chipTint(for: column, value: raw))
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
        }
        .frame(width: columnWidth, height: rowHeight, alignment: .leading)
        .padding(.horizontal, 10)
        .contentShape(Rectangle())
        .onTapGesture { onOpen(item) }
        .overlay(alignment: .bottom) {
            Rectangle().fill(theme.lineSoft.opacity(0.5)).frame(height: 1)
        }
    }

    // MARK: - Sort toggle (session-local only — never persists)

    private func toggleSort(_ column: PropertyDef) {
        if effectiveSort?.column == column.name {
            let nextDir: TableSortDirection = effectiveSort?.direction == .asc ? .desc : .asc
            localSort = (column.name, nextDir)
        } else {
            localSort = (column.name, .asc)
        }
    }

    // MARK: - Chip tint (registry choice colors)

    /// The `choiceColors` tint for a select/multi-select cell value —
    /// mirrors `BlockRow`'s private `chipTint(forKey:value:)`, adapted to
    /// take the already-resolved column def directly (the table already
    /// has it via `TableLogic.resolveColumns`, so no by-tag lookup is
    /// needed here).
    private func chipTint(for column: PropertyDef, value: String) -> Color? {
        guard column.valueType == .select || column.valueType == .multiSelect else { return nil }
        if column.choiceColors.isEmpty { return nil }
        let raw = value.trimmingCharacters(in: .whitespaces)
        guard !raw.isEmpty else { return nil }
        let parts: [String] = column.valueType == .multiSelect
            ? raw.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }
            : [raw]
        for p in parts {
            if let css = column.choiceColors[p.lowercased()],
               let hex = TagPalette.resolveOverride(css) {
                return Color(hex: hex)
            }
        }
        return nil
    }
}
