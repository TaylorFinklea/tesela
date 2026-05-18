import SwiftUI

/// The top-level tabs on the iPhone app: Daily · Inbox · Library.
/// Search and capture are NOT tabs — both live in the
/// `.tabViewBottomAccessory` slot of `AppShell` (a Mail-style search
/// field plus an adjacent capture button) and open their respective
/// surfaces on tap. This keeps the tab bar focused on places, not
/// actions.
enum AppTab: Int, CaseIterable, Identifiable, Hashable {
    case daily, inbox, library, search

    var id: Int { rawValue }

    var label: String {
        switch self {
        case .daily:   return "Daily"
        case .inbox:   return "Inbox"
        case .library: return "Library"
        case .search:  return "Search"
        }
    }

    var icon: IconName {
        switch self {
        case .daily:   return .daily
        case .inbox:   return .inbox
        case .library: return .page
        case .search:  return .search
        }
    }
}
