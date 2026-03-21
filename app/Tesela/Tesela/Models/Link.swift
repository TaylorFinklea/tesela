import Foundation

struct Link: Identifiable, Codable, Hashable, Sendable {
    let linkType: String
    let target: String
    let text: String?
    let position: Int?

    var id: String { "\(linkType):\(target):\(position ?? 0)" }

    enum CodingKeys: String, CodingKey {
        case linkType = "link_type"
        case target, text, position
    }
}
