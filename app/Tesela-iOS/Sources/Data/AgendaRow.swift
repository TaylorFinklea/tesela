import Foundation

/// One row in the `POST /agenda` response — either the canonical anchor
/// of a dated block (`is_anchor: true`) or a forward-projected
/// occurrence of a recurring block. The server emits the anchor with
/// every requested window plus future occurrences inside the window;
/// only the anchor row accepts mutations (mark-done / reschedule /
/// skip) — projected rows are read-only previews.
///
/// Matches the wire shape in `crates/tesela-core/src/query.rs`
/// `AgendaRow`; mirrors the web's TypeScript `$lib/types/AgendaRow.ts`.
struct AgendaRow: Identifiable, Codable, Equatable, Hashable {
    let block_id: String
    let source_note_id: String
    let occurrence_date: String        // YYYY-MM-DD
    let occurrence_time: String?       // HH:MM or nil
    let kind: AgendaRowKind
    let overdue: Bool
    let recurrence: String?            // Raw `recurring::` value, e.g. "every weekday"
    let is_anchor: Bool
    let text: String                   // Already bid-stripped server-side
    let status: String?                // "todo" | "doing" | "done" | nil
    let field: AgendaField             // Which dated property anchored this row

    /// Stable per-row identity for SwiftUI ForEach. A recurring block's
    /// future occurrences share the same `block_id`, so the date has
    /// to be part of the identity or the list collapses.
    var id: String { "\(block_id):\(occurrence_date)" }
}

enum AgendaRowKind: String, Codable, Equatable, Hashable {
    case task
    case event
}

/// Which dated property the row's anchor came from. Mirrors the server
/// `AgendaField` enum; drives the Overdue-bucket split (⚑ deadlines
/// vs 🕒 scheduled) and the per-bucket bulk-reschedule actions.
enum AgendaField: String, Codable, Equatable, Hashable {
    case deadline
    case scheduled
}
