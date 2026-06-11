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
    /// First line of the block's body (trailing tags stripped). Used as
    /// a compact label in previews, recents, and search snippets.
    var text: String
    /// Full multi-line body as written under the bullet — first line +
    /// every non-property continuation line, joined with `\n`. This is
    /// what the outliner renders so blocks authored across several
    /// lines display every line, matching the web client's `raw_text`.
    /// Defaults to empty so legacy call sites that only set `text`
    /// keep compiling; render sites should prefer `displayText` which
    /// falls back to `text` when `rawText` is empty.
    var rawText: String = ""
    var done: Bool = false
    var indent: Int = 0
    var tags: [String] = []
    /// Block-level properties parsed from indented `key:: value`
    /// sub-lines. Matches the web's Logseq-shaped block convention.
    /// Always kept in render order so we round-trip non-task keys
    /// (e.g. `priority::`, `due::`) on save.
    var properties: [BlockProperty] = []
    /// 0-based line index where this block's bullet appears in the
    /// parent note's body. Used to build the `<noteId>:<line>` format
    /// required by the server's `POST /blocks/recur-bump` route.
    /// Defaults to 0 for mock blocks and backwards-compat.
    var lineNumber: Int = 0
    /// The owning note's id. Populated by `parseBlocks(from:noteId:)` so
    /// `recurBump` can construct the server-expected `<noteId>:<line>`
    /// composite block id without a separate lookup.
    var noteId: String = ""
}

extension Block {
    /// Body to render in the outliner. Prefers the parser-populated
    /// `rawText` (which preserves continuation lines) and falls back to
    /// `text` for blocks constructed without a parse pass (new blocks
    /// from `appendTodayBlock`, capture, etc., which never have
    /// continuation lines anyway).
    var displayText: String {
        rawText.isEmpty ? text : rawText
    }
}

enum BlockFold {
    static func visibleBlocks(in blocks: [Block], collapsed: Set<String>) -> [Block] {
        guard !collapsed.isEmpty else { return blocks }

        var visible: [Block] = []
        var hiddenUntilIndentLte: Int?

        for block in blocks {
            if let foldedIndent = hiddenUntilIndentLte {
                if block.indent > foldedIndent {
                    continue
                }
                hiddenUntilIndentLte = nil
            }

            visible.append(block)

            if collapsed.contains(block.id) {
                hiddenUntilIndentLte = block.indent
            }
        }

        return visible
    }

    static func hasChildren(block: Block, in blocks: [Block]) -> Bool {
        guard let index = blocks.firstIndex(where: { $0.id == block.id }),
              index < blocks.index(before: blocks.endIndex)
        else { return false }
        return blocks[blocks.index(after: index)].indent > block.indent
    }
}

/// One `key:: value` property attached to a block. The web client
/// renders these as block-properties under the parent bullet.
struct BlockProperty: Equatable, Hashable, Codable {
    var key: String
    var value: String
}

/// One past daily note shown below today in the Daily feed: its date
/// id (`YYYY-MM-DD`) and parsed blocks. Display-only — tapping its
/// header opens the full editable daily page.
struct DailyEntry: Identifiable, Equatable, Hashable {
    let id: String
    let blocks: [Block]
}

/// A markdown page. `type` is the frontmatter discriminator (note,
/// daily, query, scratch, project, person, tag, template, …).
struct Page: Identifiable, Equatable, Hashable, Codable {
    let id: String       // slug — `notes/<slug>.md` stem
    var title: String
    var slug: String
    var type: String     // page type from `type:` frontmatter
    var edited: String   // human-readable "modified" timestamp
    var created: String = ""  // `created` frontmatter date (YYYY-MM-DD), "" if unknown
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

/// A link row in the Peek surface. Used for both the backlinks lens
/// (`from` = the page linking IN) and the graph lens (`from` = the page
/// linked TO). `pageId` is the related page's id, for tap-to-navigate.
struct Backlink: Identifiable, Equatable, Hashable, Codable {
    let id: UUID
    var from: String     // related page's display title
    var snippet: String
    var pageId: String = ""  // related page id — empty when unresolved
}

struct OutlineEntry: Identifiable, Equatable, Hashable, Codable {
    let id: UUID
    var depth: Int
    var text: String
}

extension OutlineEntry {
    /// Derive a page outline from its block list: one entry per
    /// non-empty block, nesting depth taken from the block's indent.
    /// The outline is a pure function of the blocks, so it is computed
    /// on demand rather than stored.
    static func derive(from blocks: [Block]) -> [OutlineEntry] {
        blocks
            .filter { !$0.text.trimmingCharacters(in: .whitespaces).isEmpty }
            .map { OutlineEntry(id: UUID(), depth: $0.indent, text: $0.text) }
    }
}
