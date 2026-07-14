import SwiftUI

/// Native touch-first kanban board for the saved-views surface
/// (tesela-ya4.5, spec decision 5): horizontally-PAGED columns (swipe or
/// tap the chevrons to move between columns, one column per screen) and a
/// long-press/button move-sheet to change a card's column — no vim keys, no
/// drag-and-drop. Column derivation, grouping, and move-target semantics
/// come from `KanbanLogic`; this view is presentation only.
struct GrKanbanBoard: View {
    let items: [QueryItem]
    let groupByDef: PropertyDef
    let groupByProp: String
    /// Navigate to the card's source page (mirrors the list surface's row tap).
    var onOpen: (QueryItem) -> Void
    /// Persist a move: `(item, columnValueToWrite)`. `""` means "clear to
    /// Unset" (`KanbanLogic.writeValue`).
    var onMove: (QueryItem, String) -> Void

    @Environment(\.theme) private var theme
    @State private var scrolledColumn: String?
    @State private var moveTarget: QueryItem?

    private var columns: [String] { KanbanLogic.columns(for: groupByDef) }
    private var grouped: [(column: String, items: [QueryItem])] {
        KanbanLogic.grouped(items, groupByProp: groupByProp, columns: columns)
    }
    private var currentIndex: Int {
        guard let scrolledColumn, let idx = columns.firstIndex(of: scrolledColumn) else { return 0 }
        return idx
    }

    var body: some View {
        VStack(spacing: 0) {
            columnHeader
            ScrollView(.horizontal, showsIndicators: false) {
                LazyHStack(alignment: .top, spacing: 0) {
                    ForEach(grouped, id: \.column) { entry in
                        columnBody(entry)
                            .containerRelativeFrame(.horizontal)
                            .id(entry.column)
                    }
                }
                .scrollTargetLayout()
            }
            .scrollTargetBehavior(.paging)
            .scrollPosition(id: $scrolledColumn)
        }
        .background(theme.bg)
        .onAppear {
            if scrolledColumn == nil { scrolledColumn = columns.first }
        }
        .sheet(item: $moveTarget) { item in
            moveSheet(for: item)
        }
    }

    // MARK: - Header (page indicator + tap-to-navigate chevrons)

    private var columnHeader: some View {
        let col = scrolledColumn ?? columns.first ?? ""
        let count = grouped.first(where: { $0.column == col })?.items.count ?? 0
        return HStack(spacing: 10) {
            Button {
                jump(to: currentIndex - 1)
            } label: {
                GrIcon(name: "arrow-left", size: 14)
            }
            .buttonStyle(.plain)
            .disabled(currentIndex == 0)
            .accessibilityLabel("Previous column")

            Spacer(minLength: 0)

            VStack(spacing: 3) {
                HStack(spacing: 6) {
                    Circle().fill(columnColor(col)).frame(width: 7, height: 7)
                    Text(columnLabel(col))
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(theme.fgDefault)
                }
                Text(count == 1 ? "1 card" : "\(count) cards")
                    .font(.system(size: 10.5, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }

            Spacer(minLength: 0)

            Button {
                jump(to: currentIndex + 1)
            } label: {
                GrIcon(name: "chevron-right", size: 14)
            }
            .buttonStyle(.plain)
            .disabled(currentIndex == columns.count - 1)
            .accessibilityLabel("Next column")
        }
        .foregroundStyle(theme.fgSubtle)
        .padding(.horizontal, 18)
        .padding(.vertical, 10)
        .overlay(alignment: .bottom) {
            Rectangle().fill(theme.lineSoft).frame(height: 1)
        }
    }

    private func jump(to index: Int) {
        guard columns.indices.contains(index) else { return }
        withAnimation { scrolledColumn = columns[index] }
    }

    // MARK: - Column body

    private func columnBody(_ entry: (column: String, items: [QueryItem])) -> some View {
        ScrollView {
            LazyVStack(spacing: 8) {
                ForEach(entry.items) { item in
                    card(item)
                }
                if entry.items.isEmpty {
                    Text("No cards")
                        .font(.system(size: 11.5))
                        .italic()
                        .foregroundStyle(theme.fgFaint)
                        .frame(maxWidth: .infinity)
                        .padding(.top, 24)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
        }
    }

    private func card(_ item: QueryItem) -> some View {
        VStack(alignment: .leading, spacing: 7) {
            Text(item.text.isEmpty ? "(empty block)" : item.text)
                .font(.system(size: 13.5))
                .foregroundStyle(theme.fgDefault)
                .multilineTextAlignment(.leading)
                .lineLimit(4)
            HStack(spacing: 8) {
                metaPill("in \(item.title.isEmpty ? item.page_id : item.title)")
                if let tag = item.primary_tag {
                    metaPill("#\(tag)")
                }
                Spacer(minLength: 0)
                Button {
                    moveTarget = item
                } label: {
                    Image(systemName: "arrow.left.arrow.right.square")
                        .font(.system(size: 14))
                        .foregroundStyle(theme.fgSubtle)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Move card")
                .accessibilityHint("Opens a sheet to move this card to another column.")
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 11)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(theme.bg2)
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(theme.lineSoft, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 10))
        .contentShape(Rectangle())
        .onTapGesture { onOpen(item) }
        .onLongPressGesture { moveTarget = item }
    }

    private func metaPill(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 10, design: .monospaced))
            .foregroundStyle(theme.fgSubtle)
            .lineLimit(1)
            .truncationMode(.middle)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(theme.bg4)
            .clipShape(RoundedRectangle(cornerRadius: 5))
    }

    // MARK: - Column label / color

    private func columnLabel(_ col: String) -> String {
        col == KanbanLogic.unsetColumn ? "Unset" : col
    }

    private func columnColor(_ col: String) -> Color {
        if col == KanbanLogic.unsetColumn { return theme.fgFaint }
        return TagPalette.color(for: col, override: groupByDef.choiceColors[col.lowercased()])
    }

    // MARK: - Move sheet

    private func moveSheet(for item: QueryItem) -> some View {
        GrKanbanMoveSheet(
            columns: columns,
            currentColumn: KanbanLogic.column(for: item, groupByProp: groupByProp, columns: columns),
            colorFor: columnColor,
            labelFor: columnLabel,
            onSelect: { target in
                moveTarget = nil
                onMove(item, KanbanLogic.writeValue(forColumn: target))
            }
        )
        .environment(\.theme, theme)
        .presentationDetents([.medium, .large])
    }
}

/// The move-to-column picker sheet — mirror of `KanbanColumnPicker.svelte`'s
/// semantics (list every column, highlight the current one, tap to move)
/// rendered as a native SwiftUI sheet rather than a positioned popover.
private struct GrKanbanMoveSheet: View {
    let columns: [String]
    let currentColumn: String
    let colorFor: (String) -> Color
    let labelFor: (String) -> String
    let onSelect: (String) -> Void

    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List(columns, id: \.self) { col in
                Button {
                    onSelect(col)
                } label: {
                    HStack(spacing: 10) {
                        Circle().fill(colorFor(col)).frame(width: 8, height: 8)
                        Text(labelFor(col))
                            .font(.system(size: 14))
                            .italic(col == KanbanLogic.unsetColumn)
                            .foregroundStyle(theme.fgDefault)
                        Spacer()
                        if col == currentColumn {
                            Image(systemName: "checkmark")
                                .foregroundStyle(theme.accentPrimary)
                        }
                    }
                }
                .listRowBackground(theme.bg2)
            }
            .scrollContentBackground(.hidden)
            .background(theme.bg)
            .navigationTitle("Move to")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
    }
}
