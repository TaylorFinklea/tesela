import Foundation
import Combine

/// Option-B relay-mode presence transport: a native
/// `URLSessionWebSocketTask` to the CF Worker's
/// `GET /groups/{group_id_hex}/presence/ws` endpoint, the iOS analog of the
/// desktop bridge `crates/tesela-server/src/presence_relay.rs`.
///
/// It byte-matches the desktop on the wire:
///   • the MAC-signed upgrade GET — five `x-tesela-*` headers computed by the
///     pure FFI `presenceWsHeaders` (fresh 16-byte MAC nonce + unix-seconds ts
///     + HMAC over the canonical `GET\n/groups/{hex}/presence/ws\n\n…` request),
///     regenerated on EVERY (re)connect so the replay window never goes stale;
///   • outbound: a raw `b"PRES"++json` frame is AEAD-sealed via `presenceSeal`
///     (XChaCha20-Poly1305 + postcard `OuterPayload`) then sent as a binary
///     message — so `publishPresence`/the editor stay transport-agnostic and
///     the sealing lives here;
///   • inbound: a binary frame is opened via `presenceOpen`, then decoded by
///     `LoroPresence.decode` and handed to `onPresence`.
///
/// Mirrors `LiveSyncSocket`'s lifecycle (generation-guarded receive loop,
/// 1/2/4…32s backoff reconnect, scenePhase suspend/nudge) and ADDS a 30s
/// heartbeat ping — CF Durable Objects evict idle WS sockets, so without it
/// relay presence silently dies after the idle window. Used only in relay
/// mode; hub mode (`.http`) keeps carrying presence over `LiveSyncSocket`'s
/// `/ws` fan-out.
@MainActor
final class PresenceRelaySocket: ObservableObject {
    /// Invoked on the main actor for each successfully opened + decoded peer
    /// caret. The shell wires this to `MockMosaicService.applyPresence`.
    var onPresence: ((LoroPresence.Frame) -> Void)?

    private let session = URLSession(configuration: .default)
    private var task: URLSessionWebSocketTask?

    /// Connection identity, kept so `nudge()` can re-open after a suspend and
    /// so each reconnect re-signs fresh headers. `nil` until the first
    /// `connect(...)` (and after `disconnect()`).
    private var url: URL?
    private var groupKey: Data?
    private var groupId: Data?
    private var deviceId: Data?

    private var connected = false
    private var reconnectAttempt = 0
    private var reconnectWork: Task<Void, Never>?
    private var heartbeatWork: Task<Void, Never>?
    /// Bumped on every (re)connect / suspend / disconnect so a stale receive
    /// loop, pending reconnect, or heartbeat from a superseded socket bows out.
    private var generation = 0

    /// WS keep-alive: ping the relay every 30s so CF doesn't evict the idle
    /// Durable Object socket. Matches the desktop bridge's `HEARTBEAT`.
    private let heartbeatInterval: TimeInterval = 30

    /// Point the socket at a relay group, tearing down any existing
    /// connection first. The four identity strings come from the cached
    /// pairing code (`relayUrl` / `groupIdHex` / `groupKeyHex`) and the
    /// stable local device id (`RelayTicker.persistedDeviceIdHex()`). A
    /// no-op when already connected to the same relay group. Silently does
    /// nothing on a malformed relay URL or hex (mirrors the desktop's
    /// "no presence" guard).
    func connect(relayUrl: String, groupIdHex: String, groupKeyHex: String, deviceIdHex: String) {
        guard let ws = Self.presenceURL(relayBase: relayUrl, groupIdHex: groupIdHex),
              let gKey = Self.data(fromHex: groupKeyHex),
              let gId = Self.data(fromHex: groupIdHex),
              let dId = Self.data(fromHex: deviceIdHex)
        else {
            disconnect()
            return
        }
        if url == ws && connected { return }
        url = ws
        groupKey = gKey
        groupId = gId
        deviceId = dId
        openSocket()
    }

    func disconnect() {
        generation += 1
        reconnectWork?.cancel(); reconnectWork = nil
        heartbeatWork?.cancel(); heartbeatWork = nil
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
        url = nil
        groupKey = nil
        groupId = nil
        deviceId = nil
        connected = false
    }

    /// Tear the socket down but remember the identity, so `nudge()` can bring
    /// it back. Called on background — iOS suspends the WS anyway, and an
    /// explicit teardown avoids a hung `receive` on resume.
    func suspend() {
        generation += 1
        reconnectWork?.cancel(); reconnectWork = nil
        heartbeatWork?.cancel(); heartbeatWork = nil
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
        connected = false
    }

    /// Called on foreground. Reconnect immediately rather than waiting out the
    /// backoff delay; re-signs fresh headers via `openSocket`.
    func nudge() {
        guard url != nil, !connected else { return }
        reconnectWork?.cancel()
        openSocket()
    }

    private func openSocket() {
        guard let url, let groupKey, let groupId, let deviceId else { return }
        generation += 1
        let myGeneration = generation
        // Fresh signed upgrade-GET on EVERY (re)connect — a new MAC nonce +
        // current ts per the relay's replay window (never cache headers).
        let headers = presenceWsHeaders(groupKey: groupKey, groupId: groupId, deviceId: deviceId)
        guard !headers.macB64.isEmpty else {
            // Couldn't sign (bad-length input). Retry on backoff rather than
            // connecting unauthenticated (the relay would reject the upgrade).
            connected = false
            scheduleReconnect()
            return
        }
        var request = URLRequest(url: url)
        request.setValue(headers.groupHex, forHTTPHeaderField: "x-tesela-group")
        request.setValue(headers.deviceHex, forHTTPHeaderField: "x-tesela-device")
        request.setValue(headers.nonceB64, forHTTPHeaderField: "x-tesela-nonce")
        request.setValue(String(headers.ts), forHTTPHeaderField: "x-tesela-ts")
        request.setValue(headers.macB64, forHTTPHeaderField: "x-tesela-mac")

        let task = session.webSocketTask(with: request)
        // Raise the receive cap to match LiveSyncSocket; presence frames are
        // small, but this avoids any default-limit drop of a batched frame.
        task.maximumMessageSize = 64 * 1024 * 1024
        self.task = task
        task.resume()
        connected = true
        reconnectAttempt = 0
        receive(on: task, generation: myGeneration)
        scheduleHeartbeat(on: task, generation: myGeneration)
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
        guard case .data(let outer) = message else { return }
        guard let groupKey, let groupId else { return }
        // AEAD-open the relay's opaque outer bytes → the inner PRES frame.
        // `nil` = a frame from another group / a forged tag — drop it.
        guard let inner = presenceOpen(groupKey: groupKey, groupId: groupId, outer: outer),
              let frame = LoroPresence.decode(inner)
        else { return }
        onPresence?(frame)
    }

    /// Seal then send a raw `b"PRES"++json` presence frame. Fire-and-forget:
    /// presence is transient + lossy-tolerant, so — like
    /// `LiveSyncSocket.sendPresence` — we don't await the completion. No-op
    /// when not connected or when the seal fails (empty result).
    func send(_ presFrame: Data) {
        guard connected, let task, let groupKey, let groupId else { return }
        let sealed = presenceSeal(groupKey: groupKey, groupId: groupId, inner: presFrame)
        guard !sealed.isEmpty else { return }
        task.send(.data(sealed)) { _ in }
    }

    private func scheduleHeartbeat(on task: URLSessionWebSocketTask, generation myGeneration: Int) {
        heartbeatWork?.cancel()
        heartbeatWork = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: UInt64(self?.heartbeatInterval ?? 30) * 1_000_000_000)
                guard let self, !Task.isCancelled, myGeneration == self.generation else { return }
                task.sendPing { _ in }
            }
        }
    }

    private func scheduleReconnect() {
        reconnectWork?.cancel()
        // 1s, 2s, 4s … capped at 32s — mirrors LiveSyncSocket / the web client.
        let delaySecs = min(1 << min(reconnectAttempt, 5), 32)
        reconnectAttempt += 1
        let myGeneration = generation
        reconnectWork = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(delaySecs) * 1_000_000_000)
            guard let self, !Task.isCancelled, myGeneration == self.generation else { return }
            self.openSocket()
        }
    }

    /// Scheme-swap a relay base URL (`http`→`ws`, `https`→`wss`) and join
    /// `/groups/{groupIdHex}/presence/ws`. Mirrors the desktop
    /// `presence_ws_url`; returns `nil` for a non-http(s) scheme.
    private static func presenceURL(relayBase: String, groupIdHex: String) -> URL? {
        let trimmed = relayBase.trimmingCharacters(in: .whitespaces)
        guard var components = URLComponents(string: trimmed) else { return nil }
        switch components.scheme {
        case "https": components.scheme = "wss"
        case "http": components.scheme = "ws"
        default: return nil
        }
        components.path = "/groups/\(groupIdHex)/presence/ws"
        components.query = nil
        return components.url
    }

    /// Decode an even-length lowercase/uppercase hex string to `Data`. `nil`
    /// on an odd length or a non-hex character. The FFI presence functions
    /// take raw `Data` (UniFFI maps `Vec<u8>`), so the pairing code's hex
    /// fields are decoded here.
    private static func data(fromHex hex: String) -> Data? {
        let chars = Array(hex)
        guard chars.count % 2 == 0 else { return nil }
        var out = Data(capacity: chars.count / 2)
        var i = 0
        while i < chars.count {
            guard let hi = chars[i].hexDigitValue, let lo = chars[i + 1].hexDigitValue else { return nil }
            out.append(UInt8(hi << 4 | lo))
            i += 2
        }
        return out
    }
}
