import Foundation
import Combine
import UIKit

/// Resume-once helper backing `RelayTicker.raceAgainstTimeout` (tesela-96y).
/// `CheckedContinuation.resume` traps if called twice; two independent
/// unstructured `Task`s (the real work + the timeout sleep) race to call
/// `resume(_:)`, and this actor serializes those calls so only the FIRST
/// one actually resumes the continuation — the loser's call is a safe
/// no-op instead of a crash.
private actor TickRaceOnce {
    private var done = false
    private let continuation: CheckedContinuation<Bool, Never>

    init(_ continuation: CheckedContinuation<Bool, Never>) {
        self.continuation = continuation
    }

    func resume(_ value: Bool) {
        guard !done else { return }
        done = true
        continuation.resume(returning: value)
    }
}

struct PendingRelocationNote: Codable, Equatable {
    let slug: String
    let version: Data
}

struct PendingRelocationFrame: Codable, Equatable {
    let frame: Data
    let notes: [PendingRelocationNote]

    init(_ prepared: PreparedDeltaFrameRecord) {
        frame = prepared.frame
        notes = prepared.notes.map {
            PendingRelocationNote(slug: $0.slug, version: $0.version)
        }
    }

    var prepared: PreparedDeltaFrameRecord {
        PreparedDeltaFrameRecord(
            frame: frame,
            notes: notes.map {
                PreparedDeltaNoteRecord(slug: $0.slug, version: $0.version)
            }
        )
    }
}

struct PendingRelocationDelivery: Codable, Equatable {
    let hubIdentity: String
    let request: BlockMoveRequest
    var prepared: PendingRelocationFrame?
    /// Optional only for decoding build-78 outboxes written before engine
    /// scoping shipped. New records always persist the sync-group scope. A
    /// legacy record is decoded only so it can be cleared fail-closed; URL,
    /// profile, and path cannot prove which physical group authored it.
    var engineScope: MosaicEngineScope? = nil
}

struct EngineSessionToken: Equatable, Sendable {
    let generation: UInt64
    let scope: MosaicEngineScope
    let hubMode: Bool
    let hubIdentity: String?
}

private struct SuspendedHubLease: Equatable {
    let scope: MosaicEngineScope
    let hubIdentity: String
}

/// Main-actor admission gate for every relay coordinator operation. The FFI
/// handles are safe to retain across awaits, but APNs/BG, flush, relocation,
/// and foreground-tick drivers must not use them concurrently or race the
/// shared tick generation and outcome state.
@MainActor
final class RelayOperationAdmission {
    enum Kind: Equatable {
        case backgroundCatchup
        case flush
        case tick
        case immediateOutbound
        case relocation
    }

    struct Lease: Equatable {
        fileprivate let generation: UInt64
        let kind: Kind
    }

    private var generation: UInt64 = 0
    private var activeLease: Lease?
    private var idleWaiters: [CheckedContinuation<Void, Never>] = []

    var isBusy: Bool {
        activeLease != nil
    }

    func tryAcquire(_ kind: Kind) -> Lease? {
        guard activeLease == nil else { return nil }
        generation &+= 1
        let lease = Lease(generation: generation, kind: kind)
        activeLease = lease
        return lease
    }

    func release(_ lease: Lease) {
        guard activeLease == lease else { return }
        activeLease = nil
        let waiters = idleWaiters
        idleWaiters.removeAll()
        for waiter in waiters {
            waiter.resume()
        }
    }

    func permits(_ lease: Lease?) -> Bool {
        activeLease == nil || (lease != nil && activeLease == lease)
    }

    func isActive(_ lease: Lease) -> Bool {
        activeLease == lease
    }

    func waitUntilIdle() async {
        guard activeLease != nil else { return }
        await withCheckedContinuation { continuation in
            idleWaiters.append(continuation)
        }
    }

    func acquireWhenIdle(_ kind: Kind) async -> Lease? {
        while !Task.isCancelled {
            if let lease = tryAcquire(kind) {
                return lease
            }
            await waitUntilIdle()
        }
        return nil
    }
}

/// Main-actor admission barrier between ordinary engine work and a block
/// relocation. Ordinary operations may overlap one another, but relocation
/// closes admission, drains every operation that already owns a reservation,
/// and then holds exclusive access through mutation and delivery.
@MainActor
final class EngineOperationAdmission {
    enum Reservation: Equatable {
        case active
        case queued
    }

    struct ExclusiveLease: Equatable {
        fileprivate let generation: UInt64
    }

    private var generation: UInt64 = 0
    private var exclusiveLease: ExclusiveLease?
    private var operationWaiters: [CheckedContinuation<Bool, Never>] = []
    private var reservedOperationWaiters: [CheckedContinuation<Bool, Never>] = []
    private var activeIdleWaiters: [CheckedContinuation<Void, Never>] = []
    private var allIdleWaiters: [CheckedContinuation<Void, Never>] = []
    private var queuedReservationCount = 0

    private(set) var activeCount = 0
    var operationCount: Int {
        activeCount + queuedReservationCount
    }

    func tryBeginOperation() -> Bool {
        guard exclusiveLease == nil else { return false }
        activeCount += 1
        return true
    }

    func beginOperationWhenAvailable() async -> Bool {
        if tryBeginOperation() {
            return true
        }
        return await withCheckedContinuation { continuation in
            operationWaiters.append(continuation)
        }
    }

    /// Synchronously preserves an edit submitted through a non-async callback.
    /// A queued reservation counts against profile activation immediately, but
    /// does not prevent the already-exclusive relocation from draining active
    /// work and proceeding.
    func reserveOperation() -> Reservation {
        if tryBeginOperation() {
            return .active
        }
        queuedReservationCount += 1
        return .queued
    }

    func beginReservedOperationWhenAvailable() async -> Bool {
        guard queuedReservationCount > 0 else { return false }
        if exclusiveLease == nil {
            queuedReservationCount -= 1
            activeCount += 1
            return true
        }
        return await withCheckedContinuation { continuation in
            reservedOperationWaiters.append(continuation)
        }
    }

    func finishOperation() {
        guard activeCount > 0 else { return }
        activeCount -= 1
        if activeCount == 0 {
            let waiters = activeIdleWaiters
            activeIdleWaiters.removeAll()
            for waiter in waiters {
                waiter.resume()
            }
        }
        if operationCount == 0 {
            let waiters = allIdleWaiters
            allIdleWaiters.removeAll()
            for waiter in waiters {
                waiter.resume()
            }
        }
    }

    func waitUntilIdle() async {
        guard operationCount > 0 else { return }
        await withCheckedContinuation { continuation in
            allIdleWaiters.append(continuation)
        }
    }

    func closeForExclusiveAccess() -> ExclusiveLease? {
        // A synchronously reserved edit is logically ahead of a later move,
        // even if its Task has not run yet. Do not let a second relocation
        // leapfrog that preserved edit during the scheduling gap.
        guard exclusiveLease == nil, queuedReservationCount == 0 else { return nil }
        generation &+= 1
        let lease = ExclusiveLease(generation: generation)
        exclusiveLease = lease
        return lease
    }

    func waitUntilExclusiveAccessReady(_ lease: ExclusiveLease) async {
        guard exclusiveLease == lease else { return }
        guard activeCount > 0 else { return }
        await withCheckedContinuation { continuation in
            activeIdleWaiters.append(continuation)
        }
    }

    func finishExclusiveAccess(_ lease: ExclusiveLease) {
        guard exclusiveLease == lease, activeCount == 0 else { return }
        exclusiveLease = nil

        // Transfer queued callers into active reservations before resuming
        // them. Profile activation therefore never observes a false zero
        // between relocation release and a queued final edit starting.
        let waiters = operationWaiters
        operationWaiters.removeAll()
        let reservedWaiters = reservedOperationWaiters
        reservedOperationWaiters.removeAll()
        queuedReservationCount -= reservedWaiters.count
        activeCount += waiters.count + reservedWaiters.count
        for waiter in waiters {
            waiter.resume(returning: true)
        }
        for waiter in reservedWaiters {
            waiter.resume(returning: true)
        }
    }
}

enum BackgroundCatchupOutcome: Equatable {
    case unavailable
    case completed(newData: Bool)
    case failed(String)

    var didRunSuccessfully: Bool {
        if case .completed = self { return true }
        return false
    }
}

struct RelocationOutboxStore {
    let url: URL

    init(url: URL = Self.defaultURL()) {
        self.url = url
    }

    func load() throws -> PendingRelocationDelivery? {
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        return try JSONDecoder().decode(
            PendingRelocationDelivery.self,
            from: Data(contentsOf: url)
        )
    }

    func save(_ delivery: PendingRelocationDelivery) throws {
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try JSONEncoder().encode(delivery).write(to: url, options: .atomic)
    }

    func clear() throws {
        guard FileManager.default.fileExists(atPath: url.path) else { return }
        try FileManager.default.removeItem(at: url)
    }

    private static func defaultURL() -> URL {
        let base = FileManager.default.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        ).first ?? FileManager.default.temporaryDirectory
        return base
            .appendingPathComponent("Tesela", isDirectory: true)
            .appendingPathComponent("relocation-outbox.json")
    }
}

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
    /// One process-wide engine owner. Background tasks and the visible shell
    /// must never open independent Loro handles on the same group root.
    static let shared = RelayTicker()

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
    /// Wall-clock of the last outbound tick that actually delivered ops to
    /// the relay (`opsSent > 0` and no batch failed) — distinct from
    /// `lastTickAt`, which advances on every tick even when nothing was
    /// sent. Settings → Sync (tesela-4mc) shows this as the honest
    /// "last-successful-push age" instead of conflating it with tick
    /// cadence.
    @Published private(set) var lastSuccessfulPushAt: Date? = nil
    /// This device's own relay URL, resolved from the cached pairing code
    /// the moment the coordinator is (re)built (`buildCoordinator`).
    /// Settings → Sync (tesela-4mc) surfaces this instead of the HTTP
    /// `backend.serverURL`, which in `.relay` mode still holds whatever
    /// loopback/LAN address was last typed into the `.http` field and is
    /// never the address actually used for sync.
    @Published private(set) var relayURL: String? = nil
    /// DIAGNOSTIC (2026-06-25, build 50): the last today-block splice's
    /// outcome — slug, the spliceBlockText op count (`applied`, normally
    /// discarded), and the resulting outbound `sent`/`failed`. Surfaced in
    /// Settings → Sync so a "my iOS edit never reaches the desktop" report
    /// is observable on-device: stays "—" if the splice seam never fires
    /// (upstream early-return); `applied=0` ≡ block-not-in-tree; `applied=1
    /// sent=0` ≡ recorded-but-not-exported; a wrong `slug` ≡ stale daily id.
    @Published private(set) var lastSpliceDiag: String = "—"
    /// Relay seq we've applied up to. Surfaces "we're at seq N" so
    /// the user can compare with the Mac's outbound cursor.
    @Published private(set) var inboundCursorSeq: Int64 = 0
    /// Is the ticker actively looping? False between `stop()` and
    /// the next `start()`.
    @Published private(set) var isRunning: Bool = false
    /// Wall-clock of the last inbound POLL that completed WITHOUT
    /// throwing — i.e. `coordinator.tickInbound()` returned, whether or
    /// not it found new ops. Distinct from `lastTickAt` (set by
    /// `noteOutboundOutcome`, which fires from several OUTBOUND call
    /// sites too — recordAndPush/spliceAndPush/flushPendingOutbound —
    /// not just the loop's own inbound half). Sync-health observability
    /// (tesela-96y): a healthy loop advances this every ~`tickIntervalSeconds`;
    /// a stale value while `isRunning` is true is the on-device signal that
    /// the loop is wedged (stuck/abandoned tick, dead coordinator rebuild
    /// loop, etc.) even though nothing "errored".
    @Published private(set) var lastSuccessfulPollAt: Date? = nil
    /// Wall-clock of the last inbound tick that actually applied ≥1 op
    /// (`inbound.applied > 0`) — i.e. the last time this device's local
    /// state genuinely changed from a peer's edit, as opposed to an empty
    /// poll. Sync-health observability (tesela-96y): pairs with
    /// `lastSuccessfulPollAt` — polling-but-never-applying against an
    /// active peer is itself a diagnosable symptom (stuck apply retries,
    /// needs-catchup notes piling up, etc).
    @Published private(set) var lastAppliedAt: Date? = nil
    /// Wall-clock of the last SUCCESSFUL snapshot deposit (`PUT
    /// /snapshot`, see `depositSnapshotsIfDue`) — the periodic,
    /// low-frequency (5-minute-cadence) durability push, distinct from
    /// the per-tick `PUT /ops` push already tracked by
    /// `lastSuccessfulPushAt`. Sync-health observability (tesela-96y):
    /// called out explicitly because live fleet monitoring during the
    /// iPad wedge showed "zero PUT /ops, one PUT /snapshot looping" — a
    /// SEND-side symptom invisible on-device up to now. A failed deposit
    /// leaves `depositSnapshotsIfDue`'s cadence stamp unset, so it retries
    /// on every tick instead of every 5 minutes; a `lastDepositAt` that
    /// never advances while deposits keep firing is the on-device signal
    /// that this retry-storm is happening, instead of a silently-eaten
    /// exception with no trace at all.
    @Published private(set) var lastDepositAt: Date? = nil
    /// Most recent snapshot-deposit failure string, cleared on the next
    /// successful deposit. Deliberately separate from `lastError` (see
    /// `depositSnapshotsIfDue`) — a deposit hiccup is best-effort and
    /// should never flip the primary Sync status pill to "Sync error".
    @Published private(set) var lastDepositError: String? = nil

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
            if hubMode != oldValue {
                engineSessionGeneration &+= 1
            }
            if hubMode && !oldValue {
                dropCoordinator()
                if pendingRelocation?.hubIdentity == liveHubIdentity {
                    Task { [weak self] in
                        _ = await self?.retryPendingRelocation()
                    }
                }
            }
        }
    }
    /// A suspended, already-verified HTTP hub may temporarily hand its exact
    /// group-scoped engine to the relay path. The lease prevents a profile or
    /// transport activation that completes in the background from being
    /// overwritten by a stale foreground `hubMode = true` restoration.
    private var suspendedHubLease: SuspendedHubLease?
    private var backgroundTransitionGeneration: UInt64 = 0
    private var backgroundFlushesInFlight = 0
    private var backgroundFlushWaiters: [CheckedContinuation<Void, Never>] = []

    // Owned FFI handles. nil until the first successful `ensure()`;
    // dropped on tick error so the next tick rebuilds. Caching keeps
    // the HTTP-to-Mac fetch (for the pairing code) to once per app
    // session in the happy path.
    private var engine: SyncEngineHandle? = nil
    private(set) var activeEngineScope: MosaicEngineScope = .legacy
    private var engineSessionGeneration: UInt64 = 0
    /// Ordinary edits hold this across record → exact frame → identity-bound
    /// send → baseline commit. Activation waits for zero, so an edit started
    /// against A cannot resume after an await and mutate or send through B.
    private let engineOperationAdmission = EngineOperationAdmission()
    private var engineOperationsInFlight: Int {
        engineOperationAdmission.operationCount
    }
    private var engineActivationInFlight = false
    private var relay: RelayClientHandle? = nil
    /// The APNs device token we last successfully registered with the relay
    /// (sync durability P3b). Guards `maybeRegisterApnsToken` so we POST
    /// /devices once per token, not every tick; re-registers if iOS rotates
    /// the token.
    private var lastRegisteredApnsKey: String? = nil
    /// In-app diagnostic for the APNs token-registration state (sync
    /// durability P3b) — shown in Sync settings so we can see WHERE
    /// registration is stuck (no relay handle / no token / POST failed /
    /// registered) without attaching Console.app.
    @Published var apnsNote: String = "—"
    private var coordinator: SyncCoordinator? = nil
    private let relayOperationAdmission = RelayOperationAdmission()

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
    private let relocationOutboxStore: RelocationOutboxStore
    private var pendingRelocation: PendingRelocationDelivery?
    private var relocationDeliveryInFlight = false
    private var relocationIdleWaiters: [CheckedContinuation<Void, Never>] = []
    private var hubBootstrapInFlight = 0
    private var hubActivationWaiters: [CheckedContinuation<Void, Never>] = []
    private var relocationRetryTask: Task<Void, Never>?
    private var relocationRetryDelaySeconds: UInt64 = 2
    private var liveDeltaSender: ((Data, String) async -> Bool)?
    private var liveHubIdentity: String?
    private var relocationOutboxLoadError: String?

    var engineSessionToken: EngineSessionToken {
        EngineSessionToken(
            generation: engineSessionGeneration,
            scope: activeEngineScope,
            hubMode: hubMode,
            hubIdentity: liveHubIdentity
        )
    }

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
    /// Feeds `backoffSleepSeconds` to compute the next sleep (doubling,
    /// capped at 60s so the loop always wakes at least once a minute).
    /// Published so the UI can show "retrying…" while it backs off.
    @Published private(set) var consecutiveErrors: UInt32 = 0
    /// Monotonic counter bumped once at the START of every `tickOnce()`
    /// call (tesela-96y). Captured locally as a tick's "generation" so its
    /// eventual result — success, failure, OR a blown `tickTimeoutSeconds`
    /// ceiling — is only ever committed to `@Published` state / cursors
    /// when `shouldCommitTick` still finds it current. Closes two related
    /// hazards discovered chasing the iPad in-memory wedge:
    ///   1. `wake()` (`stop()` then `start()`) does NOT actually halt an
    ///      in-flight tick — Swift `Task` cancellation is cooperative and
    ///      nothing in the FFI/network call chain checks it — so a
    ///      scenePhase flap mid-tick (more likely during a heavy sync
    ///      burst, where a single tick legitimately takes longer) can spin
    ///      up a SECOND overlapping `runLoop`/tick. Without generation
    ///      gating, whichever tick finishes LAST wins even if it started
    ///      first — e.g. regressing `inboundCursorSeq` backward under a
    ///      stale, since-superseded result.
    ///   2. A single tick with no bound on how long its engine-apply work
    ///      may take (see `tickTimeoutSeconds`) can wedge the ENTIRE
    ///      serial loop forever with no error surfaced — `isRunning` stays
    ///      true, nothing looks broken, but no later tick ever runs. The
    ///      timeout path in `tickOnce()` abandons a tick that blows the
    ///      ceiling so the loop keeps making forward progress instead of
    ///      hanging until the user force-quits the app.
    private var tickGeneration: UInt64 = 0
    /// Hard ceiling on a single tick's total engine work (outbound +
    /// inbound, including any catch-up/bootstrap it triggers). The relay
    /// HTTP client already times out at 15s per request
    /// (`RelayClient::new`), but that bounds only ONE network round trip —
    /// nothing bounds the Rust engine's CPU-bound Loro apply/merge work
    /// across a whole tick, and a heavy sync burst (large snapshot
    /// imports, big batched updates) is exactly when that work is
    /// biggest. 25s comfortably exceeds the HTTP layer's own 15s (so a
    /// legitimately slow-but-alive network round trip isn't mistaken for
    /// a wedge) while still keeping the loop self-healing within roughly
    /// one backoff cycle instead of hanging indefinitely (tesela-96y).
    static let tickTimeoutSeconds: UInt64 = 25
    /// Persisted-cursor UserDefaults keys, scoped per (relay URL, group
    /// id) — both derived from the pairing code (audit A5). Relay seqs
    /// are a per-relay, per-group namespace restarting at 1, so a global
    /// cursor replayed against a DIFFERENT relay/group (re-pair, relay DB
    /// wipe, the HA→Cloudflare migration) both suppressed the snapshot
    /// bootstrap and made the tail poll start past every op — a silent,
    /// permanent inbound stall. A fresh identity now starts at 0, which
    /// makes `compactionSeq > inboundCursorSeq` true → snapshot bootstrap
    /// runs. Pre-scoping bare keys have no group provenance and are
    /// quarantined rather than adopted into a clean group-scoped store.
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
    /// Persisted wall-clock (seconds since epoch) of this device's last
    /// snapshot deposit, scoped per (relay, group) identity like the
    /// cursors above — see `depositSnapshotsIfDue` (tesela-zpr).
    static func snapshotDepositAtKey(scope: String) -> String {
        "relay.lastSnapshotDepositAt.\(scope)"
    }
    /// Snapshot-deposit cadence, mirroring the server's default
    /// (`tesela-server::sync_relay::snapshot_interval_secs`, 5 minutes).
    /// Not env-tunable on iOS — the server side already exercises the
    /// gate's tunability for tests; iOS just needs a sane fixed cadence.
    static let snapshotDepositIntervalSeconds: TimeInterval = 300
    /// Per-request body budget for `putSnapshotsChunked`, mirroring the
    /// server's default (`deposit_chunk_budget_bytes`, 4 MiB) — comfortable
    /// headroom under the HA relay's cap; the 413-adaptive halving inside
    /// `put_snapshots_chunked` degrades further for a tighter cap (e.g. the
    /// CF Worker's 1 MiB default) automatically.
    static let snapshotDepositBudgetBytes: UInt64 = 4 * 1024 * 1024
    /// Pure cadence gate (mirrors the server's `due` check in
    /// `sync_relay::tick`): is a new snapshot deposit due, given the last
    /// deposit's wall-clock (`nil` = never deposited) and now?
    static func shouldDepositSnapshots(lastDepositAt: Date?, now: Date) -> Bool {
        guard let lastDepositAt else { return true }
        return now.timeIntervalSince(lastDepositAt) >= snapshotDepositIntervalSeconds
    }
    /// Seconds to sleep before the next tick given `consecutiveErrors`.
    /// Exponential (doubling) backoff from `base`, hard-capped at
    /// `maxSeconds`. The cap is on the RESULTING SECONDS, not the shift
    /// exponent: the prior code slept `base << min(errors, 12)`, i.e.
    /// `2 << 12 ≈ 8192s ≈ 2.3h`, which silently parked the sync loop for
    /// hours after a handful of transient relay failures (edits stranded,
    /// looked like data loss — 2026-06-24). Capping the seconds keeps the
    /// "wake at least once a minute" guarantee the comments always claimed.
    static func backoffSleepSeconds(
        consecutiveErrors: UInt32,
        base: UInt64 = 2,
        maxSeconds: UInt64 = 60
    ) -> UInt64 {
        // Cap the exponent first so the shift itself can never overflow,
        // then clamp the result to the seconds ceiling.
        let exponent = UInt64(min(consecutiveErrors, 16))
        let scaled = base << exponent
        return min(scaled, maxSeconds)
    }
    /// Pure predicate (tesela-96y): should a tick's outcome — success,
    /// thrown error, or a blown `tickTimeoutSeconds` ceiling — be
    /// committed to `@Published` state / persisted cursors? `false` when
    /// a NEWER tick has since started (`currentGeneration` has moved past
    /// `issuedGeneration`) — this tick was superseded, either by
    /// `wake()` re-spinning the loop while it was still in flight, or by
    /// its own timeout handler already having abandoned it. A superseded
    /// result must be silently discarded, never applied on top of
    /// whatever fresher state the newer tick already committed.
    static func shouldCommitTick(issuedGeneration: UInt64, currentGeneration: UInt64) -> Bool {
        issuedGeneration == currentGeneration
    }
    /// Race `work` against a `seconds`-long timeout. Returns `true` when
    /// `work` finished before the timeout elapsed, `false` when the
    /// timeout won.
    ///
    /// On a timeout, `work` is NOT cancelled or waited on further: Swift
    /// `Task` cancellation is cooperative, and the FFI/network calls this
    /// ticker makes inside `work` don't check it, so an abandoned `work`
    /// may keep running in the background and complete arbitrarily later
    /// (its result is discarded via `shouldCommitTick`, not awaited here).
    /// Deliberately NOT built on `withTaskGroup`: a task group's scope
    /// implicitly awaits every child before returning, even after
    /// `cancelAll()`, which would defeat the whole point — the caller
    /// needs to walk away from a stuck `work` immediately, not block on
    /// it. Two independent unstructured `Task`s plus a resume-once actor
    /// give a genuine race instead.
    ///
    /// Extracted standalone (no `SyncCoordinator`/engine dependency) so
    /// the race itself — a slow `work` gets abandoned promptly, a fast
    /// one wins cleanly — is unit-testable without a live relay/engine;
    /// see `RelayTickTimeoutTests`.
    static func raceAgainstTimeout(seconds: UInt64, work: @escaping () async -> Void) async -> Bool {
        await withCheckedContinuation { (continuation: CheckedContinuation<Bool, Never>) in
            let race = TickRaceOnce(continuation)
            Task {
                await work()
                await race.resume(true)
            }
            Task {
                try? await Task.sleep(nanoseconds: seconds * 1_000_000_000)
                await race.resume(false)
            }
        }
    }
    /// Stable key identifying THIS device's APNs-token registration. Carries
    /// the relay SCOPE (`relayUrl|groupIdHex`) as well as the token so a relay
    /// migration (HA→CF) or re-pair re-registers the token with the NEW relay
    /// — otherwise the new relay has no token to background-push and the app
    /// never wakes in the background (2026-06-24). Mirrors the inbound-cursor
    /// scoping that fixed the same migration class.
    static func apnsRegistrationKey(token: String, scope: String?) -> String {
        "\(token)|\(scope ?? "")"
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
    /// uselessly dependent on Mac's network. Backed by the Keychain
    /// (`KeychainPairingCache`, tesela-tp0.2) — the code carries the
    /// group key, so it's the same key material a pre-cutover install
    /// kept in plaintext `UserDefaults`.

    /// Callback fired whenever a tick applied ≥1 incoming op. Hosts
    /// hook this up to nudge the iOS UI to re-render (typically by
    /// calling `MockMosaicService.refresh(from:)` so HTTP fetches the
    /// freshly-arrived data from the Mac). On Wi-Fi this gives near-
    /// realtime "edits on Mac → visible on iPhone" feel; on cellular
    /// where HTTP-to-Mac fails, the data lives in the local materialized
    /// sandbox until B.3.4 swaps the read path to local-first.
    var onAppliedChanges: (() -> Void)? = nil

    init(
        tickIntervalSeconds: UInt64 = 2,
        relocationOutboxURL: URL? = nil
    ) {
        // Default 2 s in the foreground — keeps Web→iOS lag close to
        // instant on a healthy network without thrashing battery (the
        // tick body is just a relay GET if there's nothing new). On
        // consecutive errors the exponential backoff still caps the
        // poll at ~60 s so a down relay doesn't drain the device.
        self.tickIntervalSeconds = tickIntervalSeconds
        let store = RelocationOutboxStore(
            url: relocationOutboxURL ?? RelocationOutboxStore().url
        )
        relocationOutboxStore = store
        do {
            pendingRelocation = try store.load()
        } catch {
            pendingRelocation = nil
            relocationOutboxLoadError = error.localizedDescription
        }
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

    func configureLiveDeltaSender(_ sender: @escaping (Data, String) async -> Bool) {
        liveDeltaSender = sender
        if pendingRelocation != nil {
            Task { [weak self] in
                _ = await self?.retryPendingRelocation()
            }
        }
    }

    func configureLiveHub(identity: String?) {
        let normalized = identity?.trimmingCharacters(in: .whitespacesAndNewlines)
        if normalized != liveHubIdentity {
            engineSessionGeneration &+= 1
        }
        liveHubIdentity = normalized
        if pendingRelocation?.hubIdentity == liveHubIdentity {
            Task { [weak self] in
                _ = await self?.retryPendingRelocation()
            }
        }
        resumeHubActivationWaitersIfSafe()
    }

    var hubActivationIsSafe: Bool {
        Self.isHubActivationSafe(
            engineOperationsInFlight: engineOperationsInFlight,
            engineActivationInFlight: engineActivationInFlight,
            relayOperationInFlight: relayOperationAdmission.isBusy,
            backgroundFlushInFlight: backgroundFlushesInFlight > 0,
            relocationInFlight: relocationDeliveryInFlight,
            bootstrapInFlight: hubBootstrapInFlight,
            pendingPrepared: pendingRelocation.map { $0.prepared != nil },
            hasLiveHubIdentity: liveHubIdentity != nil
        )
    }

    /// An unprepared record means the engine mutation may already exist but
    /// its exact two-note frame has not reached durable outbox state yet. A
    /// profile switch must leave the current hub attached until that staging
    /// finishes. On a cold launch there is no attached hub, so activation may
    /// proceed only far enough to re-bind this exact identity and resume it.
    var unpreparedRelocationHubIdentity: String? {
        guard let pendingRelocation, pendingRelocation.prepared == nil else {
            return nil
        }
        return pendingRelocation.hubIdentity
    }

    var unpreparedRelocationEngineScope: MosaicEngineScope? {
        guard let pendingRelocation, pendingRelocation.prepared == nil else {
            return nil
        }
        return pendingRelocation.engineScope
    }

    static func isHubActivationSafe(
        engineOperationsInFlight: Int,
        engineActivationInFlight: Bool = false,
        relayOperationInFlight: Bool = false,
        backgroundFlushInFlight: Bool = false,
        relocationInFlight: Bool,
        bootstrapInFlight: Int,
        pendingPrepared: Bool?,
        hasLiveHubIdentity: Bool
    ) -> Bool {
        guard engineOperationsInFlight == 0,
              !engineActivationInFlight,
              !relayOperationInFlight,
              !backgroundFlushInFlight,
              !relocationInFlight,
              bootstrapInFlight == 0
        else { return false }
        // nil = no pending move; true = the exact frame is already durable.
        // A cold launch (no live identity) must be allowed to re-activate the
        // pending move's own hub or an unprepared outbox would deadlock before
        // it could ever be staged.
        return pendingPrepared != false || !hasLiveHubIdentity
    }

    func waitUntilHubActivationIsSafe() async {
        guard !hubActivationIsSafe else { return }
        await withCheckedContinuation { continuation in
            hubActivationWaiters.append(continuation)
        }
    }

    private func finishRelocationDelivery() {
        relocationDeliveryInFlight = false
        let waiters = relocationIdleWaiters
        relocationIdleWaiters.removeAll()
        for waiter in waiters {
            waiter.resume()
        }
        resumeHubActivationWaitersIfSafe()
    }

    private func waitUntilRelocationDeliveryFinishes() async {
        guard relocationDeliveryInFlight else { return }
        await withCheckedContinuation { continuation in
            relocationIdleWaiters.append(continuation)
        }
    }

    private func finishHubBootstrap() {
        hubBootstrapInFlight = max(0, hubBootstrapInFlight - 1)
        resumeHubActivationWaitersIfSafe()
    }

    private func finishEngineOperation() {
        engineOperationAdmission.finishOperation()
        resumeHubActivationWaitersIfSafe()
    }

    private func finishExclusiveEngineAccess(
        _ lease: EngineOperationAdmission.ExclusiveLease
    ) {
        engineOperationAdmission.finishExclusiveAccess(lease)
        resumeHubActivationWaitersIfSafe()
    }

    private func finishRelayOperation(_ lease: RelayOperationAdmission.Lease) {
        relayOperationAdmission.release(lease)
        resumeHubActivationWaitersIfSafe()
    }

    private func tryAcquireImmediateOutbound() -> RelayOperationAdmission.Lease? {
        guard pendingRelocation == nil else { return nil }
        return relayOperationAdmission.tryAcquire(.immediateOutbound)
    }

    private func beginBackgroundFlush() {
        backgroundFlushesInFlight += 1
    }

    private func finishBackgroundFlush() {
        backgroundFlushesInFlight = max(0, backgroundFlushesInFlight - 1)
        if backgroundFlushesInFlight == 0 {
            let waiters = backgroundFlushWaiters
            backgroundFlushWaiters.removeAll()
            for waiter in waiters {
                waiter.resume()
            }
        }
        resumeHubActivationWaitersIfSafe()
    }

    private func waitUntilBackgroundFlushesFinish() async {
        guard backgroundFlushesInFlight > 0 else { return }
        await withCheckedContinuation { continuation in
            backgroundFlushWaiters.append(continuation)
        }
    }

    private func waitUntilEngineIsIdle() async {
        await engineOperationAdmission.waitUntilIdle()
    }

    private func validatedEngineSession(
        requiredSession: EngineSessionToken?
    ) -> EngineSessionToken? {
        let session = requiredSession ?? engineSessionToken
        guard session == engineSessionToken else { return nil }
        // During activation hubMode is fail-closed before a verified identity
        // is published. A queued UI task from the prior profile must not open
        // or mutate whichever engine happens to become active next.
        if session.hubMode, session.hubIdentity == nil { return nil }
        return session
    }

    private func beginEngineOperationWhenAvailable(
        requiredSession: EngineSessionToken?
    ) async -> EngineSessionToken? {
        guard let session = validatedEngineSession(requiredSession: requiredSession),
              await engineOperationAdmission.beginOperationWhenAvailable()
        else { return nil }
        guard isCurrentEngineSession(session) else {
            finishEngineOperation()
            return nil
        }
        return session
    }

    /// Synchronous callers reserve immediately while admission is open. If a
    /// relocation has already closed the gate, preserve the exact edit and
    /// session in a queued task instead of silently dropping it.
    private func enqueueEngineOperation(
        requiredSession: EngineSessionToken?,
        operation: @MainActor @escaping (EngineSessionToken) async -> Void
    ) {
        guard let session = validatedEngineSession(requiredSession: requiredSession) else {
            return
        }
        switch engineOperationAdmission.reserveOperation() {
        case .active:
            Task { @MainActor [self] in
                defer { finishEngineOperation() }
                guard isCurrentEngineSession(session) else { return }
                await operation(session)
            }
        case .queued:
            Task { @MainActor [self] in
                guard await engineOperationAdmission.beginReservedOperationWhenAvailable() else {
                    return
                }
                defer { finishEngineOperation() }
                guard isCurrentEngineSession(session) else { return }
                await operation(session)
            }
        }
    }

    private func isCurrentEngineSession(_ session: EngineSessionToken) -> Bool {
        Self.isEngineSessionCurrent(required: session, current: engineSessionToken)
    }

    static func isEngineSessionCurrent(
        required: EngineSessionToken,
        current: EngineSessionToken
    ) -> Bool {
        required == current
    }

    private func resumeHubActivationWaitersIfSafe() {
        guard hubActivationIsSafe else { return }
        let waiters = hubActivationWaiters
        hubActivationWaiters.removeAll()
        for waiter in waiters {
            waiter.resume()
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
    func bootstrapNoteIfNeeded(
        slug: String,
        requiredHubIdentity: String? = nil
    ) async {
        guard let engine else { return }
        guard let mosaic else { return }
        if let requiredHubIdentity, requiredHubIdentity != liveHubIdentity {
            return
        }
        let boundHubIdentity = requiredHubIdentity ?? (hubMode ? liveHubIdentity : nil)
        if hubMode, boundHubIdentity == nil { return }
        hubBootstrapInFlight += 1
        defer { finishHubBootstrap() }
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
            if let boundHubIdentity, boundHubIdentity != liveHubIdentity {
                return
            }
            try await engine.importNoteSnapshot(slug: slug, bytes: bytes)
            if let boundHubIdentity, boundHubIdentity != liveHubIdentity {
                return
            }
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

    func recordAndPush(
        slug: String,
        title: String,
        content: String,
        createdAtMillis: Int64,
        requiredSession: EngineSessionToken? = nil
    ) async {
        guard let session = await beginEngineOperationWhenAvailable(
            requiredSession: requiredSession
        ) else {
            return
        }
        defer { finishEngineOperation() }
        await recordAndPushUnderLease(
            slug: slug,
            title: title,
            content: content,
            createdAtMillis: createdAtMillis,
            session: session
        )
    }

    /// Reserve the activation barrier synchronously, before the unstructured
    /// task is enqueued. The service's legacy `onLocalWrite` seam is
    /// synchronous; reserving inside the task leaves a switch window where
    /// activation can observe zero operations and silently discard the edit.
    func enqueueRecordAndPush(
        slug: String,
        title: String,
        content: String,
        createdAtMillis: Int64,
        requiredSession: EngineSessionToken? = nil
    ) {
        enqueueEngineOperation(requiredSession: requiredSession) { [self] session in
            await recordAndPushUnderLease(
                slug: slug,
                title: title,
                content: content,
                createdAtMillis: createdAtMillis,
                session: session
            )
        }
    }

    private func recordAndPushUnderLease(
        slug: String,
        title: String,
        content: String,
        createdAtMillis: Int64,
        session: EngineSessionToken
    ) async {
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
        await bootstrapNoteIfNeeded(
            slug: slug,
            requiredHubIdentity: session.hubIdentity
        )
        guard isCurrentEngineSession(session) else { return }
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
        if session.hubMode {
            await deliverHubDelta(slug: slug, session: session)
            return
        }
        // Best-effort push: if the coordinator is ready (i.e. we've paired
        // with the Mac at least once), drain the op to the relay
        // immediately so the other side sees it without waiting a full
        // tick. If pairing hasn't happened or the network is down, the
        // regular tick loop will catch up later.
        guard let relayLease = tryAcquireImmediateOutbound() else {
            return
        }
        defer { finishRelayOperation(relayLease) }
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

    /// Finish a hub-mode local write inside the same engine-operation lease
    /// that recorded it. The exact hub identity and engine scope are captured
    /// before the first await, so activation cannot redirect the frame to a
    /// newly selected profile. The baseline advances only after the socket's
    /// path-bound barrier acknowledgement succeeds.
    private func deliverHubDelta(
        slug: String,
        session: EngineSessionToken
    ) async {
        guard session.hubMode,
              let hubIdentity = session.hubIdentity,
              let liveDeltaSender,
              isCurrentEngineSession(session),
              let prepared = await produceDeltaFrame(slug: slug),
              isCurrentEngineSession(session)
        else { return }
        let delivered = await liveDeltaSender(prepared.frame, hubIdentity)
        guard delivered, isCurrentEngineSession(session) else { return }
        await commitPushedDelta(prepared)
    }

    /// Move one complete block subtree through the authoritative Rust engine.
    /// The request's move id is supplied by the sheet and remains stable across
    /// exact retries. No Swift-side block arrays are edited here.
    func relocateSubtree(
        _ request: BlockMoveRequest,
        requiredHubIdentity: String? = nil
    ) async throws -> BlockRelocationOutcomeRecord {
        do {
            if let requiredHubIdentity, requiredHubIdentity != liveHubIdentity {
                throw FfiSyncError.RelocationRecoveryRequired(
                    moveId: request.moveId,
                    message: "The active mosaic changed before this move could be staged. Switch back and retry the exact move."
                )
            }
            try await openEngineIfNeeded()
            guard let engine else {
                throw FfiSyncError.Other(message: "engine open failed")
            }

            let slugs = Array(Set([request.sourceSlug, request.destinationSlug])).sorted()
            for slug in slugs {
                await bootstrapNoteIfNeeded(
                    slug: slug,
                    requiredHubIdentity: requiredHubIdentity
                )
                if let requiredHubIdentity, requiredHubIdentity != liveHubIdentity {
                    throw FfiSyncError.RelocationRecoveryRequired(
                        moveId: request.moveId,
                        message: "The active mosaic changed while this move was being staged. Switch back and retry the exact move."
                    )
                }
                if lastPushedVV[slug] == nil {
                    lastPushedVV[slug] = await engine.noteVersion(slug: slug)
                }
            }

            if let requiredHubIdentity, requiredHubIdentity != liveHubIdentity {
                throw FfiSyncError.RelocationRecoveryRequired(
                    moveId: request.moveId,
                    message: "The active mosaic changed before this move could be applied. Switch back and retry the exact move."
                )
            }
            let outcome = try await engine.relocateBlockSubtree(
                request: BlockRelocationRecord(
                    moveId: request.moveId,
                    sourceSlug: request.sourceSlug,
                    rootBid: request.rootBid,
                    destinationSlug: request.destinationSlug,
                    targetBid: nil,
                    placement: .append
                )
            )

            if !hubMode,
               let relayLease = tryAcquireImmediateOutbound() {
                defer { finishRelayOperation(relayLease) }
                invalidateCoordinatorIfRepaired()
                if coordinator == nil {
                    try? await ensureCoordinator()
                }
                if let coordinator {
                    do {
                        let outbound = try await coordinator.tickOutbound(maxBytes: 1_000_000)
                        noteOutboundOutcome(outbound)
                    } catch {
                        lastError = error.localizedDescription
                    }
                }
            }
            return outcome
        } catch {
            lastError = error.localizedDescription
            throw error
        }
    }

    /// Apply and deliver one relocation as a durable two-note unit. Hub mode
    /// writes the request to an atomic on-disk outbox before touching the
    /// engine, then keeps the exact prepared frame until the server's barrier
    /// acknowledgement succeeds. Relay mode already has its own durable
    /// engine-backed outbound queue, so it only performs the relocation.
    func moveSubtreeAndDeliver(_ request: BlockMoveRequest) async throws -> [String] {
        if let relocationOutboxLoadError {
            throw FfiSyncError.Other(
                message: "A saved block move could not be recovered: \(relocationOutboxLoadError)"
            )
        }

        if let pendingRelocation, pendingRelocation.request != request {
            guard hubMode else {
                throw FfiSyncError.Other(
                    message: "A previous block move is still waiting for its desktop. Switch back before starting another move."
                )
            }
            guard await retryPendingRelocation() else {
                throw FfiSyncError.Other(
                    message: "A previous block move is still waiting for the desktop. Reconnect before starting another move."
                )
            }
        }

        if pendingRelocation != nil, !hubMode {
            throw FfiSyncError.Other(
                message: "A saved block move is still waiting for its desktop. Switch back before starting another move."
            )
        }

        guard !relocationDeliveryInFlight else {
            throw FfiSyncError.RelocationRecoveryRequired(
                moveId: request.moveId,
                message: "This move is already being delivered. Retry the same move after it finishes."
            )
        }
        relocationDeliveryInFlight = true
        defer { finishRelocationDelivery() }
        guard let relocationLease = await relayOperationAdmission.acquireWhenIdle(.relocation) else {
            throw FfiSyncError.RelocationRecoveryRequired(
                moveId: request.moveId,
                message: "The move was cancelled before its local transaction could start. Retry the same move."
            )
        }
        defer { finishRelayOperation(relocationLease) }
        guard let engineExclusiveLease = engineOperationAdmission.closeForExclusiveAccess() else {
            throw FfiSyncError.RelocationRecoveryRequired(
                moveId: request.moveId,
                message: "Another local transaction is still closing. Retry this exact move."
            )
        }
        defer { finishExclusiveEngineAccess(engineExclusiveLease) }
        await engineOperationAdmission.waitUntilExclusiveAccessReady(engineExclusiveLease)

        guard hubMode else {
            return try await relocateSubtree(request).notes.map(\.slug)
        }

        if pendingRelocation == nil {
            guard let liveHubIdentity else {
                throw FfiSyncError.Other(message: "No live desktop sync destination is configured.")
            }
            let delivery = PendingRelocationDelivery(
                hubIdentity: liveHubIdentity,
                request: request,
                prepared: nil,
                engineScope: activeEngineScope
            )
            do {
                try relocationOutboxStore.save(delivery)
                pendingRelocation = delivery
            } catch {
                throw FfiSyncError.Other(
                    message: "The move could not be queued safely: \(error.localizedDescription)"
                )
            }
        }

        do {
            return try await deliverPendingRelocation()
        } catch let relocationError as FfiSyncError {
            if Self.isTerminalRelocationError(relocationError) {
                do {
                    try clearPendingRelocation()
                } catch {
                    scheduleRelocationRetry()
                    throw FfiSyncError.RelocationRecoveryRequired(
                        moveId: request.moveId,
                        message: "The desktop rejected this move, but its saved delivery record could not be cleared. Retry this exact move."
                    )
                }
                throw relocationError
            }
            switch relocationError {
            case .RelocationRecoveryRequired:
                scheduleRelocationRetry()
                throw relocationError
            default:
                scheduleRelocationRetry()
                throw FfiSyncError.RelocationRecoveryRequired(
                    moveId: request.moveId,
                    message: "The move is saved locally but is still waiting for the desktop. Retry this exact move."
                )
            }
        } catch {
            scheduleRelocationRetry()
            throw FfiSyncError.RelocationRecoveryRequired(
                moveId: request.moveId,
                message: "The move is saved locally but is still waiting for the desktop. Retry this exact move."
            )
        }
    }

    @discardableResult
    func retryPendingRelocation() async -> Bool {
        guard hubMode, pendingRelocation != nil else { return pendingRelocation == nil }
        guard !relocationDeliveryInFlight else { return false }
        relocationDeliveryInFlight = true
        defer { finishRelocationDelivery() }
        guard let relocationLease = await relayOperationAdmission.acquireWhenIdle(.relocation) else {
            return false
        }
        defer { finishRelayOperation(relocationLease) }
        guard let engineExclusiveLease = engineOperationAdmission.closeForExclusiveAccess() else {
            return false
        }
        defer { finishExclusiveEngineAccess(engineExclusiveLease) }
        await engineOperationAdmission.waitUntilExclusiveAccessReady(engineExclusiveLease)

        do {
            let changed = try await deliverPendingRelocation()
            if !changed.isEmpty { onAppliedChanges?() }
            lastError = nil
            return true
        } catch let relocationError as FfiSyncError {
            if Self.isTerminalRelocationError(relocationError) {
                do {
                    try clearPendingRelocation()
                    lastError = relocationError.localizedDescription
                    return true
                } catch {
                    lastError = "A rejected saved block move could not be cleared: \(error.localizedDescription)"
                    scheduleRelocationRetry()
                    return false
                }
            }
            lastError = relocationError.localizedDescription
            scheduleRelocationRetry()
            return false
        } catch {
            lastError = error.localizedDescription
            scheduleRelocationRetry()
            return false
        }
    }

    var hasPendingRelocation: Bool {
        pendingRelocation != nil
    }

    private func deliverPendingRelocation() async throws -> [String] {
        guard var delivery = pendingRelocation else { return [] }
        guard let deliveryScope = delivery.engineScope else {
            // Build 78 did not persist the physical group namespace. A URL,
            // profile, and path can all be reused after the mosaic at that
            // path is replaced, so adopting this frame into the active scope
            // would be a fail-open cross-group mutation. Clear it as a
            // terminal conflict and require the user to issue a fresh move.
            throw FfiSyncError.RelocationConflict(
                message: "A block move saved by an older build cannot be matched safely to this mosaic. It was not replayed; retry the move."
            )
        }
        guard delivery.hubIdentity == liveHubIdentity else {
            throw FfiSyncError.Other(
                message: "This block move belongs to another mosaic. Switch back to its desktop before retrying."
            )
        }
        if deliveryScope != activeEngineScope {
            throw FfiSyncError.Other(
                message: "This block move belongs to another local mosaic. Switch back before retrying."
            )
        }

        let prepared: PreparedDeltaFrameRecord
        if let staged = delivery.prepared {
            prepared = staged.prepared
        } else {
            let outcome: BlockRelocationOutcomeRecord
            do {
                outcome = try await relocateSubtree(
                    delivery.request,
                    requiredHubIdentity: delivery.hubIdentity
                )
            } catch let error as FfiSyncError {
                switch error {
                case .RelocationRejected, .RelocationConflict:
                    throw error
                case .RelocationRecoveryRequired:
                    throw error
                default:
                    throw FfiSyncError.RelocationRecoveryRequired(
                        moveId: delivery.request.moveId,
                        message: "The move may already be saved locally. Retry this exact move."
                    )
                }
            }

            do {
                guard delivery.hubIdentity == liveHubIdentity else {
                    throw FfiSyncError.RelocationRecoveryRequired(
                        moveId: delivery.request.moveId,
                        message: "The active mosaic changed before this move's sync frame could be prepared. Switch back and retry the exact move."
                    )
                }
                guard let frame = try await prepareRelocationDeltaFrame(outcome) else {
                    throw FfiSyncError.RelocationRecoveryRequired(
                        moveId: delivery.request.moveId,
                        message: "The move is saved locally but its sync frame could not be prepared. Retry this exact move."
                    )
                }
                guard delivery.hubIdentity == liveHubIdentity else {
                    throw FfiSyncError.RelocationRecoveryRequired(
                        moveId: delivery.request.moveId,
                        message: "The active mosaic changed while this move's sync frame was being prepared. Switch back and retry the exact move."
                    )
                }
                prepared = frame
                delivery.prepared = PendingRelocationFrame(frame)
                try relocationOutboxStore.save(delivery)
                pendingRelocation = delivery
            } catch let error as FfiSyncError {
                if case .RelocationRecoveryRequired = error { throw error }
                throw FfiSyncError.RelocationRecoveryRequired(
                    moveId: delivery.request.moveId,
                    message: "The move is saved locally but its sync frame could not be prepared. Retry this exact move."
                )
            } catch {
                throw FfiSyncError.RelocationRecoveryRequired(
                    moveId: delivery.request.moveId,
                    message: "The move is saved locally but its sync frame could not be persisted. Retry this exact move."
                )
            }
        }

        guard delivery.hubIdentity == liveHubIdentity else {
            throw FfiSyncError.RelocationRecoveryRequired(
                moveId: delivery.request.moveId,
                message: "The active mosaic changed while preparing this move. Switch back and retry the exact move."
            )
        }
        guard let liveDeltaSender,
              await liveDeltaSender(prepared.frame, delivery.hubIdentity) else {
            throw FfiSyncError.RelocationRecoveryRequired(
                moveId: delivery.request.moveId,
                message: "The move is saved locally but could not reach the desktop. Reconnect and retry this exact move."
            )
        }
        guard delivery.hubIdentity == liveHubIdentity else {
            throw FfiSyncError.RelocationRecoveryRequired(
                moveId: delivery.request.moveId,
                message: "The active mosaic changed while delivering this move. Switch back and retry the exact move."
            )
        }

        await commitPushedDelta(prepared)
        do {
            try clearPendingRelocation()
        } catch {
            throw FfiSyncError.RelocationRecoveryRequired(
                moveId: delivery.request.moveId,
                message: "The desktop confirmed the move, but local delivery cleanup failed. Retry this exact move."
            )
        }
        return [delivery.request.sourceSlug, delivery.request.destinationSlug]
    }

    static func clearPendingRelocationState(
        _ pending: inout PendingRelocationDelivery?,
        removePersistedState: () throws -> Void
    ) throws {
        try removePersistedState()
        pending = nil
    }

    static func isTerminalRelocationError(_ error: FfiSyncError) -> Bool {
        switch error {
        case .RelocationRejected, .RelocationConflict:
            return true
        default:
            return false
        }
    }

    private func clearPendingRelocation() throws {
        let store = relocationOutboxStore
        try Self.clearPendingRelocationState(&pendingRelocation) {
            try store.clear()
        }
        relocationOutboxLoadError = nil
        relocationRetryTask?.cancel()
        relocationRetryTask = nil
        relocationRetryDelaySeconds = 2
    }

    private func scheduleRelocationRetry() {
        guard pendingRelocation != nil, relocationRetryTask == nil else { return }
        let delay = relocationRetryDelaySeconds
        relocationRetryDelaySeconds = min(relocationRetryDelaySeconds * 2, 32)
        relocationRetryTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: delay * 1_000_000_000)
            guard let self, !Task.isCancelled else { return }
            self.relocationRetryTask = nil
            _ = await self.retryPendingRelocation()
        }
    }

    static func hubIdentity(
        serverURL: String,
        profileID: UUID?,
        mosaicPath: String?,
        groupIdHex: String? = nil
    ) -> String {
        let url = serverURL.trimmingCharacters(in: .whitespacesAndNewlines)
            .trimmingCharacters(in: CharacterSet(charactersIn: "/"))
            .lowercased()
        let legacy = "\(url)|\(profileID?.uuidString.lowercased() ?? "legacy")|\(mosaicPath ?? "")"
        guard let groupIdHex else { return legacy }
        return "\(legacy)|\(groupIdHex.lowercased())"
    }

    /// Prepare one destination-first live frame for every note changed by a
    /// relocation. Each note exports from the transaction's exact pre-version,
    /// and the returned versions are the only baselines safe to commit after
    /// the frame is confirmed sent.
    func prepareRelocationDeltaFrame(
        _ outcome: BlockRelocationOutcomeRecord
    ) async throws -> PreparedDeltaFrameRecord? {
        try await openEngineIfNeeded()
        guard let engine else {
            throw FfiSyncError.Other(message: "engine open failed")
        }
        let requests = outcome.notes
            .reversed()
            .filter(\.changed)
            .map {
                NoteDeltaRequestRecord(
                    slug: $0.slug,
                    sinceVersion: $0.preVersion
                )
            }
        guard !requests.isEmpty else { return nil }
        return try await engine.prepareDeltaFrame(requests: requests)
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
        if Self.isSuccessfulPush(opsSent: outcome.opsSent, batchesFailed: outcome.batchesFailed) {
            lastSuccessfulPushAt = Date()
        }
    }

    /// Pure predicate: does this outbound-tick outcome represent an actual
    /// successful PUSH — as opposed to an empty poll (`opsSent == 0`, e.g.
    /// nothing new authored) or a failed batch? Extracted for unit testing
    /// (tesela-4mc: Settings → Sync's "last-successful-push age" must not
    /// advance on a tick that sent nothing or that failed to deliver).
    static func isSuccessfulPush(opsSent: UInt32, batchesFailed: UInt32) -> Bool {
        opsSent > 0 && batchesFailed == 0
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
        insert: String,
        requiredSession: EngineSessionToken? = nil
    ) async {
        guard let session = await beginEngineOperationWhenAvailable(
            requiredSession: requiredSession
        ) else {
            return
        }
        defer { finishEngineOperation() }
        await spliceAndPushUnderLease(
            slug: slug,
            blockIdHex: blockIdHex,
            utf16Offset: utf16Offset,
            utf16DeleteLen: utf16DeleteLen,
            insert: insert,
            session: session
        )
    }

    /// Synchronous reservation companion to `enqueueRecordAndPush`; closes
    /// the same pre-task activation window for character splices.
    func enqueueSpliceAndPush(
        slug: String,
        blockIdHex: String,
        utf16Offset: Int,
        utf16DeleteLen: Int,
        insert: String,
        requiredSession: EngineSessionToken? = nil
    ) {
        enqueueEngineOperation(requiredSession: requiredSession) { [self] session in
            await spliceAndPushUnderLease(
                slug: slug,
                blockIdHex: blockIdHex,
                utf16Offset: utf16Offset,
                utf16DeleteLen: utf16DeleteLen,
                insert: insert,
                session: session
            )
        }
    }

    private func spliceAndPushUnderLease(
        slug: String,
        blockIdHex: String,
        utf16Offset: Int,
        utf16DeleteLen: Int,
        insert: String,
        session: EngineSessionToken
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
        await bootstrapNoteIfNeeded(
            slug: slug,
            requiredHubIdentity: session.hubIdentity
        )
        guard isCurrentEngineSession(session) else { return }
        // Part A: seed the per-note push floor BEFORE recording this edit
        // so the first delta exports `updates(floor)` = this edit only,
        // not a full snapshot. Seed only when absent (don't regress a
        // baseline a prior bootstrap/commit already advanced).
        if lastPushedVV[slug] == nil {
            lastPushedVV[slug] = await engine.noteVersion(slug: slug)
        }
        let applied: UInt32
        do {
            // Capture the op count (build-50 diagnostic): spliceBlockText
            // returns Ok(0) — NOT a throw — when the block isn't a live tree
            // node, which previously was silently discarded (`_ =`).
            applied = try await engine.spliceBlockText(
                slug: slug,
                blockIdHex: blockIdHex,
                utf16Offset: UInt32(max(0, utf16Offset)),
                utf16DeleteLen: UInt32(max(0, utf16DeleteLen)),
                insert: insert
            )
        } catch {
            lastSpliceDiag = "slug=\(slug) bid=\(blockIdHex.prefix(8)) ERR \(error.localizedDescription)"
            lastError = error.localizedDescription
            return
        }
        lastSpliceDiag = "slug=\(slug) bid=\(blockIdHex.prefix(8)) applied=\(applied)"
        // Engine durability is guaranteed. In hub mode the live `/ws`
        // socket owns delivery (the caller pushes a delta after this
        // returns), so the relay coordinator must NOT also drain this op.
        if session.hubMode {
            await deliverHubDelta(slug: slug, session: session)
            return
        }
        guard let relayLease = tryAcquireImmediateOutbound() else {
            lastSpliceDiag += " queued"
            return
        }
        defer { finishRelayOperation(relayLease) }
        invalidateCoordinatorIfRepaired()
        if coordinator == nil {
            try? await ensureCoordinator()
        }
        guard let coordinator else {
            lastSpliceDiag += " no-coordinator"
            return
        }
        do {
            let outcome = try await coordinator.tickOutbound(maxBytes: 1_000_000)
            noteOutboundOutcome(outcome)
            lastSpliceDiag += " sent=\(outcome.opsSent) failed=\(outcome.batchesFailed)"
        } catch {
            lastError = error.localizedDescription
            lastSpliceDiag += " tickErr"
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
        value: String,
        valueType: String? = nil,
        requiredSession: EngineSessionToken? = nil
    ) async -> Bool {
        guard let session = await beginEngineOperationWhenAvailable(
            requiredSession: requiredSession
        ) else {
            return false
        }
        defer { finishEngineOperation() }
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
        await bootstrapNoteIfNeeded(
            slug: slug,
            requiredHubIdentity: session.hubIdentity
        )
        guard isCurrentEngineSession(session) else { return false }
        // Part A: seed the per-note push floor BEFORE recording so the
        // next delta exports only this edit, never a full snapshot.
        if lastPushedVV[slug] == nil {
            lastPushedVV[slug] = await engine.noteVersion(slug: slug)
        }
        let applied: UInt32
        do {
            if let valueType {
                applied = try await engine.setBlockPropertyTyped(
                    slug: slug,
                    blockIdHex: bidHex,
                    key: key,
                    valueType: valueType,
                    value: value
                )
            } else {
                applied = try await engine.setBlockProperty(
                    slug: slug,
                    blockIdHex: bidHex,
                    key: key,
                    value: value
                )
            }
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
        if session.hubMode {
            await deliverHubDelta(slug: slug, session: session)
            return true
        }
        guard let relayLease = tryAcquireImmediateOutbound() else {
            return true
        }
        defer { finishRelayOperation(relayLease) }
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

    /// Multi-select sibling of `setBlockPropertyAndPush`. The bridge records
    /// independent LoroList member ops (including the visible baseline needed
    /// to promote legacy in-text values), then uses the identical hub/relay
    /// delivery tail as every other property mutation.
    func updateBlockPropertyListAndPush(
        slug: String,
        bidHex: String,
        key: String,
        current: [String],
        add: [String],
        remove: [String],
        requiredSession: EngineSessionToken? = nil
    ) async -> Bool {
        guard let session = await beginEngineOperationWhenAvailable(
            requiredSession: requiredSession
        ) else {
            return false
        }
        defer { finishEngineOperation() }
        do {
            try await openEngineIfNeeded()
        } catch {
            lastError = error.localizedDescription
            return false
        }
        guard let engine else { return false }
        await bootstrapNoteIfNeeded(
            slug: slug,
            requiredHubIdentity: session.hubIdentity
        )
        guard isCurrentEngineSession(session) else { return false }
        if lastPushedVV[slug] == nil {
            lastPushedVV[slug] = await engine.noteVersion(slug: slug)
        }
        let applied: UInt32
        do {
            applied = try await engine.updateBlockPropertyList(
                slug: slug,
                blockIdHex: bidHex,
                key: key,
                current: current,
                add: add,
                remove: remove
            )
        } catch {
            lastError = error.localizedDescription
            return false
        }
        guard applied == 1 else {
            lastError = "property list write: block \(bidHex) not found in \(slug)"
            return false
        }
        if session.hubMode {
            await deliverHubDelta(slug: slug, session: session)
            return true
        }
        guard let relayLease = tryAcquireImmediateOutbound() else {
            return true
        }
        defer { finishRelayOperation(relayLease) }
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
        guard await beginEngineOperationWhenAvailable(requiredSession: nil) != nil else {
            return nil
        }
        defer { finishEngineOperation() }
        do {
            try await openEngineIfNeeded()
        } catch {
            lastError = error.localizedDescription
            return nil
        }
        guard let engine else { return nil }
        let hasPairing = KeychainPairingCache.load() != nil
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

    /// Every page's index entry (id/title/slug/tags) from the engine's
    /// always-resident Loro index — the COMPLETE page list for this mosaic,
    /// including notes never materialized to local disk on this device.
    /// Powers `[[` link autocomplete. nil when the engine can't open.
    func indexEntries() async -> [IndexEntryRecord]? {
        guard await beginEngineOperationWhenAvailable(requiredSession: nil) != nil else {
            return nil
        }
        defer { finishEngineOperation() }
        do {
            try await openEngineIfNeeded()
        } catch {
            lastError = error.localizedDescription
            return nil
        }
        guard let engine else { return nil }
        return await engine.indexEntries()
    }

    /// Create/update a saved view in the engine's registry and drain the
    /// op to the relay so other devices converge. Throws when the engine
    /// can't open or the upsert is rejected — the editor surfaces the
    /// message instead of pretending the save landed.
    func viewsUpsertAndPush(
        _ record: ViewRecord,
        requiredSession: EngineSessionToken? = nil
    ) async throws {
        guard await beginEngineOperationWhenAvailable(requiredSession: requiredSession) != nil else {
            throw FfiSyncError.Other(message: "mosaic activation changed before the view save began")
        }
        defer { finishEngineOperation() }
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
    func viewsDeleteAndPush(
        viewId: String,
        requiredSession: EngineSessionToken? = nil
    ) async throws {
        guard await beginEngineOperationWhenAvailable(requiredSession: requiredSession) != nil else {
            throw FfiSyncError.Other(message: "mosaic activation changed before the view delete began")
        }
        defer { finishEngineOperation() }
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
        guard let relayLease = tryAcquireImmediateOutbound() else {
            return
        }
        defer { finishRelayOperation(relayLease) }
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
        guard await beginEngineOperationWhenAvailable(requiredSession: nil) != nil else {
            return nil
        }
        defer { finishEngineOperation() }
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
    func applyInboundDelta(
        _ frame: Data,
        requiredHubIdentity: String? = nil
    ) async -> Bool {
        let requiredSession: EngineSessionToken?
        if let requiredHubIdentity {
            let current = engineSessionToken
            guard current.hubMode,
                  current.hubIdentity == requiredHubIdentity
            else { return false }
            requiredSession = current
        } else {
            requiredSession = nil
        }
        guard let session = await beginEngineOperationWhenAvailable(
            requiredSession: requiredSession
        ) else {
            return false
        }
        defer { finishEngineOperation() }
        if session.hubMode, requiredHubIdentity == nil { return false }
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
                await bootstrapNoteIfNeeded(
                    slug: slug,
                    requiredHubIdentity: session.hubIdentity
                )
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
    func produceDeltaFrame(slug: String) async -> PreparedDeltaFrameRecord? {
        do {
            try await openEngineIfNeeded()
        } catch {
            lastError = error.localizedDescription
            return nil
        }
        guard let engine else { return nil }
        let sinceVv = lastPushedVV[slug]
        do {
            return try await engine.prepareDeltaFrame(requests: [
                NoteDeltaRequestRecord(slug: slug, sinceVersion: sinceVv)
            ])
        } catch {
            lastError = error.localizedDescription
            return nil
        }
    }

    /// Advance the per-note delta baseline AFTER a frame produced by
    /// [`produceDeltaFrame(slug:)`] was confirmed sent over the live WS.
    /// Commits only the exact per-note versions captured with `prepared`.
    /// Never re-reads the engine after the network await: a concurrent edit
    /// may already be newer than this frame and must remain pending.
    func commitPushedDelta(_ prepared: PreparedDeltaFrameRecord) async {
        lastPushedVV = Self.baselinesAfterDelivery(
            existing: lastPushedVV,
            prepared: prepared,
            delivered: true
        )
    }

    static func baselinesAfterDelivery(
        existing: [String: Data],
        prepared: PreparedDeltaFrameRecord,
        delivered: Bool
    ) -> [String: Data] {
        guard delivered else { return existing }
        var updated = existing
        for note in prepared.notes {
            updated[note.slug] = note.version
        }
        return updated
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

    /// Foreground entry point: resume prompt syncing when the app returns
    /// to the foreground. `start()` alone no-ops when a loop already
    /// exists — even if that loop is parked in a backoff `Task.sleep`
    /// (now ≤60s, formerly hours) — so a bare `.active → start()` left the
    /// user waiting out the backoff before any pending edit pushed. `wake()`
    /// resets the error count and tears the loop down + restarts it, so the
    /// fresh loop ticks IMMEDIATELY at base cadence. Idempotent and safe to
    /// call on every `.active`.
    func wake() {
        consecutiveErrors = 0
        stop()
        start()
    }

    /// Suspend the foreground transport before scheduling the final relay
    /// flush. A hub session becomes relay-eligible only while its socket is
    /// suspended, and only against the already-active group scope.
    static func canSuspendForBackgroundRelay(
        relocationInFlight: Bool,
        hasPendingRelocation: Bool
    ) -> Bool {
        !relocationInFlight && !hasPendingRelocation
    }

    static func shouldCommitBackgroundTransition(
        issuedGeneration: UInt64,
        currentGeneration: UInt64
    ) -> Bool {
        issuedGeneration == currentGeneration
    }

    func suspendForBackground() {
        stop()
        guard Self.canSuspendForBackgroundRelay(
            relocationInFlight: relocationDeliveryInFlight,
            hasPendingRelocation: pendingRelocation != nil
        ) else { return }
        guard suspendedHubLease == nil,
              hubMode,
              let liveHubIdentity
        else { return }
        suspendedHubLease = SuspendedHubLease(
            scope: activeEngineScope,
            hubIdentity: liveHubIdentity
        )
        hubMode = false
    }

    /// Wait for any APNs/BG relay operation to release the shared engine,
    /// then restore foreground hub routing before reconnecting the socket.
    func resumeFromBackground() async {
        backgroundTransitionGeneration &+= 1
        guard let lease = suspendedHubLease else { return }
        while backgroundFlushesInFlight > 0
            || relayOperationAdmission.isBusy
            || engineOperationsInFlight > 0 {
            if backgroundFlushesInFlight > 0 {
                await waitUntilBackgroundFlushesFinish()
            }
            if relayOperationAdmission.isBusy {
                await relayOperationAdmission.waitUntilIdle()
            }
            if engineOperationsInFlight > 0 {
                await waitUntilEngineIsIdle()
            }
        }
        suspendedHubLease = nil
        guard !hubMode,
              activeEngineScope == lease.scope,
              liveHubIdentity == lease.hubIdentity
        else { return }
        hubMode = true
    }

    /// Drain the outbound queue to the relay BEFORE iOS suspends the app —
    /// call this from the shell's scenePhase → `.background` hook instead of
    /// a bare `stop()`. Stops the tick loop (so no concurrent ticks), then
    /// runs a final `flushPendingOutbound()` inside a `UIApplication`
    /// background task so the push has up to ~30s to reach the relay even as
    /// we background.
    ///
    /// Sync-durability Phase 1: without this, a capture made right before
    /// backgrounding sits stranded in the in-memory outbound queue until the
    /// next launch (the "added a block, didn't reach the relay for 2 hours"
    /// gap). The on-device write is always durable (SQLite + file); this
    /// closes the gap to the relay so OTHER devices can pull it.
    func flushOnBackground() {
        stop()
        backgroundTransitionGeneration &+= 1
        let transitionGeneration = backgroundTransitionGeneration
        beginBackgroundFlush()
        let app = UIApplication.shared
        var bgTask: UIBackgroundTaskIdentifier = .invalid
        bgTask = app.beginBackgroundTask(withName: "relay-flush-on-background") {
            // iOS is reclaiming the time — end the task so we aren't killed.
            if bgTask != .invalid {
                app.endBackgroundTask(bgTask)
                bgTask = .invalid
            }
        }
        Task { [weak self] in
            defer {
                self?.finishBackgroundFlush()
                if bgTask != .invalid {
                    app.endBackgroundTask(bgTask)
                    bgTask = .invalid
                }
            }
            guard let self else { return }
            await self.waitUntilRelocationDeliveryFinishes()
            guard Self.shouldCommitBackgroundTransition(
                issuedGeneration: transitionGeneration,
                currentGeneration: self.backgroundTransitionGeneration
            ) else { return }
            guard Self.canSuspendForBackgroundRelay(
                relocationInFlight: self.relocationDeliveryInFlight,
                hasPendingRelocation: self.pendingRelocation != nil
            ) else { return }
            self.suspendForBackground()
            _ = await self.flushPendingOutbound()
        }
    }

    private func runLoop() async {
        while !Task.isCancelled {
            await tickOnce()
            // Backoff: on consecutive errors, sleep longer between ticks
            // (doubling, hard-capped at 60s — see backoffSleepSeconds).
            // A successful tick resets consecutiveErrors → base cadence.
            let sleepSecs = Self.backoffSleepSeconds(
                consecutiveErrors: consecutiveErrors,
                base: tickIntervalSeconds
            )
            do {
                try await Task.sleep(nanoseconds: sleepSecs * 1_000_000_000)
            } catch {
                // Task cancelled mid-sleep — exit cleanly.
                return
            }
        }
    }

    /// Single tick, generation-gated and timeout-bounded (tesela-96y). Bumps
    /// `tickGeneration` and races the REAL tick body (`runSingleTick`)
    /// against `tickTimeoutSeconds`. Two outcomes:
    ///   - `runSingleTick` finishes in time → its own guard already
    ///     committed (or discarded, if superseded) its result; nothing
    ///     more to do here.
    ///   - The timeout wins → `runSingleTick` is ABANDONED (not
    ///     cancelled — see `raceAgainstTimeout`) and this function records
    ///     the failure itself, but ONLY if no newer tick has started in
    ///     the meantime (`shouldCommitTick`) — otherwise a slow tick's
    ///     eventual timeout-handler could stomp a fresher tick's state.
    ///
    /// Before this generation/timeout gating existed, a single tick with
    /// no bound on its engine-apply time (or a `wake()`-triggered
    /// overlapping loop racing an in-flight tick) could wedge the ENTIRE
    /// serial loop forever with `isRunning` still reading true and no
    /// error surfaced — exactly the iPad "sync stopped applying inbound
    /// changes entirely, even manual refresh showed nothing new, only
    /// killing the app fixed it" report. Root-cause note: `.relay` mode's
    /// manual pull-to-refresh (`MockMosaicService.refresh(from: .relay)`)
    /// is a PURE LOCAL READ of the materialized sandbox files — it does
    /// NOT go through `applyRemoteChange`'s edit-suppression gate at all,
    /// so a stuck ect coalescing gate was ruled out as the cause of a
    /// refresh-resistant staleness; the files themselves only change when
    /// THIS loop's inbound tick materializes them, so a refresh showing
    /// nothing new is direct evidence the loop itself had stopped making
    /// progress.
    private func tickOnce() async {
        guard pendingRelocation == nil else { return }
        guard let relayLease = relayOperationAdmission.tryAcquire(.tick) else {
            return
        }
        tickGeneration &+= 1
        let myGeneration = tickGeneration
        let finishedInTime = await Self.raceAgainstTimeout(seconds: Self.tickTimeoutSeconds) {
            await self.runSingleTick(
                generation: myGeneration,
                relayOperationLease: relayLease
            )
        }
        guard !finishedInTime else { return }
        // The timeout continuation and work completion can become runnable
        // together. If the work already released this exact lease, its real
        // outcome won; never let a stale timeout mutate a newer operation.
        guard relayOperationAdmission.isActive(relayLease) else { return }
        guard Self.shouldCommitTick(issuedGeneration: myGeneration, currentGeneration: tickGeneration) else { return }
        // The admitted work is deliberately non-cancelling and still owns
        // the relay lease. Supersede its generation now so a late finish can
        // release that lease but cannot overwrite this timeout outcome.
        tickGeneration &+= 1
        lastError = "sync tick exceeded \(Self.tickTimeoutSeconds)s without finishing — abandoned so the loop keeps going (tesela-96y)"
        consecutiveErrors = consecutiveErrors &+ 1
        // The abandoned tick may still be mid-flight against the SAME
        // coordinator/engine handles; drop them so the NEXT tick rebuilds
        // fresh rather than layering a new attempt on top of whatever
        // state the stuck one left things in.
        dropCoordinator()
    }

    /// The actual ensure-coordinator → outbound → inbound body (formerly
    /// `tickOnce()` itself — see that function's doc for why this is now
    /// generation-gated and timeout-raced). Every point that mutates
    /// shared `@Published` state or persisted cursors is guarded by
    /// `shouldCommitTick(issuedGeneration: generation, ...)` so a result
    /// computed under a superseded generation is silently discarded
    /// instead of racing a newer tick's writes.
    private func runSingleTick(
        generation: UInt64,
        relayOperationLease: RelayOperationAdmission.Lease
    ) async {
        defer { finishRelayOperation(relayOperationLease) }
        await runSingleTickUnderRelayAdmission(generation: generation)
    }

    private func runSingleTickUnderRelayAdmission(generation: UInt64) async {
        // Hub mode: the live `/ws` socket is the sync hub; the relay poll
        // loop is gated off (Part E2). The runLoop keeps sleeping/waking;
        // each wake is a no-op until `hubMode` flips back to false.
        guard !hubMode else { return }
        invalidateCoordinatorIfRepaired()
        do {
            if coordinator == nil {
                try await ensureCoordinator(requiredTickGeneration: generation)
            }
            guard let coordinator,
                  let tickEngine = engine,
                  let tickRelay = relay
            else { return }
            // Capture the scope WITH the coordinator: a re-pair completing
            // during the awaits below swaps self.cursorScope to the new
            // identity, and persisting the OLD group's cursor under the NEW
            // key would silently stall the new group's inbound (review
            // finding on ddd8def — same class as the hubMode mid-tick flip).
            let tickScope = cursorScope
            let outbound = try await coordinator.tickOutbound(maxBytes: 1_000_000)
            let inbound = try await coordinator.tickInbound()
            // tesela-96y: the two awaits above are exactly where a
            // `wake()`-triggered loop restart (heavy sync ⇒ slower
            // responses ⇒ a wider overlap window on every foreground/
            // background flip) or this tick's own timeout handler may
            // have moved on to a NEWER generation. Bail without touching
            // any shared state if so — see `shouldCommitTick`.
            guard Self.shouldCommitTick(issuedGeneration: generation, currentGeneration: tickGeneration) else { return }
            noteOutboundOutcome(outbound)
            lastApplied = inbound.applied
            lastSuccessfulPollAt = Date()
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
                lastAppliedAt = Date()
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
            if !inbound.needsCatchupNoteIdsHex.isEmpty {
                await catchUpFromRelaySnapshots(
                    idsHex: inbound.needsCatchupNoteIdsHex,
                    relay: tickRelay,
                    engine: tickEngine,
                    generation: generation
                )
            }
            guard Self.shouldCommitTick(
                issuedGeneration: generation,
                currentGeneration: tickGeneration
            ) else { return }
            // Stranded-behind-compaction convergence fix (mirrors the desktop
            // `tick()`): an empty/zero-applied poll is NOT proof we're caught
            // up. When the relay's GC watermark (`compactionSeq`) is past our
            // inbound cursor the ops we still need were compacted away, so the
            // tail poll is a black hole. Re-run the snapshot bootstrap on the
            // LIVE coordinator (NO teardown) to converge. Inbound cursor only;
            // outbound is untouched (no pending local edit is lost).
            if Self.shouldBootstrapMidRun(
                applied: Int(inbound.applied),
                compactionSeq: inbound.compactionSeq,
                cursor: inbound.newCursorSeq
            ), let scope = tickScope {
                await runSnapshotBootstrap(
                    engine: tickEngine,
                    relay: tickRelay,
                    coordinator: coordinator,
                    scope: scope,
                    generation: generation
                )
            }
            guard Self.shouldCommitTick(
                issuedGeneration: generation,
                currentGeneration: tickGeneration
            ) else { return }
            // Sync durability P3b: once we have a relay handle + an APNs
            // device token (captured by AppDelegate at launch), register it
            // so the relay can silent-push our other devices on deposit.
            // Idempotent + best-effort; the guard makes this a no-op on every
            // tick after the first successful registration.
            await maybeRegisterApnsToken(
                relay: tickRelay,
                scope: tickScope,
                generation: generation
            )
            guard Self.shouldCommitTick(
                issuedGeneration: generation,
                currentGeneration: tickGeneration
            ) else { return }
            // tesela-zpr: mirror the server's snapshot-deposit cadence gate
            // so iOS contributes its resident notes to the relay's snapshot
            // pool too (durability — see `depositSnapshotsIfDue`'s doc
            // comment). Best-effort; a failure here doesn't fail the tick.
            if let scope = tickScope {
                await depositSnapshotsIfDue(
                    relay: tickRelay,
                    engine: tickEngine,
                    scope: scope,
                    generation: generation
                )
            }
        } catch let err as FfiSyncError {
            guard Self.shouldCommitTick(issuedGeneration: generation, currentGeneration: tickGeneration) else { return }
            lastError = err.localizedDescription
            consecutiveErrors = consecutiveErrors &+ 1
            dropCoordinator()
        } catch {
            guard Self.shouldCommitTick(issuedGeneration: generation, currentGeneration: tickGeneration) else { return }
            lastError = error.localizedDescription
            consecutiveErrors = consecutiveErrors &+ 1
            dropCoordinator()
        }
    }

    /// Path to the iOS sandbox mosaic root. Stable across launches;
    /// the engine + the MockMosaicService local-fallback both read
    /// from here so iOS-authored writes are visible to local reads
    /// even before any pairing has happened.
    /// Select one physical mosaic's local CRDT namespace. Call only after the
    /// activation barrier is clear. Dropping the old handle before copying or
    /// opening prevents a live engine from writing into a directory while it
    /// is being adopted by the canonical scope.
    func activateEngine(
        scope: MosaicEngineScope,
        relayOperationLease: RelayOperationAdmission.Lease? = nil
    ) async throws {
        guard relayOperationAdmission.permits(relayOperationLease) else {
            throw FfiSyncError.Other(
                message: "The previous mosaic is still finishing a relay operation."
            )
        }
        if activeEngineScope == scope, engine != nil { return }
        guard engineOperationsInFlight == 0,
              !engineActivationInFlight,
              !relocationDeliveryInFlight,
              hubBootstrapInFlight == 0
        else {
            throw FfiSyncError.Other(
                message: "The previous mosaic is still finishing a local operation."
            )
        }
        engineActivationInFlight = true
        defer {
            engineActivationInFlight = false
            resumeHubActivationWaitersIfSafe()
        }

        engineSessionGeneration &+= 1
        tickGeneration &+= 1
        dropCoordinator()
        engine = nil
        lastPushedVV = [:]
        lastCatchupAt = [:]
        forcedCatchupInFlight = []
        viewsSeeded = false
        relayURL = nil
        lastRegisteredApnsKey = nil
        lastApplied = 0
        lastSent = 0
        inboundCursorSeq = 0
        lastSuccessfulPollAt = nil
        lastAppliedAt = nil
        lastDepositAt = nil
        lastDepositError = nil
        relocationRetryTask?.cancel()
        relocationRetryTask = nil
        relocationRetryDelaySeconds = 2
        activeEngineScope = scope

        try await prepareStorageForActiveScope()
        try await openEngineIfNeeded()
    }

    private func prepareStorageForActiveScope() async throws {
        try await Self.prepareStorage(for: activeEngineScope)
    }

    static func prepareStorage(
        for scope: MosaicEngineScope,
        documentsURL: URL? = nil
    ) async throws {
        let root = scope.rootURL(documentsURL: documentsURL)
        guard scope != .legacy else {
            try FileManager.default.createDirectory(
                at: root,
                withIntermediateDirectories: true
            )
            return
        }

        let fileManager = FileManager.default
        if fileManager.fileExists(atPath: root.path) {
            return
        }
        precondition(!Self.automaticallyAdoptsLegacyEngineStore)
        try await Task.detached(priority: .utility) {
            // A build-78 engine root has no durable group provenance. A
            // current pairing proves the destination, not which group wrote
            // those legacy bytes, so automatic adoption can contaminate a
            // replacement profile. Leave the legacy root untouched as a
            // recovery artifact and bootstrap this verified scope cleanly.
            try FileManager.default.createDirectory(
                at: root,
                withIntermediateDirectories: true
            )
        }.value
    }

    /// The unscoped pre-build-79 root is retained only for explicit recovery;
    /// automatic adoption cannot prove its authoring group.
    static let automaticallyAdoptsLegacyEngineStore = false

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
        let mosaicRoot = activeEngineScope.rootURL()
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
    private func ensureCoordinator(requiredTickGeneration: UInt64? = nil) async throws {
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
        let expectedScope = activeEngineScope
        try await openEngineIfNeeded()
        guard let engine else {
            throw FfiSyncError.Other(message: "engine open failed")
        }

        // Try the cached pairing code first. If we have one, the
        // path below skips the Mac HTTP fetch entirely.
        let cached = KeychainPairingCache.load()
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
            try await buildCoordinator(
                engine: engine,
                codeStr: codeStr,
                expectedScope: expectedScope,
                requiredTickGeneration: requiredTickGeneration
            )
            // Survived the build → cache the code for future ticks.
            if cached == nil {
                KeychainPairingCache.save(codeStr)
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
                KeychainPairingCache.clear()
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
        case .RelocationRejected, .RelocationConflict, .RelocationRecoveryRequired:
            return false
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
    private func buildCoordinator(
        engine: SyncEngineHandle,
        codeStr: String,
        expectedScope: MosaicEngineScope,
        requiredTickGeneration: UInt64?
    ) async throws {
        let pairing = try decodePairingCode(code: codeStr)
        let pairingScope = MosaicEngineScope(groupIdHex: pairing.groupIdHex)
        guard pairingScope == expectedScope else {
            throw FfiSyncError.Other(
                message: "The cached relay pairing belongs to another mosaic. Re-pair before syncing."
            )
        }
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
        Self.quarantineLegacyCursors()
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
        // with the Mac off. Shared with the LIVE mid-run bootstrap in tickOnce
        // (the stranded-behind-compaction convergence fix) — see
        // `runSnapshotBootstrap`.
        await runSnapshotBootstrap(
            engine: engine,
            relay: relay,
            coordinator: coordinator,
            scope: scope,
            generation: requiredTickGeneration
        )
        guard activeEngineScope == expectedScope,
              requiredTickGeneration == nil || requiredTickGeneration == tickGeneration
        else { throw CancellationError() }
        self.relay = relay
        self.coordinator = coordinator
        self.cursorScope = scope
        self.coordinatorPairingCode = codeStr
        self.relayURL = relayURL
    }

    /// Pure half of the mid-run bootstrap gate (stranded-behind-compaction
    /// convergence fix, mirrors the desktop `tick()`): a poll that returns
    /// `applied == 0` is NOT proof of "caught up" — when the relay's GC
    /// watermark (`compactionSeq`) is past our inbound cursor, the ops we
    /// still need were compacted away, so the empty tail is a black hole.
    /// In that case re-run the snapshot bootstrap on the LIVE coordinator.
    /// `applied` is accepted for call-site clarity but intentionally NOT
    /// part of the predicate: the bug is precisely that the device strands
    /// behind the watermark even when nothing applied.
    static func shouldBootstrapMidRun(applied: Int, compactionSeq: Int64, cursor: Int64) -> Bool {
        compactionSeq > cursor
    }

    /// Pull the relay's compacted snapshot set and (when behind the GC
    /// watermark) import each note snapshot, then jump the inbound cursor
    /// to the watermark so the next poll fetches only the un-compacted tail.
    /// Shared by `buildCoordinator` (cold start) and `tickOnce` (mid-run, on
    /// the LIVE coordinator — NO teardown). Clobber guards preserved exactly:
    /// `importNoteSnapshotById` is a NON-DESTRUCTIVE merge; the cursor jump is
    /// ALL-OR-NOTHING (`shouldJumpBootstrapCursor` holds the cursor on any
    /// failed import so the covered-but-GC'd note isn't skipped permanently);
    /// and it touches ONLY the inbound cursor — never the persisted OUTBOUND
    /// cursor. Best-effort — a fetch failure leaves the cursor as-is and the
    /// normal poll handles the (un-GC'd) tail.
    private func runSnapshotBootstrap(
        engine: SyncEngineHandle,
        relay: RelayClientHandle,
        coordinator: SyncCoordinator,
        scope: String,
        generation: UInt64? = nil
    ) async {
        do {
            let snaps = try await relay.fetchSnapshots()
            guard generation == nil || generation == tickGeneration else { return }
            if Self.shouldRunSnapshotBootstrap(
                compactionSeq: snaps.compactionSeq,
                inboundCursorSeq: inboundCursorSeq
            ) {
                var imported = 0
                var failed = 0
                for s in snaps.snapshots {
                    do {
                        try await engine.importNoteSnapshotById(noteId: s.streamId, bytes: s.payload)
                        guard generation == nil || generation == tickGeneration else { return }
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
                    // cursor holds, so the next rebuild/tick retries the imports.
                    await coordinator.setInboundCursorSeq(seq: snaps.compactionSeq)
                    guard generation == nil || generation == tickGeneration else { return }
                    inboundCursorSeq = snaps.compactionSeq
                    UserDefaults.standard.set(
                        snaps.compactionSeq,
                        forKey: Self.inboundCursorKey(scope: scope)
                    )
                } else {
                    lastError = "snapshot bootstrap: \(failed) of \(snaps.snapshots.count) imports failed; retrying"
                }
                guard generation == nil || generation == tickGeneration else { return }
                if imported > 0 {
                    lastAppliedAt = Date()
                    onAppliedChanges?()
                }
            }
        } catch {
            // Leave the cursor as-is; the regular poll handles the un-GC'd tail.
        }
    }

    /// Build-78 cursors were stored without relay/group provenance. The
    /// matching unscoped engine bytes are deliberately not adopted into a
    /// group-scoped root, so carrying these cursors forward would pair an
    /// empty engine with stale progress and skip history. Existing scoped
    /// cursors remain authoritative; bare keys are deletion-only quarantine.
    static func quarantineLegacyCursors(defaults: UserDefaults = .standard) {
        defaults.removeObject(forKey: legacyInboundCursorKey)
        defaults.removeObject(forKey: legacyOutboundCursorKey)
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
        guard let cached = KeychainPairingCache.load(),
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
        engine: SyncEngineHandle,
        generation: UInt64
    ) async {
        let wanted = Set(idsHex.map { $0.lowercased() })
        do {
            let snaps = try await relay.fetchSnapshots()
            guard generation == tickGeneration else { return }
            var imported = 0
            for s in snaps.snapshots {
                let idHex = s.streamId.map { String(format: "%02x", $0) }.joined()
                guard wanted.contains(idHex) else { continue }
                do {
                    try await engine.importNoteSnapshotById(noteId: s.streamId, bytes: s.payload)
                    guard generation == tickGeneration else { return }
                    imported += 1
                } catch {
                    // Leave it for the next surfacing; the import is the
                    // same authoritative re-base the bootstrap uses.
                }
            }
            guard generation == tickGeneration else { return }
            if imported > 0 { onAppliedChanges?() }
        } catch {
            // Snapshot fetch failed (network). Best-effort: a pending
            // note resurfaces on its next inbound delta; `lastError` is
            // not set here so a one-off blip doesn't flag the whole tick.
        }
    }

    /// tesela-zpr: mirror iOS's resident notes into the relay's snapshot
    /// pool on a cadence, so a device recovering past the relay's GC window
    /// can bootstrap from iOS-authored content — not just the Mac's (ARCH
    /// REVIEW 2026-07-01: today only `tesela-server` deposits, making the
    /// Mac a silent single point of durability).
    ///
    /// Mirrors the server's snapshot-deposit posture (`tesela-server::
    /// sync_relay::tick`'s heal-snapshot deposit) but takes the MINIMAL
    /// sensible subset: a cadence gate (`shouldDepositSnapshots`,
    /// `snapshotDepositIntervalSeconds`) instead of the server's per-note
    /// content-hash throttle — iOS's tick loop runs far more often than the
    /// server's, so a 5-minute cadence alone already keeps deposits rare.
    /// Deliberately **always inert** (`coversSeq = 0`): iOS never advances
    /// the group's compaction watermark, so it can never GC an op a peer
    /// hasn't applied yet — that stays the Mac's call.
    private func depositSnapshotsIfDue(
        relay: RelayClientHandle,
        engine: SyncEngineHandle,
        scope: String,
        generation: UInt64
    ) async {
        let key = Self.snapshotDepositAtKey(scope: scope)
        let lastAt = (UserDefaults.standard.object(forKey: key) as? Double).map(Date.init(timeIntervalSince1970:))
        guard Self.shouldDepositSnapshots(lastDepositAt: lastAt, now: Date()) else { return }
        let snapshots = await engine.exportAllNoteSnapshots()
        guard generation == tickGeneration else { return }
        guard !snapshots.isEmpty else { return }
        do {
            _ = try await relay.putSnapshotsChunked(
                coversSeq: 0,
                snapshots: snapshots,
                budgetBytes: Self.snapshotDepositBudgetBytes
            )
            guard generation == tickGeneration else { return }
            // tesela-c7s F1: the deposit durably landed each note's snapshot on
            // the relay — re-anchor any STRANDED outbound cursor to its
            // snapshot-time version (forwarded verbatim from the records just
            // deposited, each carrying its export-time `versionVv`) so the next
            // local edit ships an INCREMENTAL delta over the ops stream instead
            // of another snapshot the peers' `?since=` poll never reads. This is
            // the iOS half of the single, shared cursor-repair mechanism (the
            // server wires it into `deposit_snapshots`): `tickOutbound`'s
            // produce→commit heals the common case, and this deposit backstops a
            // note whose broadcast PUT failed while its chunked deposit here
            // succeeded. Only a genuinely stale-ahead / undecodable cursor is
            // rewound; healthy cursors are untouched (engine-side
            // `broadcast_cursor_needs_repair`).
            await engine.repairBroadcastCursorsAfterSnapshot(deposited: snapshots)
            guard generation == tickGeneration else { return }
            let depositedAt = Date()
            UserDefaults.standard.set(depositedAt.timeIntervalSince1970, forKey: key)
            lastDepositAt = depositedAt
            lastDepositError = nil
        } catch {
            guard generation == tickGeneration else { return }
            // Best-effort: leave the cadence stamp unset so the next tick
            // retries promptly instead of waiting out the full window.
            // Deliberately NOT swallowed silently (tesela-96y): a dedicated
            // `lastDepositError` (NOT the shared `lastError` the main
            // sync-status pill keys off) records it, so a deposit stuck in
            // a retry-every-tick loop (the "PUT /snapshot looping" fleet
            // symptom) is visible in the Sync Health card without falsely
            // flipping the whole app to "Sync error" over a best-effort,
            // non-tick-failing background push.
            lastDepositError = error.localizedDescription
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
        guard !hubMode else { return 0 }
        guard pendingRelocation == nil else { return 0 }
        guard let relayLease = await relayOperationAdmission.acquireWhenIdle(.flush) else {
            return 0
        }
        defer { finishRelayOperation(relayLease) }
        guard !hubMode else { return 0 }
        guard pendingRelocation == nil else { return 0 }
        guard let session = await beginEngineOperationWhenAvailable(
            requiredSession: engineSessionToken
        ) else {
            return 0
        }
        defer { finishEngineOperation() }
        guard isCurrentEngineSession(session) else { return 0 }
        invalidateCoordinatorIfRepaired()
        if coordinator == nil {
            try? await ensureCoordinator()
        }
        guard let coordinator else { return 0 }
        do {
            let outcome = try await coordinator.tickOutbound(maxBytes: 1_000_000)
            noteOutboundOutcome(outcome)
            if let relay {
                await maybeRegisterApnsToken(
                    relay: relay,
                    scope: cursorScope,
                    generation: tickGeneration
                )
            }
            return outcome.opsSent
        } catch {
            lastError = error.localizedDescription
            return 0
        }
    }

    /// One-shot relay catch-up for a background wake. The app uses the shared
    /// ticker so a warm-process APNs callback cannot open a second Loro handle
    /// on the shell's physical root. A cold launch derives the exact engine
    /// scope directly from the cached pairing code; it does not depend on a
    /// shell-owned `MockMosaicService` being connected.
    func runBackgroundCatchup() async -> BackgroundCatchupOutcome {
        guard !hubMode else { return .unavailable }
        guard pendingRelocation == nil else { return .unavailable }
        guard let relayLease = relayOperationAdmission.tryAcquire(.backgroundCatchup) else {
            return .unavailable
        }
        defer { finishRelayOperation(relayLease) }
        guard let code = KeychainPairingCache.load() else { return .unavailable }

        let pairing: PairingCodeRecord
        do {
            pairing = try decodePairingCode(code: code)
        } catch {
            if Self.isDefinitivePairingFailure(error) {
                KeychainPairingCache.clear()
            }
            lastError = error.localizedDescription
            return .failed(error.localizedDescription)
        }
        guard let scope = Self.backgroundEngineScope(pairing: pairing) else {
            return .unavailable
        }
        // A warm visible shell may intentionally be attached to a different
        // profile than the cached relay pairing. Never switch that live UI
        // behind its back; the next wake for the selected relay profile can
        // perform the catch-up.
        if mosaic != nil, activeEngineScope != scope {
            return .unavailable
        }

        do {
            try await activateEngine(
                scope: scope,
                relayOperationLease: relayLease
            )
            guard let session = await beginEngineOperationWhenAvailable(
                requiredSession: engineSessionToken
            ) else {
                return .unavailable
            }
            defer { finishEngineOperation() }
            guard !session.hubMode,
                  session.scope == scope,
                  isCurrentEngineSession(session),
                  let engine
            else { return .unavailable }

            let priorPoll = lastSuccessfulPollAt
            let priorApply = lastAppliedAt
            invalidateCoordinatorIfRepaired()
            if coordinator == nil {
                try await buildCoordinator(
                    engine: engine,
                    codeStr: code,
                    expectedScope: scope,
                    requiredTickGeneration: nil
                )
            }
            guard isCurrentEngineSession(session) else { return .unavailable }
            // Do not use tickOnce's timeout race here. Its timeout is
            // intentionally non-cancelling, so it can return while the old
            // runSingleTick still mutates the shared engine. A background
            // wake must retain its operation lease until the real tick ends.
            tickGeneration &+= 1
            let backgroundGeneration = tickGeneration
            await runSingleTickUnderRelayAdmission(generation: backgroundGeneration)
            guard lastSuccessfulPollAt != priorPoll, lastError == nil else {
                return .failed(lastError ?? "Background relay catch-up did not complete.")
            }
            return .completed(newData: lastAppliedAt != priorApply)
        } catch {
            if Self.isDefinitivePairingFailure(error) {
                KeychainPairingCache.clear()
            }
            lastError = error.localizedDescription
            return .failed(error.localizedDescription)
        }
    }

    static func backgroundEngineScope(
        pairing: PairingCodeRecord
    ) -> MosaicEngineScope? {
        guard let relayURL = pairing.relayUrl,
              !relayURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else { return nil }
        return MosaicEngineScope(groupIdHex: pairing.groupIdHex)
    }

    /// Register this device's APNs token with the relay (sync durability
    /// P3b) so the relay can wake our OTHER devices with a content-available
    /// silent push on every op deposit. The token is captured by
    /// `AppDelegate` at launch (`AppDelegate.deviceTokenHex`); this pulls it
    /// when a relay handle exists and POSTs it via the FFI. No-op until both
    /// the token and the handle are ready; POSTs once per token (re-POSTs on
    /// rotation). Best-effort — a failure just retries on a later tick and
    /// never surfaces as a sync error.
    private func maybeRegisterApnsToken(
        relay: RelayClientHandle,
        scope: String?,
        generation: UInt64
    ) async {
        guard generation == tickGeneration else { return }
        guard let token = AppDelegate.deviceTokenHex else {
            // No APNs token captured. Surface WHY: a registration error
            // (entitlement/Push/network) vs still pending.
            apnsNote = AppDelegate.lastRegistrationError.map { "no token — iOS reg failed: \($0)" }
                ?? "no token yet (APNs registration pending)"
            return
        }
        // Key the registration by (token, relay scope): a relay migration or
        // re-pair changes the scope → re-register with the NEW relay so it has
        // a token to background-push (2026-06-24 HA→CF gap). cursorScope is set
        // by the coordinator we ticked through to get here.
        let key = Self.apnsRegistrationKey(token: token, scope: scope)
        if key == lastRegisteredApnsKey {
            apnsNote = "registered ✓ (\(token.prefix(8))…)"
            return
        }
        do {
            try await relay.registerDevice(apnsToken: token)
            guard generation == tickGeneration else { return }
            lastRegisteredApnsKey = key
            apnsNote = "registered ✓ (\(token.prefix(8))…)"
        } catch {
            guard generation == tickGeneration else { return }
            // Leave lastRegisteredApnsKey unset so the next tick retries.
            apnsNote = "POST /devices failed: \(error.localizedDescription)"
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
    /// writes after a successful HTTP-fetched build. Keychain-backed
    /// (`KeychainPairingCache`, tesela-tp0.2) — the code carries the group key.
    static func cachePairingCode(_ rawCode: String) {
        KeychainPairingCache.save(rawCode)
    }

    /// The cached pairing code (set by `cachePairingCode` / a successful
    /// HTTP-fetched build), or `nil` if the device hasn't paired. The
    /// presence transport decodes it to source the relay URL + group identity
    /// for `PresenceRelaySocket` (same code `buildCoordinator` consumes).
    static func cachedPairingCode() -> String? {
        KeychainPairingCache.load()
    }
}
