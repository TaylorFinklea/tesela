import Foundation

/// Where a `CaptureBar` submission lands.
///
/// `.today` prepends to today's daily (matches the existing
/// `mosaic.capture(_:)` behavior). `.inbox` appends to today with a
/// `#inbox` tag so the block surfaces in `InboxView`'s "Tagged #inbox"
/// section (Tesela's inbox is a virtual filter, not a separate page).
/// `.page` appends to the named page via `appendPageBlock`.
enum CaptureTarget: Hashable, Sendable {
    case today
    case inbox
    case page(slug: String, title: String)
    /// Append a new block as a child (one indent level deeper than the
    /// parent block) on `parent`'s page. Page slug is `nil` for today's
    /// daily.
    case childOf(parentId: String, parentPreview: String, pageSlug: String?)

    /// Short label shown on the target chip / in the menu.
    var label: String {
        switch self {
        case .today:                          return "Today"
        case .inbox:                          return "Inbox"
        case .page(_, let title):             return title
        case .childOf(_, let preview, _):     return "Child of " + previewLabel(preview)
        }
    }

    /// SF Symbol shown alongside the label in the chip / menu.
    var systemImage: String {
        switch self {
        case .today:    return "calendar"
        case .inbox:    return "tray"
        case .page:     return "doc.text"
        case .childOf:  return "arrow.turn.down.right"
        }
    }

    /// Trim and truncate a block preview to ~24 chars so it fits in a
    /// menu row.
    private func previewLabel(_ s: String) -> String {
        let trimmed = s.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { return "block" }
        if trimmed.count <= 24 { return "\u{201C}" + trimmed + "\u{201D}" }
        return "\u{201C}" + trimmed.prefix(22) + "\u{2026}\u{201D}"
    }
}

/// User-configurable default for where the capture bar sends submissions
/// when the user hasn't manually picked a target via the chip.
/// Stored in `@AppStorage("captureDefaultTarget")` (raw string).
enum CaptureDefault: String, CaseIterable, Codable, Sendable {
    /// Daily tab → today, Inbox tab → inbox, Library w/ a page open → that page.
    /// Fallback: `.today`.
    case contextAware
    case alwaysToday
    case alwaysInbox

    var label: String {
        switch self {
        case .contextAware: return "Context-aware"
        case .alwaysToday:  return "Always Today"
        case .alwaysInbox:  return "Always Inbox"
        }
    }
}
