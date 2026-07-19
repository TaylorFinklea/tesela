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
struct GrAppShell: View {
    @StateObject private var mosaic = MockMosaicService()
    @StateObject private var backend = BackendSettings()
    @StateObject private var relayTicker = RelayTicker.shared
    /// Live WS push channel (note_created/updated/deleted) — mirrors
    /// AppShell. Gives instant Mac→app updates instead of waiting for the
    /// RelayTicker poll, and routes through `applyRemoteChange()` so the
    /// refresh respects the edit-suppression guards.
    @StateObject private var liveSync = LiveSyncSocket()
    @State private var hubActivation = HubActivationSequencer()
    @State private var activationRetryTask: Task<Void, Never>?
    /// Option-B relay-mode presence transport (live remote carets). In hub
    /// mode (`.http`) presence rides `liveSync`'s `/ws` fan-out; in relay mode
    /// (a cached pairing code with a relay URL) it goes over this dedicated CF
    /// presence socket instead. Wired in `activateBackend()` and driven by the
    /// scenePhase suspend/nudge below.
    @StateObject private var presenceRelay = PresenceRelaySocket()
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
    @StateObject private var releaseNotes = ReleaseNotesPresenter()

    @State private var activeTab: AppTab = .daily
    @State private var captureContext: CaptureContext = .init()
    @State private var showSettings: Bool = false
    @State private var showCommandPalette: Bool = false
    /// The command palette's manifest (tesela-cib / ADR-4). Seeded from the
    /// bundled checked-in snapshot so `.relay`/`.mock` (no reachable server)
    /// always has a working palette; `activateBackend()` refreshes it from
    /// `GET /commands` when an `.http` backend is reachable.
    @State private var commandManifest: [CommandManifestEntry] = CommandManifestSource.loadBundled()
    /// Periodic (~3s) prune of stale remote presence carets, armed at SHELL
    /// scope (NOT per-view) so it runs regardless of the active tab. The
    /// shell-level presence sockets call `applyPresence` whatever view is up
    /// (and each remote launch mints a fresh peer id), so a view-scoped prune
    /// let `RemoteCursorStore.byPeer` leak while off the Daily tab. Mirrors the
    /// web's 3s prune.
    @State private var presencePruneTimer: Timer? = nil

    @AppStorage("onboardingComplete") private var onboardingComplete: Bool = false
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        Group {
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
                mosaicRegistry.willChangeActiveProfile = { [weak mosaic, weak hubActivation] in
                    mosaic?.closeBackendMutationAdmissionForActivation()
                    hubActivation?.invalidateCurrentRequest()
                }
                relayTicker.connect(mosaic: mosaic)
                relayTicker.configureLiveDeltaSender { [weak liveSync] frame, hubIdentity in
                    await liveSync?.sendDelta(
                        frame,
                        requiredHubIdentity: hubIdentity
                    ) == true
                }
                liveSync.onConnectionAttempt = { [weak relayTicker] in
                    Task { _ = await relayTicker?.retryPendingRelocation() }
                }
                liveSync.onBindingInvalidated = {
                    suspendCurrentHub()
                    requestBackendActivation()
                }
                mosaic.onLocalWrite = { [weak relayTicker] slug, title, content, createdAt in
                    let session = relayTicker?.engineSessionToken
                    relayTicker?.enqueueRecordAndPush(
                        slug: slug,
                        title: title,
                        content: content,
                        createdAtMillis: createdAt,
                        requiredSession: session
                    )
                }
                // Collab editing C1 outbound: a single in-block character
                // splice (the user's actual keystroke). Mirrors
                // onLocalWrite but records via `spliceBlockText` (text_seq
                // sequence CRDT) instead of a whole-text re-author, so a
                // peer's concurrent same-block edit merges instead of
                // being clobbered. Same record → produce → send → commit
                // tail so the splice reaches peers sub-second over /ws.
                mosaic.onLocalSplice = { [weak relayTicker] slug, blockIdHex, offset, deleteLen, insert in
                    let session = relayTicker?.engineSessionToken
                    relayTicker?.enqueueSpliceAndPush(
                        slug: slug,
                        blockIdHex: blockIdHex,
                        utf16Offset: offset,
                        utf16DeleteLen: deleteLen,
                        insert: insert,
                        requiredSession: session
                    )
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
                mosaic.onLocalPropertySet = { [weak relayTicker] slug, bidHex, key, value, valueType in
                    guard let relayTicker else { return false }
                    let session = relayTicker.engineSessionToken
                    return await relayTicker.setBlockPropertyAndPush(
                        slug: slug,
                        bidHex: bidHex,
                        key: key,
                        value: value,
                        valueType: valueType,
                        requiredSession: session
                    )
                }
                mosaic.onLocalPropertyListUpdate = {
                    [weak relayTicker] slug, bidHex, key, current, add, remove in
                    guard let relayTicker else { return false }
                    let session = relayTicker.engineSessionToken
                    return await relayTicker.updateBlockPropertyListAndPush(
                        slug: slug,
                        bidHex: bidHex,
                        key: key,
                        current: current,
                        add: add,
                        remove: remove,
                        requiredSession: session
                    )
                }
                // Awaitable whole-note write (relay-mode saveInboxDsl):
                // identical record → produce → send → commit tail to
                // onLocalWrite, but the caller can read-after-write (the
                // inbox reloads its DSL immediately after saving).
                mosaic.onLocalNoteWrite = { [weak relayTicker] slug, title, content, createdAt in
                    let session = relayTicker?.engineSessionToken
                    await relayTicker?.recordAndPush(
                        slug: slug,
                        title: title,
                        content: content,
                        createdAtMillis: createdAt,
                        requiredSession: session
                    )
                }
                mosaic.onLocalBlockMove = { [weak relayTicker] request in
                    guard let relayTicker else {
                        throw FfiSyncError.Other(message: "sync engine unavailable")
                    }
                    return try await relayTicker.moveSubtreeAndDeliver(request)
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
                    let session = relayTicker.engineSessionToken
                    try await relayTicker.viewsUpsertAndPush(
                        view.ffiRecord,
                        requiredSession: session
                    )
                }
                mosaic.onViewsDelete = { [weak relayTicker] viewId in
                    guard let relayTicker else {
                        throw URLError(.cannotWriteToFile)
                    }
                    let session = relayTicker.engineSessionToken
                    try await relayTicker.viewsDeleteAndPush(
                        viewId: viewId,
                        requiredSession: session
                    )
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
                liveSync.onBinaryDelta = { [weak relayTicker] frame, hubIdentity in
                    Task {
                        await relayTicker?.applyInboundDelta(
                            frame,
                            requiredHubIdentity: hubIdentity
                        )
                    }
                }
                // Backend-dependent bring-up — also re-run on a backend change.
                requestBackendActivation()
                await hubActivation.waitUntilIdle()
                await WidgetSnapshotPublisher.publish(from: mosaic)
            }
            .onAppear { startPresencePrune() }
            .onDisappear { stopPresencePrune() }
            .onChange(of: backendToken) { _, _ in
                streamRecorder.invalidateForProfileSwitch()
                // The user changed the server URL / mock↔HTTP mode in Settings
                // — re-establish the live WS + hub mode against the NEW backend
                // (mirrors AppShell re-running activateMosaic on a profile
                // switch). Without this, a runtime backend change — notably
                // setting the server URL after a fresh install — left the WS
                // disconnected and the relay coordinator spinning ("Mac has no
                // relay configured"), so no device saw live edits.
                requestBackendActivation()
            }
            .onChange(of: scenePhase) { _, newPhase in
                switch newPhase {
                case .active:
                    Task {
                        await relayTicker.resumeFromBackground()
                        // Restore hub routing before reconnecting the
                        // foreground socket; a background relay tick may
                        // still hold the shared engine briefly.
                        relayTicker.wake()
                        liveSync.nudge()
                        presenceRelay.nudge()
                        if await mosaic.refreshAttachedBackend() {
                            await mosaic.refreshLoadedPages()
                        }
                    }
                case .background:
                    liveSync.suspend()
                    presenceRelay.suspend()
                    // Drain any queued outbound ops to the relay before iOS
                    // suspends us (sync-durability Phase 1) instead of a bare
                    // stop() that strands a just-made capture until relaunch.
                    relayTicker.flushOnBackground()
                default:
                    break
                }
            }
            .onChange(of: streamRecorder.lastTranscript) { _, transcript in
                // A finished voice transcript — append it to the composer
                // here, at the stable app root, mirroring AppShell.
                guard let transcript else { return }
                defer { streamRecorder.clearLastTranscript() }
                guard let text = transcript.text(ifCurrent: voiceCaptureScope) else { return }
                composer.append(text)
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
        .onOpenURL(perform: handleDeepLink)
        .task(id: widgetSnapshotRevision) {
            guard onboardingComplete else { return }
            await WidgetSnapshotPublisher.publish(from: mosaic)
        }
        .releaseNotesPresentation(
            presenter: releaseNotes,
            onboardingComplete: onboardingComplete
        )
        .environment(\.theme, .graphite)
        .preferredColorScheme(.dark)
    }

    /// Identity of the current backend (mode + server URL + relay group). Drives
    /// the `.onChange` that re-establishes live sync when the user reconfigures
    /// the server in Settings — a plain `String` so SwiftUI can diff it.
    ///
    /// Includes the cached relay pairing code so a relay→relay RE-PAIR — the
    /// user scans a NEW group QR while already in `.relay` mode, so mode +
    /// serverURL are unchanged but `PairScanView.adopt` caches a new code —
    /// still flips the token and re-runs `activateBackend()` → `wirePresence()`,
    /// repointing `presenceRelay` at the new group. (The adopt() sets
    /// `backend.mode = .relay`, which re-renders this body so the token is
    /// recomputed against the freshly-cached code.) Mirrors the data path's
    /// `invalidateCoordinatorIfRepaired`, which migrates the coordinator on its
    /// next tick; without this the presence socket stayed on the old group.
    private var backendToken: String {
        "\(backend.mode.rawValue)|\(relocationHubIdentity)|\(RelayTicker.cachedPairingCode() ?? "")"
    }

    private var widgetSnapshotRevision: String {
        let connection: String
        switch mosaic.connection {
        case .idle: connection = "idle"
        case .connecting: connection = "connecting"
        case .switching: connection = "switching"
        case .ready: connection = "ready"
        case .failed: connection = "failed"
        }
        return "\(onboardingComplete)|\(connection)|\(mosaic.refreshTick)|\(mosaic.viewsTick)"
    }

    private func handleDeepLink(_ url: URL) {
        guard let destination = TeselaDeepLink.destination(for: url) else { return }
        activeTab = destination.tab
    }

    private var voiceCaptureScope: VoiceCaptureScope {
        VoiceCaptureScope(
            profileIdentity: backendToken,
            backendGeneration: mosaic.backendGenerationLease
        )
    }

    private var relocationHubIdentity: String {
        let destination = Self.resolvedHubDestination(
            activeProfile: mosaicRegistry.activeProfile,
            fallbackServerURL: backend.serverURL
        )
        return RelayTicker.hubIdentity(
            serverURL: destination.serverURL,
            profileID: mosaicRegistry.activeProfile?.id,
            mosaicPath: destination.mosaicPath
        )
    }

    static func resolvedHubDestination(
        activeProfile: MosaicProfile?,
        fallbackServerURL: String
    ) -> (serverURL: String, mosaicPath: String?) {
        (
            serverURL: activeProfile?.serverURL ?? fallbackServerURL,
            mosaicPath: activeProfile?.mosaicPath
        )
    }

    private func requestBackendActivation() {
        mosaic.closeBackendMutationAdmissionForActivation()
        activationRetryTask?.cancel()
        activationRetryTask = nil
        hubActivation.request { lease in
            await activateBackend(lease: lease)
        }
    }

    private func suspendCurrentHub() {
        relayTicker.configureLiveHub(identity: nil)
        liveSync.disconnect()
        presenceRelay.disconnect()
        mosaic.sendPresence = nil
        relayTicker.hubMode = true
        mosaic.detachForActivation()
    }

    /// Point the mosaic + live-sync transport at the current backend. Server
    /// switching is serialized, and only the newest request may publish a
    /// verified socket binding.
    private func activateBackend(lease: HubActivationSequencer.Lease) async {
        guard await waitForActivationAdmission(lease: lease) else { return }
        suspendCurrentHub()

        let activeProfile = mosaicRegistry.activeProfile
        let destination = Self.resolvedHubDestination(
            activeProfile: activeProfile,
            fallbackServerURL: backend.serverURL
        )
        let mode = backend.mode
        let resolvedBackend = BackendSettings.resolveBackend(
            mode: mode,
            serverURL: destination.serverURL
        )
        if backend.serverURL != destination.serverURL {
            backend.serverURL = destination.serverURL
        }

        var confirmedPath: String?
        var confirmedGroupIdHex: String?
        var confirmedHubIdentity: String?
        var engineScope = MosaicEngineScope.legacy
        if case .http = resolvedBackend {
            do {
                if let mosaicPath = destination.mosaicPath {
                    confirmedPath = try await mosaic.ensureServerMosaic(
                        path: mosaicPath,
                        serverURL: destination.serverURL
                    )
                } else {
                    confirmedPath = try await MosaicServerClient.currentPath(
                        serverURL: destination.serverURL
                    )
                }
                let observed = try await MosaicServerClient.currentIdentity(
                    serverURL: destination.serverURL
                )
                guard observed.path == confirmedPath else {
                    throw MosaicServerClient.ClientError.mosaicSwitchNotConfirmed(
                        expected: confirmedPath ?? "",
                        observed: observed.path
                    )
                }
                confirmedGroupIdHex = observed.groupIdHex
                engineScope = MosaicEngineScope(groupIdHex: observed.groupIdHex)
            } catch {
                guard hubActivation.isCurrent(lease) else { return }
                failActivation(error.localizedDescription, retry: true)
                return
            }
            guard hubActivation.isCurrent(lease) else { return }
            guard let confirmedPath, let confirmedGroupIdHex else { return }
            let legacyIdentity = RelayTicker.hubIdentity(
                serverURL: destination.serverURL,
                profileID: activeProfile?.id,
                mosaicPath: confirmedPath
            )
            let identity = RelayTicker.hubIdentity(
                serverURL: destination.serverURL,
                profileID: activeProfile?.id,
                mosaicPath: confirmedPath,
                groupIdHex: confirmedGroupIdHex
            )
            if let pendingIdentity = relayTicker.unpreparedRelocationHubIdentity,
               pendingIdentity != identity,
               !(relayTicker.unpreparedRelocationEngineScope == nil
                   && pendingIdentity == legacyIdentity) {
                failActivation(
                    "A saved block move belongs to another mosaic. Switch back to finish it.",
                    retry: false
                )
                return
            }
            if let pendingScope = relayTicker.unpreparedRelocationEngineScope,
               pendingScope != engineScope {
                failActivation(
                    "A saved block move belongs to another local mosaic. Switch back to finish it.",
                    retry: false
                )
                return
            }
            confirmedHubIdentity = identity
        } else {
            if mode == .relay {
                guard let code = RelayTicker.cachedPairingCode(),
                      let pairing = try? decodePairingCode(code: code)
                else {
                    failActivation("Relay pairing is unavailable. Pair this device again.", retry: false)
                    return
                }
                engineScope = MosaicEngineScope(groupIdHex: pairing.groupIdHex)
            }
            if relayTicker.unpreparedRelocationHubIdentity != nil {
                failActivation(
                    "A saved block move is waiting for its desktop mosaic. Switch back to finish it.",
                    retry: false
                )
                return
            }
        }

        do {
            try await relayTicker.activateEngine(scope: engineScope)
        } catch {
            guard hubActivation.isCurrent(lease) else { return }
            failActivation(error.localizedDescription, retry: true)
            return
        }
        guard hubActivation.isCurrent(lease) else { return }

        // Refresh the command-palette manifest from the live `GET /commands`
        // route when a Mac is reachable, so the palette picks up anything
        // added since this build's bundled snapshot. Best-effort — any
        // failure (unreachable, malformed) keeps the bundled fallback.
        var freshManifest: [CommandManifestEntry]?
        if case .http(let baseURL) = resolvedBackend {
            let fresh = try? await CommandManifestSource.fetchRemote(baseURL: baseURL)
            guard hubActivation.isCurrent(lease) else { return }
            if let fresh, !fresh.isEmpty {
                freshManifest = fresh
            }
        }
        if let freshManifest {
            commandManifest = freshManifest
        }
        if case .http = resolvedBackend,
           let confirmedPath,
           let confirmedGroupIdHex,
           let hubIdentity = confirmedHubIdentity {
            let verified = await liveSync.connectAndVerify(
                serverURL: destination.serverURL,
                expectedMosaicPath: confirmedPath,
                expectedGroupIdHex: confirmedGroupIdHex,
                hubIdentity: hubIdentity
            )
            guard hubActivation.isCurrent(lease) else {
                liveSync.disconnect()
                return
            }
            guard verified,
                  liveSync.isVerifiedBinding(
                      serverURL: destination.serverURL,
                      expectedMosaicPath: confirmedPath,
                      expectedGroupIdHex: confirmedGroupIdHex,
                      hubIdentity: hubIdentity
                  )
            else {
                liveSync.disconnect()
                failActivation(
                    "The selected mosaic's live identity could not be verified.",
                    retry: true
                )
                return
            }
        }
        guard hubActivation.isCurrent(lease) else { return }
        relayTicker.hubMode = {
            if case .relay = resolvedBackend { return false }
            return true
        }()
        if case .http = resolvedBackend,
           let hubIdentity = confirmedHubIdentity {
            relayTicker.configureLiveHub(identity: hubIdentity)
        }
        mosaic.attach(
            backend: resolvedBackend,
            engineScope: engineScope,
            openMutationAdmission: false
        )
        guard await mosaic.refreshAttachedBackend() else {
            guard hubActivation.isCurrent(lease) else { return }
            suspendCurrentHub()
            failActivation("The selected mosaic could not be loaded.", retry: true)
            return
        }
        guard hubActivation.isCurrent(lease) else { return }
        // Presence transport: hub mode (.http) carries carets over the live
        // /ws fan-out; relay mode uses the dedicated CF presence socket. Re-run
        // on every backend change so a runtime mock↔relay switch repoints it.
        wirePresence(backend: resolvedBackend)

        if let hubIdentity = confirmedHubIdentity {
            await relayTicker.bootstrapNoteIfNeeded(
                slug: mosaic.todayDailySlug,
                requiredHubIdentity: hubIdentity
            )
        } else {
            await relayTicker.bootstrapNoteIfNeeded(slug: mosaic.todayDailySlug)
        }
        guard hubActivation.isCurrent(lease) else { return }
        if case .http = resolvedBackend {
            guard let confirmedPath,
                  let confirmedGroupIdHex,
                  let hubIdentity = confirmedHubIdentity,
                  liveSync.publishVerifiedBinding(
                      serverURL: destination.serverURL,
                      expectedMosaicPath: confirmedPath,
                      expectedGroupIdHex: confirmedGroupIdHex,
                      hubIdentity: hubIdentity
                  )
            else {
                liveSync.disconnect()
                failActivation(
                    "The selected mosaic's live identity was lost during activation.",
                    retry: true
                )
                return
            }
        }
        activationRetryTask?.cancel()
        activationRetryTask = nil

        // Start the relay tick loop here (idempotent) — `activateBackend` runs on
        // launch (via `.task`) AND on every backend change, with `hubMode` set
        // just above. `.onChange(of: scenePhase) → .active` ALSO calls start(),
        // but `.onChange` does NOT fire for the initial value, so a fresh launch
        // straight into `.active` never started the loop — fatal for .relay mode
        // (the tick is the only sync path; hub mode just no-ops the loop).
        relayTicker.start()
        mosaic.commitBackendMutationAdmission()
    }

    private func waitForActivationAdmission(
        lease: HubActivationSequencer.Lease
    ) async -> Bool {
        while hubActivation.isCurrent(lease) {
            await mosaic.waitUntilBackendOperationsFinish()
            await relayTicker.waitUntilHubActivationIsSafe()
            guard hubActivation.isCurrent(lease) else { return false }
            if mosaic.backendActivationIsSafe, relayTicker.hubActivationIsSafe {
                return true
            }
        }
        return false
    }

    private func failActivation(_ message: String, retry: Bool) {
        mosaic.reportActivationFailure(message)
        activationRetryTask?.cancel()
        activationRetryTask = nil
        guard retry else { return }
        activationRetryTask = Task { @MainActor in
            do {
                try await Task.sleep(nanoseconds: 2_000_000_000)
            } catch {
                return
            }
            requestBackendActivation()
        }
    }

    /// Start the ~3s presence-prune tick at shell scope. Idempotent — re-arms
    /// on re-appear. Runs regardless of the active tab so off-Daily presence
    /// (the shell sockets keep feeding `applyPresence`) can't leak.
    private func startPresencePrune() {
        presencePruneTimer?.invalidate()
        presencePruneTimer = Timer.scheduledTimer(withTimeInterval: 3, repeats: true) { _ in
            Task { @MainActor in mosaic.pruneRemoteCursors() }
        }
    }

    private func stopPresencePrune() {
        presencePruneTimer?.invalidate()
        presencePruneTimer = nil
    }

    /// Wire the presence egress (`mosaic.sendPresence`) + ingress
    /// (`onPresence → mosaic.applyPresence`) to the transport that matches the
    /// current backend. Hub mode reuses `LiveSyncSocket`'s `/ws` fan-out
    /// (forwards the raw PRES frame opaquely); relay mode uses
    /// `PresenceRelaySocket` (seals/opens via the pure FFI). `publishPresence`
    /// stays transport-agnostic — sealing happens inside the relay socket.
    /// Gated strictly so the two paths never double-publish.
    private func wirePresence(backend resolvedBackend: MockMosaicService.Backend) {
        if case .http = resolvedBackend {
            // Hub mode: the live /ws socket already fans presence out.
            presenceRelay.disconnect()
            liveSync.onPresence = { [mosaic] frame in
                Task { @MainActor in mosaic.applyPresence(frame) }
            }
            mosaic.sendPresence = { [weak liveSync] data in
                Task { @MainActor in liveSync?.sendPresence(data) }
            }
            return
        }
        // Relay mode: a cached pairing code carrying a relay URL → the CF
        // presence socket. The device id for the MAC/echo-exclusion is the
        // stable per-install id (NOT the per-launch presence peer id).
        if let code = RelayTicker.cachedPairingCode(),
           let pairing = try? decodePairingCode(code: code),
           let relayUrl = pairing.relayUrl, !relayUrl.isEmpty {
            presenceRelay.onPresence = { [mosaic] frame in
                Task { @MainActor in mosaic.applyPresence(frame) }
            }
            mosaic.sendPresence = { [weak presenceRelay] data in
                Task { @MainActor in presenceRelay?.send(data) }
            }
            presenceRelay.connect(
                relayUrl: relayUrl,
                groupIdHex: pairing.groupIdHex,
                groupKeyHex: pairing.groupKeyHex,
                deviceIdHex: RelayTicker.persistedDeviceIdHex()
            )
        } else {
            // No relay pairing (pure mock / LAN-only) → no presence transport.
            presenceRelay.disconnect()
            mosaic.sendPresence = nil
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
            Tab(AppTab.views.label, systemImage: AppTab.views.systemImage, value: AppTab.views) {
                GrInboxView(mosaic: mosaic, backend: backend)
            }
            Tab(AppTab.library.label, systemImage: AppTab.library.systemImage, value: AppTab.library) {
                GrLibraryView(mosaic: mosaic, backend: backend)
            }
            Tab(value: AppTab.search, role: .search) {
                GrSearchView(mosaic: mosaic, backend: backend)
            }
        }
        .allowsHitTesting(shellAllowsInteraction)
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
                    composer: composer,
                    profileIdentity: backendToken
                )
                .environment(\.theme, .graphite)
                .transition(.move(edge: .bottom))
            }
        }
        .environment(\.captureContext, captureContext)
        .environment(\.openSearch, { activeTab = .search })
        .environment(\.openSettings, { showSettings = true })
        .environment(\.openCommandPalette, { showCommandPalette = true })
        .sheet(isPresented: $showSettings) {
            GrSettingsView(
                mosaic: mosaic,
                backend: backend,
                relayTicker: relayTicker,
                registry: mosaicRegistry,
                liveSync: liveSync,
                transcription: transcription
            )
            .environment(\.theme, .graphite)
            .preferredColorScheme(.dark)
        }
        .sheet(isPresented: $showCommandPalette) {
            GrCommandPalette(commands: GrCommand.palette(from: commandManifest), onRun: runCommand)
                .environment(\.theme, .graphite)
                .preferredColorScheme(.dark)
        }
    }

    /// Freeze the data surface during a profile/backend transition: the
    /// selection token may already identify B while the visible snapshot still
    /// belongs to A. On failure, navigation becomes interactive again so the
    /// user can reopen Settings; service mutation admission remains closed.
    private var shellAllowsInteraction: Bool {
        if mosaic.backendMutationAdmissionIsOpen { return true }
        if case .failed = mosaic.connection { return true }
        return false
    }

    /// Execute a command-palette command, dispatching on the manifest's
    /// stable id (the native executor map — mirrors `GrCommand.executableIds`,
    /// which gates what's OFFERED; this switch is what RUNS it). Navigation
    /// is immediate; opening another sheet (Settings) is deferred so the
    /// palette finishes dismissing first (one sheet at a time on the shell).
    private func runCommand(_ cmd: GrCommand) {
        switch cmd.id {
        case "daily":  activeTab = .daily
        case "agenda": activeTab = .agenda
        case "views", "inbox": activeTab = .views
        case "settings-general", "settings-devices", "settings-sync",
             "settings-mosaic", "settings-data":
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(350))
                showSettings = true
            }
        case "whats-new":
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(350))
                releaseNotes.presentCurrent()
            }
        default: break
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

/// Opens the command palette (the `:`/leader stand-in) from anywhere —
/// the editor's keyboard-toolbar Commands button calls it.
private struct OpenCommandPaletteKey: EnvironmentKey {
    static let defaultValue: () -> Void = {}
}

extension EnvironmentValues {
    var openCommandPalette: () -> Void {
        get { self[OpenCommandPaletteKey.self] }
        set { self[OpenCommandPaletteKey.self] = newValue }
    }
}

#Preview {
    GrAppShell()
        .environment(\.theme, .graphite)
}
