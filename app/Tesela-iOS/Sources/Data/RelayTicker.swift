import Foundation
import Combine

/// Background poll loop driving the iOS-side WAN relay sync.
///
/// Lives at the app shell level (one instance for the app's lifetime),
/// owns a single `(SyncEngineHandle, RelayClientHandle, SyncCoordinator)`
/// tuple, and ticks `tick_outbound + tick_inbound` every
/// `tickIntervalSeconds`. Mac-originated edits arrive on iPhone within
/// a tick interval; iPhone-originated edits drain to the relay in the
/// same window.
///
/// Lifecycle:
/// - `start()` on app foreground (scene phase `.active`).
/// - `stop()` on background (saves battery; iOS would suspend us anyway).
/// - On any tick error (network drop, relay 5xx, Mac pairing not yet
///   set up) the coordinator is torn down + rebuilt next tick from
///   the Mac's pairing code. That's an HTTP call back to the Mac, so
///   when the Mac is unreachable (cellular without Tailscale) the
///   rebuild fails, surfaces in `lastError`, and the next tick retries.
///
/// Once the data layer (B.3.4/B.3.5) reads notes from the local
/// materialized files instead of HTTP-to-Mac, the "Mac unreachable
/// → can't sync" failure mode goes away entirely; the relay path is
/// already independent of Mac reachability after first pairing.
@MainActor
final class RelayTicker: ObservableObject {
    /// Wall-clock of the most recent successful tick (either direction).
    @Published private(set) var lastTickAt: Date? = nil
    /// Most recent error string from a failing tick; cleared on next
    /// successful tick. UI renders this as a transient banner.
    @Published private(set) var lastError: String? = nil
    /// Ops applied on the last inbound tick (0 ≡ relay had nothing
    /// new since our last poll).
    @Published private(set) var lastApplied: UInt32 = 0
    /// Ops sent on the last outbound tick (0 ≡ engine had nothing
    /// new authored since the last push).
    @Published private(set) var lastSent: UInt32 = 0
    /// Relay seq we've applied up to. Surfaces "we're at seq N" so
    /// the user can compare with the Mac's outbound cursor.
    @Published private(set) var inboundCursorSeq: Int64 = 0
    /// Is the ticker actively looping? False between `stop()` and
    /// the next `start()`.
    @Published private(set) var isRunning: Bool = false

    // Owned FFI handles. nil until the first successful `ensure()`;
    // dropped on tick error so the next tick rebuilds. Caching keeps
    // the HTTP-to-Mac fetch (for the pairing code) to once per app
    // session in the happy path.
    private var engine: SyncEngineHandle? = nil
    private var relay: RelayClientHandle? = nil
    private var coordinator: SyncCoordinator? = nil

    private var loopTask: Task<Void, Never>? = nil

    /// Pull source — same MockMosaicService used by the rest of the
    /// app. Set via [`connect(mosaic:)`] after the SwiftUI shell has
    /// constructed its @StateObjects (the ticker is itself a
    /// @StateObject, so it needs a synchronous initial value before
    /// it can be given the real mosaic reference).
    private var mosaic: MockMosaicService? = nil
    /// Tick cadence. 5 s matches the Mac's relay tick by default; UI
    /// can swap in a longer interval later when battery becomes a
    /// concern (e.g. 30 s when the screen is off).
    private let tickIntervalSeconds: UInt64

    /// Callback fired whenever a tick applied ≥1 incoming op. Hosts
    /// hook this up to nudge the iOS UI to re-render (typically by
    /// calling `MockMosaicService.refresh(from:)` so HTTP fetches the
    /// freshly-arrived data from the Mac). On Wi-Fi this gives near-
    /// realtime "edits on Mac → visible on iPhone" feel; on cellular
    /// where HTTP-to-Mac fails, the data lives in the local materialized
    /// sandbox until B.3.4 swaps the read path to local-first.
    var onAppliedChanges: (() -> Void)? = nil

    init(tickIntervalSeconds: UInt64 = 5) {
        self.tickIntervalSeconds = tickIntervalSeconds
    }

    /// Late-bind the mosaic this ticker uses to fetch pairing codes.
    /// Idempotent — calling repeatedly with the same reference is a
    /// no-op. Calling with a *different* reference replaces it but
    /// does NOT tear down the coordinator (which is keyed to the
    /// already-pulled group identity, not the mosaic per se).
    func connect(mosaic: MockMosaicService) {
        self.mosaic = mosaic
    }

    func start() {
        guard loopTask == nil else { return }
        isRunning = true
        loopTask = Task { [weak self] in
            await self?.runLoop()
        }
    }

    func stop() {
        loopTask?.cancel()
        loopTask = nil
        isRunning = false
    }

    private func runLoop() async {
        while !Task.isCancelled {
            await tickOnce()
            // Plain Task.sleep — when the app goes background iOS
            // suspends us; on resume we wake up where we left off. No
            // need for a separate scene-phase observer beyond start/stop.
            do {
                try await Task.sleep(nanoseconds: tickIntervalSeconds * 1_000_000_000)
            } catch {
                // Task cancelled mid-sleep — exit cleanly.
                return
            }
        }
    }

    /// Single tick: ensure coordinator → outbound → inbound. Any
    /// thrown error drops the coordinator + surfaces via `lastError`.
    private func tickOnce() async {
        do {
            if coordinator == nil {
                try await ensureCoordinator()
            }
            guard let coordinator else { return }
            let outbound = try await coordinator.tickOutbound(maxBytes: 1_000_000)
            let inbound = try await coordinator.tickInbound()
            lastSent = outbound.opsSent
            lastApplied = inbound.applied
            inboundCursorSeq = inbound.newCursorSeq
            lastTickAt = Date()
            lastError = nil
            if inbound.applied > 0 {
                // Tell the host UI that new data has landed in the
                // local engine + sandbox. AppShell wires this to a
                // MockMosaicService.refresh() so the page the user is
                // looking at updates without a manual pull-down.
                onAppliedChanges?()
            }
        } catch let err as FfiSyncError {
            lastError = err.localizedDescription
            dropCoordinator()
        } catch {
            lastError = error.localizedDescription
            dropCoordinator()
        }
    }

    /// (Re)pair: fetch the Mac's pairing code over HTTP, decode it,
    /// open the local engine with a sandbox mosaic root, instantiate
    /// the relay client + coordinator. Caches the result so subsequent
    /// ticks reuse the same handles.
    private func ensureCoordinator() async throws {
        guard let mosaic else {
            throw FfiSyncError.Other(message: "ticker not connected to mosaic")
        }
        let server = try await mosaic.fetchPairingCode()
        let pairing = try decodePairingCode(code: server.code)
        guard let relayURL = pairing.relayUrl else {
            throw FfiSyncError.Other(message: "Mac has no relay configured")
        }

        let docs = FileManager.default.urls(
            for: .documentDirectory,
            in: .userDomainMask
        )[0]
        let mosaicRoot = docs.appendingPathComponent("sync-ios-mosaic")
        try? FileManager.default.createDirectory(
            at: mosaicRoot,
            withIntermediateDirectories: true
        )
        let sqliteURL = "sqlite:\(mosaicRoot.path)/sync.db"
        let deviceHex = Self.persistedDeviceIdHex()

        let engine = try await SyncEngineHandle.openWithMosaic(
            sqliteUrl: sqliteURL,
            mosaicPath: mosaicRoot.path,
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
        self.engine = engine
        self.relay = relay
        self.coordinator = coordinator
    }

    private func dropCoordinator() {
        coordinator = nil
        engine = nil
        relay = nil
    }

    /// Stable per-install device id, persisted across launches. iOS's
    /// HLC monotonicity depends on the device id staying constant; a
    /// fresh id every launch would look like a fresh device to peers.
    static func persistedDeviceIdHex() -> String {
        let key = "b2.engine.deviceIdHex"
        if let existing = UserDefaults.standard.string(forKey: key) {
            return existing
        }
        let fresh = generateDeviceIdHex()
        UserDefaults.standard.set(fresh, forKey: key)
        return fresh
    }
}
