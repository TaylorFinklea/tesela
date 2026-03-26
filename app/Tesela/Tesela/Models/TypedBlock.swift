import Foundation

// MARK: - TypedBlock
// A block from the server's /types/:name/blocks endpoint.
// Contains the block's text, tags, and DB-indexed properties.

struct TypedBlock: Identifiable, Codable, Sendable {
    let id: String
    let text: String
    let rawText: String
    let tags: [String]
    let properties: [String: String]
    let indentLevel: Int
    let noteId: String

    enum CodingKeys: String, CodingKey {
        case id, text, tags, properties
        case rawText = "raw_text"
        case indentLevel = "indent_level"
        case noteId = "note_id"
    }
}
