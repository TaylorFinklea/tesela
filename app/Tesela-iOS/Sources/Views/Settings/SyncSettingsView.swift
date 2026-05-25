import SwiftUI
#if canImport(UIKit)
import UIKit
#endif

/// Sync surface — connected (with peer list) and disconnected (with
/// retry + diagnostics). Symmetric P2P language only — no host /
/// relay / source-of-truth roles. Per decision #4.
struct SyncSettingsView: View {
    @ObservedObject var syncState: SyncState
    @ObservedObject var mosaic: MockMosaicService
    @State private var simulatedOffline: Bool = false
    @State private var simulatedPending: Bool = false
    @State private var relayStatus: RelayStatusInfo? = nil
    @State private var relayLoaded: Bool = false
    // B.1.4 — FFI smoke probe state. Removed once B.2/B.3 land and the
    // real iOS-as-peer Settings UI replaces this debug button.
    @State private var smokeResult: String? = nil
    @State private var smokeRunning: Bool = false

    /// User-facing name for this device. Advertised to peers once the
    /// sync backend is wired (see roadmap "iOS sync"); for now it's
    /// just a local setting so the user can see their own label in
    /// other clients when sync lands. Default seeded from the iOS
    /// device name (e.g. "Roshar") on first read.
    @AppStorage("device.friendlyName") private var deviceName: String = ""

    @Environment(\.theme) private var theme

    var body: some View {
        Form {
            relaySection
            ffiSmokeSection
            deviceNameSection

            if simulatedOffline {
                disconnectedBanner
                disconnectedPeerList
                diagnosticsSection
            } else {
                connectedBanner
                peerListSection
            }

            Section("Strategy") {
                ToggleRow(title: "Local Wi-Fi peers",
                          detail: "Bonjour discovery on this network",
                          initialOn: true)
                ToggleRow(title: "Cross-network peers",
                          detail: "Reach peers via Tailscale or direct internet",
                          initialOn: true)
                ToggleRow(title: "Only on Wi-Fi",
                          detail: "Skip sync on cellular to save data",
                          initialOn: false)
            }

            Section("Conflict policy") {
                ToggleRow(title: "Show resolution sheet on conflict",
                          detail: "Otherwise pick newest-wins",
                          initialOn: true)
                LabeledContent("History retention", value: "90 days")
            }

            Section {
                Toggle("Simulate offline", isOn: $simulatedOffline)
                    .tint(theme.accentPrimary)
                    .onChange(of: simulatedOffline) { _, newValue in
                        syncState.isReachable = !newValue
                    }
                Toggle("Simulate pending edits", isOn: $simulatedPending)
                    .tint(theme.accentPrimary)
                    .onChange(of: simulatedPending) { _, newValue in
                        syncState.hasPendingEdits = newValue
                    }
            } header: {
                Text("Debug")
            } footer: {
                Text("Enable both to surface the ● indicator on every page title.")
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }

            Section("Advanced") {
                Button {
                    // Phase 15: copy sync token to clipboard
                } label: {
                    LabeledContent("Sync token") {
                        Text("copy")
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundStyle(theme.accentPrimary)
                    }
                }
                Button(role: .destructive) {
                    // Phase 15: reset sync state
                } label: {
                    Text("Reset sync state")
                }
            }
        }
        .scrollContentBackground(.hidden)
        .background(theme.bg)
        .navigationTitle("Sync")
        .navigationBarTitleDisplayMode(.inline)
        .task {
            await refreshRelayStatus()
        }
        .refreshable {
            await refreshRelayStatus()
        }
    }

    // ── WAN relay (read-only — surfaces the Mac's relay state) ──────────

    /// iOS isn't a sync peer yet (UniFFI track is multi-week work);
    /// this surface is intentionally read-only. It calls
    /// `GET /sync/relay/status` on the configured Mac backend so the
    /// user can see "your Mac is paired with relay X, last poll N
    /// seconds ago" — the architecture is honest about the fact that
    /// iOS still talks to the Mac over HTTP, and the Mac is what
    /// talks to the relay.
    @ViewBuilder
    private var relaySection: some View {
        Section {
            if !relayLoaded {
                HStack(spacing: 10) {
                    ProgressView().scaleEffect(0.7)
                    Text("Checking the Mac for relay status…")
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                }
            } else if relayStatus == nil {
                relayUnreachable
            } else if let status = relayStatus, !status.configured {
                relayUnconfigured
            } else if let status = relayStatus {
                relayConfigured(status)
            }
        } header: {
            Text("WAN Relay")
        } footer: {
            Text("iOS itself isn't a sync peer yet — this iPhone talks to your Mac over HTTP. When the Mac is configured with a relay, edits from other devices land on the Mac through that relay, then flow to you. Future iOS sync (native peer) is on the roadmap.")
                .font(.caption2)
        }
    }

    // ─── B.1.4 — FFI smoke probe ────────────────────────────────────
    //
    // Temporary section that proves the Rust→UniFFI→Swift pipeline is
    // wired correctly for iOS-as-relay-peer (Path B). On tap: opens an
    // ephemeral local sync engine, instantiates a RelayClientHandle
    // pointed at the *same* relay URL the Mac is using, runs the full
    // register / verify / poll handshake from Swift, and shows the
    // result. No persistence — fresh group each tap so it can't clash
    // with the Mac's real group state on the relay.
    //
    // Removed when B.2 (real producer) + B.3 (real consumer) land —
    // those replace this with the actual sync UI.
    @ViewBuilder
    private var ffiSmokeSection: some View {
        Section {
            Button {
                Task { await runFfiSmoke() }
            } label: {
                HStack {
                    if smokeRunning {
                        ProgressView().scaleEffect(0.7)
                        Text("Running smoke test…")
                    } else {
                        Image(systemName: "testtube.2")
                        Text("Run FFI relay smoke test")
                    }
                }
            }
            .disabled(smokeRunning || relayStatus?.url == nil)
            if let result = smokeResult {
                Text(result)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(result.hasPrefix("✅") ? theme.typeQuery : theme.typeTask)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.vertical, 4)
            }
        } header: {
            Text("B.1 — FFI smoke (dev)")
        } footer: {
            Text("Uses the Mac's relay URL with an ephemeral group. Won't touch your real sync. Removed once iOS becomes a real sync peer.")
                .font(.caption2)
        }
    }

    /// Runs the B.1.4 FFI smoke: open engine → build relay client →
    /// register → verify → poll. Captures every error so the result is
    /// always renderable on screen, never propagated as an unhandled
    /// exception.
    private func runFfiSmoke() async {
        smokeRunning = true
        defer { smokeRunning = false }
        smokeResult = nil

        guard let relayURL = relayStatus?.url else {
            smokeResult = "❌ no relay URL — Mac isn't paired with a relay yet"
            return
        }

        // Fresh ephemeral group per run — keeps the probe from clobbering
        // the Mac's real registration on the relay.
        let group = generateGroupIdentity()
        let deviceHex = generateDeviceIdHex()

        // Engine lives in a temp file. Cleaned up by iOS when the
        // sandbox is purged; we don't bother removing it here since the
        // file is tiny and the next run picks a fresh name.
        let dbPath = FileManager.default.temporaryDirectory
            .appendingPathComponent("smoke-\(UUID().uuidString.prefix(8)).db")
            .path
        let sqliteURL = "sqlite:\(dbPath)"

        do {
            let engine = try await SyncEngineHandle.open(
                sqliteUrl: sqliteURL,
                deviceIdHex: deviceHex
            )
            let relay = try RelayClientHandle(
                relayUrl: relayURL,
                groupIdHex: group.groupIdHex,
                deviceIdHex: deviceHex,
                groupKeyHex: group.groupKeyHex
            )
            let registeredAt = try await relay.registerOrRecover()
            try await relay.verifyRegistration()
            let probe = try await relay.pollCount(sinceSeq: 0)
            smokeResult = """
                ✅ smoke passed
                  device: \(engine.deviceHex().prefix(8))…
                  group:  \(group.groupIdHex.prefix(8))…
                  relay:  registered@\(registeredAt)
                  poll:   count=\(probe.count) maxSeq=\(probe.highestSeq)
                """
        } catch let err as FfiSyncError {
            smokeResult = "❌ \(err.localizedDescription)"
        } catch {
            smokeResult = "❌ \(error.localizedDescription)"
        }
    }

    private var relayUnreachable: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Image(systemName: "cloud.slash")
                    .foregroundStyle(theme.fgFaint)
                Text("Can't reach the Mac server")
                    .font(.system(size: 13, weight: .medium))
            }
            Text("Check the backend URL in Settings → Backend, or that the Mac's tesela-server is running.")
                .font(.system(size: 11))
                .foregroundStyle(theme.fgFaint)
        }
        .padding(.vertical, 4)
    }

    private var relayUnconfigured: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Image(systemName: "minus.circle")
                    .foregroundStyle(theme.fgFaint)
                Text("Mac is LAN-only — no relay configured")
                    .font(.system(size: 13, weight: .medium))
            }
            Text("To enable cross-network sync, add a `[sync.relay]` block to the Mac's `.tesela/config.toml`. See `crates/tesela-relay/DEPLOY.md` in the repo for the Docker recipe.")
                .font(.system(size: 11))
                .foregroundStyle(theme.fgFaint)
        }
        .padding(.vertical, 4)
    }

    @ViewBuilder
    private func relayConfigured(_ s: RelayStatusInfo) -> some View {
        let healthy = s.last_error == nil && s.last_poll_at != nil
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Circle()
                    .fill(s.last_error != nil
                        ? theme.typeTask
                        : (s.last_poll_at != nil ? theme.typeQuery : theme.accentPrimary))
                    .frame(width: 8, height: 8)
                Text(healthy ? "Connected" : (s.last_error != nil ? "Error" : "Configured"))
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(healthy ? theme.typeQuery : (s.last_error != nil ? theme.typeTask : theme.fgDefault))
            }
            if let url = s.url {
                Text(url)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
                    .lineLimit(2)
                    .truncationMode(.middle)
            }
            if let err = s.last_error {
                Text(err)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.typeTask)
                    .padding(8)
                    .background(theme.typeTask.opacity(0.1))
                    .clipShape(RoundedRectangle(cornerRadius: 4))
            }
            VStack(alignment: .leading, spacing: 4) {
                relayMetricRow("Registered", relativeTime(s.registered_at))
                relayMetricRow("Last poll", relativeTime(s.last_poll_at))
                relayMetricRow("Last put", relativeTime(s.last_put_at))
                relayMetricRow("Inbound seq", "\(s.inbound_cursor)")
            }
        }
        .padding(.vertical, 4)
    }

    private func relayMetricRow(_ label: String, _ value: String) -> some View {
        HStack {
            Text(label)
                .font(.system(size: 11))
                .foregroundStyle(theme.fgFaint)
            Spacer()
            Text(value)
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(theme.fgSubtle)
        }
    }

    private func relativeTime(_ unixSecs: Int64?) -> String {
        guard let unixSecs else { return "never" }
        let ageSec = max(0, Int64(Date().timeIntervalSince1970) - unixSecs)
        if ageSec < 60 { return "\(ageSec)s ago" }
        let min = ageSec / 60
        if min < 60 { return "\(min)m ago" }
        let hr = min / 60
        if hr < 24 { return "\(hr)h ago" }
        return Date(timeIntervalSince1970: TimeInterval(unixSecs)).formatted()
    }

    private func refreshRelayStatus() async {
        relayStatus = await mosaic.fetchRelayStatus()
        relayLoaded = true
    }

    // ── This device ─────────────────────────────────────────────────────

    private var deviceNameSection: some View {
        Section {
            TextField("Device name", text: $deviceName, prompt: Text(systemDeviceName))
                .textInputAutocapitalization(.words)
                .submitLabel(.done)
                .onAppear {
                    if deviceName.isEmpty {
                        deviceName = systemDeviceName
                    }
                }
        } header: {
            Text("This device")
        } footer: {
            Text("Shown to other devices once they pair. Leave blank to fall back to the iOS device name.")
                .font(.caption2)
        }
    }

    private var systemDeviceName: String {
        #if canImport(UIKit)
        UIDevice.current.name
        #else
        "This device"
        #endif
    }

    // ── Connected banner ────────────────────────────────────────────────

    private var connectedBanner: some View {
        Section {
            HStack(alignment: .center, spacing: 10) {
                Image(systemName: "checkmark.circle.fill")
                    .font(.title2)
                    .foregroundStyle(theme.typeQuery)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Up to date")
                        .font(.headline)
                        .foregroundStyle(theme.typeQuery)
                    Text("3 of 3 peers reachable · last sync 12s ago")
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(theme.fgSubtle)
                }
                Spacer()
                Button("Sync now") { /* trigger sync */ }
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(theme.typeQuery)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(theme.typeQuery.opacity(0.15))
                    .clipShape(Capsule())
            }
            .padding(.vertical, 6)
        }
    }

    private var peerListSection: some View {
        Section {
            // Local device — always present, even before any pairing.
            peerRow(
                MockPeer(
                    name: deviceName.isEmpty ? systemDeviceName : deviceName,
                    host: "This device · \(systemPlatformLabel)",
                    systemSymbol: localDeviceSymbol,
                    lastSeen: "now"
                ),
                online: true
            )
        } header: {
            Text("Paired devices")
        } footer: {
            Text("Pair another device from the Pair button to start syncing. Devices you've paired appear here once the LAN sync backend ships.")
                .font(.caption2)
        }
    }

    private var localDeviceSymbol: String {
        #if os(iOS)
        UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "iphone"
        #elseif os(macOS)
        "laptopcomputer"
        #else
        "questionmark.circle"
        #endif
    }

    private var systemPlatformLabel: String {
        #if canImport(UIKit)
        "\(UIDevice.current.systemName) \(UIDevice.current.systemVersion)"
        #else
        "macOS"
        #endif
    }

    // ── Disconnected banner ─────────────────────────────────────────────

    private var disconnectedBanner: some View {
        Section {
            VStack(alignment: .leading, spacing: 12) {
                HStack(alignment: .center, spacing: 10) {
                    Image(systemName: "cloud.slash.fill")
                        .font(.title2)
                        .foregroundStyle(theme.typeTask)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Can't reach any peers")
                            .font(.headline)
                            .foregroundStyle(theme.typeTask)
                        Text("offline 2h 14m · 12 local edits will sync when peers return")
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundStyle(theme.fgSubtle)
                    }
                    Spacer()
                }
                HStack(spacing: 8) {
                    Button {
                        simulatedOffline = false
                    } label: {
                        Text("Retry now")
                            .font(.system(size: 12, design: .monospaced))
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 8)
                            .foregroundStyle(theme.bg)
                            .background(theme.typeTask)
                            .clipShape(RoundedRectangle(cornerRadius: 6))
                    }
                    .buttonStyle(.plain)
                    Button { /* placeholder */ } label: {
                        Text("Diagnose")
                            .font(.system(size: 12, design: .monospaced))
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 8)
                            .foregroundStyle(theme.fgMuted)
                            .overlay(
                                RoundedRectangle(cornerRadius: 6)
                                    .stroke(theme.line, lineWidth: 1)
                            )
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.vertical, 6)
        }
    }

    private var disconnectedPeerList: some View {
        Section {
            peerRow(
                MockPeer(
                    name: deviceName.isEmpty ? systemDeviceName : deviceName,
                    host: "This device · \(systemPlatformLabel)",
                    systemSymbol: localDeviceSymbol,
                    lastSeen: "—"
                ),
                online: false
            )
        } header: {
            Text("Offline")
        }
    }

    private var diagnosticsSection: some View {
        Section("Diagnostics") {
            LabeledContent("Wi-Fi",            value: "connected")
            LabeledContent("Last attempt",     value: "2m ago")
            LabeledContent("Pending edits",    value: "12 local")
        }
    }

    // ── Peer row ────────────────────────────────────────────────────────

    private func peerRow(_ peer: MockPeer, online: Bool) -> some View {
        HStack(spacing: 12) {
            Image(systemName: peer.systemSymbol)
                .font(.title3)
                .foregroundStyle(online ? theme.typeQuery : theme.typeTask)
                .frame(width: 24, alignment: .center)
            VStack(alignment: .leading, spacing: 2) {
                Text(peer.name)
                    .foregroundStyle(theme.fgDefault)
                Text(peer.host)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
                    .lineLimit(1)
            }
            Spacer()
            VStack(alignment: .trailing, spacing: 2) {
                if online {
                    HStack(spacing: 4) {
                        Circle().fill(theme.typeQuery).frame(width: 6, height: 6)
                        Text("up")
                    }
                } else {
                    HStack(spacing: 4) {
                        Circle().fill(theme.typeTask).frame(width: 6, height: 6)
                        Text("unreachable")
                    }
                }
                Text(online ? peer.lastSeen : "seen \(peer.lastSeen)")
                    .foregroundStyle(theme.fgFaint)
            }
            .font(.system(size: 10.5, design: .monospaced))
        }
    }
}

// MARK: - Mock peer model (Phase 15 swaps in real values)

struct MockPeer: Identifiable {
    let id = UUID()
    let name: String
    let host: String
    let systemSymbol: String
    let lastSeen: String
}

enum MockPeers {
    static let connected: [MockPeer] = [
        MockPeer(name: "workshop",     host: "taylor-workshop · macOS 15",   systemSymbol: "laptopcomputer",    lastSeen: "now"),
        MockPeer(name: "tower",        host: "taylor-tower · Linux",          systemSymbol: "desktopcomputer",   lastSeen: "12s"),
        MockPeer(name: "kitchen-ipad", host: "iPad Pro · iPadOS 26",          systemSymbol: "ipad",              lastSeen: "4m"),
    ]
}

/// Wrapper that gives each Toggle row its own State without forcing
/// callers to manage a binding for each.
struct ToggleRow: View {
    let title: String
    let detail: String?
    @State var isOn: Bool

    init(title: String, detail: String? = nil, initialOn: Bool) {
        self.title = title
        self.detail = detail
        self._isOn = State(initialValue: initialOn)
    }

    var body: some View {
        Toggle(isOn: $isOn) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                if let detail {
                    Text(detail)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(.secondary)
                }
            }
        }
    }
}
