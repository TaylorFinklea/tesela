import SwiftUI

/// The top-level tabs on the iPhone app: Daily · Views · Library · Search.
///
/// Search uses iOS 26's `Tab(role: .search)` so the system pins it as a
/// standalone Liquid Glass circle at the trailing edge of the tab bar
/// (Phone/Mail/Photos pattern) — separate from the labeled pill of
/// place-tabs. Capture is still a sheet, triggered from the TopBar.
enum AppTab: Int, CaseIterable, Identifiable, Hashable {
    case daily, agenda, views, library, search

    var id: Int { rawValue }

    /// Labeled tabs — exposed to TabView. Excludes `.search`, which the
    /// system labels and icons automatically through `role: .search`.
    /// Order matches the mental flow: today's journal → planning →
    /// triage backlog → reference.
    static var places: [AppTab] { [.daily, .agenda, .views, .library] }

    var label: String {
        switch self {
        case .daily:   return "Daily"
        case .agenda:  return "Agenda"
        case .views:   return "Views"
        case .library: return "Library"
        case .search:  return "Search"
        }
    }

    var icon: IconName {
        switch self {
        case .daily:   return .daily
        case .agenda:  return .cal
        case .views:   return .inbox
        case .library: return .page
        case .search:  return .search
        }
    }
}
