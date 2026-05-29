import SwiftUI

/// Graphite app shell — the new presentation root that MIRRORS
/// `Sources/Views/AppShell.swift` and BINDS TO THE SAME data/sync
/// services. No behavior is rebuilt: the iOS-26 native `TabView`
/// (4 labeled tabs + the `Tab(role: .search)` glass circle), the
/// `@StateObject MockMosaicService` + `RelayTicker`, the
/// `CaptureComposer` / `StreamingVoiceRecorder` capture wiring, and the
/// relay-ticker poll/push loop + the `LiveSyncSocket` WS push are reused as
/// AppShell wires them. Only the chrome (Graphite header, capture pill →
/// sheet) and the `.graphite` theme are new; the four content tabs render
/// the real daily-driver views (`GrDailyView` / `GrAgendaView` /
/// `GrInboxView` / `GrLibraryView`) over the shared `MockMosaicService`,
/// while `.search` keeps the placeholder (native search is wired separately).
///
/// Reachable today behind the `-graphite` launch arg / `tesela.useGraphiteShell`
/// default (see `TeselaApp.swift`); the default entry is still the shipping
/// `AppShell`, and GrAppShell becomes the sole root at cutover.
///
/// DEFERRED to cutover (NOT yet reused): `MosaicRegistry` — so GrAppShell
/// attaches the single `backend.serverURL` profile directly and has no
/// multi-profile switching / per-profile serverURL routing yet.
struct GrAppShell: View {
    @StateObject private var mosaic = MockMosaicService()
    @StateObject private var backend = BackendSettings()
    @StateObject private var relayTicker = RelayTicker()
    /// Live WS push channel (note_created/updated/deleted) — mirrors
    /// AppShell. Gives instant Mac→app updates instead of waiting for the
    /// RelayTicker poll, and routes through `applyRemoteChange()` so the
    /// refresh respects the edit-suppression guards.
    @StateObject private var liveSync = LiveSyncSocket()
    /// Lifted out of the capture bar so the `AVAudioEngine` init isn't
    /// paid every time the accessory is added/removed — mirrors AppShell.
    @StateObject private var streamRecorder = StreamingVoiceRecorder()
    /// The capture composer's text. Owned here (not as bar `@State`) so a
    /// voice transcript appends reliably despite `tabViewBottomAccessory`
    /// recreating the bar — mirrors AppShell.
    @StateObject private var composer = CaptureComposer()
    @StateObject private var transcription = TranscriptionStore()

    @State private var activeTab: AppTab = .daily
    @State private var captureContext: CaptureContext = .init()

    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        shell
            // Force the Graphite theme regardless of the user's saved
            // appearance — this is the Graphite shell. (Cutover folds it
            // back into the appearance controller.)
            .environment(\.theme, .graphite)
            .preferredColorScheme(.dark)
            .task {
                // Mirror AppShell's relay-ticker bring-up: attach the
                // mosaic, open the local engine eagerly, route iOS writes
                // through the engine + relay, re-pull on inbound changes,
                // and start the loop. connect()/openEngineIfNeeded() are
                // idempotent, so this is safe to re-run.
                mosaic.attach(backend: backend.backend)
                await mosaic.refresh(from: backend.backend)
                relayTicker.connect(mosaic: mosaic)
                do { try await relayTicker.openEngineIfNeeded() }
                catch { /* surfaced via relayTicker.lastError */ }
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
                relayTicker.onAppliedChanges = { [weak mosaic, weak backend] in
                    guard let mosaic, let backend else { return }
                    Task {
                        await mosaic.refresh(from: backend.backend)
                        await mosaic.refreshLoadedPages()
                    }
                }
                // Live WS push (mirrors AppShell.activateMosaic): instant
                // re-pull on Mac-originated note changes, routed through
                // applyRemoteChange() so it defers while editing.
                liveSync.onNoteChange = { [mosaic] in
                    Task { await mosaic.applyRemoteChange() }
                }
                if case .http = backend.backend {
                    liveSync.connect(serverURL: backend.serverURL)
                } else {
                    liveSync.connect(serverURL: nil)
                }
                relayTicker.start()
            }
            .onChange(of: scenePhase) { _, newPhase in
                switch newPhase {
                case .active:
                    relayTicker.start()
                    liveSync.nudge()
                    Task {
                        await mosaic.refresh(from: backend.backend)
                        await mosaic.refreshLoadedPages()
                    }
                case .background:
                    relayTicker.stop()
                    liveSync.suspend()
                default:
                    break
                }
            }
            .onChange(of: streamRecorder.lastTranscript) { _, transcript in
                // A finished voice transcript — append it to the composer
                // here, at the stable app root, mirroring AppShell.
                guard let transcript else { return }
                composer.append(transcript)
                streamRecorder.lastTranscript = nil
            }
    }

    /// The native iOS-26 `TabView`. Mirrors AppShell exactly: the four
    /// labeled `Tab`s + the trailing `Tab(role: .search)` glass circle,
    /// the compact capture bar in `tabViewBottomAccessory`, and the
    /// expanded composer riding above the keyboard via
    /// `safeAreaInset(.bottom)`. The four content tabs render their
    /// Graphite daily-driver views; `.search` keeps the placeholder.
    private var shell: some View {
        TabView(selection: $activeTab) {
            Tab(AppTab.daily.label, systemImage: AppTab.daily.systemImage, value: AppTab.daily) {
                GrDailyView(mosaic: mosaic, backend: backend)
            }
            Tab(AppTab.agenda.label, systemImage: AppTab.agenda.systemImage, value: AppTab.agenda) {
                GrAgendaView(mosaic: mosaic, backend: backend)
            }
            Tab(AppTab.inbox.label, systemImage: AppTab.inbox.systemImage, value: AppTab.inbox) {
                GrInboxView(mosaic: mosaic, backend: backend)
            }
            Tab(AppTab.library.label, systemImage: AppTab.library.systemImage, value: AppTab.library) {
                GrLibraryView(mosaic: mosaic, backend: backend)
            }
            Tab(value: AppTab.search, role: .search) {
                GrTabPlaceholder(tab: .search)
            }
        }
        .tint(Theme.graphite.accentPrimary)
        .tabViewBottomAccessory {
            GrCaptureBar(
                mosaic: mosaic,
                activeTab: activeTab,
                transcription: transcription,
                context: captureContext,
                recorder: streamRecorder,
                composer: composer
            )
            .environment(\.theme, .graphite)
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            if composer.isExpanded {
                GrCaptureSheet(
                    mosaic: mosaic,
                    activeTab: activeTab,
                    transcription: transcription,
                    context: captureContext,
                    recorder: streamRecorder,
                    composer: composer
                )
                .environment(\.theme, .graphite)
                .transition(.move(edge: .bottom))
            }
        }
        .environment(\.captureContext, captureContext)
        .environment(\.openSearch, { activeTab = .search })
    }
}

#Preview {
    GrAppShell()
        .environment(\.theme, .graphite)
}
