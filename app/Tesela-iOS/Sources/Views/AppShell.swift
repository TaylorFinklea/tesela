import SwiftUI

/// Top-level scaffold. Uses iOS 26's native `TabView` so the bottom
/// chrome is a real system-managed Liquid Glass tab bar — correct
/// height, safe-area offset, scroll-edge blur, and minimize-on-scroll
/// all come from the system. Search uses iOS 26's `Tab(role: .search)`
/// so the system pins it as a standalone Liquid Glass circle at the
/// trailing edge (Phone/Mail/Photos pattern). Capture stays a sheet,
/// triggered from the TopBar.
struct AppShell: View {
    @StateObject private var appearance = AppearanceController()
    @StateObject private var mosaic = MockMosaicService()
    @StateObject private var pageStack = PageStack()
    @StateObject private var syncState = SyncState()
    @StateObject private var backend = BackendSettings()
    @StateObject private var transcription = TranscriptionStore()
    @StateObject private var mosaicRegistry = MosaicRegistry()
    @State private var activeTab: AppTab = .daily
    @State private var captureContext: CaptureContext = .init()
    /// Lifted out of CaptureBar so the AVAudioEngine init isn't paid
    /// every time the bar is added/removed from `tabViewBottomAccessory`
    /// (e.g., when a block enters/leaves edit mode).
    @StateObject private var streamRecorder = StreamingVoiceRecorder()

    @AppStorage("onboardingComplete") private var onboardingComplete: Bool = false
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        TeselaAppearance(controller: appearance) {
            if onboardingComplete {
                shell
                    .task {
                        // First launch: if no mosaic profiles exist
                        // yet, seed one from the legacy
                        // `backend.serverURL` value so existing users
                        // don't lose their connection.
                        mosaicRegistry.seedFromLegacyIfNeeded(
                            legacyURL: backend.serverURL,
                            defaultName: "My mosaic"
                        )
                        applyActiveMosaic()
                        mosaic.attach(backend: backend.backend)
                        await mosaic.refresh(from: backend.backend)
                    }
                    .onChange(of: mosaicRegistry.activeID) { _, _ in
                        applyActiveMosaic()
                        mosaic.attach(backend: backend.backend)
                        Task { await mosaic.refresh(from: backend.backend) }
                    }
                    .onChange(of: scenePhase) { _, newPhase in
                        // Foreground auto-refresh: when the user
                        // brings the app back, pull both the daily
                        // and any pages they had open so cross-device
                        // edits land without manual pull-to-refresh.
                        if newPhase == .active {
                            Task {
                                await mosaic.refresh(from: backend.backend)
                                await mosaic.refreshLoadedPages()
                            }
                        }
                    }
            } else {
                OnboardingView(
                    onboardingComplete: $onboardingComplete,
                    backend: backend,
                    mosaic: mosaic
                )
            }
        }
    }

    /// Sync `BackendSettings.serverURL` with the active mosaic's URL.
    /// Called on first launch and whenever the user switches profiles.
    private func applyActiveMosaic() {
        guard let active = mosaicRegistry.activeProfile else { return }
        if backend.serverURL != active.serverURL {
            backend.serverURL = active.serverURL
        }
    }

    private var shell: some View {
        TabView(selection: $activeTab) {
            Tab(AppTab.daily.label, systemImage: AppTab.daily.systemImage, value: AppTab.daily) {
                DailyView(
                    mosaic: mosaic,
                    backend: backend,
                    appearance: appearance,
                    syncState: syncState,
                    transcription: transcription
                )
            }
            Tab(AppTab.inbox.label, systemImage: AppTab.inbox.systemImage, value: AppTab.inbox) {
                InboxView(
                    mosaic: mosaic,
                    backend: backend,
                    appearance: appearance,
                    syncState: syncState,
                    transcription: transcription
                )
            }
            Tab(AppTab.library.label, systemImage: AppTab.library.systemImage, value: AppTab.library) {
                LibraryView(
                    mosaic: mosaic,
                    appearance: appearance,
                    pageStack: pageStack,
                    syncState: syncState,
                    backend: backend,
                    transcription: transcription
                )
            }
            Tab(value: AppTab.search, role: .search) {
                SearchView(mosaic: mosaic, pageStack: pageStack, syncState: syncState)
            }
        }
        .tint(appearance.theme.accentPrimary)
        .tabViewBottomAccessory {
            // Always show the bar. iOS lifts it above the keyboard
            // automatically when a TextField is focused (Slack
            // composer pattern). Hiding it during edits left a weird
            // empty zone above the keyboard, so we keep it visible.
            CaptureBar(
                mosaic: mosaic,
                activeTab: activeTab,
                transcription: transcription,
                context: captureContext,
                recorder: streamRecorder
            )
            .environment(\.theme, appearance.theme)
        }
        .environment(\.captureContext, captureContext)
        .environment(\.openSearch, { activeTab = .search })
        .environmentObject(mosaicRegistry)
    }
}

// MARK: - Capture context

/// Lightweight reference to a page so the capture bar's target menu
/// can offer "Add to <this page>" when applicable.
struct CapturePageRef: Hashable, Sendable {
    let slug: String
    let title: String
}

/// Lightweight reference to a focused block so the capture bar's
/// target menu can offer "Add as child of <this block>". `pageSlug`
/// is `nil` for today's daily.
struct CaptureBlockRef: Hashable, Sendable {
    let id: String
    let preview: String
    let pageSlug: String?
}

/// Single source of truth for the ambient capture context: the page
/// currently being viewed (Library) and the block currently being
/// edited. The capture bar reads this to build its target menu.
///
/// Note: an earlier decision hid the bar entirely while a block was
/// being edited. We reversed that here because the user wants
/// "Add as child of <focused block>" available from the menu — which
/// only makes sense if the bar is visible during the edit.
@Observable
final class CaptureContext {
    var currentPage: CapturePageRef? = nil
    var focusedBlock: CaptureBlockRef? = nil
}

private struct CaptureContextKey: EnvironmentKey {
    static let defaultValue: CaptureContext = .init()
}

// MARK: - Action environment values

/// Search-tab switcher exposed to TopBar. Capture no longer needs an
/// equivalent — the persistent `CaptureBar` is always visible (except
/// during block edits) and reachable by tap.
private struct OpenSearchKey: EnvironmentKey {
    static let defaultValue: () -> Void = {}
}

extension EnvironmentValues {
    var captureContext: CaptureContext {
        get { self[CaptureContextKey.self] }
        set { self[CaptureContextKey.self] = newValue }
    }

    var openSearch: () -> Void {
        get { self[OpenSearchKey.self] }
        set { self[OpenSearchKey.self] = newValue }
    }
}

// MARK: - Tab SF-Symbol mapping

extension AppTab {
    /// SF Symbol name for the tab's glyph. Native system icons on
    /// iOS per Taylor's "use the SF equivalents" direction.
    var systemImage: String {
        switch self {
        case .daily:   return "calendar"
        case .inbox:   return "tray"
        case .library: return "doc.text"
        case .search:  return "magnifyingglass"
        }
    }
}
