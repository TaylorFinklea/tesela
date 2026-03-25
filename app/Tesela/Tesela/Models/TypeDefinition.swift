import Foundation

// MARK: - TypeDefinition
// Mirrors the Rust TypeDefinition. Loaded from server's GET /types endpoint.

struct TypeDefinition: Codable, Identifiable, Sendable {
    let name: String
    let description: String
    let icon: String
    let color: String
    let properties: [PropertyDef]

    var id: String { name }
}

struct PropertyDef: Codable, Identifiable, Sendable {
    let name: String
    let valueType: String
    let values: [String]?
    let `default`: String?
    let required: Bool

    var id: String { name }

    enum CodingKeys: String, CodingKey {
        case name
        case valueType = "value_type"
        case values
        case `default`
        case required
    }
}
