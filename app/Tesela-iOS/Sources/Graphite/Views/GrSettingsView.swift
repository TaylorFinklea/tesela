import SwiftUI
#if canImport(UIKit)
import UIKit
#endif

/// Graphite Settings — a full port of the native `SettingsView` tree into
/// the Graphite design system. Reachable from the Daily header's gear.
///
/// The PRIMARY purpose (task #156): let the user set the backend server
/// URL + mode in-app, so device builds no longer need the Mac IP baked in.
/// The Server / Backend section ports `BackendSettingsView`'s save logic
/// verbatim (set `backend.mode`/`serverURL`, `mosaic.attach(backend:)`,
/// `await mosaic.refresh(from:)`) onto Graphite-themed controls.
///
/// No data layer or behavior is rebuilt: it binds the SAME state the
/// shipping `AppShell` settings bind (`BackendSettings`, `MosaicRegistry`,
/// `MockMosaicService.connection`, `RelayTicker`, `TranscriptionStore`,
/// the `@AppStorage` keys). Heavy existing screens (pairing, mosaic
/// add/edit, voice, transcription models) are PRESENTED AS-IS in sheets
/// with the `.graphite` theme injected rather than rebuilt — reuse over
/// re-theme. Only the Server, Mosaics list, Sync-status, Capture, and the
/// section chrome are rebuilt natively in Graphite.
///
/// Renders the Graphite screen idiom (mirrors `GrLibraryView` /
/// `GrDailyView`): `NavigationStack { VStack { GrHeader; ScrollView {
/// sections; 96pt spacer } } }.background(theme.bg)`. Reads
/// `@Environment(\.theme)` (forced to `.graphite` by `GrAppShell`).
struct GrSettingsView: View {
    @ObservedObject var mosaic: MockMosaicService
    @ObservedObject var backend: BackendSettings
    @ObservedObject var relayTicker: RelayTicker
    @ObservedObject var registry: MosaicRegistry
    var transcription: TranscriptionStore? = nil

    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss

    // ── Backend draft state (mirrors BackendSettingsView) ───────────────
    @State private var pickerMode: BackendSettings.Mode
    @State private var urlField: String
    @State private var isReloading: Bool = false
    @State private var showDisconnectConfirm: Bool = false

    // ── @AppStorage carried over verbatim from SettingsView ─────────────
    @AppStorage("captureDefaultTarget") private var captureDefault: CaptureDefault = .contextAware
    @AppStorage("bareDateField") private var bareDateField: String = "scheduled"
    @AppStorage("device.friendlyName") private var deviceName: String = ""

    // ── Sheet routing for reused heavy views ────────────────────────────
    @State private var showPair: Bool = false
    @State private var showAddMosaic: Bool = false
    @State private var editingProfile: MosaicProfile? = nil
    @State private var showVoice: Bool = false
    @State private var showModels: Bool = false

    init(
        mosaic: MockMosaicService,
        backend: BackendSettings,
        relayTicker: RelayTicker,
        registry: MosaicRegistry,
        transcription: TranscriptionStore? = nil
    ) {
        self.mosaic = mosaic
        self.backend = backend
        self.relayTicker = relayTicker
        self.registry = registry
        self.transcription = transcription
        self._pickerMode = State(initialValue: backend.mode)
        self._urlField = State(initialValue: backend.serverURL)
    }

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                GrHeader(title: "Settings", subtitle: "DEVICE") {
                    GrButton(variant: .ghost, label: "Done") { dismiss() }
                }
                ScrollView {
                    VStack(alignment: .leading, spacing: 22) {
                        Spacer().frame(height: 10)
                        serverSection
                        mosaicsSection
                        pairSection
                        syncSection
                        captureSection
                        voiceSection
                        appearanceSection
                        aboutSection
                        Spacer().frame(height: 96)
                    }
                    .padding(.horizontal, 14)
                }
            }
            .background(theme.bg)
            .onAppear {
                registry.seedFromLegacyIfNeeded(
                    legacyURL: backend.serverURL,
                    defaultName: "My mosaic"
                )
            }
            .confirmationDialog(
                "Disconnect from server?",
                isPresented: $showDisconnectConfirm,
                titleVisibility: .visible
            ) {
                Button("Disconnect", role: .destructive) {
                    Task { await disconnect() }
                }
                Button("Cancel", role: .cancel) {}
            } message: {
                Text("Backend will revert to mock data. Re-pair to reconnect to a server.")
            }
            // Reused heavy views — presented as-is with the Graphite theme
            // injected, rather than rebuilt.
            .sheet(isPresented: $showPair) {
                NavigationStack {
                    PairDeviceView(backend: backend, mosaic: mosaic, registry: registry)
                }
                .environment(\.theme, theme)
                .preferredColorScheme(.dark)
            }
            .sheet(isPresented: $showAddMosaic) {
                AddMosaicView(registry: registry)
                    .environment(\.theme, theme)
                    .preferredColorScheme(.dark)
            }
            .sheet(item: $editingProfile) { profile in
                MosaicEditView(registry: registry, existing: profile)
                    .environment(\.theme, theme)
                    .preferredColorScheme(.dark)
            }
            .sheet(isPresented: $showVoice) {
                NavigationStack {
                    VoiceSettingsView(transcription: transcription)
                }
                .environment(\.theme, theme)
                .preferredColorScheme(.dark)
            }
            .sheet(isPresented: $showModels) {
                if let transcription {
                    NavigationStack {
                        TranscriptionModelsView(store: transcription)
                    }
                    .environment(\.theme, theme)
                    .preferredColorScheme(.dark)
                }
            }
        }
    }

    // ── Section header label (Graphite idiom) ───────────────────────────

    private func sectionLabel(_ text: String) -> some View {
        Text(text.uppercased())
            .font(.system(size: 11, weight: .semibold))
            .tracking(0.6)
            .foregroundStyle(theme.fgMuted)
            .padding(.horizontal, 4)
    }

    private func sectionCaption(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 10.5, design: .monospaced))
            .foregroundStyle(theme.fgFaint)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, 4)
    }

    /// A `bg2` card with a hairline border that hosts a vertical stack of
    /// rows — the Graphite list container (mirrors `GrLibraryView`'s
    /// pages/tags lists).
    private func card<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        VStack(spacing: 0) { content() }
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(theme.bg2)
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(theme.lineSoft, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    // ── Server / Backend (the priority — task #156) ─────────────────────

    private var serverSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionLabel("Server")
            card {
                VStack(alignment: .leading, spacing: 12) {
                    // Mode segmented control (Graphite-themed).
                    modeSegment
                    if pickerMode == .http {
                        urlEditor
                    }
                    connectionStatusRow
                    if let warning = localhostMisconfigWarning {
                        misconfigWarningRow(warning)
                    }
                    HStack(spacing: 8) {
                        GrButton(
                            variant: .cta,
                            label: isReloading ? "Refreshing…" : "Save & refresh"
                        ) {
                            Task { await save() }
                        }
                        .disabled(isReloading)
                        if backend.mode == .http {
                            GrButton(variant: .ghost, label: "Disconnect") {
                                showDisconnectConfirm = true
                            }
                        }
                    }
                }
            }
            sectionCaption(
                "Mock is a built-in snapshot. HTTP hits a tesela-server on your Mac or LAN "
                + "(Simulator: 127.0.0.1; real device: the Mac's LAN/Tailscale address). "
                + "Relay reads your on-device notes synced through the encrypted relay — "
                + "set automatically when you pair to a desktop; works with the Mac off."
            )
        }
    }

    private var modeSegment: some View {
        HStack(spacing: 4) {
            segmentButton(title: "Mock", on: pickerMode == .mock) { pickerMode = .mock }
            segmentButton(title: "HTTP", on: pickerMode == .http) { pickerMode = .http }
            segmentButton(title: "Relay", on: pickerMode == .relay) { pickerMode = .relay }
        }
        .padding(3)
        .background(theme.bg3)
        .overlay(
            RoundedRectangle(cornerRadius: 9)
                .stroke(theme.lineSoft, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 9))
    }

    private func segmentButton(title: String, on: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title)
                .font(.system(size: 12.5, weight: on ? .semibold : .regular))
                .foregroundStyle(on ? theme.fgDefault : theme.fgMuted)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 7)
                .background(on ? theme.bg4 : .clear)
                .clipShape(RoundedRectangle(cornerRadius: 7))
        }
        .buttonStyle(.plain)
    }

    private var urlEditor: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("SERVER URL")
                .font(.system(size: 10, weight: .semibold))
                .tracking(0.8)
                .foregroundStyle(theme.fgFaint)
            TextField("http://127.0.0.1:7474", text: $urlField)
                .font(.system(size: 13.5, design: .monospaced))
                .foregroundStyle(theme.fgDefault)
                .tint(theme.accentPrimary)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .keyboardType(.URL)
                .submitLabel(.done)
                .padding(.horizontal, 11)
                .padding(.vertical, 10)
                .background(theme.bg3)
                .overlay(
                    RoundedRectangle(cornerRadius: 8)
                        .stroke(theme.line, lineWidth: 1)
                )
                .clipShape(RoundedRectangle(cornerRadius: 8))
        }
    }

    private var connectionStatusRow: some View {
        HStack(spacing: 10) {
            Circle()
                .fill(statusDotColor)
                .frame(width: 9, height: 9)
            VStack(alignment: .leading, spacing: 2) {
                Text(statusLabel)
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(theme.fgDefault)
                if let detail = statusDetail {
                    Text(detail)
                        .font(.system(size: 10.5, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                        .lineLimit(2)
                        .truncationMode(.middle)
                }
            }
            Spacer()
        }
        .padding(.vertical, 2)
    }

    private var statusDotColor: Color {
        switch mosaic.connection {
        case .ready:                  return theme.accentSecondary
        case .connecting, .switching: return theme.typeNote
        case .failed:                 return theme.typeTask
        case .idle:                   return theme.fgFaint
        }
    }

    private var statusLabel: String {
        switch mosaic.connection {
        case .idle:       return backend.mode == .mock ? "Mock data" : "Not yet connected"
        case .connecting: return "Connecting…"
        case .switching:  return "Switching mosaic…"
        case .ready:      return "Connected"
        case .failed:     return "Connection failed"
        }
    }

    /// `.relay` mode never talks to `backend.serverURL` (see
    /// `BackendSettings.resolveBackend` — relay ignores the server URL
    /// entirely), so showing it here as the "Connected" detail was the
    /// dead `127.0.0.1:7474` that actively misled diagnosis (tesela-4mc).
    /// The real address lives in `relayTicker.relayURL`, shown in the
    /// Sync section below; this row shows nothing extra for relay mode.
    private var statusDetail: String? {
        switch mosaic.connection {
        case .failed(let msg): return msg
        case .ready:           return backend.mode == .relay ? nil : backend.serverURL
        case .connecting:      return backend.mode == .relay ? nil : backend.serverURL
        default:               return nil
        }
    }

    /// On a PHYSICAL device, an HTTP backend pointed at 127.0.0.1/localhost
    /// can NEVER reach the Mac — that address is the phone itself, where no
    /// server runs. HTTP reads then silently fall back to a local scan and
    /// writes go nowhere the relay can see, so the device drifts apart with
    /// NO error shown ("Connected" stays green). This is the exact trap that
    /// silently desynced the iPhone (2026-06-21). The Simulator is exempt —
    /// there 127.0.0.1 IS the Mac.
    private var localhostMisconfigWarning: String? {
        #if targetEnvironment(simulator)
        return nil
        #else
        let s = urlField.lowercased()
        guard pickerMode == .http,
              s.contains("127.0.0.1") || s.contains("localhost") || s.contains("::1")
        else { return nil }
        return "127.0.0.1 is THIS device, not your Mac — edits silently go nowhere and your "
            + "devices drift apart. Use your Mac's LAN/Tailscale address, or switch to Relay "
            + "(syncs through the encrypted relay; works with the Mac off)."
        #endif
    }

    private func misconfigWarningRow(_ text: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .top, spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: 12))
                    .foregroundStyle(theme.typeTask)
                Text(text)
                    .font(.system(size: 11.5))
                    .foregroundStyle(theme.fgDefault)
                    .fixedSize(horizontal: false, vertical: true)
            }
            GrButton(variant: .cta, label: "Switch to Relay") {
                pickerMode = .relay
                Task { await save() }
            }
        }
        .padding(10)
        .background(theme.typeTask.opacity(0.10))
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(theme.typeTask.opacity(0.35), lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    /// Ported verbatim from `BackendSettingsView.save()` — the core #156
    /// behavior: persist mode/URL, re-attach, refresh.
    @MainActor
    private func save() async {
        backend.mode = pickerMode
        backend.serverURL = urlField
        mosaic.attach(backend: backend.backend)
        isReloading = true
        await mosaic.refresh(from: backend.backend)
        isReloading = false
    }

    /// Ported verbatim from `BackendSettingsView.disconnect()`.
    @MainActor
    private func disconnect() async {
        backend.mode = .mock
        backend.serverURL = "http://127.0.0.1:7474"
        pickerMode = .mock
        urlField = backend.serverURL
        mosaic.attach(backend: backend.backend)
        await mosaic.refresh(from: backend.backend)
    }

    // ── Mosaics ─────────────────────────────────────────────────────────

    private var mosaicsSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionLabel("Mosaics")
            card {
                if registry.profiles.isEmpty {
                    Text("No mosaics yet — add one to point Tesela at a server.")
                        .font(.system(size: 12))
                        .foregroundStyle(theme.fgMuted)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.vertical, 6)
                        .padding(.horizontal, 2)
                } else {
                    ForEach(registry.profiles) { profile in
                        mosaicRow(profile)
                    }
                }
                Divider().overlay(theme.lineSoft).padding(.vertical, 4)
                GrRow(icon: "plus", label: "Add a mosaic") {
                    showAddMosaic = true
                }
            }
        }
    }

    private func mosaicRow(_ profile: MosaicProfile) -> some View {
        let isActive = registry.activeID == profile.id
        return Button {
            registry.setActive(profile.id)
        } label: {
            HStack(spacing: 10) {
                Image(systemName: profile.iconSymbol)
                    .font(.system(size: 15))
                    .foregroundStyle(isActive ? theme.accentPrimary : theme.fgSubtle)
                    .frame(width: 20)
                VStack(alignment: .leading, spacing: 2) {
                    Text(profile.name)
                        .font(.system(size: 13))
                        .foregroundStyle(isActive ? theme.fgDefault : theme.fgMuted)
                        .lineLimit(1)
                    Text(profile.serverURL)
                        .font(.system(size: 10.5, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
                Spacer(minLength: 8)
                if isActive {
                    GrIcon(name: "square-check", size: 14)
                        .foregroundStyle(theme.accentPrimary)
                }
                Button {
                    editingProfile = profile
                } label: {
                    GrIcon(name: "dots-vertical", size: 15)
                        .foregroundStyle(theme.fgSubtle)
                        .frame(width: 26, height: 26)
                }
                .buttonStyle(.plain)
            }
            .padding(.vertical, 6)
            .padding(.horizontal, 6)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    // ── Pair a device ───────────────────────────────────────────────────

    private var pairSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionLabel("Pair a device")
            card {
                GrRow(icon: "link", label: "Scan QR or enter a 6-character code") {
                    showPair = true
                }
            }
            sectionCaption(
                "Point this iPhone at a server, or share this device's QR / short code with a third device."
            )
        }
    }

    // ── Sync status (RelayTicker + device name) ─────────────────────────

    private var syncSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionLabel("Sync")
            card {
                VStack(alignment: .leading, spacing: 10) {
                    HStack(spacing: 8) {
                        Circle()
                            .fill(relayDotColor)
                            .frame(width: 9, height: 9)
                        Text(relayStatusLabel)
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(theme.fgDefault)
                        Spacer()
                        if relayTicker.hubMode {
                            Text("hub")
                                .font(.system(size: 9.5, design: .monospaced))
                                .foregroundStyle(theme.fgFaint)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(theme.bg3)
                                .clipShape(Capsule())
                        }
                    }
                    if let err = relayTicker.lastError {
                        Text(err)
                            .font(.system(size: 10.5, design: .monospaced))
                            .foregroundStyle(theme.typeTask)
                            .padding(8)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(theme.typeTask.opacity(0.1))
                            .clipShape(RoundedRectangle(cornerRadius: 6))
                    }
                    VStack(spacing: 0) {
                        metricRow("Relay URL", relayTicker.relayURL ?? "resolving…")
                        metricRow("Last push", relativeTime(relayTicker.lastSuccessfulPushAt))
                        metricRow("Last tick", relativeTime(relayTicker.lastTickAt))
                        metricRow("Last received", "\(relayTicker.lastApplied) op\(relayTicker.lastApplied == 1 ? "" : "s")")
                        metricRow("Last sent", "\(relayTicker.lastSent) op\(relayTicker.lastSent == 1 ? "" : "s")")
                        metricRow("Inbound seq", "\(relayTicker.inboundCursorSeq)")
                        metricRow("APNs push", relayTicker.apnsNote)
                        metricRow("Last splice", relayTicker.lastSpliceDiag)
                    }
                }
            }
            // Device name editor.
            card {
                VStack(alignment: .leading, spacing: 6) {
                    Text("THIS DEVICE")
                        .font(.system(size: 10, weight: .semibold))
                        .tracking(0.8)
                        .foregroundStyle(theme.fgFaint)
                    TextField("Device name", text: $deviceName, prompt: Text(systemDeviceName))
                        .font(.system(size: 13.5))
                        .foregroundStyle(theme.fgDefault)
                        .tint(theme.accentPrimary)
                        .textInputAutocapitalization(.words)
                        .submitLabel(.done)
                        .padding(.horizontal, 11)
                        .padding(.vertical, 10)
                        .background(theme.bg3)
                        .overlay(
                            RoundedRectangle(cornerRadius: 8)
                                .stroke(theme.line, lineWidth: 1)
                        )
                        .clipShape(RoundedRectangle(cornerRadius: 8))
                        .onAppear {
                            if deviceName.isEmpty { deviceName = systemDeviceName }
                        }
                }
            }
        }
    }

    private var relayDotColor: Color {
        if relayTicker.lastError != nil { return theme.typeTask }
        return relayTicker.isRunning ? theme.accentSecondary : theme.fgFaint
    }

    private var relayStatusLabel: String {
        if relayTicker.lastError != nil { return "Sync error" }
        if !relayTicker.isRunning { return "Sync paused" }
        return relayTicker.lastTickAt != nil ? "Syncing" : "Starting…"
    }

    private func metricRow(_ label: String, _ value: String) -> some View {
        HStack {
            Text(label)
                .font(.system(size: 11))
                .foregroundStyle(theme.fgFaint)
            Spacer()
            Text(value)
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(theme.fgSubtle)
        }
        .padding(.vertical, 4)
    }

    private var systemDeviceName: String {
        #if canImport(UIKit)
        UIDevice.current.name
        #else
        "This device"
        #endif
    }

    private func relativeTime(_ date: Date?) -> String {
        guard let date else { return "never" }
        let ageSec = max(0, Int64(Date().timeIntervalSince(date)))
        if ageSec < 60 { return "\(ageSec)s ago" }
        let mins = ageSec / 60
        if mins < 60 { return "\(mins)m ago" }
        let hrs = mins / 60
        if hrs < 24 { return "\(hrs)h ago" }
        return date.formatted()
    }

    // ── Capture ─────────────────────────────────────────────────────────

    private var captureSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionLabel("Capture")
            card {
                VStack(spacing: 0) {
                    pickerRow(label: "Default target") {
                        Picker("", selection: $captureDefault) {
                            ForEach(CaptureDefault.allCases, id: \.self) { option in
                                Text(option.label).tag(option)
                            }
                        }
                        .labelsHidden()
                        .tint(theme.accentPrimary)
                    }
                    Divider().overlay(theme.lineSoft)
                    pickerRow(label: "Default date field") {
                        Picker("", selection: $bareDateField) {
                            Text("Scheduled").tag("scheduled")
                            Text("Deadline").tag("deadline")
                        }
                        .labelsHidden()
                        .tint(theme.accentPrimary)
                    }
                }
            }
        }
    }

    private func pickerRow<Control: View>(label: String, @ViewBuilder control: () -> Control) -> some View {
        HStack {
            Text(label)
                .font(.system(size: 12.5))
                .foregroundStyle(theme.fgMuted)
            Spacer()
            control()
                .font(.system(size: 12.5))
        }
        .padding(.vertical, 4)
        .padding(.horizontal, 4)
    }

    // ── Voice ───────────────────────────────────────────────────────────

    private var voiceSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionLabel("Voice")
            card {
                GrRow(icon: "microphone", label: "Transcription", meta: voiceModelLabel) {
                    showVoice = true
                }
                if transcription != nil {
                    Divider().overlay(theme.lineSoft).padding(.vertical, 4)
                    GrRow(icon: "adjustments", label: "Manage models", meta: modelsCountLabel) {
                        showModels = true
                    }
                }
            }
        }
    }

    private var voiceModelLabel: String {
        guard let transcription, !transcription.activeModelId.isEmpty else {
            return "no model"
        }
        return TranscriptionCatalog.find(transcription.activeModelId)?.displayName
            ?? transcription.activeModelId
    }

    private var modelsCountLabel: String {
        guard let store = transcription else { return "" }
        let downloaded = store.states.values.filter {
            if case .downloaded = $0 { return true } else { return false }
        }.count
        return "\(downloaded)/\(TranscriptionCatalog.all.count)"
    }

    // ── Appearance (read-only — the Graphite shell forces .graphite) ────

    private var appearanceSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionLabel("Appearance")
            card {
                HStack {
                    Text("Theme")
                        .font(.system(size: 12.5))
                        .foregroundStyle(theme.fgMuted)
                    Spacer()
                    Text("Graphite (dark)")
                        .font(.system(size: 11.5, design: .monospaced))
                        .foregroundStyle(theme.fgSubtle)
                }
                .padding(.vertical, 4)
                .padding(.horizontal, 4)
            }
            sectionCaption("The Graphite shell is locked to its dark theme. A theme picker returns at cutover.")
        }
    }

    // ── About ───────────────────────────────────────────────────────────

    private var aboutSection: some View {
        Text("Tesela for iPhone · v0.4.1 · tesela-core 0.9.2")
            .font(.system(size: 10.5, design: .monospaced))
            .foregroundStyle(theme.fgFaint)
            .frame(maxWidth: .infinity, alignment: .center)
            .padding(.top, 6)
    }
}
