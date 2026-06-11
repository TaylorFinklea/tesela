import SwiftUI
import UIKit

/// The Daily front door. Today's blocks at the top with inline edit,
/// then a Yesterday section, then a tappable "+" affordance to append
/// a new empty block.
///
/// Wrapped in a `NavigationStack` so wiki-link taps inside blocks can
/// push to the linked page and the calendar button can push to any
/// past day's daily.
struct DailyView: View {
    @ObservedObject var mosaic: MockMosaicService
    /// Optional — drives pull-to-refresh and the dynamic sync dot.
    var backend: BackendSettings? = nil
    /// Settings entry hooks. Provided by AppShell so the top-bar gear
    /// and the (tappable) sync dot can both lead to the same place.
    var appearance: AppearanceController? = nil
    var syncState: SyncState? = nil
    var relayTicker: RelayTicker? = nil
    var transcription: TranscriptionStore? = nil

    @Environment(\.theme) private var theme
    @Environment(\.captureContext) private var captureContext
    @EnvironmentObject private var mosaicRegistry: MosaicRegistry
    @State private var editingBlockId: String? = nil
    @State private var navigationPath = NavigationPath()
    @State private var showDatePicker: Bool = false
    @State private var showSettings: Bool = false
    @State private var showMosaicSwitcher: Bool = false
    @State private var showSyncSettings: Bool = false
    @State private var pickedDate: Date = Date()
    @State private var collapsedBlockIds: Set<String> = []

    var body: some View {
        NavigationStack(path: $navigationPath) {
            VStack(spacing: 0) {
                TabHeader(
                    title: mosaic.todayLongLabel,
                    subtitle: mosaic.todayLabel,
                    syncStatus: syncStatus,
                    onTapCalendar: { showDatePicker = true },
                    onTapSettings: { showSettings = true },
                    onTapMosaic: { showMosaicSwitcher = true },
                    onTapSync: { showSyncSettings = true }
                )
                ConnectionBanner(connection: mosaic.connection) {
                    if let backend {
                        Task { await mosaic.refresh(from: backend.backend, userInitiated: true) }
                    }
                }
                ScrollView {
                    VStack(alignment: .leading, spacing: 0) {
                        Spacer().frame(height: 12)
                        if showsSkeleton {
                            // Cold-launch with nothing local AND HTTP
                            // still in flight — render placeholder
                            // bullets so the screen isn't a black
                            // void for the up-to-3-second HTTP
                            // window. Subsequent launches with a
                            // populated sandbox skip this branch
                            // entirely (todayBlocks populates from
                            // the engine-materialized files in
                            // <100ms via the local-first refresh).
                            skeletonBlocks
                        } else {
                            todayBlocks
                            addBlockRow
                            daySpacer
                            SectionEyebrow(title: "Yesterday")
                            yesterdayBlocks
                            pastDays
                        }
                        Spacer().frame(height: 80) // bottom chrome breathing room
                    }
                }
                .refreshable {
                    if let backend {
                        await mosaic.refresh(from: backend.backend, userInitiated: true)
                    }
                }
            }
            .background(theme.bg)
            .onChange(of: editingBlockId) { _, newValue in
                // Hold off live remote refreshes while a block is being
                // edited so an incoming WS event can't replace the text
                // out from under the cursor.
                mosaic.isEditingBlock = (newValue != nil)
                if let id = newValue, let block = mosaic.todayBlocks.first(where: { $0.id == id })
                    ?? mosaic.yesterdayBlocks.first(where: { $0.id == id })
                {
                    captureContext.focusedBlock = CaptureBlockRef(
                        id: id,
                        preview: block.text,
                        pageSlug: nil
                    )
                } else {
                    captureContext.focusedBlock = nil
                }
            }
            .onChange(of: mosaic.todayBlocks.map(\.id) + mosaic.yesterdayBlocks.map(\.id)) { _, newIds in
                // If the day rolled over (or a remote delete dropped the
                // block we were editing), the previously-focused block id
                // no longer exists in either visible section. Clear focus
                // so the keyboard doesn't latch onto a stale id from a
                // prior day's blocks.
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
                    pushPage(slug: slug)
                    return .handled
                }
                return .systemAction
            })
            .navigationDestination(for: DailyPageRoute.self) { route in
                if let page = route.resolvedPage(mosaic) {
                    PageViewWrapper(
                        page: page,
                        mosaic: mosaic,
                        syncState: SyncState() // local instance — sync indicator on Daily isn't shared
                    )
                    .environment(\.theme, theme)
                } else {
                    placeholderPage(slug: route.slug)
                }
            }
            .sheet(isPresented: $showDatePicker) {
                datePickerSheet
            }
            .sheet(isPresented: $showMosaicSwitcher) {
                MosaicSwitcherSheet(registry: mosaicRegistry)
                    .environment(\.theme, theme)
            }
            .sheet(isPresented: $showSettings) {
                if let appearance, let backend, let syncState, let relayTicker {
                    SettingsView(
                        appearance: appearance,
                        mosaic: mosaic,
                        syncState: syncState,
                        backend: backend,
                        relayTicker: relayTicker,
                        transcription: transcription
                    )
                    .environment(\.theme, theme)
                    .environment(\.density, appearance.density)
                } else {
                    Text("Settings unavailable in this context.")
                        .padding()
                }
            }
            .sheet(isPresented: $showSyncSettings) {
                // Deep-link to the Sync panel directly from the status
                // pill menu. Reuses the same SyncSettingsView the gear
                // → Sync path uses, just skipping the intermediate
                // Settings root.
                if let syncState, let relayTicker {
                    NavigationStack {
                        SyncSettingsView(
                            syncState: syncState,
                            mosaic: mosaic,
                            relayTicker: relayTicker
                        )
                        .navigationTitle("Sync")
                        .navigationBarTitleDisplayMode(.inline)
                    }
                    .environment(\.theme, theme)
                    .environment(\.density, appearance?.density ?? .compact)
                }
            }
        }
    }

    @ViewBuilder
    private var todayBlocks: some View {
        ForEach(BlockFold.visibleBlocks(in: mosaic.todayBlocks, collapsed: collapsedBlockIds)) { block in
            BlockRow(
                id: block.id,
                kind: block.kind,
                text: block.displayText,
                indent: block.indent,
                isDone: block.done,
                tags: block.tags,
                properties: block.properties,
                isEditing: editingBlockId == block.id,
                isFoldable: BlockFold.hasChildren(block: block, in: mosaic.todayBlocks),
                isCollapsed: collapsedBlockIds.contains(block.id),
                onToggleFold: { toggleFold(block.id) },
                onToggleTask: { mosaic.toggleTask(id: block.id) },
                onTap: { editingBlockId = block.id },
                onCommitEdit: { newText in
                    mosaic.editTodayBlock(id: block.id, text: newText)
                    editingBlockId = nil
                },
                onTextChanged: { newText in
                    mosaic.editTodayBlock(id: block.id, text: newText)
                },
                onMenuAction: { action in
                    handleTodayAction(action, on: block)
                },
                onSplitToNewBlock: { committedText in
                    mosaic.editTodayBlock(id: block.id, text: committedText)
                    let newId = mosaic.appendTodayBlock(kind: .note)
                    editingBlockId = newId
                },
                onIndent: { delta in
                    mosaic.indentTodayBlock(id: block.id, by: delta)
                },
                onCycleStatus: {
                    mosaic.cycleBlockStatus(id: block.id)
                },
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
        case .edit:
            editingBlockId = block.id
        case .delete:
            mosaic.deleteTodayBlock(id: block.id)
        case .yankLink:
            UIPasteboard.general.string = "tesela://block/\(block.id)"
        case .indent:
            mosaic.indentTodayBlock(id: block.id, by: 1)
        case .archive:
            // Strip from today by removing — true archive backlog flag
            // can land when the server exposes it.
            mosaic.deleteTodayBlock(id: block.id)
        case .promote, .convertToTag, .moveTo:
            // These are surfaced; backend support lands in a later
            // phase. For now treat them as no-ops with a haptic.
            break
        }
    }

    @ViewBuilder
    private var yesterdayBlocks: some View {
        ForEach(BlockFold.visibleBlocks(in: mosaic.yesterdayBlocks, collapsed: collapsedBlockIds)) { block in
            BlockRow(
                id: block.id,
                kind: block.kind,
                text: block.displayText,
                indent: block.indent,
                isDone: block.done,
                tags: block.tags,
                properties: block.properties,
                isEditing: editingBlockId == block.id,
                isFoldable: BlockFold.hasChildren(block: block, in: mosaic.yesterdayBlocks),
                isCollapsed: collapsedBlockIds.contains(block.id),
                onToggleFold: { toggleFold(block.id) },
                onToggleTask: { mosaic.toggleTask(id: block.id) },
                onTap: { editingBlockId = block.id },
                onCommitEdit: { newText in
                    mosaic.editYesterdayBlock(id: block.id, text: newText)
                    editingBlockId = nil
                },
                onTextChanged: { newText in
                    mosaic.editYesterdayBlock(id: block.id, text: newText)
                },
                onMenuAction: { action in
                    handleYesterdayAction(action, on: block)
                },
                onSplitToNewBlock: { committedText in
                    mosaic.editYesterdayBlock(id: block.id, text: committedText)
                    let newId = mosaic.appendYesterdayBlock(kind: .note)
                    editingBlockId = newId
                },
                onIndent: { delta in
                    mosaic.indentYesterdayBlock(id: block.id, by: delta)
                },
                onCycleStatus: {
                    // Today's cycle pulls from pageSlug=nil branch which
                    // mutates todayBlocks; yesterday needs its own routing.
                    // For now leave as a no-op until cycleBlockStatus
                    // gains a yesterday-aware branch (cheap follow-up).
                }
            )
            .opacity(0.7)
        }
    }

    private func handleYesterdayAction(_ action: BlockAction, on block: Block) {
        switch action {
        case .edit:
            editingBlockId = block.id
        case .delete:
            mosaic.deleteYesterdayBlock(id: block.id)
        case .yankLink:
            UIPasteboard.general.string = "tesela://block/\(block.id)"
        case .indent:
            mosaic.indentYesterdayBlock(id: block.id, by: 1)
        case .archive:
            mosaic.deleteYesterdayBlock(id: block.id)
        case .promote, .convertToTag, .moveTo:
            break
        }
    }

    /// Daily notes older than yesterday — dimmed, display-only. The
    /// date header is tappable and pushes the full editable daily page.
    @ViewBuilder
    private var pastDays: some View {
        ForEach(mosaic.pastDailies) { day in
            daySpacer
            Button {
                pushPage(slug: day.id)
            } label: {
                SectionEyebrow(title: dayLabel(day.id), hint: "open")
            }
            .buttonStyle(.plain)
            ForEach(BlockFold.visibleBlocks(in: day.blocks, collapsed: collapsedBlockIds)) { block in
                BlockRow(
                    id: block.id,
                    kind: block.kind,
                    text: block.displayText,
                    indent: block.indent,
                    isDone: block.done,
                    tags: block.tags,
                    isFoldable: BlockFold.hasChildren(block: block, in: day.blocks),
                    isCollapsed: collapsedBlockIds.contains(block.id),
                    onToggleFold: { toggleFold(block.id) }
                )
                .opacity(0.7)
            }
        }
    }

    /// A deliberate ~1/3-viewport gap between day sections so each day
    /// reads as its own space when scrolling the Daily feed.
    private var daySpacer: some View {
        Color.clear
            .containerRelativeFrame(.vertical) { length, _ in length / 3 }
    }

    /// True when we should show skeleton bullets instead of the real
    /// (empty) layout: HTTP is in flight AND we don't have any blocks
    /// to render yet (neither today nor yesterday). This is the
    /// genuine cold-launch case — a returning user with a populated
    /// sandbox falls through to the real layout in <100ms via the
    /// local-first hydrate path.
    private var showsSkeleton: Bool {
        mosaic.connection == .connecting
            && mosaic.todayBlocks.isEmpty
            && mosaic.yesterdayBlocks.isEmpty
    }

    private func toggleFold(_ blockId: String) {
        if collapsedBlockIds.contains(blockId) {
            collapsedBlockIds.remove(blockId)
        } else {
            collapsedBlockIds.insert(blockId)
        }
    }

    /// Five placeholder bullets at varying widths. Wrapped in a
    /// shimmer-ish opacity pulse so the user sees it's loading, not
    /// stuck. Keep this *visually quiet* — it's a placeholder, not
    /// content; over-styled skeletons fight for attention with the
    /// real thing that's about to land.
    private var skeletonBlocks: some View {
        VStack(alignment: .leading, spacing: 14) {
            ForEach([0.65, 0.85, 0.5, 0.78, 0.42], id: \.self) { fraction in
                HStack(spacing: 10) {
                    Circle()
                        .fill(theme.fgFaint)
                        .frame(width: 4, height: 4)
                    Capsule()
                        .fill(theme.fgFaint.opacity(0.5))
                        .frame(height: 14)
                        .containerRelativeFrame(.horizontal) { length, _ in length * fraction }
                }
                .padding(.leading, 18)
            }
        }
        .padding(.top, 4)
        .opacity(0.7)
        .transition(.opacity)
    }

    /// "2026-05-20" → "Tuesday, May 20". `SectionEyebrow` uppercases it.
    private func dayLabel(_ id: String) -> String {
        let parser = DateFormatter()
        parser.dateFormat = "yyyy-MM-dd"
        guard let date = parser.date(from: id) else { return id }
        let display = DateFormatter()
        display.dateFormat = "EEEE, MMMM d"
        return display.string(from: date)
    }

    private var addBlockRow: some View {
        Button {
            let newId = mosaic.appendTodayBlock(kind: .note)
            editingBlockId = newId
        } label: {
            HStack(spacing: 10) {
                Image(systemName: "plus.circle")
                    .font(.system(size: 14, weight: .regular))
                    .foregroundStyle(theme.fgFaint)
                    .frame(width: 14, alignment: .center)
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

    /// Date picker sheet — pick any date, push that day's daily.
    private var datePickerSheet: some View {
        NavigationStack {
            VStack(spacing: 20) {
                DatePicker(
                    "Pick a daily",
                    selection: $pickedDate,
                    displayedComponents: [.date]
                )
                .datePickerStyle(.graphical)
                .padding(.horizontal, 16)

                Button {
                    let f = DateFormatter()
                    f.dateFormat = "yyyy-MM-dd"
                    let slug = f.string(from: pickedDate)
                    showDatePicker = false
                    pushPage(slug: slug)
                } label: {
                    Text("Open daily")
                        .font(.system(size: 15, weight: .semibold))
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 12)
                        .foregroundStyle(theme.bg)
                        .background(theme.accentPrimary)
                        .clipShape(RoundedRectangle(cornerRadius: 10))
                }
                .buttonStyle(.plain)
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

    /// Resolve a wiki-link slug (or a date string) to a Page in the
    /// mosaic and push onto the nav stack. If the page isn't loaded
    /// yet, we push a placeholder so PageView can fetch it.
    private func pushPage(slug: String) {
        navigationPath.append(DailyPageRoute(slug: slug))
    }

    private func placeholderPage(slug: String) -> some View {
        VStack(spacing: 12) {
            Text("Page not found")
                .font(.system(size: 20, weight: .semibold))
                .foregroundStyle(theme.fgDefault)
            Text("Nothing in the mosaic has the id `\(slug)`.")
                .font(.system(size: 13, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 24)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(theme.bg)
    }

    private var syncStatus: TabHeader.SyncDotState {
        TabHeader.SyncDotState(mosaic.connection)
    }
}

/// A nav route used by DailyView. Holds a slug (page id) and resolves
/// to a `Page` via the mosaic. Hashable so it can ride on NavigationPath.
struct DailyPageRoute: Hashable {
    let slug: String

    @MainActor
    func resolvedPage(_ mosaic: MockMosaicService) -> Page? {
        // Prefer an existing record. Otherwise synthesize a minimal
        // Page that PageView can fetch into on appear.
        if let existing = mosaic.pages.first(where: { $0.id == slug }) {
            return existing
        }
        return Page(
            id: slug,
            title: slug,
            slug: slug,
            type: "note",
            edited: "",
            blocks: 0,
            refs: 0
        )
    }
}

/// Tiny wrapper that constructs a PageStack so PageView's required
/// dependency is satisfied when pushed from Daily/Search nav stacks.
struct PageViewWrapper: View {
    let page: Page
    @ObservedObject var mosaic: MockMosaicService
    @ObservedObject var syncState: SyncState
    @StateObject private var stack = PageStack()

    var body: some View {
        PageView(page: page, mosaic: mosaic, pageStack: stack, syncState: syncState)
            .onAppear { stack.open(page) }
    }
}
