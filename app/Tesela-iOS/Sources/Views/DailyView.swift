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
    var transcription: TranscriptionStore? = nil

    @Environment(\.theme) private var theme
    @Environment(\.captureContext) private var captureContext
    @EnvironmentObject private var mosaicRegistry: MosaicRegistry
    @State private var editingBlockId: String? = nil
    @State private var navigationPath = NavigationPath()
    @State private var showDatePicker: Bool = false
    @State private var showSettings: Bool = false
    @State private var showMosaicSwitcher: Bool = false
    @State private var pickedDate: Date = Date()

    var body: some View {
        NavigationStack(path: $navigationPath) {
            VStack(spacing: 0) {
                DailyTopBar(
                    title: mosaic.todayLongLabel,
                    dateLabel: mosaic.todayLabel,
                    syncStatus: syncStatus,
                    onTapCalendar: { showDatePicker = true },
                    onTapSettings: { showSettings = true },
                    onTapMosaic: { showMosaicSwitcher = true }
                )
                ScrollView {
                    VStack(alignment: .leading, spacing: 0) {
                        Spacer().frame(height: 12)
                        todayBlocks
                        addBlockRow
                        SectionEyebrow(title: "Yesterday")
                        yesterdayBlocks
                        Spacer().frame(height: 80) // bottom chrome breathing room
                    }
                }
                .refreshable {
                    if let backend {
                        await mosaic.refresh(from: backend.backend)
                    }
                }
            }
            .background(theme.bg)
            .onChange(of: editingBlockId) { _, newValue in
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
            .onDisappear { captureContext.focusedBlock = nil }
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
                if let appearance, let backend, let syncState {
                    SettingsView(
                        appearance: appearance,
                        mosaic: mosaic,
                        syncState: syncState,
                        backend: backend,
                        transcription: transcription
                    )
                    .environment(\.theme, theme)
                    .environment(\.density, appearance.density)
                } else {
                    Text("Settings unavailable in this context.")
                        .padding()
                }
            }
        }
    }

    @ViewBuilder
    private var todayBlocks: some View {
        ForEach(mosaic.todayBlocks) { block in
            BlockRow(
                id: block.id,
                kind: block.kind,
                text: block.text,
                indent: block.indent,
                isDone: block.done,
                tags: block.tags,
                isEditing: editingBlockId == block.id,
                onToggleTask: { mosaic.toggleTask(id: block.id) },
                onTap: { editingBlockId = block.id },
                onCommitEdit: { newText in
                    mosaic.editTodayBlock(id: block.id, text: newText)
                    editingBlockId = nil
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
        ForEach(mosaic.yesterdayBlocks) { block in
            BlockRow(
                id: block.id,
                kind: block.kind,
                text: block.text,
                indent: block.indent,
                isDone: block.done,
                tags: block.tags,
                onToggleTask: { mosaic.toggleTask(id: block.id) }
            )
            .opacity(0.7)
        }
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

    private var syncStatus: DailyTopBar.SyncDotState {
        switch mosaic.connection {
        case .ready, .idle:           return .ok
        case .connecting, .switching: return .warn
        case .failed:                 return .err
        }
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
