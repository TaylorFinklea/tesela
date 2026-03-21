import Foundation

// MARK: - WsEvent
// Matches Rust server's #[serde(tag = "event", rename_all = "snake_case")] format:
//   {"event": "note_created", "note": {...}}
//   {"event": "note_updated", "note": {...}}
//   {"event": "note_deleted", "id": "some-id"}

enum WsEvent: Decodable, Sendable {
    case noteCreated(Page)
    case noteUpdated(Page)
    case noteDeleted(String)

    private enum CodingKeys: String, CodingKey {
        case event, note, id
    }

    init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        switch try c.decode(String.self, forKey: .event) {
        case "note_created":
            self = .noteCreated(try c.decode(Page.self, forKey: .note))
        case "note_updated":
            self = .noteUpdated(try c.decode(Page.self, forKey: .note))
        case "note_deleted":
            self = .noteDeleted(try c.decode(String.self, forKey: .id))
        case let unknown:
            throw DecodingError.dataCorrupted(
                .init(codingPath: c.codingPath,
                      debugDescription: "Unknown WsEvent type: \(unknown)")
            )
        }
    }
}
