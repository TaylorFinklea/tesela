import Foundation

// MARK: - WebSocket event envelope
struct WsMessage: Codable, Sendable {
    let event: String
    let data: WsEventData?
}

enum WsEventData: Codable, Sendable {
    case noteCreated(Page)
    case noteUpdated(Page)
    case noteDeleted(String)   // note id

    init(from decoder: any Decoder) throws {
        // Decoded by WsMessage parsing logic in WebSocketClient
        throw DecodingError.dataCorrupted(
            .init(codingPath: [], debugDescription: "Use WsMessage + event field to decode")
        )
    }

    func encode(to encoder: any Encoder) throws {}
}

// Flat representation used by the WebSocket client
struct WsNoteEvent: Codable, Sendable {
    let id: String?
    let note: Page?
}
