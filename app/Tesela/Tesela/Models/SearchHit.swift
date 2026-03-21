import Foundation

struct SearchHit: Identifiable, Codable, Hashable, Sendable {
    let noteId: String
    let title: String
    let snippet: String
    let rank: Double
    let tags: [String]
    let path: String

    var id: String { noteId }

    enum CodingKeys: String, CodingKey {
        case noteId = "note_id"
        case title, snippet, rank, tags, path
    }
}
