import SwiftUI
import Combine

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
    var onBinaryDelta: ((Data) -> Void)?

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

    private let session = URLSession(configuration: .default)
    private var task: URLSessionWebSocketTask?
    private var currentURL: URL?
    private var connected = false
    private var reconnectAttempt = 0
    private var reconnectWork: Task<Void, Never>?
    /// Bumped on every (re)connect and on disconnect so a stale receive
    /// loop or pending reconnect from a superseded socket bows out.
    private var generation = 0

    /// Point the socket at a server, tearing down any existing
    /// connection first. Pass `nil` (mock mode / no server) to just
    /// disconnect. A no-op when already connected to the same URL.
    func connect(serverURL: String?) {
        guard let serverURL, let ws = Self.wsURL(from: serverURL) else {
            disconnect()
            return
        }
        if currentURL == ws && connected { return }
        currentURL = ws
        openSocket()
    }

    func disconnect() {
        generation += 1
        reconnectWork?.cancel()
        reconnectWork = nil
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
        currentURL = nil
        connected = false
    }

    /// Tear the socket down but remember the URL, so `nudge()` can
    /// bring it back. Called when the app is backgrounded — iOS would
    /// suspend the connection anyway, and an explicit teardown avoids a
    /// hung `receive` on resume.
    func suspend() {
        generation += 1
        reconnectWork?.cancel()
        reconnectWork = nil
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
        connected = false
    }

    /// Called when the app returns to the foreground. Reconnect
    /// immediately rather than waiting out the backoff delay.
    func nudge() {
        guard currentURL != nil, !connected else { return }
        reconnectWork?.cancel()
        openSocket()
    }

    private func openSocket() {
        guard let url = currentURL else { return }
        generation += 1
        let myGeneration = generation
        let task = session.webSocketTask(with: url)
        // Raise the WS receive cap so large inbound Loro frames (full
        // snapshots of big notes) aren't dropped by the default 1 MiB
        // limit (multi-device convergence spec, Part B).
        task.maximumMessageSize = 64 * 1024 * 1024
        self.task = task
        task.resume()
        connected = true
        reconnectAttempt = 0
        receive(on: task, generation: myGeneration)
    }

    private func receive(on task: URLSessionWebSocketTask, generation myGeneration: Int) {
        task.receive { [weak self] result in
            Task { @MainActor in
                guard let self, myGeneration == self.generation else { return }
                switch result {
                case .success(let message):
                    self.handle(message)
                    self.receive(on: task, generation: myGeneration)
                case .failure:
                    self.connected = false
                    self.scheduleReconnect()
                }
            }
        }
    }

    private func handle(_ message: URLSessionWebSocketTask.Message) {
        switch message {
        case .data(let d):
            // Ephemeral presence (PRES) is checked FIRST — a transient peer
            // caret, not a document delta; it must never reach the engine.
            if let frame = LoroPresence.decode(d) {
                onPresence?(frame)
                return
            }
            // Binary frame = TLR2 Loro delta (instant-multidevice spec
            // §4). Hand the raw bytes to the engine owner via the
            // callback; do NOT attempt to UTF-8/JSON-decode them.
            onBinaryDelta?(d)
        case .string(let s):
            handleTextFrame(s)
        @unknown default:
            break
        }
    }

    private func handleTextFrame(_ text: String) {
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
    /// Returns `true` ONLY after the send completion confirms the frame
    /// actually went out on the wire; `false` when there is no socket or
    /// the send (or the handshake it was queued behind) ultimately failed.
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
    func sendDelta(_ frame: Data) async -> Bool {
        guard connected, let task else { return false }
        return await withCheckedContinuation { cont in
            task.send(.data(frame)) { error in
                cont.resume(returning: error == nil)
            }
        }
    }

    /// Push an ephemeral presence frame (PRES). Fire-and-forget: presence is
    /// transient + lossy-tolerant, so — unlike `sendDelta` — we don't await the
    /// completion or gate any baseline on it. No-op when not connected.
    func sendPresence(_ frame: Data) {
        guard connected, let task else { return }
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
        components.scheme = (components.scheme == "https") ? "wss" : "ws"
        components.path = "/ws"
        components.query = nil
        return components.url
    }

    private struct WSEnvelope: Decodable {
        let event: String
    }
}
