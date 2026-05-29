import SwiftUI

/// Graphite app shell — the new presentation root that MIRRORS
/// `Sources/Views/AppShell.swift` and BINDS TO THE SAME data/sync
/// services. No behavior is rebuilt: the iOS-26 native `TabView`
/// (4 labeled tabs + the `Tab(role: .search)` glass circle), the
/// `@StateObject MockMosaicService` + `RelayTicker`, the
/// `CaptureComposer` / `StreamingVoiceRecorder` capture wiring, and the
/// relay-ticker poll/push loop are all reused exactly as AppShell wires
/// them. Only the chrome (Graphite header, capture pill → sheet) and the
/// `.graphite` theme are new; tab CONTENT is `GrTabPlaceholder` this
/// phase (the daily-driver views are the next plan).
///
/// `GrAppShell` is NOT the app entry yet — `TeselaApp.swift` still mounts
/// the legacy `AppShell`. This becomes the root only at cutover.
struct GrAppShell: View {
    @StateObject private var mosaic = MockMosaicService()
    @StateObject private var backend = BackendSettings()
    @StateObject private var relayTicker = RelayTicker()
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
                relayTicker.start()
            }
            .onChange(of: scenePhase) { _, newPhase in
                switch newPhase {
                case .active:
                    relayTicker.start()
                    Task {
                        await mosaic.refresh(from: backend.backend)
                        await mosaic.refreshLoadedPages()
                    }
                case .background:
                    relayTicker.stop()
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
    /// `safeAreaInset(.bottom)`. Tab content is the Graphite placeholder.
    private var shell: some View {
        TabView(selection: $activeTab) {
            Tab(AppTab.daily.label, systemImage: AppTab.daily.systemImage, value: AppTab.daily) {
                GrTabPlaceholder(tab: .daily)
            }
            Tab(AppTab.agenda.label, systemImage: AppTab.agenda.systemImage, value: AppTab.agenda) {
                GrTabPlaceholder(tab: .agenda)
            }
            Tab(AppTab.inbox.label, systemImage: AppTab.inbox.systemImage, value: AppTab.inbox) {
                GrTabPlaceholder(tab: .inbox)
            }
            Tab(AppTab.library.label, systemImage: AppTab.library.systemImage, value: AppTab.library) {
                GrTabPlaceholder(tab: .library)
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
