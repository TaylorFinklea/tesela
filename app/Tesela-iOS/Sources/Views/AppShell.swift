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
    @State private var hubActivation = HubActivationSequencer()
    @State private var hubActivationReady = false
    @State private var activationRetryTask: Task<Void, Never>?
    /// Option-B relay-mode presence transport. Hub mode (`.http`) carries
    /// carets over `liveSync`'s `/ws` fan-out; relay mode uses this dedicated
    /// CF presence socket. Wired in `activateMosaic` and driven by scenePhase.
    @StateObject private var presenceRelay = PresenceRelaySocket()
    /// B.3.3 — background relay poll/push loop. Runs whenever the app
    /// is foregrounded; pauses in background. Mac-originated edits
    /// arrive via this loop within ~5s instead of the prior "tap the
    /// dev pull button or wait minutes" behaviour.
    @StateObject private var relayTicker = RelayTicker.shared
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
    @StateObject private var releaseNotes = ReleaseNotesPresenter()

    @AppStorage("onboardingComplete") private var onboardingComplete: Bool = false
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        TeselaAppearance(controller: appearance) {
            Group {
                if onboardingComplete {
                    shell
                    .task {
                        mosaicRegistry.willChangeActiveProfile = { [weak mosaic, weak hubActivation] in
                            mosaic?.closeBackendMutationAdmissionForActivation()
                            hubActivation?.invalidateCurrentRequest()
                        }
                        mosaicRegistry.seedFromLegacyIfNeeded(
                            legacyURL: backend.serverURL,
                            defaultName: "My mosaic"
                        )
                        liveSync.onNoteChange = { [mosaic] in
                            Task { await mosaic.applyRemoteChange() }
                        }
                        liveSync.onBinaryDelta = { [weak relayTicker] frame, hubIdentity in
                            Task {
                                await relayTicker?.applyInboundDelta(
                                    frame,
                                    requiredHubIdentity: hubIdentity
                                )
                            }
                        }
                        // Bind + start the relay ticker once the app
                        // is up. connect() is idempotent so re-runs
                        // (e.g. on mosaic switch) don't churn.
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
                            requestMosaicActivation()
                        }
                        // Route iOS-authored writes through the engine
                        // + relay alongside the existing HTTP PUT. On
                        // LAN both succeed (HTTP first); on cellular
                        // when Mac is unreachable the engine path is
                        // the only one that gets there.
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
                        // P1.11: relay-mode property writes (Inbox
                        // triage swipes, Agenda mark-done / reschedule)
                        // — mirrors GrAppShell. AWAITED by the service
                        // so its post-write re-read sees the
                        // materialized file; returns whether the engine
                        // recorded the write so a not-found bid throws
                        // instead of silently vanishing the row.
                        mosaic.onLocalPropertySet = { [weak relayTicker] slug, bidHex, key, value in
                            guard let relayTicker else { return false }
                            let session = relayTicker.engineSessionToken
                            return await relayTicker.setBlockPropertyAndPush(
                                slug: slug,
                                bidHex: bidHex,
                                key: key,
                                value: value,
                                requiredSession: session
                            )
                        }
                        // Awaitable whole-note write (relay-mode
                        // saveInboxDsl): same record → produce → send →
                        // commit tail as onLocalWrite, but the caller
                        // can read-after-write (the inbox reloads its
                        // DSL immediately after saving).
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
                        // When the ticker applies new inbound ops,
                        // re-pull the user-visible pages over HTTP so
                        // the UI shows the change immediately. On
                        // cellular this HTTP call will likely fail
                        // (and silently swallow the URLError.cancelled
                        // we filtered out above) — the data already
                        // lives in the local engine + sandbox; B.3.4
                        // makes the iOS UI read from there directly.
                        relayTicker.onAppliedChanges = { [weak mosaic] in
                            // Route through applyRemoteChange() — NOT a direct
                            // refresh() — so the isEditingBlock + post-local-
                            // write suppression guards defer the re-pull instead
                            // of clobbering an in-progress edit. Phase C's sub-
                            // second WS delivery can land an applied delta mid-
                            // keystroke; the direct refresh raced the editor.
                            Task { await mosaic?.applyRemoteChange() }
                        }
                        // Bootstrap the server's note doc as a base when a
                        // note becomes visible (daily on refresh, any opened
                        // page) — so a receive-only device holds the base for
                        // live deltas and produces converging pushes, not only
                        // when it first edits (delivery-layer redesign
                        // 2026-05-31, T2). Idempotent (resident-check), so
                        // firing on every open is safe-but-cheap. Mirrors
                        // onLocalWrite/onAppliedChanges.
                        mosaic.onNoteOpened = { [weak relayTicker] slug in
                            Task { await relayTicker?.bootstrapNoteIfNeeded(slug: slug) }
                        }
                        hubActivationReady = true
                        requestMosaicActivation()
                        await hubActivation.waitUntilIdle()
                        relayTicker.start()
                    }
                    .onChange(of: activationToken) { _, _ in
                        streamRecorder.invalidateForProfileSwitch()
                        guard hubActivationReady else { return }
                        requestMosaicActivation()
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
                            Task {
                                await relayTicker.resumeFromBackground()
                                // Restore hub routing before reconnecting the
                                // foreground socket; a background relay tick
                                // may still hold the shared engine briefly.
                                liveSync.nudge()
                                presenceRelay.nudge()
                                relayTicker.wake()
                                if await mosaic.refreshAttachedBackend() {
                                    await mosaic.refreshLoadedPages()
                                }
                            }
                        case .background:
                            liveSync.suspend()
                            presenceRelay.suspend()
                            // Drain queued outbound ops before suspend
                            // (sync-durability Phase 1) — see GrAppShell.
                            relayTicker.flushOnBackground()
                        default:
                            break
                        }
                    }
                    .onChange(of: streamRecorder.lastTranscript) { _, transcript in
                        // A finished voice transcript — append it to the
                        // composer here, at the stable app root, rather
                        // than inside the churny capture-bar accessory.
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
                }
            }
            .releaseNotesPresentation(
                presenter: releaseNotes,
                onboardingComplete: onboardingComplete
            )
        }
    }

    private var activationToken: String {
        let profile = mosaicRegistry.activeProfile
        return [
            backend.mode.rawValue,
            profile?.id.uuidString ?? "legacy",
            profile?.serverURL ?? backend.serverURL,
            profile?.mosaicPath ?? ""
        ].joined(separator: "|")
    }

    private var voiceCaptureScope: VoiceCaptureScope {
        VoiceCaptureScope(
            profileIdentity: activationToken,
            backendGeneration: mosaic.backendGenerationLease
        )
    }

    private func requestMosaicActivation() {
        mosaic.closeBackendMutationAdmissionForActivation()
        activationRetryTask?.cancel()
        activationRetryTask = nil
        hubActivation.request { lease in
            await activateMosaic(lease: lease)
        }
    }

    private func suspendCurrentHub() {
        relayTicker.configureLiveHub(identity: nil)
        liveSync.disconnect()
        presenceRelay.disconnect()
        mosaic.sendPresence = nil
        // Fail closed while the next backend is unresolved: never let the
        // relay coordinator feed the shared engine during a hub transition.
        relayTicker.hubMode = true
        mosaic.detachForActivation()
    }

    /// Point the data service at the active mosaic. Await-heavy server
    /// switching is serialized, and only the newest request may publish a
    /// socket binding.
    private func activateMosaic(lease: HubActivationSequencer.Lease) async {
        guard await waitForActivationAdmission(lease: lease) else { return }
        suspendCurrentHub()

        let activeProfile = mosaicRegistry.activeProfile
        let profileID = activeProfile?.id
        let serverURL = activeProfile?.serverURL ?? backend.serverURL
        let requestedPath = activeProfile?.mosaicPath
        let mode = backend.mode
        let resolvedBackend = BackendSettings.resolveBackend(
            mode: mode,
            serverURL: serverURL
        )

        if backend.serverURL != serverURL {
            backend.serverURL = serverURL
        }

        var confirmedPath: String?
        var confirmedGroupIdHex: String?
        var confirmedHubIdentity: String?
        var engineScope = MosaicEngineScope.legacy
        if case .http = resolvedBackend {
            do {
                if let requestedPath {
                    confirmedPath = try await mosaic.ensureServerMosaic(
                        path: requestedPath,
                        serverURL: serverURL
                    )
                } else {
                    confirmedPath = try await MosaicServerClient.currentPath(
                        serverURL: serverURL
                    )
                }
                let observed = try await MosaicServerClient.currentIdentity(
                    serverURL: serverURL
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
                serverURL: serverURL,
                profileID: profileID,
                mosaicPath: confirmedPath
            )
            let identity = RelayTicker.hubIdentity(
                serverURL: serverURL,
                profileID: profileID,
                mosaicPath: confirmedPath,
                groupIdHex: confirmedGroupIdHex
            )
            // A cold launch may recover an unprepared outbox only by binding
            // the exact hub that owns it. Leave the UI detached for any other
            // selected profile until the user switches back.
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

        if case .http = resolvedBackend,
           let confirmedPath,
           let confirmedGroupIdHex,
           let hubIdentity = confirmedHubIdentity {
            let verified = await liveSync.connectAndVerify(
                serverURL: serverURL,
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
                      serverURL: serverURL,
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
        wirePresence(backend: resolvedBackend)

        // Refresh called onNoteOpened before the verified hub identity was
        // publishable, so perform the first catch-up explicitly now. The
        // identity check prevents this shared engine from importing bytes for
        // a profile that became stale during the await.
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
                      serverURL: serverURL,
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
            requestMosaicActivation()
        }
    }

    /// Point presence at the transport matching the current backend: hub mode
    /// (`.http`) reuses `LiveSyncSocket`'s `/ws` fan-out; relay mode uses
    /// `PresenceRelaySocket` (seal/open via the pure FFI). `publishPresence`
    /// stays transport-agnostic. Gated so the two paths never double-publish.
    private func wirePresence(backend resolvedBackend: MockMosaicService.Backend) {
        if case .http = resolvedBackend {
            presenceRelay.disconnect()
            liveSync.onPresence = { [mosaic] frame in
                Task { @MainActor in mosaic.applyPresence(frame) }
            }
            mosaic.sendPresence = { [weak liveSync] data in
                Task { @MainActor in liveSync?.sendPresence(data) }
            }
            return
        }
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
            presenceRelay.disconnect()
            mosaic.sendPresence = nil
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
            Tab(AppTab.views.label, systemImage: AppTab.views.systemImage, value: AppTab.views) {
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
        .allowsHitTesting(shellAllowsInteraction)
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
                composer: composer,
                profileIdentity: activationToken
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
                    profileIdentity: activationToken,
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

    /// While an activation is in flight, the rendered snapshot may still be
    /// profile A even though the selection already says B. Block data-surface
    /// gestures until B commits. A failed activation re-enables navigation so
    /// Settings remains reachable, while service-level admission guards keep
    /// every content mutation fail-closed.
    private var shellAllowsInteraction: Bool {
        if mosaic.backendMutationAdmissionIsOpen { return true }
        if case .failed = mosaic.connection { return true }
        return false
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
        case .views:   return "tray"
        case .library: return "doc.text"
        case .search:  return "magnifyingglass"
        }
    }
}
