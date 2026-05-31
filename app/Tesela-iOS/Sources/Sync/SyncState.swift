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
        default:
            break  // deadline / scheduled notifications — not handled here
        }
    }

    /// Push a TLR2-framed Loro delta to the hub as a binary WS frame.
    /// No-op when the socket isn't connected — the relay tick remains
    /// the fallback delivery path, and a reconnect catch-up (Phase D)
    /// will reconcile anything dropped here. The bytes are produced by
    /// the engine owner (`RelayTicker.produceDeltaFrame(slug:)`); this
    /// type never touches the engine.
    func sendDelta(_ frame: Data) {
        guard connected, let task else { return }
        task.send(.data(frame)) { _ in
            // Best-effort: a send failure surfaces as the next receive
            // failure, which drives the existing reconnect/backoff path.
        }
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
