import Foundation

/// Domain models the SwiftUI views render. Same shape as the design
/// canvas's `data.jsx` so the screens look right in mock + the eventual
/// FFI-backed service can fill in the same structs.

/// Block kinds the outliner can render. Matches the web's block.kind
/// frontmatter discriminator.
enum BlockKind: String, Codable, Hashable {
    case note
    case task
    case event
    case project
    case person
    case query
}

struct Block: Identifiable, Equatable, Hashable, Codable {
    let id: String
    var kind: BlockKind
    var text: String
    var done: Bool = false
    var indent: Int = 0
    var tags: [String] = []
}

/// A markdown page. `type` is the frontmatter discriminator (note,
/// daily, query, scratch, project, person, tag, template, …).
struct Page: Identifiable, Equatable, Hashable, Codable {
    let id: String       // slug — `notes/<slug>.md` stem
    var title: String
    var slug: String
    var type: String     // page type from `type:` frontmatter
    var edited: String   // human-readable timestamp
    var blocks: Int      // block count (frontmatter-summarized)
    var refs: Int        // refs-in count
    var hidden: Bool = false
    var query: String? = nil  // for `type: query` pages
    var body: [String] = []   // first few lines of body for preview
}

struct Tag: Identifiable, Equatable, Hashable, Codable {
    let id: String       // slug
    var title: String
    var parent: String?
    var count: Int
    var recent: String
    var slug: String { id }
}

struct RecentEntry: Identifiable, Equatable, Hashable, Codable {
    let id: String       // slug
    var title: String
    var at: String
}

struct PinnedEntry: Identifiable, Equatable, Hashable, Codable {
    let id: String       // slug
    var title: String
}

struct SearchResult: Identifiable, Equatable, Hashable, Codable {
    enum Kind: String, Codable, Hashable { case page, block, tag }
    let id: String
    var kind: Kind
    var title: String
    var snippet: String
}

struct PaletteVerb: Identifiable, Equatable, Hashable, Codable {
    let id: String       // verb name with leading colon (":scratch")
    var name: String { id }
    var hint: String
}

struct Backlink: Identifiable, Equatable, Hashable, Codable {
    let id: UUID
    var from: String     // source page title
    var snippet: String
}

struct OutlineEntry: Identifiable, Equatable, Hashable, Codable {
    let id: UUID
    var depth: Int
    var text: String
}
