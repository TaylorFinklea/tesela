import Foundation
import Observation

// MARK: - WebSocketClient
// Connects to ws://localhost:7474/ws and streams live note events

@Observable
@MainActor
final class WebSocketClient {
    private(set) var isConnected = false

    // Callbacks — set by AppState
    var onNoteCreated: ((Page) -> Void)?
    var onNoteUpdated: ((Page) -> Void)?
    var onNoteDeleted: ((String) -> Void)?

    private var task: URLSessionWebSocketTask?
    private var reconnectTask: Task<Void, Never>?
    private var retryDelay: Duration = .seconds(1)
    private let maxRetryDelay: Duration = .seconds(30)
    private let wsURL = URL(string: "ws://localhost:7474/ws")!
    private let decoder = JSONDecoder()

    init() {
        decoder.dateDecodingStrategy = .iso8601
    }

    func connect() async {
        guard task == nil else { return }
        await openConnection()
    }

    func disconnect() {
        reconnectTask?.cancel()
        reconnectTask = nil
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
        isConnected = false
    }

    // MARK: - Private

    private func openConnection() async {
        let session = URLSession.shared
        task = session.webSocketTask(with: wsURL)
        task?.resume()
        isConnected = true
        retryDelay = .seconds(1)
        await receiveLoop()
    }

    private func receiveLoop() async {
        guard let task else { return }
        do {
            while !Task.isCancelled {
                let message = try await task.receive()
                handleMessage(message)
            }
        } catch {
            isConnected = false
            self.task = nil
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

        guard let envelope = try? decoder.decode(WsMessage.self, from: data) else { return }

        switch envelope.event {
        case "note_created":
            if let noteData = try? decoder.decode(WsNoteEvent.self, from: data),
               let note = noteData.note {
                onNoteCreated?(note)
            }
        case "note_updated":
            if let noteData = try? decoder.decode(WsNoteEvent.self, from: data),
               let note = noteData.note {
                onNoteUpdated?(note)
            }
        case "note_deleted":
            if let noteData = try? decoder.decode(WsNoteEvent.self, from: data),
               let id = noteData.id {
                onNoteDeleted?(id)
            }
        default:
            break
        }
    }

    private func scheduleReconnect() {
        reconnectTask = Task { [weak self] in
            guard let self else { return }
            try? await Task.sleep(for: retryDelay)
            guard !Task.isCancelled else { return }
            retryDelay = min(retryDelay * 2, maxRetryDelay)
            await self.openConnection()
        }
    }
}
