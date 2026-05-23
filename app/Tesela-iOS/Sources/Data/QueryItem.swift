import Foundation

/// Result of `POST /search/query`. Mirrors the web's TypeScript
/// `$lib/types/QueryResult.ts` / `QueryGroup.ts` / `QueryItem.ts` and
/// the Rust `crates/tesela-core/src/query.rs` types.
struct QueryResult: Codable, Equatable {
    let groups: [QueryGroup]
}

struct QueryGroup: Codable, Equatable {
    let key: String
    let items: [QueryItem]
}

/// One row of a query result — a whole page when `kind == .page`, or a
/// single block when `kind == .block` (in which case `block_id` is set).
struct QueryItem: Codable, Equatable, Hashable, Identifiable {
    let block_id: String?
    let page_id: String
    let title: String
    let text: String
    let parent_breadcrumb: [String]
    let kind: QueryItemKind
    let primary_tag: String?
    let properties: [String: String]
    /// `note_type` of the containing page — used to drop blocks from
    /// system pages (Tag / Property / Query / Template) when post-
    /// filtering for the Inbox surface.
    let page_note_type: String?

    /// Identity for ForEach — block_id when present, page_id otherwise.
    var id: String { block_id ?? page_id }
}

enum QueryItemKind: String, Codable, Equatable, Hashable {
    case page
    case block
    case tag
}
