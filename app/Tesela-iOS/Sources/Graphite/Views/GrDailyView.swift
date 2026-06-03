import SwiftUI
import UIKit

/// Graphite Daily — the journal front door, re-themed to Graphite over
/// the SAME `MockMosaicService` + `BlockRow` editor the legacy
/// `DailyView` uses. No editing engine is rebuilt: today's blocks bind
/// to `mosaic.todayBlocks`, yesterday's to `mosaic.yesterdayBlocks`, and
/// every interaction routes through the existing service mutations
/// (`editTodayBlock`, `appendTodayBlock`, `toggleTask`, `indentTodayBlock`,
/// `cycleBlockStatus`, `setBlockProperties`, `recurBump`).
///
/// Only the chrome is new: a `GrHeader` large-title, the Graphite
/// day-divider (`.gr-dayhdr`), and a `NavigationStack` that pushes
/// `GrPageView` on wiki-link / past-day taps. Reads `@Environment(\.theme)`
/// (forced to `.graphite` by `GrAppShell`).
struct GrDailyView: View {
    @ObservedObject var mosaic: MockMosaicService
    var backend: BackendSettings? = nil

    @Environment(\.theme) private var theme
    @Environment(\.captureContext) private var captureContext
    @Environment(\.openSettings) private var openSettings

    @State private var editingBlockId: String? = nil
    @State private var navigationPath = NavigationPath()
    @State private var showDatePicker: Bool = false
    @State private var pickedDate: Date = Date()

    var body: some View {
        NavigationStack(path: $navigationPath) {
            VStack(spacing: 0) {
                GrHeader(title: mosaic.todayLongLabel, subtitle: "JOURNAL") {
                    GrButton(icon: "calendar") { showDatePicker = true }
                    GrButton(icon: "settings") { openSettings() }
                }
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        Spacer().frame(height: 8)
                        todaySection
                        addBlockRow
                        daySpacer
                        dayDivider(label: "Yesterday", dow: yesterdayDow, today: false)
                        yesterdaySection
                        pastDaysSection
                        Spacer().frame(height: 96)
                    }
                    .padding(.horizontal, 10)
                }
                .refreshable {
                    if let backend {
                        await mosaic.refresh(from: backend.backend, userInitiated: true)
                    }
                }
            }
            .background(theme.bg)
            .onChange(of: editingBlockId) { _, newValue in
                // Hold off live remote refreshes while editing so an
                // inbound WS event can't replace text under the cursor —
                // mirrors DailyView.
                mosaic.isEditingBlock = (newValue != nil)
                // C1-inbound: tell the service which block is open so an
                // inbound remote splice on it can be live-applied to the
                // editor (the deferred full refresh still covers the rest).
                mosaic.editingBlockId = newValue
                // Drop any previously-registered editor inserter on EVERY
                // change (close OR switch-to-another-block). onChange fires
                // before the newly-focused block's onAppear re-registers its
                // own, so a remote splice arriving in that gap finds a nil
                // inserter (no-op) rather than the wrong block's text view.
                mosaic.openBlockInserter = nil
                if let id = newValue,
                   let block = mosaic.todayBlocks.first(where: { $0.id == id })
                    ?? mosaic.yesterdayBlocks.first(where: { $0.id == id })
                {
                    captureContext.focusedBlock = CaptureBlockRef(
                        id: id, preview: block.text, pageSlug: nil
                    )
                } else {
                    captureContext.focusedBlock = nil
                }
            }
            .onChange(of: mosaic.todayBlocks.map(\.id) + mosaic.yesterdayBlocks.map(\.id)) { _, newIds in
                if let editing = editingBlockId, !newIds.contains(editing) {
                    editingBlockId = nil
                }
            }
            .onDisappear {
                captureContext.focusedBlock = nil
                mosaic.isEditingBlock = false
            }
            .environment(\.openURL, OpenURLAction { url in
                if let slug = TeselaLink.pageSlug(from: url) {
                    navigationPath.append(GrPageRoute(slug: slug))
                    return .handled
                }
                return .systemAction
            })
            .navigationDestination(for: GrPageRoute.self) { route in
                GrPageView(slug: route.slug, mosaic: mosaic, path: $navigationPath)
                    .environment(\.theme, theme)
            }
            .sheet(isPresented: $showDatePicker) {
                datePickerSheet
            }
        }
    }

    // ── Today ───────────────────────────────────────────────────────────

    @ViewBuilder
    private var todaySection: some View {
        ForEach(mosaic.todayBlocks) { block in
            BlockRow(
                id: block.id,
                kind: block.kind,
                text: block.displayText,
                indent: block.indent,
                isDone: block.done,
                tags: block.tags,
                properties: block.properties,
                isEditing: editingBlockId == block.id,
                onToggleTask: { mosaic.toggleTask(id: block.id) },
                onTap: { editingBlockId = block.id },
                onCommitEdit: { _ in
                    // Collab (splice) path: the block text was already
                    // persisted keystroke-by-keystroke via splices, so
                    // commit must NOT re-author the whole text (that would
                    // Myers-diff against the engine and could re-clobber a
                    // peer's concurrent chars). Just finalize the edit.
                    editingBlockId = nil
                },
                onTextSplice: { offset, deleteLen, insert in
                    // Collab editing C1 outbound: route the user's actual
                    // keystroke to the engine's per-block LoroText so a
                    // concurrent same-block edit merges instead of being
                    // clobbered.
                    mosaic.spliceTodayBlock(
                        id: block.id,
                        utf16Offset: offset,
                        utf16DeleteLen: deleteLen,
                        insert: insert
                    )
                },
                onActiveCollabInserter: { inserter in
                    // Collab editing C1 inbound: register the open editor's
                    // inserter so a remote splice on this block live-applies.
                    mosaic.openBlockInserter = inserter
                },
                onMenuAction: { action in handleTodayAction(action, on: block) },
                onSplitToNewBlock: { _ in
                    // The current block's text (incl. the trailing-newline
                    // trim) was already persisted via splices, so do NOT
                    // re-author it here — just append a new sibling and
                    // move focus, mirroring the old split's tail.
                    let newId = mosaic.appendTodayBlock(kind: .note)
                    editingBlockId = newId
                },
                onIndent: { delta in mosaic.indentTodayBlock(id: block.id, by: delta) },
                onCycleStatus: { mosaic.cycleBlockStatus(id: block.id) },
                onSetProperties: { updated in
                    mosaic.setBlockProperties(id: block.id, properties: updated)
                },
                onSkipRecurrence: {
                    Task { try? await mosaic.recurBump(blockId: block.id, mode: .skip) }
                }
            )
        }
    }

    private func handleTodayAction(_ action: BlockAction, on block: Block) {
        switch action {
        case .edit:        editingBlockId = block.id
        case .delete:      mosaic.deleteTodayBlock(id: block.id)
        case .yankLink:    UIPasteboard.general.string = "tesela://block/\(block.id)"
        case .indent:      mosaic.indentTodayBlock(id: block.id, by: 1)
        case .archive:     mosaic.deleteTodayBlock(id: block.id)
        case .promote, .convertToTag, .moveTo: break
        }
    }

    // ── Yesterday ───────────────────────────────────────────────────────

    @ViewBuilder
    private var yesterdaySection: some View {
        ForEach(mosaic.yesterdayBlocks) { block in
            BlockRow(
                id: block.id,
                kind: block.kind,
                text: block.displayText,
                indent: block.indent,
                isDone: block.done,
                tags: block.tags,
                properties: block.properties,
                isEditing: editingBlockId == block.id,
                onToggleTask: { mosaic.toggleTask(id: block.id) },
                onTap: { editingBlockId = block.id },
                onCommitEdit: { newText in
                    mosaic.editYesterdayBlock(id: block.id, text: newText)
                    editingBlockId = nil
                },
                onTextChanged: { newText in
                    mosaic.editYesterdayBlock(id: block.id, text: newText)
                },
                onMenuAction: { action in handleYesterdayAction(action, on: block) },
                onSplitToNewBlock: { committedText in
                    mosaic.editYesterdayBlock(id: block.id, text: committedText)
                    let newId = mosaic.appendYesterdayBlock(kind: .note)
                    editingBlockId = newId
                },
                onIndent: { delta in mosaic.indentYesterdayBlock(id: block.id, by: delta) }
            )
            .opacity(0.72)
        }
    }

    private func handleYesterdayAction(_ action: BlockAction, on block: Block) {
        switch action {
        case .edit:        editingBlockId = block.id
        case .delete:      mosaic.deleteYesterdayBlock(id: block.id)
        case .yankLink:    UIPasteboard.general.string = "tesela://block/\(block.id)"
        case .indent:      mosaic.indentYesterdayBlock(id: block.id, by: 1)
        case .archive:     mosaic.deleteYesterdayBlock(id: block.id)
        case .promote, .convertToTag, .moveTo: break
        }
    }

    // ── Past days (display-only; header pushes the editable page) ────────

    @ViewBuilder
    private var pastDaysSection: some View {
        ForEach(mosaic.pastDailies) { day in
            daySpacer
            Button {
                navigationPath.append(GrPageRoute(slug: day.id))
            } label: {
                dayDivider(label: dayLabel(day.id), dow: dowLabel(day.id), today: false)
            }
            .buttonStyle(.plain)
            ForEach(day.blocks) { block in
                BlockRow(
                    id: block.id,
                    kind: block.kind,
                    text: block.displayText,
                    indent: block.indent,
                    isDone: block.done,
                    tags: block.tags
                )
                .opacity(0.6)
            }
        }
    }

    // ── Graphite day divider (.gr-dayhdr) ───────────────────────────────

    private func dayDivider(label: String, dow: String, today: Bool) -> some View {
        HStack(spacing: 11) {
            Text(label)
                .font(.system(size: 15, weight: .semibold))
                .tracking(-0.15)
                .foregroundStyle(today ? theme.accentPrimary : theme.fgDefault)
            Text(dow)
                .font(.system(size: 10.5, design: .monospaced))
                .tracking(0.8)
                .foregroundStyle(theme.fgFaint)
            Rectangle()
                .fill(theme.lineSoft)
                .frame(height: 1)
                .frame(maxWidth: .infinity)
        }
        .padding(.horizontal, 8)
        .padding(.top, 6)
        .padding(.bottom, 10)
    }

    private var addBlockRow: some View {
        Button {
            let newId = mosaic.appendTodayBlock(kind: .note)
            editingBlockId = newId
        } label: {
            HStack(spacing: 10) {
                GrIcon(name: "plus", size: 13)
                    .foregroundStyle(theme.fgFaint)
                    .frame(width: 14)
                Text("Add block")
                    .font(.system(size: 13))
                    .foregroundStyle(theme.fgFaint)
                Spacer()
            }
            .padding(.leading, 18)
            .padding(.trailing, 18)
            .padding(.vertical, 8)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Add block to today")
    }

    /// ~1/3-viewport gap between day sections so each day reads as its
    /// own space — mirrors DailyView's `daySpacer`.
    private var daySpacer: some View {
        Color.clear
            .containerRelativeFrame(.vertical) { length, _ in length / 4 }
    }

    private var datePickerSheet: some View {
        NavigationStack {
            VStack(spacing: 20) {
                DatePicker("Pick a daily", selection: $pickedDate, displayedComponents: [.date])
                    .datePickerStyle(.graphical)
                    .tint(theme.accentPrimary)
                    .padding(.horizontal, 16)
                GrButton(variant: .cta, label: "Open daily") {
                    let f = DateFormatter()
                    f.dateFormat = "yyyy-MM-dd"
                    let slug = f.string(from: pickedDate)
                    showDatePicker = false
                    navigationPath.append(GrPageRoute(slug: slug))
                }
                .frame(maxWidth: .infinity)
                .padding(.horizontal, 16)
            }
            .padding(.top, 12)
            .background(theme.bg)
            .navigationTitle("Open a daily")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { showDatePicker = false }
                }
            }
        }
        .presentationDetents([.medium, .large])
    }

    // ── Date helpers ────────────────────────────────────────────────────

    private var yesterdayDow: String {
        let cal = Calendar.current
        let date = cal.date(byAdding: .day, value: -1, to: mosaic.todayDate) ?? mosaic.todayDate
        return dowString(from: date)
    }

    /// "2026-05-20" → "Tuesday, May 20".
    private func dayLabel(_ id: String) -> String {
        guard let date = Self.dayParser.date(from: id) else { return id }
        let display = DateFormatter()
        display.dateFormat = "EEEE, MMMM d"
        return display.string(from: date)
    }

    private func dowLabel(_ id: String) -> String {
        guard let date = Self.dayParser.date(from: id) else { return "" }
        return dowString(from: date)
    }

    private func dowString(from date: Date) -> String {
        let f = DateFormatter()
        f.dateFormat = "EEE"
        return f.string(from: date).uppercased()
    }

    private static let dayParser: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd"
        return f
    }()
}

/// A nav route on a Graphite NavigationStack. Holds a page slug; the
/// destination resolves it via `GrPageView`.
struct GrPageRoute: Hashable {
    let slug: String
}
