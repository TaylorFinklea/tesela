import SwiftUI

/// Sync surface — connected (with peer list) and disconnected (with
/// retry + diagnostics). Symmetric P2P language only — no host /
/// relay / source-of-truth roles. Per decision #4.
struct SyncSettingsView: View {
    @ObservedObject var syncState: SyncState
    @State private var simulatedOffline: Bool = false
    @State private var simulatedPending: Bool = false

    @Environment(\.theme) private var theme

    var body: some View {
        Form {
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
            ForEach(MockPeers.connected) { peer in
                peerRow(peer, online: true)
            }
        } header: {
            Text("\(MockPeers.connected.count) devices")
        }
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
            ForEach(MockPeers.connected) { peer in
                peerRow(peer, online: false)
            }
        } header: {
            Text("0 reachable")
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
