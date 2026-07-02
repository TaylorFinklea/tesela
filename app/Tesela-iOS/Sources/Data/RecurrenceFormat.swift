import Foundation

/// Human-readable rendering of a `recurring::` property value. Delegates to
/// the Rust FFI (`tesela_core::recurrence::format`, exposed as
/// `formatRecurrence`) — the standalone Swift mirror that used to live here
/// was deleted in tesela-pfix.2. Behavior is unchanged: unrecognized input
/// is returned **unchanged** (never crashes).
enum RecurrenceFormat {
    static func human(_ value: String) -> String {
        formatRecurrence(value: value)
    }
}
