import Foundation

/// One button that can appear on the keyboard accessory toolbar while
/// a block is being edited. The user picks which items appear, and in
/// what order, from Settings → Keyboard toolbar.
enum KeyboardToolbarItem: String, CaseIterable, Codable, Identifiable, Sendable {
    case hideKeyboard = "hide"
    case slashCommand = "slash"
    case dedent
    case indent
    case cycleStatus = "status"
    case mic
    case deadline
    case tags
    case schedule

    var id: String { rawValue }

    /// Human-readable label used in Settings and as the accessibility
    /// label on the toolbar button.
    var label: String {
        switch self {
        case .hideKeyboard: return "Hide keyboard"
        case .slashCommand: return "Slash command"
        case .dedent:       return "Dedent"
        case .indent:       return "Indent"
        case .cycleStatus:  return "Cycle status"
        case .mic:          return "Voice"
        case .deadline:     return "Deadline"
        case .tags:         return "Tags"
        case .schedule:     return "Schedule"
        }
    }

    /// SF Symbol shown on the toolbar button.
    var systemImage: String {
        switch self {
        case .hideKeyboard: return "keyboard.chevron.compact.down"
        case .slashCommand: return "slash.circle"
        case .dedent:       return "decrease.indent"
        case .indent:       return "increase.indent"
        case .cycleStatus:  return "circle.dotted"
        case .mic:          return "mic"
        case .deadline:     return "exclamationmark.circle"
        case .tags:         return "tag"
        case .schedule:     return "calendar"
        }
    }
}

/// Default keyboard toolbar layout. Matches the originally-shipped
/// ordering before customization landed.
let defaultKeyboardToolbarItems: [KeyboardToolbarItem] =
    [.hideKeyboard, .slashCommand, .dedent, .indent, .cycleStatus, .mic]

/// Encode an item list as the comma-separated raw-value string stored
/// in `@AppStorage("keyboardToolbarItems")`.
func encodeKeyboardToolbarItems(_ items: [KeyboardToolbarItem]) -> String {
    items.map { $0.rawValue }.joined(separator: ",")
}

/// Decode the stored raw-value string back into a typed list. Unknown
/// raw values are dropped (forward-compatibility for newer enum cases
/// when downgrading); duplicates are also de-duped.
func decodeKeyboardToolbarItems(_ raw: String) -> [KeyboardToolbarItem] {
    var seen = Set<KeyboardToolbarItem>()
    var out: [KeyboardToolbarItem] = []
    for piece in raw.split(separator: ",") {
        guard let item = KeyboardToolbarItem(rawValue: String(piece)) else { continue }
        if seen.insert(item).inserted { out.append(item) }
    }
    return out
}

let defaultKeyboardToolbarItemsRaw: String = encodeKeyboardToolbarItems(defaultKeyboardToolbarItems)
