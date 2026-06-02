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
    /// Multi-profile server registry — drives the Graphite Settings
    /// Mosaics list + add/switch. Mirrors how `AppShell` owns its own
    /// `MosaicRegistry`. Was DEFERRED here until the Graphite Settings
    /// page (task #156) gave it a home.
    @StateObject private var mosaicRegistry = MosaicRegistry()

    @State private var activeTab: AppTab = .daily
    @State private var captureContext: CaptureContext = .init()
    @State private var showSettings: Bool = false

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
                // Hub mode (Part E2): when the backend is an HTTP Mac
                // server, the live `/ws` socket below is the sync hub.
                // Gate the relay coordinator loop off so it can't inject
                // stale foreign-history ops into the same Loro docs the WS
                // path drives. Mirrors how `liveSync.connect` is gated on
                // `.http`. Reversible — the cached pairing code is kept.
                if case .http = backend.backend {
                    relayTicker.hubMode = true
                }
                do { try await relayTicker.openEngineIfNeeded() }
                catch { /* surfaced via relayTicker.lastError */ }
                mosaic.onLocalWrite = { [weak relayTicker, weak liveSync] slug, title, content, createdAt in
                    Task { @MainActor [weak relayTicker, weak liveSync] in
                        // 1) Record the edit into the engine + push to the
                        //    relay (the fallback delivery path).
                        await relayTicker?.recordAndPush(
                            slug: slug,
                            title: title,
                            content: content,
                            createdAtMillis: createdAt
                        )
                        // 2) Produce the cursor-free delta from the
                        //    now-recorded engine state and send it over the
                        //    live WS for sub-second delivery (Phase C). The
                        //    delta baseline only advances when the send is
                        //    confirmed (commitPushedDelta) — a dropped frame
                        //    keeps the VV back so the next delta re-includes it.
                        if let frame = await relayTicker?.produceDeltaFrame(slug: slug) {
                            if liveSync?.sendDelta(frame) == true {
                                await relayTicker?.commitPushedDelta(slug: slug)
                            }
                        }
                    }
                }
                relayTicker.onAppliedChanges = { [weak mosaic] in
                    // Route through applyRemoteChange() — NOT a direct
                    // refresh() — so the isEditingBlock + post-local-write
                    // suppression guards defer the re-pull instead of
                    // clobbering an in-progress edit. With Phase C's sub-second
                    // WS delivery an applied delta can land mid-keystroke; the
                    // direct refresh raced the editor. Mirrors onNoteChange.
                    Task { await mosaic?.applyRemoteChange() }
                }
                // Bootstrap the server's note doc as a base when a note
                // becomes visible (daily on refresh, any opened page) —
                // so a receive-only device holds the base for live deltas
                // and produces converging pushes, not only when it first
                // edits (delivery-layer redesign 2026-05-31, T2).
                // Idempotent (resident-check), so firing on every open is
                // safe-but-cheap. Mirrors onLocalWrite/onAppliedChanges.
                mosaic.onNoteOpened = { [weak relayTicker] slug in
                    Task { await relayTicker?.bootstrapNoteIfNeeded(slug: slug) }
                }
                // The initial `mosaic.refresh(...)` above ran before
                // `onNoteOpened` was wired, so explicitly bootstrap the
                // currently-visible daily once now (T2). Covers a pure
                // receive-only device that never edits or backgrounds.
                await relayTicker.bootstrapNoteIfNeeded(slug: mosaic.todayDailySlug)
                // Live WS push (mirrors AppShell.activateMosaic): instant
                // re-pull on Mac-originated note changes, routed through
                // applyRemoteChange() so it defers while editing.
                liveSync.onNoteChange = { [mosaic] in
                    Task { await mosaic.applyRemoteChange() }
                }
                // Binary frames = inbound Loro deltas. Apply through the
                // RelayTicker (sole engine owner) for sub-second remote
                // edits (Phase C).
                liveSync.onBinaryDelta = { [weak relayTicker] frame in
                    Task { await relayTicker?.applyInboundDelta(frame) }
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
        .environment(\.openSettings, { showSettings = true })
        .sheet(isPresented: $showSettings) {
            GrSettingsView(
                mosaic: mosaic,
                backend: backend,
                relayTicker: relayTicker,
                registry: mosaicRegistry,
                transcription: transcription
            )
            .environment(\.theme, .graphite)
            .preferredColorScheme(.dark)
        }
    }
}

// MARK: - Settings action environment value

/// Opens the Graphite Settings sheet from any content view's header
/// (mirrors `openSearch`). The Daily header's gear button calls it.
private struct OpenSettingsKey: EnvironmentKey {
    static let defaultValue: () -> Void = {}
}

extension EnvironmentValues {
    var openSettings: () -> Void {
        get { self[OpenSettingsKey.self] }
        set { self[OpenSettingsKey.self] = newValue }
    }
}

#Preview {
    GrAppShell()
        .environment(\.theme, .graphite)
}
