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

    /// Hub-mode gate (multi-device convergence spec, Part E2). When the
    /// app is talking to a Mac server over the live `/ws` WebSocket, that
    /// socket is the sync hub. The relay coordinator loop (driven by the
    /// cached pairing code) shares the SAME engine handle as the WS path,
    /// so leaving it running injects stale foreign-history ops into the
    /// same Loro docs and mints duplicate TreeIDs. While `hubMode` is true
    /// the relay coordinator is gated off: `tickOnce()` no-ops and
    /// `recordAndPush` skips the coordinator build+tick (local durability
    /// via `recordNoteDiff` is preserved; the WS push runs in the caller).
    ///
    /// Reversible (invariant 5): this does NOT clear the cached pairing
    /// code, so setting `hubMode=false` rebuilds the coordinator from the
    /// cache on the next tick with no Mac HTTP fetch. On the transition to
    /// `true` we `dropCoordinator()` so any already-built/in-flight
    /// coordinator is torn down (R7) — otherwise an in-flight tick could
    /// still fire after the gate closes.
    var hubMode: Bool = false {
        didSet {
            if hubMode && !oldValue {
                dropCoordinator()
            }
        }
    }

    // Owned FFI handles. nil until the first successful `ensure()`;
    // dropped on tick error so the next tick rebuilds. Caching keeps
    // the HTTP-to-Mac fetch (for the pairing code) to once per app
    // session in the happy path.
    private var engine: SyncEngineHandle? = nil
    private var relay: RelayClientHandle? = nil
    private var coordinator: SyncCoordinator? = nil

    private var loopTask: Task<Void, Never>? = nil

    /// Per-note encoded version vector (Loro VV bytes from `noteVersion`) as
    /// of our most recent WS delta for that note (#150). `produceDeltaFrame`
    /// passes this as `sinceVv` so steady-state frames carry only the ops
    /// authored since the last push — a true DELTA — instead of re-shipping a
    /// full snapshot every keystroke.
    ///
    /// Part A (WS-push clobber, 2026-06-02): this is now SEEDED before the
    /// first push of a note, not left empty. `bootstrapNoteIfNeeded` captures
    /// the imported base VV here (first-view + resident catch-up), and
    /// `recordAndPush` captures a pre-edit VV floor when bootstrap couldn't
    /// (resident note, debounced/failed catch-up). Both seeds are taken BEFORE
    /// `recordNoteDiff` records the edit, so the edit is strictly past the
    /// baseline and IS included in the next delta (no under-send), while the
    /// first frame is a bounded delta — NOT a whole-note snapshot that could
    /// re-assert iOS's stale copy of a peer-edited block. The baseline then
    /// advances via `commitPushedDelta` after each confirmed send.
    ///
    /// Dropped-delta handling: this VV is advanced (via `commitPushedDelta`)
    /// ONLY after `sendDelta` confirms the frame reached a connected socket.
    /// A dropped send leaves the baseline back, so the next `produceDeltaFrame`
    /// re-includes the dropped ops — restoring the self-heal that a
    /// full-snapshot-per-keystroke gave for free before #150. (In hub mode the
    /// WS is the SOLE author→hub delivery path — `hubMode` gates the relay tick
    /// off and the iOS HTTP-PUT write path was removed — so we cannot rely on a
    /// relay fallback to re-deliver a dropped frame; the keep-the-VV-back
    /// behavior is what guarantees redelivery.) A continuously-open PEER that
    /// misses a frame still self-heals on note re-open via
    /// `bootstrapNoteIfNeeded` (full server-snapshot catch-up).
    private var lastPushedVV: [String: Data] = [:]

    /// Last successful/attempted catch-up time per slug, for the resident
    /// catch-up debounce in `bootstrapNoteIfNeeded`. Keeps the resident
    /// snapshot re-fetch to at most once per `catchupMinInterval` per slug
    /// so the catch-up doesn't fire on every keystroke / refresh tick.
    private var lastCatchupAt: [String: Date] = [:]
    /// Slugs with a forced authoritative re-base catch-up currently in flight
    /// (triggered by a PENDING inbound delta — `applyInboundDelta`). Collapses
    /// a burst of disjoint frames to ONE snapshot fetch per slug.
    private var forcedCatchupInFlight: Set<String> = []
    /// Minimum interval between resident catch-up snapshot fetches for the
    /// SAME slug. ~3s — long enough that a typing burst or a storm of
    /// `onNoteOpened`/refresh callbacks collapses to one fetch, short enough
    /// that a freshly web-authored block lands on the next open.
    private static let catchupMinInterval: TimeInterval = 3.0

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
    /// Persisted-cursor UserDefaults keys, scoped per (relay URL, group
    /// id) — both derived from the pairing code (audit A5). Relay seqs
    /// are a per-relay, per-group namespace restarting at 1, so a global
    /// cursor replayed against a DIFFERENT relay/group (re-pair, relay DB
    /// wipe, the HA→Cloudflare migration) both suppressed the snapshot
    /// bootstrap and made the tail poll start past every op — a silent,
    /// permanent inbound stall. A fresh identity now starts at 0, which
    /// makes `compactionSeq > inboundCursorSeq` true → snapshot bootstrap
    /// runs. The pre-scoping bare keys are migrated once (adopted by the
    /// first pairing that builds a coordinator post-upgrade, so in-place
    /// upgrades keep their progress) and then removed.
    static let legacyInboundCursorKey = "relay.inboundCursorSeq"
    static let legacyOutboundCursorKey = "relay.outboundCursorNtp"
    static func cursorScope(relayUrl: String, groupIdHex: String) -> String {
        "\(relayUrl)|\(groupIdHex)"
    }
    static func inboundCursorKey(scope: String) -> String {
        "relay.inboundCursorSeq.\(scope)"
    }
    static func outboundCursorKey(scope: String) -> String {
        "relay.outboundCursorNtp.\(scope)"
    }
    /// The (relay URL, group id) identity the live coordinator's cursors
    /// persist under. Set by `buildCoordinator`; nil while no coordinator.
    private var cursorScope: String? = nil
    /// The raw pairing code the live coordinator was built from. Compared
    /// against the cached code each tick so adopting a NEW code (re-pair)
    /// tears the old coordinator down instead of letting it keep ticking
    /// the previous group (audit A5).
    private var coordinatorPairingCode: String? = nil
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
    /// Give the engine the server's note doc as a **shared base** before
    /// this device authors locally. With the base resident, the next
    /// `recordNoteDiff` resolves its BlockUpserts to the server's existing
    /// tree nodes instead of minting rival TreeIDs, so concurrent edits
    /// converge instead of duplicating (multi-device convergence — Part D).
    ///
    /// First-view: import the server snapshot to establish the base.
    /// Resident: perform a CATCH-UP — re-fetch the server snapshot and
    /// re-import so the engine learns any server-side ops (e.g. a web-
    /// authored new block) it hasn't yet seen. `importNoteSnapshot`
    /// MERGES (Loro import is commutative + idempotent), so importing a
    /// full snapshot into a resident doc never clobbers a local-only op;
    /// it only adds what the engine lacks. This is the fix for "iOS never
    /// updates" — a resident-but-divergent daily previously skipped catch-up
    /// entirely and stayed locally stale.
    ///
    /// Catch-up is **debounced per slug** (`catchupMinInterval`) so the
    /// resident path doesn't fetch a full snapshot on every keystroke,
    /// refresh tick, or `onNoteOpened` callback — at most once per window
    /// per slug. The first-view (non-resident) import is NEVER debounced:
    /// a brand-new note must get its base immediately or live deltas can't
    /// materialize.
    ///
    /// Best-effort: any network/non-200 failure returns silently — the
    /// device keeps working without the catch-up (graceful degradation), and
    /// a later open/edit retries. The fetch+import is CRDT-safe under R3/R4:
    /// the import is idempotent and merges commutatively, so a catch-up that
    /// races a concurrent edit or imports a snapshot captured mid-edit never
    /// loses data — even mid-typing, the user's in-engine edit survives the
    /// merge.
    func bootstrapNoteIfNeeded(slug: String) async {
        guard let engine else { return }
        guard let mosaic else { return }
        let resident = await engine.noteVersion(slug: slug) != nil
        if resident {
            // Resident → catch-up, but debounced so we don't fetch a full
            // snapshot on every open/refresh/keystroke. Only the visible
            // note hits this path (callers pass the opened/daily slug).
            let now = Date()
            if let last = lastCatchupAt[slug],
               now.timeIntervalSince(last) < Self.catchupMinInterval {
                return
            }
            lastCatchupAt[slug] = now
        }
        do {
            guard let bytes = try await mosaic.fetchLoroSnapshot(slug: slug) else {
                return  // server has no doc for this slug yet (404)
            }
            try await engine.importNoteSnapshot(slug: slug, bytes: bytes)
            // Part A (WS-push clobber, 2026-06-02): seed the per-note push
            // baseline from the freshly-imported server base, so the FIRST
            // `produceDeltaFrame` for this note exports `updates(baseVV)` =
            // ONLY the ops iOS authors AFTER the base — never a full snapshot
            // that re-asserts iOS's (possibly stale) copy of blocks a peer
            // edited. `bootstrapNoteIfNeeded` always runs BEFORE
            // `recordNoteDiff` in `recordAndPush`, so the VV captured here is
            // strictly the base (pre-edit); the iOS edit that follows is past
            // it and is therefore INCLUDED in the next delta — no under-send.
            //
            // Seed only when ABSENT: an existing entry was set either by a
            // prior bootstrap this session or by `commitPushedDelta` after a
            // confirmed send. Both are valid floors at-or-ahead of this base;
            // overwriting with the (older) base VV could REGRESS a baseline
            // that `commitPushedDelta` advanced, re-shipping already-confirmed
            // ops — exactly the re-assertion we're eliminating. So leave any
            // existing entry alone (commitPushedDelta keeps it advancing).
            if lastPushedVV[slug] == nil {
                lastPushedVV[slug] = await engine.noteVersion(slug: slug)
            }
        } catch {
            // Graceful degradation: keep working without the base/catch-up.
            // Clear the debounce stamp so the next open retries promptly
            // rather than waiting out the full window after a failure.
            if resident { lastCatchupAt[slug] = nil }
            return
        }
        if resident {
            // The import re-materialized the note's file (engine side effect),
            // so nudge the UI to re-read it — same seam an inbound WS delta
            // uses. First-view import is followed by the caller's own refresh
            // path, so it doesn't need this.
            onAppliedChanges?()
        }
    }

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
        // Part D: pull the server's doc as a shared base before the first
        // local edit so this note's BlockUpserts resolve to the server's
        // existing tree nodes (no rival TreeIDs / duplicate bullets). No-op
        // once the doc is resident; best-effort otherwise.
        await bootstrapNoteIfNeeded(slug: slug)
        // Part A (WS-push clobber, 2026-06-02): if the note is resident from a
        // PRIOR session's local edits, `bootstrapNoteIfNeeded` early-returns
        // on the resident debounce (or its catch-up fetch failed), so it never
        // imported a base and never seeded `lastPushedVV[slug]`. Without a
        // baseline the FIRST `produceDeltaFrame` below would ship a FULL
        // SNAPSHOT of this device's (possibly stale) state — the clobber bug.
        // Capture the engine's CURRENT version vector as the push floor NOW,
        // BEFORE `recordNoteDiff` records this edit. Because the edit is
        // recorded after this point, it is strictly past the floor, so the
        // next `produceDeltaFrame` exports `updates(floor)` = this edit (and
        // any later ones) — the genuine edit is NEVER excluded (no under-send),
        // while the pre-existing resident state is NOT re-asserted. Seed only
        // when absent so we don't regress a baseline bootstrap already set.
        if lastPushedVV[slug] == nil {
            lastPushedVV[slug] = await engine.noteVersion(slug: slug)
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
        // Engine durability is now guaranteed. In hub mode the live `/ws`
        // socket owns delivery (the caller pushes a delta via
        // `produceDeltaFrame`/`sendDelta` after this returns), so the
        // relay coordinator must NOT also drain this op — doing so would
        // re-inject the same edit through the cached-pairing relay path
        // into the shared engine. Return BEFORE the coordinator block;
        // local durability above is intact (Part E2).
        if hubMode { return }
        // Best-effort push: if the coordinator is ready (i.e. we've paired
        // with the Mac at least once), drain the op to the relay
        // immediately so the other side sees it without waiting a full
        // tick. If pairing hasn't happened or the network is down, the
        // regular tick loop will catch up later.
        invalidateCoordinatorIfRepaired()
        if coordinator == nil {
            try? await ensureCoordinator()
        }
        guard let coordinator else { return }
        do {
            let outcome = try await coordinator.tickOutbound(maxBytes: 1_000_000)
            noteOutboundOutcome(outcome)
        } catch {
            lastError = error.localizedDescription
        }
    }

    /// Fold a `tickOutbound` outcome into the published status (audit A7).
    /// The FFI returns Ok even when relay PUTs failed — the failed batch
    /// just retries next tick — so the honesty lives HERE: a non-zero
    /// `batchesFailed` sets `lastError` (Settings → Sync goes red) instead
    /// of clearing it. `opsSent == 0` alone is indistinguishable from
    /// "nothing to send" and must not be treated as a failure.
    private func noteOutboundOutcome(_ outcome: TickOutboundRecord) {
        lastSent = outcome.opsSent
        lastTickAt = Date()
        if outcome.batchesFailed > 0 {
            lastError = outcome.lastError
                ?? "relay delivery failed (\(outcome.batchesFailed)/\(outcome.batchesAttempted) batches)"
        } else {
            lastError = nil
        }
    }

    /// Collab editing C1 outbound: record ONE in-block CHARACTER SPLICE
    /// (the user's actual keystroke) into the engine's per-block
    /// `LoroText` and drain it, mirroring `recordAndPush` but calling
    /// `engine.spliceBlockText(...)` instead of `recordNoteDiff(...)`.
    /// Because the splice goes through the `text_seq` sequence CRDT, two
    /// devices splicing the SAME block concurrently INTERLEAVE — the
    /// whole-text re-author path emitted DELETEs of the peer's chars.
    ///
    /// Same bootstrap-before-edit + push-floor sequence as `recordAndPush`
    /// (so the next `produceDeltaFrame` exports only this edit, never a
    /// full snapshot re-asserting stale state), and the same hub-mode gate
    /// (the live `/ws` socket owns delivery; the caller pushes the delta
    /// after this returns). The actual `produceDeltaFrame` → `sendDelta`
    /// → `commitPushedDelta` tail lives in `GrAppShell.onLocalSplice`,
    /// mirroring `onLocalWrite`.
    func spliceAndPush(
        slug: String,
        blockIdHex: String,
        utf16Offset: Int,
        utf16DeleteLen: Int,
        insert: String
    ) async {
        do {
            try await openEngineIfNeeded()
        } catch {
            lastError = error.localizedDescription
            return
        }
        guard let engine else { return }
        // Part D: pull the server's doc as a shared base before the first
        // local edit so this note's blocks resolve to the server's tree
        // nodes. No-op once resident; best-effort otherwise. (A splice is
        // an in-place edit — the block node must already exist, which the
        // base guarantees for a note the user can see.)
        await bootstrapNoteIfNeeded(slug: slug)
        // Part A: seed the per-note push floor BEFORE recording this edit
        // so the first delta exports `updates(floor)` = this edit only,
        // not a full snapshot. Seed only when absent (don't regress a
        // baseline a prior bootstrap/commit already advanced).
        if lastPushedVV[slug] == nil {
            lastPushedVV[slug] = await engine.noteVersion(slug: slug)
        }
        do {
            _ = try await engine.spliceBlockText(
                slug: slug,
                blockIdHex: blockIdHex,
                utf16Offset: UInt32(max(0, utf16Offset)),
                utf16DeleteLen: UInt32(max(0, utf16DeleteLen)),
                insert: insert
            )
        } catch {
            lastError = error.localizedDescription
            return
        }
        // Engine durability is guaranteed. In hub mode the live `/ws`
        // socket owns delivery (the caller pushes a delta after this
        // returns), so the relay coordinator must NOT also drain this op.
        if hubMode { return }
        invalidateCoordinatorIfRepaired()
        if coordinator == nil {
            try? await ensureCoordinator()
        }
        guard let coordinator else { return }
        do {
            let outcome = try await coordinator.tickOutbound(maxBytes: 1_000_000)
            noteOutboundOutcome(outcome)
        } catch {
            lastError = error.localizedDescription
        }
    }

    /// P1.11 outbound: record ONE typed property write (an Inbox triage
    /// swipe / Agenda mark-done / reschedule) into the engine's
    /// `props`/`prop_keys` containers and drain it, mirroring
    /// `spliceAndPush` but calling `engine.setBlockProperty(...)` — the FFI
    /// mirror of the server's set-property route. Because the property
    /// merges independently of the block's `text_seq`, a peer's concurrent
    /// prose edit on the same block is never clobbered, and the engine
    /// re-materializes the sandbox `.md` (with the `key:: value` line)
    /// before this returns.
    ///
    /// Same bootstrap-before-edit + push-floor sequence as `spliceAndPush`,
    /// and the same hub-mode gate (the live `/ws` socket owns delivery; the
    /// caller pushes the delta after this returns via
    /// `produceDeltaFrame`/`sendDelta`/`commitPushedDelta`).
    ///
    /// Returns whether the engine RECORDED the write — `false` when the
    /// engine can't open or the bid isn't found in the note (the FFI's
    /// mirror of the route's 404). The caller must surface a `false`
    /// instead of optimistically dropping the triaged row: a silent
    /// no-op here is the exact bug class this seam exists to close.
    func setBlockPropertyAndPush(
        slug: String,
        bidHex: String,
        key: String,
        value: String
    ) async -> Bool {
        do {
            try await openEngineIfNeeded()
        } catch {
            lastError = error.localizedDescription
            return false
        }
        guard let engine else { return false }
        // Part D: shared base before the first local edit (no-op once
        // resident) — the block node must exist for the property op to
        // address it, which the base guarantees for a note the user sees.
        await bootstrapNoteIfNeeded(slug: slug)
        // Part A: seed the per-note push floor BEFORE recording so the
        // next delta exports only this edit, never a full snapshot.
        if lastPushedVV[slug] == nil {
            lastPushedVV[slug] = await engine.noteVersion(slug: slug)
        }
        let applied: UInt32
        do {
            applied = try await engine.setBlockProperty(
                slug: slug,
                blockIdHex: bidHex,
                key: key,
                value: value
            )
        } catch {
            lastError = error.localizedDescription
            return false
        }
        guard applied == 1 else {
            lastError = "property write: block \(bidHex) not found in \(slug)"
            return false
        }
        // Engine durability is guaranteed. In hub mode the live `/ws`
        // socket owns delivery (the caller pushes the delta after this
        // returns), so the relay coordinator must NOT also drain this op.
        if hubMode { return true }
        invalidateCoordinatorIfRepaired()
        if coordinator == nil {
            try? await ensureCoordinator()
        }
        guard let coordinator else { return true }
        do {
            let outcome = try await coordinator.tickOutbound(maxBytes: 1_000_000)
            noteOutboundOutcome(outcome)
        } catch {
            lastError = error.localizedDescription
        }
        return true
    }

    // ─── Saved views registry (saved-views spec, 2026-06-10) ───────────
    //
    // The registry is ONE dedicated Loro doc (`tesela_sync::VIEWS_DOC_ID`)
    // that rides the coordinator's existing produce/apply streams like any
    // note doc — inbound registry edits arrive via the normal tick (and
    // surface through `onAppliedChanges`, which the host already wires to
    // a refresh), so reads need no extra sync plumbing. Writes mirror
    // `setBlockPropertyAndPush`'s tail (record into the engine, then drain
    // the coordinator) MINUS the per-note bootstrap/push-floor steps: those
    // are note-slug machinery for the live-WS delta path, and views writes
    // through the engine only happen in `.relay` mode where the relay tick
    // (not the WS) owns delivery.

    /// Has this session already run the idempotent builtin seed? The
    /// engine-side `ensureBuiltinViews` no-ops when the Inbox entry exists
    /// (locally seeded or synced), so this flag only saves the FFI hop.
    private var viewsSeeded = false

    /// Pure half of the builtin-views seed gate (adversarial review,
    /// 2026-06-10 — same ordering rule as the server's main.rs): a device
    /// with a pairing must NOT seed before the snapshot bootstrap has run,
    /// or a first-launch list on a fresh install would author a default
    /// Inbox entry while the group's registry (possibly user-edited) is
    /// still in flight. `coordinator != nil` IS the bootstrap-completed
    /// signal — `buildCoordinator` runs the snapshot-bootstrap step inline
    /// and only assigns the coordinator after it (the step itself no-ops
    /// when the persisted cursor already covers the relay's watermark). A
    /// device with no cached pairing has no group to receive from — it
    /// seeds immediately, like a relay-less server. Hub-mode consequence:
    /// the coordinator is gated off there, so a paired hub device defers
    /// the seed; the UI's `SavedView.fallbackInbox` covers reads, and a
    /// builtin edit still lands safely (the engine routes it through the
    /// deterministic seed container).
    static func shouldSeedBuiltinViews(hasPairing: Bool, bootstrapCompleted: Bool) -> Bool {
        !hasPairing || bootstrapCompleted
    }

    /// All saved views from the synced registry, sorted by `(order, id)`.
    /// Seeds the builtin Inbox first when `shouldSeedBuiltinViews` allows
    /// it (same bring-up posture as the server's `ensure_builtin_views`
    /// in main.rs — idempotent, edit-preserving, deferred until after the
    /// snapshot bootstrap when a pairing exists). Returns nil when the
    /// engine can't open.
    func viewsList() async -> [ViewRecord]? {
        do {
            try await openEngineIfNeeded()
        } catch {
            lastError = error.localizedDescription
            return nil
        }
        guard let engine else { return nil }
        let hasPairing = UserDefaults.standard.string(forKey: Self.pairingCodeKey) != nil
        if !viewsSeeded,
           Self.shouldSeedBuiltinViews(
               hasPairing: hasPairing,
               bootstrapCompleted: coordinator != nil
           ) {
            do {
                try await engine.ensureBuiltinViews()
                viewsSeeded = true
            } catch {
                // Non-fatal: serve whatever the registry holds; the next
                // list retries the seed.
            }
        }
        return await engine.viewsList()
    }

    /// Create/update a saved view in the engine's registry and drain the
    /// op to the relay so other devices converge. Throws when the engine
    /// can't open or the upsert is rejected — the editor surfaces the
    /// message instead of pretending the save landed.
    func viewsUpsertAndPush(_ record: ViewRecord) async throws {
        try await openEngineIfNeeded()
        guard let engine else {
            throw FfiSyncError.Other(message: "engine open failed")
        }
        try await engine.viewsUpsert(record: record)
        await drainViewsWrite()
    }

    /// Delete a saved view and drain the op. The engine's builtin guard
    /// surfaces as a thrown `FfiSyncError` (builtins are editable, never
    /// deletable); an already-gone id is an idempotent no-op.
    func viewsDeleteAndPush(viewId: String) async throws {
        try await openEngineIfNeeded()
        guard let engine else {
            throw FfiSyncError.Other(message: "engine open failed")
        }
        _ = try await engine.viewsDelete(viewId: viewId)
        await drainViewsWrite()
    }

    /// Best-effort immediate outbound tick after a views-registry write —
    /// the same tail `setBlockPropertyAndPush` uses. In hub mode the relay
    /// coordinator is gated off (the registry write still reaches peers:
    /// `.http` mode routes views writes through the server, so this path
    /// only runs in `.relay`); if the coordinator isn't ready the regular
    /// tick loop drains the op later.
    private func drainViewsWrite() async {
        if hubMode { return }
        invalidateCoordinatorIfRepaired()
        if coordinator == nil {
            try? await ensureCoordinator()
        }
        guard let coordinator else { return }
        do {
            let outcome = try await coordinator.tickOutbound(maxBytes: 1_000_000)
            noteOutboundOutcome(outcome)
        } catch {
            lastError = error.localizedDescription
        }
    }

    /// Read a block's current engine-exact text (collab editing C1-inbound).
    /// The engine is the source of truth: after a remote splice lands via
    /// `applyInboundDelta`, the open editor reads the MERGED block text here
    /// and reconciles its `UITextView`. Returns nil if no engine is open or
    /// the note/block is absent.
    func readBlockText(slug: String, blockIdHex: String) async -> String? {
        guard let engine else { return nil }
        return try? await engine.readBlockText(slug: slug, blockIdHex: blockIdHex)
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
        let outcome: DeltaApplyOutcome
        do {
            outcome = try await engine.applyDeltaFrame(frame: frame)
        } catch {
            lastError = error.localizedDescription
            return false
        }
        // `needsCatchup` = loro left ops PENDING: the frame referenced tree
        // nodes this device doesn't have, because we're on a DISJOINT lineage
        // (or missing a causal predecessor). A delta can NEVER heal that — only
        // a full-snapshot catch-up can, and `bootstrapNoteIfNeeded` now imports
        // AUTHORITATIVELY (server-wins re-base), so the device adopts the
        // server's lineage and subsequent deltas apply. Force it now (bypass the
        // per-slug catch-up debounce) so live web edits stop silently vanishing.
        // Self-limiting: once re-based, later frames apply cleanly → no pending →
        // no further forced catch-up. The note id in the frame can't be reversed
        // to a slug (it's a blake3 hash), so re-base the visible note(s).
        if outcome.needsCatchup, let mosaic {
            // Coalesce a burst: only ONE forced re-base per slug in flight at a
            // time. The @MainActor serializes these, so concurrent pending
            // frames arriving while the snapshot fetch is suspended see the
            // in-flight flag and skip — instead of each clearing the debounce +
            // firing its own fetch. Once the re-base lands, later frames apply
            // cleanly → no pending → no further forced catch-up.
            let slug = mosaic.todayDailySlug
            if !forcedCatchupInFlight.contains(slug) {
                forcedCatchupInFlight.insert(slug)
                lastCatchupAt[slug] = nil
                await bootstrapNoteIfNeeded(slug: slug)
                forcedCatchupInFlight.remove(slug)
            }
        }
        guard outcome.applied > 0 else {
            // A pending-only frame applied nothing locally, but a re-base above
            // may have changed the engine — nudge the UI through the same seam.
            if outcome.needsCatchup { onAppliedChanges?() }
            return false
        }
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
    /// it. Returns `nil` when the doc isn't resident (nothing to send) or
    /// the engine can't open.
    ///
    /// #150 — steady-state ships a DELTA, not a full snapshot:
    /// `sinceVv = lastPushedVV[slug]` exports only the ops authored since our
    /// last push (`export_doc_update(note, Some(vv))` = `ExportMode::updates`).
    ///
    /// Part A (WS-push clobber, 2026-06-02): `recordAndPush` now ALWAYS seeds
    /// `lastPushedVV[slug]` BEFORE recording the edit — from the bootstrap base
    /// (first-view / resident catch-up) or, failing that, from the engine's
    /// pre-edit VV floor. So by the time the host calls this method after a
    /// `recordAndPush`, `sinceVv` is non-nil and the frame is a bounded DELTA
    /// (only iOS's own ops since the base), NEVER a whole-note snapshot that
    /// re-asserts a stale copy of a block a peer just edited. A `nil` here can
    /// now arise only on a push path that did NOT go through `recordAndPush`
    /// (none today): rather than risk EXCLUDING a genuine edit by seeding the
    /// post-edit VV, we fall back to the full-snapshot export — correct (the
    /// peer still needs a base) and guarded by the server's Part C WS-apply
    /// protection. The peer also acquires its base via this path
    /// (`partial_delta_needs_base.rs`).
    ///
    /// **The baseline is NOT advanced here.** Because a snapshot is no longer
    /// re-sent every keystroke, a dropped WS frame would never be re-included
    /// if we advanced the VV optimistically — the next delta would start past
    /// the dropped ops. So the caller MUST call [`commitPushedDelta(slug:)`]
    /// only AFTER the frame is confirmed handed to a connected socket
    /// (`LiveSyncSocket.sendDelta` returns `true`). A dropped send leaves the
    /// baseline back, so the next produce re-includes the dropped ops — that's
    /// the dropped-frame self-heal that full snapshots used to give for free.
    func produceDeltaFrame(slug: String) async -> Data? {
        do {
            try await openEngineIfNeeded()
        } catch {
            lastError = error.localizedDescription
            return nil
        }
        guard let engine else { return nil }
        let sinceVv = lastPushedVV[slug]
        do {
            return try await engine.produceNoteDelta(slug: slug, sinceVv: sinceVv)
        } catch {
            lastError = error.localizedDescription
            return nil
        }
    }

    /// Advance the per-note delta baseline AFTER a frame produced by
    /// [`produceDeltaFrame(slug:)`] was confirmed sent over the live WS.
    /// Reads the post-edit VV fresh from the engine (the caller recorded the
    /// edit before producing), so the NEXT frame is a delta relative to this
    /// confirmed push. Call this ONLY when `sendDelta` returned `true`; if the
    /// send was dropped, do NOT call it, so the dropped ops re-ship next time.
    func commitPushedDelta(slug: String) async {
        guard let engine else { return }
        if let vv = await engine.noteVersion(slug: slug) {
            lastPushedVV[slug] = vv
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
        // Hub mode: the live `/ws` socket is the sync hub; the relay poll
        // loop is gated off (Part E2). The runLoop keeps sleeping/waking;
        // each wake is a no-op until `hubMode` flips back to false.
        guard !hubMode else { return }
        invalidateCoordinatorIfRepaired()
        do {
            if coordinator == nil {
                try await ensureCoordinator()
            }
            guard let coordinator else { return }
            // Capture the scope WITH the coordinator: a re-pair completing
            // during the awaits below swaps self.cursorScope to the new
            // identity, and persisting the OLD group's cursor under the NEW
            // key would silently stall the new group's inbound (review
            // finding on ddd8def — same class as the hubMode mid-tick flip).
            let tickScope = cursorScope
            let outbound = try await coordinator.tickOutbound(maxBytes: 1_000_000)
            let inbound = try await coordinator.tickInbound()
            noteOutboundOutcome(outbound)
            lastApplied = inbound.applied
            inboundCursorSeq = inbound.newCursorSeq
            // Audit A7: tick_outbound returns Ok even when relay PUTs
            // failed (skip-not-abort — the failed batch's cursors stay
            // uncommitted and re-produce next tick), so an Ok return is
            // NOT "healthy". batchesFailed > 0 keeps lastError set (via
            // noteOutboundOutcome) and backs the loop off; the green
            // "Syncing" while edits never left the device was the
            // 413-over-budget incident class.
            if outbound.batchesFailed > 0 {
                consecutiveErrors = consecutiveErrors &+ 1
            } else {
                consecutiveErrors = 0
            }
            // Persist cursors (scoped per relay+group, audit A5) so a
            // cold launch resumes where we left off instead of re-polling
            // the full relay history.
            if let scope = tickScope {
                UserDefaults.standard.set(inbound.newCursorSeq, forKey: Self.inboundCursorKey(scope: scope))
                if let ntp = outbound.newCursorNtp {
                    UserDefaults.standard.set(ntp, forKey: Self.outboundCursorKey(scope: scope))
                }
            }
            if inbound.applied > 0 {
                // Tell the host UI that new data has landed in the
                // local engine + sandbox. AppShell wires this to a
                // MockMosaicService.refresh() so the page the user is
                // looking at updates without a manual pull-down.
                onAppliedChanges?()
            }
            // Audit A4: notes the FFI flagged for an authoritative-snapshot
            // catch-up — updates Loro left PENDING (causal gap) or per-note
            // applies that failed past the retry budget. A delta can never
            // heal these; import the relay's deposited snapshot for exactly
            // those notes or they silently freeze.
            if !inbound.needsCatchupNoteIdsHex.isEmpty, let relay, let engine {
                await catchUpFromRelaySnapshots(
                    idsHex: inbound.needsCatchupNoteIdsHex,
                    relay: relay,
                    engine: engine
                )
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
            // Only a DEFINITIVE staleness signal invalidates the cached
            // code (decode failure, registration intent that doesn't
            // verify under our group key). A transient error — network
            // drop, relay restart, timeout — must KEEP the cache: in
            // `.relay` mode there is no Mac HTTP to refetch the code from
            // (`fetchPairingCode` throws for non-.http backends), so
            // deleting it on a connectivity blip permanently bricked sync
            // until the user re-scanned the QR (audit A6). Either way,
            // surface the error and let the next tick (with backoff)
            // retry — don't recurse here.
            if cached != nil, Self.isDefinitivePairingFailure(error) {
                UserDefaults.standard.removeObject(forKey: Self.pairingCodeKey)
            }
            throw error
        }
    }

    /// Does `error` PROVE the cached pairing code is unusable (vs a
    /// transient connectivity failure that the same code will survive)?
    /// - `InvalidPairingCode` — the cached blob doesn't even decode.
    /// - A crypto intent-verify failure — the relay's stored registration
    ///   doesn't verify under our group key (group rotated / hijacked),
    ///   so retrying with this code can never succeed.
    /// Everything else (reqwest send errors, relay 5xx, timeouts) keeps
    /// the cache; the next tick retries with the same code.
    static func isDefinitivePairingFailure(_ error: Error) -> Bool {
        guard let ffi = error as? FfiSyncError else { return false }
        switch ffi {
        case .InvalidPairingCode:
            return true
        case .Other(let message):
            // `SyncError::Crypto` verify failures from
            // `register_or_recover`/`verify_registration` — both hijack
            // messages contain these markers (transport/net errors never do).
            let m = message.lowercased()
            return m.contains("hijack") || m.contains("does not verify")
        }
    }

    /// Pure half of the snapshot-bootstrap gate (audit A4/A5): the
    /// bootstrap runs only when the relay's GC watermark is PAST our
    /// inbound cursor — i.e. ops we've never polled were compacted away
    /// and only the deposited snapshots can cover them. A cursor at (or
    /// past) the watermark means the tail poll covers everything.
    static func shouldRunSnapshotBootstrap(compactionSeq: Int64, inboundCursorSeq: Int64) -> Bool {
        compactionSeq > inboundCursorSeq
    }

    /// Pure half of the bootstrap-cursor rule (audit A4, mirrors the
    /// server fix): jump the inbound cursor to the GC watermark only when
    /// EVERY snapshot import landed. The covered ops are already GC'd, so
    /// jumping past a failed import would skip that note permanently —
    /// and the `compactionSeq > cursor` guard would make every future
    /// bootstrap a no-op. On any failure the cursor HOLDS so the next
    /// rebuild retries the imports.
    static func shouldJumpBootstrapCursor(failedImports: Int) -> Bool {
        failedImports == 0
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
        // Restore cursors persisted for THIS (relay, group) identity only
        // (audit A5). A different identity has no scoped keys → cursors
        // start at 0 → the snapshot bootstrap below runs and the tail poll
        // fetches the new group's ops from the start, instead of replaying
        // a stale-high cursor that silently black-holes inbound forever.
        let scope = Self.cursorScope(relayUrl: relayURL, groupIdHex: pairing.groupIdHex)
        Self.migrateLegacyCursors(toScope: scope)
        if let inbound = UserDefaults.standard.object(forKey: Self.inboundCursorKey(scope: scope)) as? Int64 {
            await coordinator.setInboundCursorSeq(seq: inbound)
            inboundCursorSeq = inbound
        } else {
            // Fresh identity: reset the published value too — the bootstrap
            // guard below compares against it, and a stale in-session value
            // from a previous pairing would wrongly suppress the bootstrap.
            inboundCursorSeq = 0
        }
        if let outbound = UserDefaults.standard.object(forKey: Self.outboundCursorKey(scope: scope)) as? Int64 {
            await coordinator.setOutboundCursorNtp(ntp: outbound)
        }
        // Bootstrap-from-snapshots (offline-bootstrap spine, phase 3): pull the
        // relay's compacted snapshot set so a device that's been offline past
        // the relay's GC window converges even when the Mac (the depositor) is
        // unreachable — the case the Mac-HTTP `bootstrapNoteIfNeeded` can't cover
        // with the Mac off. Import each note_id-keyed snapshot, then jump the
        // inbound cursor to the relay's watermark so the next poll fetches only
        // the un-compacted tail. Guard: skip when our cursor already covers the
        // watermark (mirrors the server's `bootstrap_from_snapshots`). Best-effort
        // — a fetch failure just leaves the cursor as-is and the normal poll
        // handles the (un-GC'd) tail.
        do {
            let snaps = try await relay.fetchSnapshots()
            if Self.shouldRunSnapshotBootstrap(
                compactionSeq: snaps.compactionSeq,
                inboundCursorSeq: inboundCursorSeq
            ) {
                var imported = 0
                var failed = 0
                for s in snaps.snapshots {
                    do {
                        try await engine.importNoteSnapshotById(noteId: s.streamId, bytes: s.payload)
                        imported += 1
                    } catch {
                        failed += 1
                    }
                }
                if Self.shouldJumpBootstrapCursor(failedImports: failed) {
                    // Only jump the cursor past the GC watermark when EVERY
                    // import landed (audit A4, mirrors the server fix): the
                    // covered ops are already GC'd, so jumping past a failed
                    // import would skip that note permanently — and the
                    // `compactionSeq > inboundCursorSeq` guard would make
                    // every future bootstrap a no-op. On partial failure the
                    // cursor holds, so the next rebuild retries the imports.
                    await coordinator.setInboundCursorSeq(seq: snaps.compactionSeq)
                    inboundCursorSeq = snaps.compactionSeq
                    UserDefaults.standard.set(
                        snaps.compactionSeq,
                        forKey: Self.inboundCursorKey(scope: scope)
                    )
                } else {
                    lastError = "snapshot bootstrap: \(failed) of \(snaps.snapshots.count) imports failed; retrying"
                }
                if imported > 0 { onAppliedChanges?() }
            }
        } catch {
            // Leave the cursor as-is; the regular poll handles the un-GC'd tail.
        }
        self.relay = relay
        self.coordinator = coordinator
        self.cursorScope = scope
        self.coordinatorPairingCode = codeStr
    }

    /// One-time migration (audit A5): cursors persisted under the
    /// pre-scoping bare keys carry no identity, so they're treated as
    /// belonging to the CURRENT pairing once (the first coordinator built
    /// post-upgrade adopts them — in-place upgrades keep their progress)
    /// and then re-keyed; the bare keys are removed either way.
    static func migrateLegacyCursors(toScope scope: String, defaults: UserDefaults = .standard) {
        if let inbound = defaults.object(forKey: legacyInboundCursorKey) as? Int64 {
            if defaults.object(forKey: inboundCursorKey(scope: scope)) == nil {
                defaults.set(inbound, forKey: inboundCursorKey(scope: scope))
            }
            defaults.removeObject(forKey: legacyInboundCursorKey)
        }
        if let outbound = defaults.object(forKey: legacyOutboundCursorKey) as? Int64 {
            if defaults.object(forKey: outboundCursorKey(scope: scope)) == nil {
                defaults.set(outbound, forKey: outboundCursorKey(scope: scope))
            }
            defaults.removeObject(forKey: legacyOutboundCursorKey)
        }
    }

    /// Tear down the live coordinator when the cached pairing code no
    /// longer matches the one it was built from — i.e. the user adopted a
    /// NEW code (PairScanView re-pair). Without this the old coordinator
    /// kept ticking the PREVIOUS group until its next error (tickOnce only
    /// rebuilds when `coordinator == nil`), and the next build would have
    /// restored the old group's cursors (audit A5; the cursor side is now
    /// also covered by per-identity scoping). The WS-push baselines were
    /// earned against the old group's peers, so drop them too — the next
    /// push re-seeds per note (`recordAndPush`/`bootstrapNoteIfNeeded`).
    private func invalidateCoordinatorIfRepaired() {
        guard coordinator != nil, let built = coordinatorPairingCode else { return }
        guard let cached = UserDefaults.standard.string(forKey: Self.pairingCodeKey),
              cached != built
        else { return }
        dropCoordinator()
        lastPushedVV = [:]
    }

    /// Drop the coordinator + relay so the next tick rebuilds them.
    /// **Engine handle is preserved** — it's purely local and tied to
    /// SQLite state, not to any network identity. Dropping it on a
    /// transient relay error would orphan any local write that came in
    /// between this tick and the next.
    private func dropCoordinator() {
        coordinator = nil
        relay = nil
        cursorScope = nil
        coordinatorPairingCode = nil
    }

    /// Audit A4 (Swift half): heal notes the inbound tick flagged via
    /// `needsCatchupNoteIdsHex` — Loro left their updates PENDING (causal
    /// gap) or their per-note applies failed past the retry budget. A
    /// delta can never integrate those; only an authoritative snapshot
    /// can. Fetch the relay's deposited snapshot set once and import the
    /// entries for exactly those notes. Best-effort: a note without a
    /// deposited snapshot stays broken until the depositor's next deposit
    /// (pending notes resurface on their next inbound delta), and a fetch
    /// failure is retried next tick.
    private func catchUpFromRelaySnapshots(
        idsHex: [String],
        relay: RelayClientHandle,
        engine: SyncEngineHandle
    ) async {
        let wanted = Set(idsHex.map { $0.lowercased() })
        do {
            let snaps = try await relay.fetchSnapshots()
            var imported = 0
            for s in snaps.snapshots {
                let idHex = s.streamId.map { String(format: "%02x", $0) }.joined()
                guard wanted.contains(idHex) else { continue }
                do {
                    try await engine.importNoteSnapshotById(noteId: s.streamId, bytes: s.payload)
                    imported += 1
                } catch {
                    // Leave it for the next surfacing; the import is the
                    // same authoritative re-base the bootstrap uses.
                }
            }
            if imported > 0 { onAppliedChanges?() }
        } catch {
            // Snapshot fetch failed (network). Best-effort: a pending
            // note resurfaces on its next inbound delta; `lastError` is
            // not set here so a one-off blip doesn't flag the whole tick.
        }
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
        invalidateCoordinatorIfRepaired()
        if coordinator == nil {
            try? await ensureCoordinator()
        }
        guard let coordinator else { return 0 }
        do {
            let outcome = try await coordinator.tickOutbound(maxBytes: 1_000_000)
            noteOutboundOutcome(outcome)
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

    /// Is this a RELAY-only pairing — i.e. the inviter is a node we can't reach
    /// directly (loopback/empty `url`, e.g. the Tauri desktop embed) but it gave
    /// us a relay URL? Then we pair via the relay (Mock mode + the relay tick)
    /// instead of pointing the HTTP backend at an unreachable loopback. A node
    /// with a real reachable `url` (LAN/Tailscale) still pairs HTTP-direct.
    static func isRelayOnlyPairing(_ record: PairingCodeRecord) -> Bool {
        guard let relay = record.relayUrl, !relay.isEmpty else { return false }
        let url = record.url
        if url.isEmpty { return true }
        return url.contains("127.0.0.1")
            || url.contains("//localhost")
            || url.contains("[::1]")
            || url.contains("//0.0.0.0")
    }

    /// Cache a scanned/entered pairing code so the relay tick can build its
    /// coordinator from it in Mock mode (no Mac HTTP fetch needed) — the relay
    /// URL + group identity ride inside. Mirrors the cache the tick itself
    /// writes after a successful HTTP-fetched build.
    static func cachePairingCode(_ rawCode: String) {
        UserDefaults.standard.set(rawCode, forKey: pairingCodeKey)
    }
}
