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
    @StateObject private var liveSync = LiveSyncSocket()
    /// B.3.3 — background relay poll/push loop. Runs whenever the app
    /// is foregrounded; pauses in background. Mac-originated edits
    /// arrive via this loop within ~5s instead of the prior "tap the
    /// dev pull button or wait minutes" behaviour.
    @StateObject private var relayTicker = RelayTicker()
    @State private var activeTab: AppTab = .daily
    @State private var captureContext: CaptureContext = .init()
    /// Lifted out of CaptureBar so the AVAudioEngine init isn't paid
    /// every time the bar is added/removed from `tabViewBottomAccessory`
    /// (e.g., when a block enters/leaves edit mode).
    @StateObject private var streamRecorder = StreamingVoiceRecorder()
    /// The capture composer's text. Owned here (not as `CaptureBar`
    /// `@State`) so a voice transcript can be appended reliably even
    /// though the `tabViewBottomAccessory` recreates the bar.
    @StateObject private var composer = CaptureComposer()

    @AppStorage("onboardingComplete") private var onboardingComplete: Bool = false
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        TeselaAppearance(controller: appearance) {
            if onboardingComplete {
                shell
                    .task {
                        await activateMosaic(initial: true)
                        // Bind + start the relay ticker once the app
                        // is up. connect() is idempotent so re-runs
                        // (e.g. on mosaic switch) don't churn.
                        relayTicker.connect(mosaic: mosaic)
                        // Open the local sync engine eagerly — purely
                        // local, no network, so this succeeds even on
                        // an offline cold launch. Writes that happen
                        // before pairing completes (or while the user
                        // is on cellular without tailscale) still land
                        // durably in SQLite + the materialized notes
                        // dir, instead of being silently dropped.
                        do { try await relayTicker.openEngineIfNeeded() }
                        catch { /* surfaced via relayTicker.lastError */ }
                        // Route iOS-authored writes through the engine
                        // + relay alongside the existing HTTP PUT. On
                        // LAN both succeed (HTTP first); on cellular
                        // when Mac is unreachable the engine path is
                        // the only one that gets there.
                        mosaic.onLocalWrite = { [weak relayTicker] slug, title, content, createdAt in
                            Task { @MainActor [weak relayTicker] in
                                await relayTicker?.recordAndPush(
                                    slug: slug,
                                    title: title,
                                    content: content,
                                    createdAtMillis: createdAt
                                )
                            }
                        }
                        // When the ticker applies new inbound ops,
                        // re-pull the user-visible pages over HTTP so
                        // the UI shows the change immediately. On
                        // cellular this HTTP call will likely fail
                        // (and silently swallow the URLError.cancelled
                        // we filtered out above) — the data already
                        // lives in the local engine + sandbox; B.3.4
                        // makes the iOS UI read from there directly.
                        relayTicker.onAppliedChanges = { [weak mosaic, weak backend] in
                            guard let mosaic, let backend else { return }
                            Task {
                                await mosaic.refresh(from: backend.backend)
                                await mosaic.refreshLoadedPages()
                            }
                        }
                        relayTicker.start()
                    }
                    .onChange(of: mosaicRegistry.activeID) { _, _ in
                        Task { await activateMosaic(initial: false) }
                    }
                    .onChange(of: scenePhase) { _, newPhase in
                        // Foreground auto-refresh: when the user
                        // brings the app back, pull both the daily
                        // and any pages they had open so cross-device
                        // edits land without manual pull-to-refresh.
                        // The live-sync socket keeps things fresh while
                        // the app is open; it is torn down in the
                        // background and reconnected here.
                        switch newPhase {
                        case .active:
                            liveSync.nudge()
                            relayTicker.start()
                            Task {
                                await mosaic.refresh(from: backend.backend)
                                await mosaic.refreshLoadedPages()
                            }
                        case .background:
                            liveSync.suspend()
                            relayTicker.stop()
                        default:
                            break
                        }
                    }
                    .onChange(of: streamRecorder.lastTranscript) { _, transcript in
                        // A finished voice transcript — append it to the
                        // composer here, at the stable app root, rather
                        // than inside the churny capture-bar accessory.
                        guard let transcript else { return }
                        composer.append(transcript)
                        streamRecorder.lastTranscript = nil
                    }
            } else {
                OnboardingView(
                    onboardingComplete: $onboardingComplete,
                    backend: backend,
                    mosaic: mosaic,
                    registry: mosaicRegistry
                )
            }
        }
    }

    /// Point the data service at the active mosaic. Runs on first
    /// launch and whenever the user switches profiles. When the active
    /// profile names a specific on-disk mosaic, the server is asked to
    /// switch+restart onto it before the data is loaded.
    private func activateMosaic(initial: Bool) async {
        if initial {
            // First launch: if no profiles exist yet, seed one from the
            // legacy `backend.serverURL` so existing users keep working.
            mosaicRegistry.seedFromLegacyIfNeeded(
                legacyURL: backend.serverURL,
                defaultName: "My mosaic"
            )
            // Route live-sync events into the mosaic. Set once — the
            // socket itself is repointed per-mosaic below.
            liveSync.onNoteChange = { [mosaic] in
                Task { await mosaic.applyRemoteChange() }
            }
        }
        if let active = mosaicRegistry.activeProfile {
            if backend.serverURL != active.serverURL {
                backend.serverURL = active.serverURL
            }
            mosaic.attach(backend: backend.backend)
            if let path = active.mosaicPath, case .http = backend.backend {
                await mosaic.ensureServerMosaic(path: path, serverURL: active.serverURL)
            }
        } else {
            mosaic.attach(backend: backend.backend)
        }
        await mosaic.refresh(from: backend.backend)
        // Point the live-sync socket at the active server (or tear it
        // down in mock mode).
        if case .http = backend.backend {
            liveSync.connect(serverURL: backend.serverURL)
        } else {
            liveSync.connect(serverURL: nil)
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
                    relayTicker: relayTicker,
                    transcription: transcription
                )
            }
            Tab(AppTab.agenda.label, systemImage: AppTab.agenda.systemImage, value: AppTab.agenda) {
                AgendaView(
                    mosaic: mosaic,
                    backend: backend,
                    appearance: appearance,
                    syncState: syncState,
                    relayTicker: relayTicker,
                    transcription: transcription
                )
            }
            Tab(AppTab.inbox.label, systemImage: AppTab.inbox.systemImage, value: AppTab.inbox) {
                InboxView(
                    mosaic: mosaic,
                    backend: backend,
                    appearance: appearance,
                    syncState: syncState,
                    relayTicker: relayTicker,
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
                    relayTicker: relayTicker,
                    transcription: transcription
                )
            }
            Tab(value: AppTab.search, role: .search) {
                SearchView(mosaic: mosaic, pageStack: pageStack, syncState: syncState)
            }
        }
        // Tab bar / nav-chrome tint. `accentSpark` equals the primary
        // accent for every theme except Prism Spark, which lifts it to
        // the neon coral — so picking Prism Spark visibly lights up iOS.
        .tint(appearance.theme.accentSpark)
        .tabViewBottomAccessory {
            // The compact capture bar. Its text slot is a tap target,
            // not a focusable field — tapping it expands the composer
            // (below) rather than focusing here, where the keyboard
            // would drop behind the accessory.
            CaptureBar(
                mosaic: mosaic,
                activeTab: activeTab,
                transcription: transcription,
                context: captureContext,
                recorder: streamRecorder,
                composer: composer
            )
            .environment(\.theme, appearance.theme)
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            // The expanded composer. `safeAreaInset(.bottom)` is the
            // reliable "rides above the keyboard" placement (the chat
            // input-bar pattern) — `tabViewBottomAccessory` does not
            // lift an editable field above the keyboard.
            if composer.isExpanded {
                CaptureBar(
                    mosaic: mosaic,
                    activeTab: activeTab,
                    transcription: transcription,
                    context: captureContext,
                    recorder: streamRecorder,
                    composer: composer,
                    expanded: true
                )
                .environment(\.theme, appearance.theme)
                .transition(.move(edge: .bottom))
            }
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
        case .agenda:  return "list.bullet.below.rectangle"
        case .inbox:   return "tray"
        case .library: return "doc.text"
        case .search:  return "magnifyingglass"
        }
    }
}
