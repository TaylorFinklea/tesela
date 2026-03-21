import Foundation

// MARK: - Page (mirrors Rust Note)
struct Page: Identifiable, Codable, Hashable, Sendable {
    let id: String
    let title: String
    let content: String   // raw full file content (frontmatter + body)
    let body: String      // body without frontmatter
    let metadata: PageMetadata
    let path: String
    let checksum: String
    let createdAt: Date
    let modifiedAt: Date

    enum CodingKeys: String, CodingKey {
        case id, title, content, body, metadata, path, checksum
        case createdAt = "created_at"
        case modifiedAt = "modified_at"
    }
}

// MARK: - PageMetadata
struct PageMetadata: Codable, Hashable, Sendable {
    let title: String
    let tags: [String]
    let aliases: [String]
    let custom: [String: JSONValue]
    let created: Date?
    let modified: Date?

    enum CodingKeys: String, CodingKey {
        case title, tags, aliases, custom, created, modified
    }

    init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        title = try container.decodeIfPresent(String.self, forKey: .title) ?? ""
        tags = try container.decodeIfPresent([String].self, forKey: .tags) ?? []
        aliases = try container.decodeIfPresent([String].self, forKey: .aliases) ?? []
        custom = try container.decodeIfPresent([String: JSONValue].self, forKey: .custom) ?? [:]
        created = try container.decodeIfPresent(Date.self, forKey: .created)
        modified = try container.decodeIfPresent(Date.self, forKey: .modified)
    }
}

// MARK: - JSONValue (for arbitrary frontmatter fields)
enum JSONValue: Codable, Hashable, Sendable {
    case string(String)
    case number(Double)
    case bool(Bool)
    case array([JSONValue])
    case object([String: JSONValue])
    case null

    init(from decoder: any Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            self = .null
        } else if let bool = try? container.decode(Bool.self) {
            self = .bool(bool)
        } else if let num = try? container.decode(Double.self) {
            self = .number(num)
        } else if let str = try? container.decode(String.self) {
            self = .string(str)
        } else if let arr = try? container.decode([JSONValue].self) {
            self = .array(arr)
        } else if let obj = try? container.decode([String: JSONValue].self) {
            self = .object(obj)
        } else {
            self = .null
        }
    }

    func encode(to encoder: any Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .null: try container.encodeNil()
        case .bool(let v): try container.encode(v)
        case .number(let v): try container.encode(v)
        case .string(let v): try container.encode(v)
        case .array(let v): try container.encode(v)
        case .object(let v): try container.encode(v)
        }
    }
}
