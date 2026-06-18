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

    @AppStorage("onboardingComplete") private var onboardingComplete: Bool = false
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        if onboardingComplete {
            shell
                // Force the Graphite theme regardless of the user's saved
                // appearance — this is the Graphite shell. (Cutover folds it
                // back into the appearance controller.)
                .environment(\.theme, .graphite)
                .preferredColorScheme(.dark)
                .task {
                // ONE-TIME wiring: bind the ticker to the mosaic and install
                // the event closures. The BACKEND-dependent bring-up
                // (attach/refresh/hubMode/WS-connect/bootstrap/start) lives in
                // `activateBackend()`, called once here and re-run by
                // `.onChange(of: backendToken)` whenever the user changes the
                // server URL / mode in Settings — so live sync RE-ESTABLISHES
                // on a runtime backend switch (notably: setting the server URL
                // after a fresh install, where the app launched in mock mode).
                relayTicker.connect(mosaic: mosaic)
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
                            if await liveSync?.sendDelta(frame) == true {
                                await relayTicker?.commitPushedDelta(slug: slug)
                            }
                        }
                    }
                }
                // Collab editing C1 outbound: a single in-block character
                // splice (the user's actual keystroke). Mirrors
                // onLocalWrite but records via `spliceBlockText` (text_seq
                // sequence CRDT) instead of a whole-text re-author, so a
                // peer's concurrent same-block edit merges instead of
                // being clobbered. Same record → produce → send → commit
                // tail so the splice reaches peers sub-second over /ws.
                mosaic.onLocalSplice = { [weak relayTicker, weak liveSync] slug, blockIdHex, offset, deleteLen, insert in
                    Task { @MainActor [weak relayTicker, weak liveSync] in
                        await relayTicker?.spliceAndPush(
                            slug: slug,
                            blockIdHex: blockIdHex,
                            utf16Offset: offset,
                            utf16DeleteLen: deleteLen,
                            insert: insert
                        )
                        if let frame = await relayTicker?.produceDeltaFrame(slug: slug) {
                            if await liveSync?.sendDelta(frame) == true {
                                await relayTicker?.commitPushedDelta(slug: slug)
                            }
                        }
                    }
                }
                // P1.11: relay-mode property writes (Inbox triage swipes,
                // Agenda mark-done / reschedule). Mirrors onLocalSplice but
                // records via the FFI setBlockProperty (typed container op,
                // merges independently of the block's prose) and is AWAITED
                // by the service so its post-write local re-read sees the
                // materialized file. Same produce → send → commit tail for
                // sub-second peer delivery; returns whether the engine
                // recorded the write so a not-found bid surfaces as a throw
                // instead of a silently vanished row.
                mosaic.onLocalPropertySet = { [weak relayTicker, weak liveSync] slug, bidHex, key, value in
                    guard let relayTicker else { return false }
                    let applied = await relayTicker.setBlockPropertyAndPush(
                        slug: slug, bidHex: bidHex, key: key, value: value
                    )
                    if applied, let frame = await relayTicker.produceDeltaFrame(slug: slug) {
                        if await liveSync?.sendDelta(frame) == true {
                            await relayTicker.commitPushedDelta(slug: slug)
                        }
                    }
                    return applied
                }
                // Awaitable whole-note write (relay-mode saveInboxDsl):
                // identical record → produce → send → commit tail to
                // onLocalWrite, but the caller can read-after-write (the
                // inbox reloads its DSL immediately after saving).
                mosaic.onLocalNoteWrite = { [weak relayTicker, weak liveSync] slug, title, content, createdAt in
                    await relayTicker?.recordAndPush(
                        slug: slug,
                        title: title,
                        content: content,
                        createdAtMillis: createdAt
                    )
                    if let frame = await relayTicker?.produceDeltaFrame(slug: slug) {
                        if await liveSync?.sendDelta(frame) == true {
                            await relayTicker?.commitPushedDelta(slug: slug)
                        }
                    }
                }
                // Saved views (spec 2026-06-10): the Inbox tab's view
                // switcher reads/writes the engine's synced registry doc
                // in `.relay` mode through these seams. List seeds the
                // builtin Inbox idempotently; the writes record + drain to
                // the relay so other devices converge. (`.http` mode never
                // fires these — the service hits the server's /views
                // routes directly.)
                mosaic.onViewsList = { [weak relayTicker] in
                    await relayTicker?.viewsList()?.map(SavedView.init(ffi:))
                }
                // Complete page list (Loro index) for `[[` link autocomplete,
                // so pages never opened on this device are still found.
                mosaic.onIndexEntries = { [weak relayTicker] in
                    await relayTicker?.indexEntries()
                }
                mosaic.onViewsUpsert = { [weak relayTicker] view in
                    guard let relayTicker else {
                        throw URLError(.cannotWriteToFile)
                    }
                    try await relayTicker.viewsUpsertAndPush(view.ffiRecord)
                }
                mosaic.onViewsDelete = { [weak relayTicker] viewId in
                    guard let relayTicker else {
                        throw URLError(.cannotWriteToFile)
                    }
                    try await relayTicker.viewsDeleteAndPush(viewId: viewId)
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
                // Collab editing C1-inbound: let the service read a block's
                // current engine-exact text (the MERGED result after a remote
                // splice) so it can live-apply the change into the open editor
                // with caret remap, instead of waiting for the blur refresh.
                mosaic.readEngineBlockText = { [weak relayTicker] slug, bid in
                    await relayTicker?.readBlockText(slug: slug, blockIdHex: bid)
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
                // Live WS push (mirrors AppShell.activateMosaic): instant
                // re-pull on Mac-originated note changes, routed through
                // applyRemoteChange() so it defers while editing.
                liveSync.onNoteChange = { [mosaic] in
                    Task { await mosaic.applyRemoteChange() }
                }
                // `views_changed` (saved-views spec): a desktop edit to
                // the views registry re-reads the switcher live in `.http`
                // mode. Lighter than applyRemoteChange — no note refetch,
                // just the views tick the Inbox tab observes.
                liveSync.onViewsChange = { [weak mosaic] in
                    mosaic?.noteViewsChanged()
                }
                // Binary frames = inbound Loro deltas. Apply through the
                // RelayTicker (sole engine owner) for sub-second remote
                // edits (Phase C).
                liveSync.onBinaryDelta = { [weak relayTicker] frame in
                    Task { await relayTicker?.applyInboundDelta(frame) }
                }
                // Backend-dependent bring-up — also re-run on a backend change.
                await activateBackend()
            }
            .onChange(of: backendToken) { _, _ in
                // The user changed the server URL / mock↔HTTP mode in Settings
                // — re-establish the live WS + hub mode against the NEW backend
                // (mirrors AppShell re-running activateMosaic on a profile
                // switch). Without this, a runtime backend change — notably
                // setting the server URL after a fresh install — left the WS
                // disconnected and the relay coordinator spinning ("Mac has no
                // relay configured"), so no device saw live edits.
                Task { await activateBackend() }
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
        } else {
            OnboardingView(
                onboardingComplete: $onboardingComplete,
                backend: backend,
                mosaic: mosaic,
                registry: mosaicRegistry
            )
            .environment(\.theme, .graphite)
            .preferredColorScheme(.dark)
        }
    }

    /// Identity of the current backend (mode + server URL). Drives the
    /// `.onChange` that re-establishes live sync when the user reconfigures
    /// the server in Settings — a plain `String` so SwiftUI can diff it.
    private var backendToken: String {
        "\(backend.mode.rawValue)|\(backend.serverURL)"
    }

    /// Point the mosaic + live-sync transport at the CURRENT backend. Called
    /// once from `.task` and again whenever `backendToken` changes (the user
    /// edits the server URL / toggles mock↔HTTP in Settings). Idempotent and
    /// re-entrant: `liveSync.connect` no-ops on the same URL and tears down +
    /// repoints on a new one; `hubMode` is set BOTH ways; `openEngineIfNeeded`
    /// / `bootstrapNoteIfNeeded` / `start` are all safe to re-run.
    private func activateBackend() async {
        mosaic.attach(backend: backend.backend)
        await mosaic.refresh(from: backend.backend)
        // Hub mode (Part E2): an HTTP Mac backend makes the live `/ws` socket
        // the sync hub, so gate the relay coordinator loop OFF; mock mode
        // re-enables it. Set BOTH ways so a runtime switch is correct (the
        // fresh-install bug: hubMode stayed false because this only ran once
        // at launch in mock mode).
        if case .http = backend.backend {
            relayTicker.hubMode = true
        } else {
            relayTicker.hubMode = false
        }
        do { try await relayTicker.openEngineIfNeeded() }
        catch { /* surfaced via relayTicker.lastError */ }
        // Point the live-sync socket at the active server (or tear it down in
        // mock mode). Re-entrant — same URL → no-op, new URL → reconnect.
        if case .http = backend.backend {
            liveSync.connect(serverURL: backend.serverURL)
        } else {
            liveSync.connect(serverURL: nil)
        }
        // Bootstrap the currently-visible daily as a shared base (T2), now
        // that the engine + socket point at this backend.
        await relayTicker.bootstrapNoteIfNeeded(slug: mosaic.todayDailySlug)
        // Start the relay tick loop here (idempotent) — `activateBackend` runs on
        // launch (via `.task`) AND on every backend change, with `hubMode` set
        // just above. `.onChange(of: scenePhase) → .active` ALSO calls start(),
        // but `.onChange` does NOT fire for the initial value, so a fresh launch
        // straight into `.active` never started the loop — fatal for .relay mode
        // (the tick is the only sync path; hub mode just no-ops the loop).
        relayTicker.start()
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
