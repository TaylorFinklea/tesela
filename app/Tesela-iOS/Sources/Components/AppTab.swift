import SwiftUI

/// The three top-level tabs on the iPhone app: Daily · Library · Search.
/// The capture action is NOT a tab — it lives in the
/// `.tabViewBottomAccessory` slot of `AppShell` (a pill above the tab
/// bar) and opens `CaptureSheet` on tap. This keeps the native
/// `TabView` semantics clean while preserving the canvas's
/// always-visible-capture pattern.
enum AppTab: Int, CaseIterable, Identifiable, Hashable {
    case daily, library, search

    var id: Int { rawValue }

    var label: String {
        switch self {
        case .daily:   return "Daily"
        case .library: return "Library"
        case .search:  return "Search"
        }
    }

    var icon: IconName {
        switch self {
        case .daily:   return .daily
        case .library: return .page
        case .search:  return .search
        }
    }
}
