import SwiftUI
import Combine

private final class DeltaSendCompletion: @unchecked Sendable {
    private let lock = NSLock()
    private var continuation: CheckedContinuation<Bool, Never>?

    init(_ continuation: CheckedContinuation<Bool, Never>) {
        self.continuation = continuation
    }

    func resume(_ value: Bool) {
        lock.lock()
        let continuation = self.continuation
        self.continuation = nil
        lock.unlock()
        continuation?.resume(returning: value)
    }
}

/// Workspace-level sync state. Exposes the two flags the modified-marker
/// reads:
///   • `isReachable` — true when at least one peer is reachable
///   • `hasPendingEdits` — true when local edits haven't been seen by any peer yet
///
/// Per decision #13, the page-title `●` indicator renders only when
/// **both** `!isReachable && hasPendingEdits` are true. Continuous-save
/// is assumed invisible — the marker is a sync-state indicator, not a
/// file-write indicator.
///
/// For now the values are mocked via a debug toggle in Settings → Sync.
/// Phase 15 will hook them into the real Rust sync layer.
@MainActor
final class SyncState: ObservableObject {
    @Published var isReachable: Bool = true
    @Published var hasPendingEdits: Bool = false

    /// Drives the `●` indicator visibility.
    var showsModifiedMarker: Bool {
        !isReachable && hasPendingEdits
    }
}

/// Live-update channel: a WebSocket to the server's `/ws` endpoint.
/// The server broadcasts an event whenever any client changes a note;
/// iOS reacts by re-fetching so desktop edits appear without a manual
/// pull. Mirrors the web client's `ws-client.svelte.ts`.
///
/// Note *content* is deliberately not decoded — iOS has its own data
/// models and simply re-fetches over HTTP on any note event, so only
/// the event discriminator matters here.
@MainActor
final class LiveSyncSocket: ObservableObject {
    struct ConnectionBinding: Equatable {
        let websocketURL: URL
        let expectedMosaicPath: String?
        let expectedGroupIdHex: String?
        let hubIdentity: String?
    }

    /// One-shot lease for the binding a shell has fully published. Handshake
    /// failures before publication have no lease and therefore cannot trigger
    /// a second activation loop; a later identity loss consumes the lease and
    /// returns its callback exactly once.
    final class BindingInvalidationState {
        var onInvalidated: (() -> Void)?
        private var publishedBinding: ConnectionBinding?

        func publish(_ binding: ConnectionBinding) {
            publishedBinding = binding
        }

        func clear() {
            publishedBinding = nil
        }

        func takeInvalidationCallback(
            for binding: ConnectionBinding
        ) -> (() -> Void)? {
            guard publishedBinding == binding else { return nil }
            publishedBinding = nil
            return onInvalidated ?? {}
        }
    }

    /// Called whenever a fresh socket is resumed. The relocation outbox uses
    /// this to replay a frame that survived app termination; the frame still
    /// has to pass `sendDelta`'s server barrier before it is cleared.
    var onConnectionAttempt: (() -> Void)?

    /// A previously-published exact binding was disproved by a later session
    /// hello or identity-bound barrier. Shells detach immediately and rerun
    /// profile activation so `/mosaics/current` can select the new group.
    var onBindingInvalidated: (() -> Void)? {
        get { bindingInvalidationState.onInvalidated }
        set { bindingInvalidationState.onInvalidated = newValue }
    }

    /// Invoked on the main actor whenever a note was created, updated,
    /// or deleted on the server (by any client, including this one).
    var onNoteChange: (() -> Void)?

    /// Invoked on the main actor whenever the socket receives a binary
    /// Loro delta frame (instant-multidevice spec §4: text = JSON
    /// `WsEvent`, binary = TLR2 Loro delta). The shell wires this to
    /// `RelayTicker.applyInboundDelta(_:)` — the ONLY owner of the Loro
    /// engine — so the bytes are applied via the engine `LiveSyncSocket`
    /// deliberately does not hold. The frame is NOT re-broadcast from
    /// here; the server handles fan-out.
    /// Includes the exact verified hub identity that delivered the frame.
    /// Consumers must carry it into the engine operation so a callback queued
    /// under profile A cannot apply after an A -> B -> A activation cycle.
    var onBinaryDelta: ((Data, String) -> Void)?

    /// Invoked on the main actor when the socket receives an EPHEMERAL presence
    /// frame (PRES magic) — a peer's live caret (Phase 3 multi-device). Routed
    /// BEFORE the binary-delta path, so it never reaches the engine. The shell
    /// wires this to the `RemoteCursorStore`.
    var onPresence: ((LoroPresence.Frame) -> Void)?

    /// Invoked on the main actor when the server's saved-views registry
    /// changed (the `views_changed` WS event, saved-views spec
    /// 2026-06-10). The shell wires this to
    /// `MockMosaicService.noteViewsChanged()` so the Inbox tab's view
    /// switcher re-reads `/views` without a full note refresh. The
    /// event's payload (the full registry) is deliberately not decoded —
    /// iOS re-fetches, matching the note-event posture above.
    var onViewsChange: (() -> Void)?

    /// Wall-clock of the last WS frame actually RECEIVED — any kind (note
    /// event, binary Loro delta, presence, or an unrecognized text frame).
    /// Sync-health observability (tesela-96y): `connected` becomes true only
    /// after the server-first session hello proves the exact mosaic path,
    /// while `lastEventAt` records continuing traffic after that proof. In
    /// hub mode (`.http` backend) this is the only inbound sync path because
    /// `RelayTicker`'s poll loop is gated off via `hubMode`.
    @Published private(set) var lastEventAt: Date? = nil

    private let session = URLSession(configuration: .default)
    private var task: URLSessionWebSocketTask?
    private var currentBinding: ConnectionBinding?
    private var connected = false
    private var sessionVerified = false
    private var reconnectAttempt = 0
    private var reconnectWork: Task<Void, Never>?
    private var sessionHelloTimeoutWork: Task<Void, Never>?
    /// Bumped on every (re)connect and on disconnect so a stale receive
    /// loop or pending reconnect from a superseded socket bows out.
    private var generation = 0
    private struct PendingDeltaBarrier {
        let task: URLSessionWebSocketTask
        let generation: Int
        let expectedMosaicPath: String
        let expectedGroupIdHex: String
        let hubIdentity: String
        let completion: DeltaSendCompletion
    }
    private var pendingDeltaBarriers: [UUID: PendingDeltaBarrier] = [:]
    private struct PendingSessionVerification {
        let task: URLSessionWebSocketTask
        let generation: Int
        let completion: DeltaSendCompletion
    }
    private var pendingSessionVerifications: [PendingSessionVerification] = []
    private var deltaSendInFlight = false
    private var deltaSendWaiters: [CheckedContinuation<Void, Never>] = []
    private let bindingInvalidationState = BindingInvalidationState()

    /// Point the socket at one exact server/mosaic/client identity binding,
    /// tearing down any different binding first. Pass `nil` (mock mode / no
    /// server) to disconnect. Missing identity arguments are compile-time
    /// compatibility only: the socket remains fail-closed until both exist.
    func connect(
        serverURL: String?,
        expectedMosaicPath: String? = nil,
        expectedGroupIdHex: String? = nil,
        hubIdentity: String? = nil
    ) {
        guard let serverURL,
              let binding = Self.connectionBinding(
                  serverURL: serverURL,
                  expectedMosaicPath: expectedMosaicPath,
                  expectedGroupIdHex: expectedGroupIdHex,
                  hubIdentity: hubIdentity
              )
        else {
            disconnect()
            return
        }
        if currentBinding == binding, task != nil { return }
        if currentBinding != binding {
            bindingInvalidationState.clear()
        }
        reconnectWork?.cancel()
        reconnectWork = nil
        currentBinding = binding
        openSocket()
    }

    /// Establish one exact hub binding and wait until the server proves it
    /// twice: first with the server-first session hello, then with a positive
    /// identity-bound barrier acknowledgement on that same socket generation.
    /// Shell activation keeps the service detached and the live hub identity
    /// unpublished until this returns true.
    func connectAndVerify(
        serverURL: String,
        expectedMosaicPath: String,
        expectedGroupIdHex: String,
        hubIdentity: String
    ) async -> Bool {
        guard !expectedMosaicPath.isEmpty,
              !expectedGroupIdHex.isEmpty,
              !hubIdentity.isEmpty,
              let binding = Self.connectionBinding(
                  serverURL: serverURL,
                  expectedMosaicPath: expectedMosaicPath,
                  expectedGroupIdHex: expectedGroupIdHex,
                  hubIdentity: hubIdentity
              )
        else {
            disconnect()
            return false
        }
        connect(
            serverURL: serverURL,
            expectedMosaicPath: expectedMosaicPath,
            expectedGroupIdHex: expectedGroupIdHex,
            hubIdentity: hubIdentity
        )
        guard currentBinding == binding, let capturedTask = task else {
            return false
        }
        let capturedGeneration = generation
        guard await awaitSessionVerification(
            on: capturedTask,
            generation: capturedGeneration
        ),
            isCurrentVerifiedSession(
                on: capturedTask,
                generation: capturedGeneration,
                binding: binding,
                requiredHubIdentity: hubIdentity
            )
        else { return false }

        await acquireDeltaSendSlot()
        defer { releaseDeltaSendSlot() }
        guard isCurrentVerifiedSession(
            on: capturedTask,
            generation: capturedGeneration,
            binding: binding,
            requiredHubIdentity: hubIdentity
        ) else { return false }
        let verified = await sendActivationBarrier(
            on: capturedTask,
            generation: capturedGeneration,
            expectedMosaicPath: expectedMosaicPath,
            expectedGroupIdHex: expectedGroupIdHex.lowercased(),
            hubIdentity: hubIdentity
        )
        return verified && isCurrentVerifiedSession(
            on: capturedTask,
            generation: capturedGeneration,
            binding: binding,
            requiredHubIdentity: hubIdentity
        )
    }

    func isVerifiedBinding(
        serverURL: String,
        expectedMosaicPath: String,
        expectedGroupIdHex: String,
        hubIdentity: String
    ) -> Bool {
        guard let binding = Self.connectionBinding(
            serverURL: serverURL,
            expectedMosaicPath: expectedMosaicPath,
            expectedGroupIdHex: expectedGroupIdHex,
            hubIdentity: hubIdentity
        ),
            currentBinding == binding,
            let task
        else { return false }
        return isCurrentVerifiedSession(
            on: task,
            generation: generation,
            binding: binding,
            requiredHubIdentity: hubIdentity
        )
    }

    /// Mark the exact verified binding as shell-published. Until this point an
    /// activation owns its own retry path; after publication, a later identity
    /// mismatch must tear down the observed scope and start activation again.
    @discardableResult
    func publishVerifiedBinding(
        serverURL: String,
        expectedMosaicPath: String,
        expectedGroupIdHex: String,
        hubIdentity: String
    ) -> Bool {
        guard isVerifiedBinding(
            serverURL: serverURL,
            expectedMosaicPath: expectedMosaicPath,
            expectedGroupIdHex: expectedGroupIdHex,
            hubIdentity: hubIdentity
        ), let binding = currentBinding
        else { return false }
        bindingInvalidationState.publish(binding)
        return true
    }

    func disconnect() {
        bindingInvalidationState.clear()
        failAllDeltaBarriers()
        completeAllSessionVerifications(false)
        generation += 1
        reconnectWork?.cancel()
        reconnectWork = nil
        sessionHelloTimeoutWork?.cancel()
        sessionHelloTimeoutWork = nil
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
        currentBinding = nil
        connected = false
        sessionVerified = false
    }

    /// Tear the socket down but remember the URL, so `nudge()` can
    /// bring it back. Called when the app is backgrounded — iOS would
    /// suspend the connection anyway, and an explicit teardown avoids a
    /// hung `receive` on resume.
    func suspend() {
        failAllDeltaBarriers()
        completeAllSessionVerifications(false)
        generation += 1
        reconnectWork?.cancel()
        reconnectWork = nil
        sessionHelloTimeoutWork?.cancel()
        sessionHelloTimeoutWork = nil
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
        connected = false
        sessionVerified = false
    }

    /// Called when the app returns to the foreground. Reconnect
    /// immediately rather than waiting out the backoff delay.
    func nudge() {
        guard currentBinding != nil, task == nil else { return }
        reconnectWork?.cancel()
        openSocket()
    }

    private func openSocket() {
        guard let binding = currentBinding else { return }
        failAllDeltaBarriers()
        completeAllSessionVerifications(false)
        sessionHelloTimeoutWork?.cancel()
        task?.cancel(with: .goingAway, reason: nil)
        generation += 1
        let myGeneration = generation
        let task = session.webSocketTask(with: binding.websocketURL)
        // Raise the WS receive cap so large inbound Loro frames (full
        // snapshots of big notes) aren't dropped by the default 1 MiB
        // limit (multi-device convergence spec, Part B).
        task.maximumMessageSize = 64 * 1024 * 1024
        self.task = task
        task.resume()
        connected = false
        sessionVerified = false
        receive(on: task, generation: myGeneration)
        scheduleSessionHelloTimeout(on: task, generation: myGeneration)
    }

    private func receive(on task: URLSessionWebSocketTask, generation myGeneration: Int) {
        task.receive { [weak self] result in
            Task { @MainActor in
                guard let self, myGeneration == self.generation else { return }
                switch result {
                case .success(let message):
                    guard self.handle(message, on: task, generation: myGeneration) else {
                        return
                    }
                    self.lastEventAt = Date()
                    self.receive(on: task, generation: myGeneration)
                case .failure:
                    self.invalidateDeltaConnection(on: task, generation: myGeneration)
                }
            }
        }
    }

    private func handle(
        _ message: URLSessionWebSocketTask.Message,
        on task: URLSessionWebSocketTask,
        generation: Int
    ) -> Bool {
        guard sessionVerified else {
            guard case .string(let text) = message,
                  let expectedMosaicPath = currentBinding?.expectedMosaicPath,
                  let expectedGroupIdHex = currentBinding?.expectedGroupIdHex,
                  let hubIdentity = currentBinding?.hubIdentity,
                  !expectedMosaicPath.isEmpty,
                  !expectedGroupIdHex.isEmpty,
                  !hubIdentity.isEmpty,
                  Self.sessionHelloMatches(
                      text,
                      expectedMosaicPath: expectedMosaicPath,
                      expectedGroupIdHex: expectedGroupIdHex
                  )
            else {
                invalidateIdentityBinding(on: task, generation: generation)
                return false
            }
            sessionVerified = true
            connected = true
            reconnectAttempt = 0
            sessionHelloTimeoutWork?.cancel()
            sessionHelloTimeoutWork = nil
            completeSessionVerifications(on: task, generation: generation, verified: true)
            onConnectionAttempt?()
            return true
        }

        guard isCurrentVerifiedSession(on: task, generation: generation) else {
            return false
        }
        switch message {
        case .data(let d):
            // Ephemeral presence (PRES) is checked FIRST — a transient peer
            // caret, not a document delta; it must never reach the engine.
            if let frame = LoroPresence.decode(d) {
                onPresence?(frame)
                return true
            }
            // Binary frame = TLR2 Loro delta (instant-multidevice spec
            // §4). Hand the raw bytes to the engine owner via the
            // callback; do NOT attempt to UTF-8/JSON-decode them.
            guard let hubIdentity = currentBinding?.hubIdentity else {
                invalidateDeltaConnection(on: task, generation: generation)
                return false
            }
            onBinaryDelta?(d, hubIdentity)
        case .string(let s):
            handleTextFrame(s, on: task, generation: generation)
        @unknown default:
            break
        }
        return true
    }

    private func handleTextFrame(
        _ text: String,
        on task: URLSessionWebSocketTask,
        generation: Int
    ) {
        if let acknowledgement = Self.decodeBarrierAcknowledgement(text) {
            finishDeltaBarrier(
                acknowledgement.id,
                on: task,
                generation: generation,
                acknowledgedMosaicPath: acknowledgement.mosaicPath,
                acknowledgedGroupIdHex: acknowledgement.groupIdHex,
                delivered: acknowledgement.ok
            )
            return
        }
        guard let data = text.data(using: .utf8),
              let envelope = try? JSONDecoder().decode(WSEnvelope.self, from: data)
        else { return }
        switch envelope.event {
        case "note_created", "note_updated", "note_deleted":
            onNoteChange?()
        case "views_changed":
            onViewsChange?()
        default:
            break  // deadline / scheduled notifications — not handled here
        }
    }

    /// Push a TLR2-framed Loro delta to the hub as a binary WS frame.
    /// Returns `true` ONLY after the same connection receives the server's
    /// positive `loro_barrier_ack`, proving every note in the preceding frame
    /// applied without a pending/failed import. A socket write completion is
    /// only queue admission and is never treated as delivery proof.
    /// The caller must NOT advance its per-note `lastPushedVV` baseline on
    /// `false`, so the dropped ops are re-included in the next delta —
    /// otherwise a since_vv delta would skip them forever (in hub mode the
    /// WS is the SOLE author→hub path; the relay tick is gated off).
    ///
    /// Audit A7: the old version returned `true` for any frame QUEUED onto
    /// a socket whose `connected` flag is set optimistically pre-handshake
    /// (openSocket flips it right after `resume()`), ignoring the send
    /// completion. A frame queued onto a connection that never completed
    /// its handshake — or racing a dying socket — was reported as sent,
    /// the baseline advanced, and the edit was permanently excluded from
    /// WS delivery (silent one-way divergence). Awaiting the completion
    /// covers both: URLSession queues pre-handshake sends and fails their
    /// completions when the connection ultimately fails.
    /// The bytes are produced by the engine owner
    /// (`RelayTicker.produceDeltaFrame(slug:)`); this type never touches
    /// the engine.
    @discardableResult
    func sendDelta(_ frame: Data, requiredHubIdentity: String? = nil) async -> Bool {
        guard let capturedTask = task,
              let capturedBinding = currentBinding,
              let expectedMosaicPath = capturedBinding.expectedMosaicPath,
              let expectedGroupIdHex = capturedBinding.expectedGroupIdHex,
              let hubIdentity = capturedBinding.hubIdentity,
              !expectedMosaicPath.isEmpty,
              !expectedGroupIdHex.isEmpty,
              !hubIdentity.isEmpty,
              requiredHubIdentity == nil || requiredHubIdentity == hubIdentity
        else { return false }
        let capturedGeneration = generation
        guard await awaitSessionVerification(
            on: capturedTask,
            generation: capturedGeneration
        ),
            isCurrentVerifiedSession(
                on: capturedTask,
                generation: capturedGeneration,
                binding: capturedBinding,
                requiredHubIdentity: requiredHubIdentity
            )
        else { return false }

        await acquireDeltaSendSlot()
        defer { releaseDeltaSendSlot() }
        guard isCurrentVerifiedSession(
            on: capturedTask,
            generation: capturedGeneration,
            binding: capturedBinding,
            requiredHubIdentity: requiredHubIdentity
        )
        else { return false }
        return await sendDeltaTransaction(
            frame,
            on: capturedTask,
            generation: capturedGeneration,
            expectedMosaicPath: expectedMosaicPath,
            expectedGroupIdHex: expectedGroupIdHex,
            hubIdentity: hubIdentity
        )
    }

    /// The server's barrier window is connection-wide, so binary+barrier
    /// transactions must never overlap. Without this slot, concurrent edits
    /// could put two frames before one barrier; a later empty-window ack could
    /// then falsely confirm the wrong frame.
    private func acquireDeltaSendSlot() async {
        guard deltaSendInFlight else {
            deltaSendInFlight = true
            return
        }
        await withCheckedContinuation { continuation in
            deltaSendWaiters.append(continuation)
        }
    }

    private func releaseDeltaSendSlot() {
        guard !deltaSendWaiters.isEmpty else {
            deltaSendInFlight = false
            return
        }
        deltaSendWaiters.removeFirst().resume()
    }

    private func sendDeltaTransaction(
        _ frame: Data,
        on task: URLSessionWebSocketTask,
        generation myGeneration: Int,
        expectedMosaicPath: String,
        expectedGroupIdHex: String,
        hubIdentity: String
    ) async -> Bool {
        let barrierId = UUID()
        let request = BarrierRequest(
            event: "loro_barrier",
            barrierId: barrierId.uuidString.lowercased(),
            expectedMosaicPath: expectedMosaicPath,
            expectedGroupIdHex: expectedGroupIdHex
        )
        guard let requestData = try? JSONEncoder().encode(request),
              let requestText = String(data: requestData, encoding: .utf8)
        else { return false }
        return await withCheckedContinuation { cont in
            let completion = DeltaSendCompletion(cont)
            pendingDeltaBarriers[barrierId] = PendingDeltaBarrier(
                task: task,
                generation: myGeneration,
                expectedMosaicPath: expectedMosaicPath,
                expectedGroupIdHex: expectedGroupIdHex,
                hubIdentity: hubIdentity,
                completion: completion
            )
            task.send(.data(frame)) { error in
                Task { @MainActor [weak self] in
                    guard let self else {
                        completion.resume(false)
                        return
                    }
                    guard error == nil,
                          self.isCurrentVerifiedSession(
                              on: task,
                              generation: myGeneration,
                              expectedMosaicPath: expectedMosaicPath,
                              expectedGroupIdHex: expectedGroupIdHex,
                              hubIdentity: hubIdentity
                          ),
                          self.pendingDeltaBarriers[barrierId] != nil
                    else {
                        if error != nil {
                            self.invalidateDeltaConnection(on: task, generation: myGeneration)
                        } else {
                            self.failDeltaBarrier(
                                barrierId,
                                on: task,
                                generation: myGeneration
                            )
                        }
                        return
                    }
                    task.send(.string(requestText)) { error in
                        guard error != nil else { return }
                        Task { @MainActor [weak self] in
                            self?.invalidateDeltaConnection(
                                on: task,
                                generation: myGeneration
                            )
                        }
                    }
                }
            }
            DispatchQueue.global().asyncAfter(deadline: .now() + 10) {
                Task { @MainActor [weak self] in
                    guard let self else {
                        completion.resume(false)
                        return
                    }
                    guard self.pendingDeltaBarriers[barrierId] != nil else { return }
                    self.invalidateDeltaConnection(on: task, generation: myGeneration)
                }
            }
        }
    }

    private func sendActivationBarrier(
        on task: URLSessionWebSocketTask,
        generation myGeneration: Int,
        expectedMosaicPath: String,
        expectedGroupIdHex: String,
        hubIdentity: String
    ) async -> Bool {
        let barrierId = UUID()
        let request = BarrierRequest(
            event: "loro_barrier",
            barrierId: barrierId.uuidString.lowercased(),
            expectedMosaicPath: expectedMosaicPath,
            expectedGroupIdHex: expectedGroupIdHex
        )
        guard let requestData = try? JSONEncoder().encode(request),
              let requestText = String(data: requestData, encoding: .utf8)
        else { return false }
        return await withCheckedContinuation { continuation in
            let completion = DeltaSendCompletion(continuation)
            pendingDeltaBarriers[barrierId] = PendingDeltaBarrier(
                task: task,
                generation: myGeneration,
                expectedMosaicPath: expectedMosaicPath,
                expectedGroupIdHex: expectedGroupIdHex,
                hubIdentity: hubIdentity,
                completion: completion
            )
            task.send(.string(requestText)) { error in
                guard error != nil else { return }
                Task { @MainActor [weak self] in
                    self?.invalidateDeltaConnection(
                        on: task,
                        generation: myGeneration
                    )
                }
            }
            DispatchQueue.global().asyncAfter(deadline: .now() + 10) {
                Task { @MainActor [weak self] in
                    guard let self else {
                        completion.resume(false)
                        return
                    }
                    guard self.pendingDeltaBarriers[barrierId] != nil else { return }
                    self.invalidateDeltaConnection(on: task, generation: myGeneration)
                }
            }
        }
    }

    static func decodeBarrierAcknowledgement(
        _ text: String
    ) -> (id: UUID, ok: Bool, mosaicPath: String, groupIdHex: String)? {
        guard let data = text.data(using: .utf8),
              let acknowledgement = try? JSONDecoder().decode(
                  BarrierAcknowledgement.self,
                  from: data
              ),
              acknowledgement.event == "loro_barrier_ack",
              let id = UUID(uuidString: acknowledgement.barrierId),
              !acknowledgement.mosaicPath.isEmpty,
              !acknowledgement.groupIdHex.isEmpty
        else { return nil }
        return (
            id,
            acknowledgement.ok,
            acknowledgement.mosaicPath,
            acknowledgement.groupIdHex.lowercased()
        )
    }

    static func decodeBarrierAcknowledgement(
        _ text: String,
        expectedMosaicPath: String,
        expectedGroupIdHex: String
    ) -> (id: UUID, ok: Bool, mosaicPath: String, groupIdHex: String)? {
        guard let acknowledgement = decodeBarrierAcknowledgement(text),
              acknowledgement.mosaicPath == expectedMosaicPath,
              acknowledgement.groupIdHex == expectedGroupIdHex.lowercased()
        else { return nil }
        return acknowledgement
    }

    private func finishDeltaBarrier(
        _ id: UUID,
        on task: URLSessionWebSocketTask,
        generation: Int,
        acknowledgedMosaicPath: String,
        acknowledgedGroupIdHex: String,
        delivered: Bool
    ) {
        guard let pending = pendingDeltaBarriers[id],
              pending.task === task,
              pending.generation == generation
        else { return }
        let sessionIsVerified = isCurrentVerifiedSession(
            on: task,
            generation: generation,
            expectedMosaicPath: pending.expectedMosaicPath,
            expectedGroupIdHex: pending.expectedGroupIdHex,
            hubIdentity: pending.hubIdentity
        )
        let identityMatches = pending.expectedMosaicPath == acknowledgedMosaicPath
            && pending.expectedGroupIdHex == acknowledgedGroupIdHex.lowercased()
        let verified = Self.activationVerificationSucceeded(
            sessionVerified: sessionIsVerified,
            expectedMosaicPath: pending.expectedMosaicPath,
            expectedGroupIdHex: pending.expectedGroupIdHex,
            acknowledgedMosaicPath: acknowledgedMosaicPath,
            acknowledgedGroupIdHex: acknowledgedGroupIdHex,
            delivered: delivered
        )
        pendingDeltaBarriers.removeValue(forKey: id)
        pending.completion.resume(verified)
        if !identityMatches {
            invalidateIdentityBinding(on: task, generation: generation)
        } else if !sessionIsVerified {
            invalidateDeltaConnection(on: task, generation: generation)
        }
    }

    private func failDeltaBarrier(
        _ id: UUID,
        on task: URLSessionWebSocketTask,
        generation: Int
    ) {
        guard let pending = pendingDeltaBarriers[id],
              pending.task === task,
              pending.generation == generation
        else { return }
        pendingDeltaBarriers.removeValue(forKey: id)
        pending.completion.resume(false)
    }

    private func failDeltaBarriers(
        on task: URLSessionWebSocketTask,
        generation: Int
    ) {
        let ids = pendingDeltaBarriers.compactMap { id, pending in
            pending.task === task && pending.generation == generation ? id : nil
        }
        for id in ids {
            failDeltaBarrier(id, on: task, generation: generation)
        }
    }

    private func failAllDeltaBarriers() {
        let pending = pendingDeltaBarriers.values
        pendingDeltaBarriers.removeAll()
        for barrier in pending {
            barrier.completion.resume(false)
        }
    }

    private func awaitSessionVerification(
        on task: URLSessionWebSocketTask,
        generation: Int
    ) async -> Bool {
        if isCurrentVerifiedSession(on: task, generation: generation) {
            return true
        }
        return await withCheckedContinuation { continuation in
            let completion = DeltaSendCompletion(continuation)
            guard self.task === task,
                  self.generation == generation,
                  currentBinding != nil
            else {
                completion.resume(false)
                return
            }
            pendingSessionVerifications.append(PendingSessionVerification(
                task: task,
                generation: generation,
                completion: completion
            ))
        }
    }

    private func completeSessionVerifications(
        on task: URLSessionWebSocketTask,
        generation: Int,
        verified: Bool
    ) {
        var retained: [PendingSessionVerification] = []
        for pending in pendingSessionVerifications {
            if pending.task === task, pending.generation == generation {
                pending.completion.resume(verified)
            } else {
                retained.append(pending)
            }
        }
        pendingSessionVerifications = retained
    }

    private func completeAllSessionVerifications(_ verified: Bool) {
        let pending = pendingSessionVerifications
        pendingSessionVerifications.removeAll()
        for verification in pending {
            verification.completion.resume(verified)
        }
    }

    private func isCurrentVerifiedSession(
        on task: URLSessionWebSocketTask,
        generation expectedGeneration: Int
    ) -> Bool {
        connected
            && sessionVerified
            && self.task === task
            && generation == expectedGeneration
    }

    private func isCurrentVerifiedSession(
        on task: URLSessionWebSocketTask,
        generation expectedGeneration: Int,
        binding: ConnectionBinding,
        requiredHubIdentity: String?
    ) -> Bool {
        isCurrentVerifiedSession(on: task, generation: expectedGeneration)
            && currentBinding == binding
            && (requiredHubIdentity == nil || binding.hubIdentity == requiredHubIdentity)
    }

    private func isCurrentVerifiedSession(
        on task: URLSessionWebSocketTask,
        generation expectedGeneration: Int,
        expectedMosaicPath: String,
        expectedGroupIdHex: String,
        hubIdentity: String
    ) -> Bool {
        isCurrentVerifiedSession(on: task, generation: expectedGeneration)
            && currentBinding?.expectedMosaicPath == expectedMosaicPath
            && currentBinding?.expectedGroupIdHex == expectedGroupIdHex
            && currentBinding?.hubIdentity == hubIdentity
    }

    private func scheduleSessionHelloTimeout(
        on task: URLSessionWebSocketTask,
        generation expectedGeneration: Int
    ) {
        sessionHelloTimeoutWork?.cancel()
        sessionHelloTimeoutWork = Task { [weak self] in
            do {
                try await Task.sleep(nanoseconds: 10_000_000_000)
            } catch {
                return
            }
            guard let self,
                  !Task.isCancelled,
                  !self.isCurrentVerifiedSession(
                      on: task,
                      generation: expectedGeneration
                  )
            else { return }
            self.invalidateDeltaConnection(on: task, generation: expectedGeneration)
        }
    }

    /// A timed-out/failed barrier means this connection can no longer prove
    /// ordered delivery. Tear it down, fail the active transaction, and let
    /// queued senders drain immediately against `connected == false` while a
    /// fresh generation reconnects in the background.
    private func invalidateDeltaConnection(
        on task: URLSessionWebSocketTask,
        generation expectedGeneration: Int
    ) {
        guard self.task === task, generation == expectedGeneration else {
            failDeltaBarriers(on: task, generation: expectedGeneration)
            completeSessionVerifications(
                on: task,
                generation: expectedGeneration,
                verified: false
            )
            return
        }
        failDeltaBarriers(on: task, generation: expectedGeneration)
        completeSessionVerifications(
            on: task,
            generation: expectedGeneration,
            verified: false
        )
        sessionHelloTimeoutWork?.cancel()
        sessionHelloTimeoutWork = nil
        generation += 1
        connected = false
        sessionVerified = false
        task.cancel(with: .goingAway, reason: nil)
        self.task = nil
        scheduleReconnect()
    }

    /// A session hello or barrier disproved the exact group/path binding. A
    /// fully-published binding is terminal for this reconnect loop: consume its
    /// callback once, forget the stale target, and let the shell rediscover the
    /// current identity. During activation there is no published lease yet, so
    /// the existing activation retry remains the sole owner of recovery.
    private func invalidateIdentityBinding(
        on task: URLSessionWebSocketTask,
        generation expectedGeneration: Int
    ) {
        guard self.task === task, generation == expectedGeneration else {
            failDeltaBarriers(on: task, generation: expectedGeneration)
            completeSessionVerifications(
                on: task,
                generation: expectedGeneration,
                verified: false
            )
            return
        }
        let invalidationCallback = currentBinding.flatMap {
            bindingInvalidationState.takeInvalidationCallback(for: $0)
        }
        failDeltaBarriers(on: task, generation: expectedGeneration)
        completeSessionVerifications(
            on: task,
            generation: expectedGeneration,
            verified: false
        )
        sessionHelloTimeoutWork?.cancel()
        sessionHelloTimeoutWork = nil
        generation += 1
        connected = false
        sessionVerified = false
        task.cancel(with: .goingAway, reason: nil)
        self.task = nil
        if let invalidationCallback {
            reconnectWork?.cancel()
            reconnectWork = nil
            currentBinding = nil
            invalidationCallback()
        } else {
            scheduleReconnect()
        }
    }

    /// Push an ephemeral presence frame (PRES). Fire-and-forget: presence is
    /// transient + lossy-tolerant, so — unlike `sendDelta` — we don't await the
    /// completion or gate any baseline on it. No-op when not connected.
    func sendPresence(_ frame: Data) {
        guard let task,
              let binding = currentBinding,
              let expectedMosaicPath = binding.expectedMosaicPath,
              let expectedGroupIdHex = binding.expectedGroupIdHex,
              let hubIdentity = binding.hubIdentity,
              !expectedMosaicPath.isEmpty,
              !expectedGroupIdHex.isEmpty,
              !hubIdentity.isEmpty,
              isCurrentVerifiedSession(
                  on: task,
                  generation: generation,
                  binding: binding,
                  requiredHubIdentity: hubIdentity
              )
        else { return }
        task.send(.data(frame)) { _ in }
    }

    private func scheduleReconnect() {
        reconnectWork?.cancel()
        // 1s, 2s, 4s … capped at 32s — mirrors the web client's backoff.
        let delaySecs = min(1 << min(reconnectAttempt, 5), 32)
        reconnectAttempt += 1
        let myGeneration = generation
        reconnectWork = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(delaySecs) * 1_000_000_000)
            guard let self, !Task.isCancelled, myGeneration == self.generation else { return }
            self.openSocket()
        }
    }

    /// Derive the `ws(s)://host/ws` URL from an `http(s)` server URL.
    private static func wsURL(from serverURL: String) -> URL? {
        let trimmed = serverURL.trimmingCharacters(in: .whitespaces)
        guard var components = URLComponents(string: trimmed) else { return nil }
        components.scheme = (components.scheme?.lowercased() == "https") ? "wss" : "ws"
        components.path = "/ws"
        components.query = nil
        return components.url
    }

    static func connectionBinding(
        serverURL: String,
        expectedMosaicPath: String?,
        expectedGroupIdHex: String?,
        hubIdentity: String?
    ) -> ConnectionBinding? {
        guard let websocketURL = wsURL(from: serverURL) else { return nil }
        return ConnectionBinding(
            websocketURL: websocketURL,
            expectedMosaicPath: expectedMosaicPath,
            expectedGroupIdHex: expectedGroupIdHex?.lowercased(),
            hubIdentity: hubIdentity
        )
    }

    static func decodeSessionHello(
        _ text: String
    ) -> (mosaicPath: String, groupIdHex: String)? {
        guard let data = text.data(using: .utf8),
              let hello = try? JSONDecoder().decode(SessionHello.self, from: data),
              hello.event == "loro_session",
              !hello.mosaicPath.isEmpty,
              !hello.groupIdHex.isEmpty
        else { return nil }
        return (hello.mosaicPath, hello.groupIdHex.lowercased())
    }

    static func sessionHelloMatches(
        _ text: String,
        expectedMosaicPath: String,
        expectedGroupIdHex: String
    ) -> Bool {
        guard let hello = decodeSessionHello(text) else { return false }
        return hello.mosaicPath == expectedMosaicPath
            && hello.groupIdHex == expectedGroupIdHex.lowercased()
    }

    static func activationVerificationSucceeded(
        sessionVerified: Bool,
        expectedMosaicPath: String,
        expectedGroupIdHex: String,
        acknowledgedMosaicPath: String,
        acknowledgedGroupIdHex: String,
        delivered: Bool
    ) -> Bool {
        sessionVerified
            && delivered
            && acknowledgedMosaicPath == expectedMosaicPath
            && acknowledgedGroupIdHex.lowercased() == expectedGroupIdHex.lowercased()
    }

    private struct WSEnvelope: Decodable {
        let event: String
    }

    private struct SessionHello: Decodable {
        let event: String
        let mosaicPath: String
        let groupIdHex: String

        enum CodingKeys: String, CodingKey {
            case event
            case mosaicPath = "mosaic_path"
            case groupIdHex = "group_id_hex"
        }
    }

    private struct BarrierRequest: Encodable {
        let event: String
        let barrierId: String
        let expectedMosaicPath: String
        let expectedGroupIdHex: String

        enum CodingKeys: String, CodingKey {
            case event
            case barrierId = "barrier_id"
            case expectedMosaicPath = "expected_mosaic_path"
            case expectedGroupIdHex = "expected_group_id_hex"
        }
    }

    private struct BarrierAcknowledgement: Decodable {
        let event: String
        let barrierId: String
        let ok: Bool
        let mosaicPath: String
        let groupIdHex: String

        enum CodingKeys: String, CodingKey {
            case event
            case barrierId = "barrier_id"
            case ok
            case mosaicPath = "mosaic_path"
            case groupIdHex = "group_id_hex"
        }
    }
}
