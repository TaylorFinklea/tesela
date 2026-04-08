import Foundation
import Observation

// MARK: - WebSocketClient
// Connects to ws://127.0.0.1:7474/ws and streams live note events.
// Automatically reconnects with exponential backoff when the connection drops.

@Observable
@MainActor
final class WebSocketClient {
    private(set) var isConnected = false

    // Callbacks — set by AppState
    var onNoteCreated: ((Page) -> Void)?
    var onNoteUpdated: ((Page) -> Void)?
    var onNoteDeleted: ((String) -> Void)?
    var onConnectionStateChanged: ((Bool) -> Void)?

    private var task: URLSessionWebSocketTask?
    private var reconnectTask: Task<Void, Never>?

    /// Tracks whether the caller explicitly disconnected. When true, reconnect
    /// attempts are suppressed so the client stays idle until connect() is
    /// called again.
    private var intentionallyStopped = false

    private var retryDelay: Duration = .seconds(1)
    private let minRetryDelay: Duration = .seconds(1)
    private let maxRetryDelay: Duration = .seconds(30)
    private let wsURL = URL(string: "ws://127.0.0.1:7474/ws")!
    private let decoder = JSONDecoder()

    init() {
        decoder.dateDecodingStrategy = .iso8601
    }

    // MARK: - Public API

    /// Begin connecting. Safe to call multiple times; idempotent when already
    /// connected or a reconnect loop is already running.
    func connect() async {
        // Clear the intentional-stop flag so reconnect loops are permitted.
        intentionallyStopped = false

        // If already connected, nothing to do.
        guard task == nil else { return }

        // Cancel any pending reconnect sleep so we connect immediately.
        reconnectTask?.cancel()
        reconnectTask = nil

        retryDelay = minRetryDelay
        await openConnection()
    }

    /// Permanently disconnect and suppress any future automatic reconnects
    /// until connect() is called again.
    func disconnect() {
        intentionallyStopped = true
        cancelReconnect()
        closeTask()
        isConnected = false
        onConnectionStateChanged?(false)
    }

    // MARK: - Private helpers

    private func openConnection() async {
        let session = URLSession.shared
        let newTask = session.webSocketTask(with: wsURL)
        task = newTask
        newTask.resume()

        // Mark connected optimistically — URLSession does not expose an
        // explicit "handshake complete" callback, so we trust that receive()
        // will throw immediately if the server is not reachable.
        isConnected = true
        onConnectionStateChanged?(true)

        // Reset backoff on every successful open attempt.
        retryDelay = minRetryDelay

        await receiveLoop(for: newTask)
    }

    /// Reads messages in a loop until the task errors or is cancelled.
    /// Passes `task` as a parameter so a stale closure never reads from a
    /// replaced task reference stored on self.
    private func receiveLoop(for wsTask: URLSessionWebSocketTask) async {
        do {
            while !Task.isCancelled {
                let message = try await wsTask.receive()
                handleMessage(message)
            }
        } catch {
            // Only react if this error belongs to the task we're currently
            // tracking. If self.task has already been replaced (e.g. by a
            // concurrent openConnection call), ignore the stale error.
            guard task === wsTask else { return }

            task = nil
            isConnected = false
            onConnectionStateChanged?(false)

            guard !intentionallyStopped else { return }
            scheduleReconnect()
        }
    }

    private func handleMessage(_ message: URLSessionWebSocketTask.Message) {
        let data: Data
        switch message {
        case .data(let d): data = d
        case .string(let s): data = Data(s.utf8)
        @unknown default: return
        }

        guard let event = try? decoder.decode(WsEvent.self, from: data) else { return }

        switch event {
        case .noteCreated(let note): onNoteCreated?(note)
        case .noteUpdated(let note): onNoteUpdated?(note)
        case .noteDeleted(let id):   onNoteDeleted?(id)
        }
    }

    private func scheduleReconnect() {
        // Cancel any previously scheduled reconnect before creating a new one
        // to prevent two concurrent reconnect tasks from both calling
        // openConnection().
        reconnectTask?.cancel()

        let delay = retryDelay
        // Double the delay for the next attempt, capped at maxRetryDelay.
        retryDelay = min(retryDelay * 2, maxRetryDelay)

        reconnectTask = Task { [weak self] in
            guard let self else { return }
            try? await Task.sleep(for: delay)
            guard !Task.isCancelled, !self.intentionallyStopped else { return }
            await self.openConnection()
        }
    }

    private func cancelReconnect() {
        reconnectTask?.cancel()
        reconnectTask = nil
    }

    private func closeTask() {
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
    }
}
