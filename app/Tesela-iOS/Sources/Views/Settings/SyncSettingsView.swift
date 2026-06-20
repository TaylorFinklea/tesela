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
    /// Live iOS-side relay state — own cursors, last tick, errors.
    /// `nil` when the host (Settings reached from a flow that doesn't
    /// have the ticker yet, e.g. a standalone preview) hasn't passed
    /// one in. UI degrades gracefully when nil.
    @ObservedObject var relayTicker: RelayTicker
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
            deviceNameSection
            ffiSmokeSection
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
    /// Live WAN relay surface — shows BOTH iPhone's own ticker state
    /// (lastTickAt, applied/sent counts, inbound cursor, errors) AND
    /// the Mac's relay status when reachable. The honest framing now
    /// that iOS is a real sync peer: this iPhone talks to the relay
    /// directly; the Mac does the same; the relay is the shared
    /// rendezvous.
    @ViewBuilder
    private var relaySection: some View {
        Section {
            iphoneRelayPanel
            if !relayLoaded {
                HStack(spacing: 10) {
                    ProgressView().scaleEffect(0.7)
                    Text("Checking the Mac…")
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                }
            } else if let status = relayStatus, status.configured {
                Divider().padding(.vertical, 4)
                macRelayPanel(status)
            } else if let status = relayStatus, !status.configured {
                Divider().padding(.vertical, 4)
                relayUnconfigured
            }
        } header: {
            Text("WAN Relay")
        } footer: {
            Text("Your iPhone is a real sync peer — edits go through both direct HTTP (LAN-fast) and the relay (cellular-tolerant). Reads fall back to a local sandbox when the Mac is unreachable. The Mac panel below shows the Mac's view of the same relay; both should be ticking when sync is healthy.")
                .font(.caption2)
        }
    }

    /// iPhone-side relay state — pulled from RelayTicker's @Published
    /// fields. Always rendered; this is where the user sees that THIS
    /// device is talking to the relay.
    @ViewBuilder
    private var iphoneRelayPanel: some View {
        let healthy = relayTicker.lastError == nil && relayTicker.lastTickAt != nil
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Image(systemName: "iphone")
                    .foregroundStyle(theme.fgFaint)
                Text("This iPhone")
                    .font(.system(size: 13, weight: .semibold))
                Spacer()
                Circle()
                    .fill(relayTicker.lastError != nil
                        ? theme.typeTask
                        : (relayTicker.isRunning ? theme.typeQuery : theme.accentPrimary))
                    .frame(width: 8, height: 8)
                Text(relayTicker.lastError != nil
                    ? "error"
                    : (relayTicker.isRunning ? (healthy ? "syncing" : "starting") : "paused"))
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }
            if let err = relayTicker.lastError {
                VStack(alignment: .leading, spacing: 4) {
                    Text(err)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(theme.typeTask)
                    if relayTicker.consecutiveErrors > 1 {
                        Text("Backing off — \(relayTicker.consecutiveErrors) consecutive failures")
                            .font(.system(size: 10, design: .monospaced))
                            .foregroundStyle(theme.fgFaint)
                    }
                }
                .padding(8)
                .background(theme.typeTask.opacity(0.1))
                .clipShape(RoundedRectangle(cornerRadius: 4))
            }
            VStack(alignment: .leading, spacing: 4) {
                relayMetricRow("Last tick", relativeTime(relayTicker.lastTickAt.map { Int64($0.timeIntervalSince1970) }))
                relayMetricRow("Last received", "\(relayTicker.lastApplied) op\(relayTicker.lastApplied == 1 ? "" : "s")")
                relayMetricRow("Last sent",     "\(relayTicker.lastSent) op\(relayTicker.lastSent == 1 ? "" : "s")")
                relayMetricRow("Inbound seq",   "\(relayTicker.inboundCursorSeq)")
                relayMetricRow("APNs push",     relayTicker.apnsNote)
            }
        }
        .padding(.vertical, 4)
    }

    /// Mac-side view — what the Mac is doing with the same relay.
    /// Useful for debugging "where's my edit?" — if iPhone shows it
    /// sent but Mac's inbound hasn't moved, the relay or Mac is the
    /// problem; if iPhone's last tick is stale, iPhone is the problem.
    @ViewBuilder
    private func macRelayPanel(_ s: RelayStatusInfo) -> some View {
        let healthy = s.last_error == nil && s.last_poll_at != nil
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Image(systemName: "desktopcomputer")
                    .foregroundStyle(theme.fgFaint)
                Text("Mac")
                    .font(.system(size: 13, weight: .semibold))
                Spacer()
                Circle()
                    .fill(s.last_error != nil
                        ? theme.typeTask
                        : (s.last_poll_at != nil ? theme.typeQuery : theme.accentPrimary))
                    .frame(width: 8, height: 8)
                Text(healthy ? "connected" : (s.last_error != nil ? "error" : "idle"))
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
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
                relayMetricRow("Last put",  relativeTime(s.last_put_at))
                relayMetricRow("Inbound seq", "\(s.inbound_cursor)")
            }
        }
        .padding(.vertical, 4)
    }

    /// Mosaic-style sandbox root: `Documents/sync-ios-mosaic/`. The
    /// engine materializes received NoteUpserts to
    /// `<root>/notes/<slug>.md`, exactly like the Mac. Path is shared
    /// by every coordinator we build so successive taps see the same
    /// engine state.
    private func iosMosaicRoot() -> String {
        let docs = FileManager.default.urls(
            for: .documentDirectory,
            in: .userDomainMask
        )[0]
        let root = docs.appendingPathComponent("sync-ios-mosaic")
        try? FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        return root.path
    }

    /// Build (engine, relay, coordinator) tuple from the live pairing
    /// code. Centralizes the dance both smoke buttons need so they
    /// stay in sync about engine path, mosaic dir, and group identity.
    private func buildB3Coordinator() async throws -> (SyncEngineHandle, RelayClientHandle, SyncCoordinator, String) {
        let server = try await mosaic.fetchPairingCode()
        let pairing = try decodePairingCode(code: server.code)
        guard let relayURL = pairing.relayUrl ?? relayStatus?.url else {
            throw FfiSyncError.Other(message: "no relay URL on Mac")
        }
        let mosaicRoot = iosMosaicRoot()
        let deviceHex = persistedDeviceIdHex()
        // Loro is the sole engine post-flag-day; the legacy SQLite
        // constructors were removed. open_loro materializes + drives the
        // relay with the v2 payload.
        let engine = try await SyncEngineHandle.openLoro(
            mosaicPath: mosaicRoot,
            deviceIdHex: deviceHex
        )
        let relay = try RelayClientHandle(
            relayUrl: relayURL,
            groupIdHex: pairing.groupIdHex,
            deviceIdHex: deviceHex,
            groupKeyHex: pairing.groupKeyHex
        )
        _ = try await relay.registerOrRecover()
        try await relay.verifyRegistration()
        let coordinator = try SyncCoordinator(
            engine: engine,
            relay: relay,
            groupIdHex: pairing.groupIdHex
        )
        return (engine, relay, coordinator, mosaicRoot)
    }

    // ─── B.2 — FFI producer smoke ───────────────────────────────────
    //
    // Proves iOS can record a local op + push it to the relay, where
    // the Mac picks it up via its existing inbound tick.
    //
    // Flow on tap:
    //   1. Fetch the Mac's pairing code over HTTP → decode → grab the
    //      real group_id + group_key the Mac is using.
    //   2. Open a stable local SyncEngine at
    //      Documents/sync-ios-b2.db (so successive taps accumulate ops).
    //   3. Build a RelayClientHandle pointed at the relay URL the
    //      pairing code carries (falls back to the Mac's read-only
    //      relay status if the code is v1).
    //   4. Construct a SyncCoordinator over those.
    //   5. Record a `NoteUpsert` whose title contains the current local
    //      time so each tap creates a distinct, visible note.
    //   6. tick_outbound(max_bytes: 1 MB) — engine produces, postcards,
    //      coordinator wraps + AEAD-seals + PUTs.
    //   7. Render the outcome.
    //
    // After the tap, check the Mac side: the note "iOS B.2 smoke @ HH:MM:SS"
    // should appear in the mosaic within `poll_interval` seconds (the
    // Mac's relay tick pulls it down + applies).
    //
    // Replaced when B.3 lands with the real iOS-as-peer UX.
    @ViewBuilder
    private var ffiSmokeSection: some View {
        Section {
            Button {
                Task { await runB2Smoke() }
            } label: {
                HStack {
                    if smokeRunning {
                        ProgressView().scaleEffect(0.7)
                        Text("Working…")
                    } else {
                        Image(systemName: "paperplane")
                        Text("Push fake edit (B.2)")
                    }
                }
            }
            .disabled(smokeRunning)
            Button {
                Task { await runB3Pull() }
            } label: {
                HStack {
                    Image(systemName: "arrow.down.circle")
                    Text("Pull + apply from relay (B.3)")
                }
            }
            .disabled(smokeRunning)
            if let result = smokeResult {
                Text(result)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(result.hasPrefix("✅") ? theme.typeQuery : theme.typeTask)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.vertical, 4)
            }
        } header: {
            Text("B.2 / B.3 — FFI smoke (dev)")
        } footer: {
            Text("Push: create + send a note from iPhone. Pull: drain relay envelopes into the local engine; received NoteUpserts materialize at Documents/sync-ios-mosaic/notes/<slug>.md. Both use the same Mac pairing code (auto-fetched), so they target the same group.")
                .font(.caption2)
        }
    }

    /// B.2 producer flow: build coordinator → record a NoteUpsert →
    /// tick_outbound. Mac picks the note up via its existing inbound
    /// tick within ~5 s.
    private func runB2Smoke() async {
        smokeRunning = true
        defer { smokeRunning = false }
        smokeResult = nil

        do {
            let (engine, _, coordinator, _) = try await buildB3Coordinator()

            let formatter = DateFormatter()
            formatter.dateFormat = "HH:mm:ss"
            let title = "iOS B.2 smoke @ \(formatter.string(from: Date()))"
            let noteIdHex = UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
            let createdAt = Int64(Date().timeIntervalSince1970 * 1000)
            let slug = "ios-smoke-\(noteIdHex.prefix(8))"
            let body = """
                ---
                title: "\(title)"
                tags: []
                ---
                - Sent from iPhone via UniFFI → relay → Mac.
                - Timestamp: \(Date())
                """
            _ = try await engine.recordNoteUpsert(
                noteIdHex: noteIdHex,
                displayAlias: slug,
                title: title,
                content: body,
                createdAtMillis: createdAt
            )

            let outcome = try await coordinator.tickOutbound(maxBytes: 1_000_000)
            let seqStr = outcome.relaySeq.map { "seq=\($0)" } ?? "seq=—"
            smokeResult = """
                ✅ pushed \(outcome.opsSent) op\(outcome.opsSent == 1 ? "" : "s")
                  title: \(title)
                  relay: \(seqStr)
                  cursor: \(outcome.newCursorNtp.map(String.init) ?? "—")
                  → check the Mac for the note
                """
        } catch let err as FfiSyncError {
            smokeResult = "❌ \(err.localizedDescription)"
        } catch {
            smokeResult = "❌ \(error.localizedDescription)"
        }
    }

    /// B.3 consumer flow: build coordinator → tick_inbound → list the
    /// resulting materialized files in the iOS sandbox so the operator
    /// can confirm the apply actually wrote .md files locally.
    private func runB3Pull() async {
        smokeRunning = true
        defer { smokeRunning = false }
        smokeResult = nil

        do {
            let (_, _, coordinator, mosaicRoot) = try await buildB3Coordinator()
            let outcome = try await coordinator.tickInbound()

            // Inspect the materialized notes directory for confirmation.
            let notesDir = URL(fileURLWithPath: mosaicRoot).appendingPathComponent("notes")
            let files = (try? FileManager.default.contentsOfDirectory(atPath: notesDir.path)) ?? []
            let mdFiles = files.filter { $0.hasSuffix(".md") }.sorted()
            let preview = mdFiles.prefix(3).joined(separator: ", ")
            let extra = mdFiles.count > 3 ? ", …(\(mdFiles.count - 3) more)" : ""
            smokeResult = """
                ✅ applied \(outcome.applied) (skipped own: \(outcome.skippedOwn), errors: \(outcome.errors))
                  inbound seq: \(outcome.newCursorSeq)
                  local notes/: \(mdFiles.count) file\(mdFiles.count == 1 ? "" : "s")
                  preview: \(preview.isEmpty ? "—" : preview)\(extra)
                """
        } catch let err as FfiSyncError {
            smokeResult = "❌ \(err.localizedDescription)"
        } catch {
            smokeResult = "❌ \(error.localizedDescription)"
        }
    }

    /// One-shot device id per install, persisted in UserDefaults. Using
    /// a stable id keeps the engine's HLC monotonic across taps + app
    /// restarts (otherwise every B.2 smoke run would look like a
    /// "fresh device" to the relay).
    private func persistedDeviceIdHex() -> String {
        let key = "b2.engine.deviceIdHex"
        if let existing = UserDefaults.standard.string(forKey: key) {
            return existing
        }
        let fresh = generateDeviceIdHex()
        UserDefaults.standard.set(fresh, forKey: key)
        return fresh
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

}
