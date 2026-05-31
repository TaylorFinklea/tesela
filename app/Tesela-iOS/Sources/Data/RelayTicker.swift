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
    /// Base tick cadence. 5 s matches the Mac's relay tick by default.
    /// Exponential backoff multiplies this on consecutive errors so we
    /// don't hammer a flaky/down relay (or drain battery on cellular
    /// when there's nothing to do).
    private let tickIntervalSeconds: UInt64
    /// Count of consecutive failed ticks. Resets to 0 on each success.
    /// Used to compute the next sleep via `2^min(consecutiveErrors, 6)`,
    /// capped so the loop wakes at least once a minute regardless.
    /// Published so the UI can show "retrying in 30s" or similar.
    @Published private(set) var consecutiveErrors: UInt32 = 0
    /// Max backoff cap, in seconds. Even after lots of consecutive
    /// failures, the loop still wakes every `tickIntervalSeconds * cap`
    /// to retry.
    private let maxBackoffMultiplier: UInt32 = 12  // → 60s when base is 5s
    /// Persisted-cursor UserDefaults keys. Scoped to the device id so
    /// switching mosaics (future) doesn't replay another mosaic's
    /// cursors over the new one.
    private static let inboundCursorKey = "relay.inboundCursorSeq"
    private static let outboundCursorKey = "relay.outboundCursorNtp"
    /// Cached pairing code (the long base64url blob from
    /// `/sync/peer/pairing-code`). Once we've fetched this from the Mac
    /// successfully, we reuse it forever — it encodes the stable group
    /// identity + relay URL, none of which changes across sessions.
    /// Without the cache, every coordinator rebuild after a tick error
    /// required Mac to be reachable over direct HTTP, which made the
    /// relay (whose whole purpose is to NOT need Mac reachable!)
    /// uselessly dependent on Mac's network.
    private static let pairingCodeKey = "relay.cachedPairingCode"

    /// Callback fired whenever a tick applied ≥1 incoming op. Hosts
    /// hook this up to nudge the iOS UI to re-render (typically by
    /// calling `MockMosaicService.refresh(from:)` so HTTP fetches the
    /// freshly-arrived data from the Mac). On Wi-Fi this gives near-
    /// realtime "edits on Mac → visible on iPhone" feel; on cellular
    /// where HTTP-to-Mac fails, the data lives in the local materialized
    /// sandbox until B.3.4 swaps the read path to local-first.
    var onAppliedChanges: (() -> Void)? = nil

    init(tickIntervalSeconds: UInt64 = 2) {
        // Default 2 s in the foreground — keeps Web→iOS lag close to
        // instant on a healthy network without thrashing battery (the
        // tick body is just a relay GET if there's nothing new). On
        // consecutive errors the exponential backoff still caps the
        // poll at ~60 s so a down relay doesn't drain the device.
        self.tickIntervalSeconds = tickIntervalSeconds
    }

    /// Late-bind the mosaic this ticker uses to fetch pairing codes.
    /// Idempotent — calling repeatedly with the same reference is a
    /// no-op. Calling with a *different* reference replaces it but
    /// does NOT tear down the coordinator (which is keyed to the
    /// already-pulled group identity, not the mosaic per se).
    ///
    /// When this is the FIRST connect (the ticker was running with a
    /// nil mosaic — typical on app launch where scenePhase.active
    /// fires `start()` before AppShell's `.task` reaches `connect()`),
    /// reset the consecutive-error counter and kick an immediate tick.
    /// Without this, the ticker would still be in its backoff sleep
    /// for 30 s+ and the user would think sync is broken.
    func connect(mosaic: MockMosaicService) {
        let wasUnconnected = (self.mosaic == nil)
        self.mosaic = mosaic
        if wasUnconnected {
            // Clear the "no mosaic" stalls so the next tick reads
            // green instead of "backing off" in Settings → Sync.
            consecutiveErrors = 0
            lastError = nil
            if isRunning {
                Task { await self.tickOnce() }
            }
        }
    }

    /// Record an iOS-authored edit to the local engine and (if the
    /// coordinator is ready) trigger an immediate outbound tick so the
    /// op hits the relay quickly. The engine open is **not gated on
    /// network reachability** — if the user is offline, the write still
    /// lands durably in SQLite and the on-disk materialized file; the
    /// next successful tick drains the buffered op to the relay.
    ///
    /// `slug` is the note's id (today's daily ID for today, or the
    /// page's slug otherwise). `content` is the full markdown body
    /// the user wants on disk. `createdAtMillis` should be stable
    /// across edits of the same note (the daily's creation timestamp
    /// at midnight, for example) so the engine HLC stays monotonic.
    ///
    /// This is what makes iOS writes survive a force-close while
    /// offline — the engine handle opens at app launch without
    /// touching the network, so the write reaches SQLite even on
    /// the first edit of a brand-new install that hasn't paired yet.
    func recordAndPush(slug: String, title: String, content: String, createdAtMillis: Int64) async {
        // Ensure the engine is open. This is purely local (SQLite +
        // sandbox path), so it succeeds even when the relay/Mac is
        // unreachable. If it can't even open SQLite, surface the error
        // but don't pretend the write succeeded.
        do {
            try await openEngineIfNeeded()
        } catch {
            lastError = error.localizedDescription
            return
        }
        guard let engine else {
            // openEngineIfNeeded set lastError already in the catch
            // above; this is the can't-happen-but-be-safe branch.
            return
        }
        do {
            // Phase 2 (sync redesign 2026-05-26): use the block-granular
            // diff path instead of `recordNoteUpsertBySlug`. The engine
            // reads the previously-materialized file from disk, parses
            // both bodies into NoteTrees, emits BlockUpsert/Move/Delete
            // ops for what actually changed, and materializes the
            // updated file as a side effect. Concurrent edits to
            // different blocks of the same note now converge correctly
            // on the relay receiver instead of stomping each other via
            // wholesale NoteUpsert apply.
            _ = try await engine.recordNoteDiff(
                slug: slug,
                newContent: content,
                title: title,
                createdAtMillis: createdAtMillis
            )
        } catch {
            lastError = error.localizedDescription
            return
        }
        // Engine durability is now guaranteed. Best-effort push: if the
        // coordinator is ready (i.e. we've paired with the Mac at least
        // once), drain the op to the relay immediately so the other
        // side sees it without waiting a full tick. If pairing hasn't
        // happened or the network is down, the regular tick loop will
        // catch up later.
        if coordinator == nil {
            try? await ensureCoordinator()
        }
        guard let coordinator else { return }
        do {
            let outcome = try await coordinator.tickOutbound(maxBytes: 1_000_000)
            lastSent = outcome.opsSent
            lastTickAt = Date()
            lastError = nil
        } catch {
            lastError = error.localizedDescription
        }
    }

    /// Apply a TLR2-framed Loro delta that arrived over the live WS
    /// (instant-multidevice spec, Phase C). Mediates the engine the
    /// `LiveSyncSocket` deliberately does not own: ensure the engine is
    /// open, apply the frame (commutative + idempotent — a delta the
    /// engine already has is a harmless no-op), and on ≥1 applied
    /// update reuse the same inbound-refresh seam the relay tick uses
    /// (`onAppliedChanges`) so the affected note's view freshens. The
    /// delta is NOT re-broadcast — the server owns fan-out; the phone
    /// only applies what it receives. Returns whether ≥1 update applied.
    @discardableResult
    func applyInboundDelta(_ frame: Data) async -> Bool {
        do {
            try await openEngineIfNeeded()
        } catch {
            lastError = error.localizedDescription
            return false
        }
        guard let engine else { return false }
        let applied: UInt32
        do {
            applied = try await engine.applyDeltaFrame(frame: frame)
        } catch {
            lastError = error.localizedDescription
            return false
        }
        guard applied > 0 else { return false }
        // Same refresh path the relay inbound tick uses — keeps the UI
        // update logic in one place.
        onAppliedChanges?()
        return true
    }

    /// Produce the live (cursor-free) TLR2 delta frame for a just-edited
    /// note so the host can push it over the WS (instant-multidevice
    /// spec, Phase C). Reads the engine state AS-IS — it does NOT record
    /// the edit; the caller must have already recorded it (via
    /// `recordAndPush`) so the engine holds the change before this exports
    /// it. `since_vv = nil` exports the full-state snapshot (bidirectional
    /// VV catch-up is Phase D). Returns `nil` when the doc isn't resident
    /// (nothing to send) or the engine can't open.
    func produceDeltaFrame(slug: String) async -> Data? {
        do {
            try await openEngineIfNeeded()
        } catch {
            lastError = error.localizedDescription
            return nil
        }
        guard let engine else { return nil }
        do {
            return try await engine.produceNoteDelta(slug: slug, sinceVv: nil)
        } catch {
            lastError = error.localizedDescription
            return nil
        }
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
            // Backoff: on consecutive errors, sleep longer between
            // ticks (capped). Successful tick resets to base cadence.
            // Doubles per error up to maxBackoffMultiplier (~60s when
            // base is 5s) so a flaky relay doesn't keep us hot-looping.
            let multiplier = UInt64(min(consecutiveErrors, maxBackoffMultiplier))
            let sleepSecs = tickIntervalSeconds * (1 << multiplier)
            do {
                try await Task.sleep(nanoseconds: sleepSecs * 1_000_000_000)
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
            consecutiveErrors = 0
            // Persist cursors so a cold launch resumes where we left
            // off instead of re-polling the full relay history.
            UserDefaults.standard.set(inbound.newCursorSeq, forKey: Self.inboundCursorKey)
            if let ntp = outbound.newCursorNtp {
                UserDefaults.standard.set(ntp, forKey: Self.outboundCursorKey)
            }
            if inbound.applied > 0 {
                // Tell the host UI that new data has landed in the
                // local engine + sandbox. AppShell wires this to a
                // MockMosaicService.refresh() so the page the user is
                // looking at updates without a manual pull-down.
                onAppliedChanges?()
            }
        } catch let err as FfiSyncError {
            lastError = err.localizedDescription
            consecutiveErrors = consecutiveErrors &+ 1
            dropCoordinator()
        } catch {
            lastError = error.localizedDescription
            consecutiveErrors = consecutiveErrors &+ 1
            dropCoordinator()
        }
    }

    /// Path to the iOS sandbox mosaic root. Stable across launches;
    /// the engine + the MockMosaicService local-fallback both read
    /// from here so iOS-authored writes are visible to local reads
    /// even before any pairing has happened.
    private static func mosaicRootURL() -> URL {
        let docs = FileManager.default.urls(
            for: .documentDirectory,
            in: .userDomainMask
        )[0]
        return docs.appendingPathComponent("sync-ios-mosaic")
    }

    /// Open the local engine if not already open. **Network-free** —
    /// only needs SQLite + a stable device id, both of which are
    /// available on cold launch regardless of reachability. Callers
    /// can invoke this at app start so iOS writes are durable from
    /// the very first edit, even before any pairing succeeds.
    ///
    /// Idempotent — subsequent calls are no-ops once the engine is
    /// open. The handle stays alive across coordinator rebuilds: a
    /// flaky relay tearing down its coordinator must not also nuke
    /// the engine, or every transient WAN error would orphan in-
    /// flight local writes.
    func openEngineIfNeeded() async throws {
        if engine != nil { return }
        let mosaicRoot = Self.mosaicRootURL()
        try? FileManager.default.createDirectory(
            at: mosaicRoot,
            withIntermediateDirectories: true
        )
        let deviceHex = Self.persistedDeviceIdHex()
        // Loro cutover: the iOS engine is now the authoritative LoroEngine.
        // It materializes <mosaic>/notes/<slug>.md (the read path is
        // unchanged — the data layer still reads those files) and drives
        // the relay with the v2 (TLR2) Loro payload. Per-note snapshots
        // live under <mosaic>/.tesela/loro/ for fast cold launches. No
        // sqlite db; SqliteEngine is bypassed.
        let opened = try await SyncEngineHandle.openLoro(
            mosaicPath: mosaicRoot.path,
            deviceIdHex: deviceHex
        )
        self.engine = opened
    }

    /// (Re)pair: build the relay client + coordinator on top of the
    /// already-open engine. **Uses a cached pairing code when one is
    /// available**, falling back to HTTP only on the very first pair
    /// or after an auth failure that invalidates the cache. This is
    /// what makes the relay tick truly resilient to Mac being
    /// unreachable: once we've paired once on any network we can
    /// reach Mac on, we can keep talking to the relay forever
    /// regardless of Mac's HTTP reachability.
    private func ensureCoordinator() async throws {
        guard let mosaic else {
            // The ticker outran the host's `.task` setup — scenePhase
            // becoming .active fires `start()` before AppShell's
            // `.task` body has progressed past its initial HTTP
            // refresh to reach `connect(mosaic:)`. Throwing here
            // marks `lastError` + advances `consecutiveErrors`, which
            // tips the backoff into a multi-minute sleep window and
            // leaves the user staring at "Backing off — N consecutive
            // failures" in Settings → Sync. Silent no-op instead;
            // `connect(mosaic:)` will reset the counter and kick a
            // fresh tick the moment the host wires us up.
            return
        }
        try await openEngineIfNeeded()
        guard let engine else {
            throw FfiSyncError.Other(message: "engine open failed")
        }

        // Try the cached pairing code first. If we have one, the
        // path below skips the Mac HTTP fetch entirely.
        let cached = UserDefaults.standard.string(forKey: Self.pairingCodeKey)
        do {
            let codeStr: String
            if let cached {
                codeStr = cached
            } else {
                // No cache yet — must fetch from Mac. This is the only
                // network call that requires Mac to be HTTP-reachable.
                let server = try await mosaic.fetchPairingCode()
                codeStr = server.code
            }
            try await buildCoordinator(engine: engine, codeStr: codeStr)
            // Survived the build → cache the code for future ticks.
            if cached == nil {
                UserDefaults.standard.set(codeStr, forKey: Self.pairingCodeKey)
            }
        } catch {
            // If the cached code is stale (group rotated, auth_key
            // mismatch, etc.), nuke it and let the next tick refetch
            // from Mac. Don't recurse here — surface the error and
            // wait for the next tick so we don't infinitely retry on
            // a Mac that's actually unreachable.
            if cached != nil {
                UserDefaults.standard.removeObject(forKey: Self.pairingCodeKey)
            }
            throw error
        }
    }

    /// Inner half of `ensureCoordinator`: decode `codeStr`, build the
    /// relay client + coordinator, restore persisted cursors. Pure —
    /// no HTTP to Mac.
    private func buildCoordinator(engine: SyncEngineHandle, codeStr: String) async throws {
        let pairing = try decodePairingCode(code: codeStr)
        guard let relayURL = pairing.relayUrl else {
            throw FfiSyncError.Other(message: "Mac has no relay configured")
        }
        let deviceHex = Self.persistedDeviceIdHex()
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
        if let inbound = UserDefaults.standard.object(forKey: Self.inboundCursorKey) as? Int64 {
            await coordinator.setInboundCursorSeq(seq: inbound)
            inboundCursorSeq = inbound
        }
        if let outbound = UserDefaults.standard.object(forKey: Self.outboundCursorKey) as? Int64 {
            await coordinator.setOutboundCursorNtp(ntp: outbound)
        }
        self.relay = relay
        self.coordinator = coordinator
    }

    /// Drop the coordinator + relay so the next tick rebuilds them.
    /// **Engine handle is preserved** — it's purely local and tied to
    /// SQLite state, not to any network identity. Dropping it on a
    /// transient relay error would orphan any local write that came in
    /// between this tick and the next.
    private func dropCoordinator() {
        coordinator = nil
        relay = nil
    }

    /// Drain any pending outbound ops to the relay synchronously,
    /// blocking until either every queued op has been acknowledged or
    /// the coordinator fails. Called by the host app right before a
    /// reconnect-triggered HTTP refresh — if the user made offline
    /// edits, those ops need to reach the relay before the refresh
    /// fetches the (still-stale) server state. Otherwise the refresh
    /// would land first and overwrite in-memory blocks with the
    /// pre-edit server view.
    ///
    /// Returns the number of ops that were sent. Zero means "nothing
    /// was pending" OR "we never managed to build a coordinator" —
    /// callers use the return only as a lower bound.
    func flushPendingOutbound() async -> UInt32 {
        if coordinator == nil {
            try? await ensureCoordinator()
        }
        guard let coordinator else { return 0 }
        do {
            let outcome = try await coordinator.tickOutbound(maxBytes: 1_000_000)
            lastSent = outcome.opsSent
            lastTickAt = Date()
            lastError = nil
            return outcome.opsSent
        } catch {
            lastError = error.localizedDescription
            return 0
        }
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
