import SwiftUI

/// The three top-level tabs on the iPhone app: Daily · Library · Search.
/// Matches the canvas's `P2_BottomTabs` chrome.
enum AppTab: Int, CaseIterable, Identifiable {
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

struct BottomTabBar: View {
    @Binding var active: AppTab
    var onSelect: (AppTab) -> Void = { _ in }

    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 0) {
            ForEach(AppTab.allCases) { tab in
                Button {
                    active = tab
                    onSelect(tab)
                } label: {
                    VStack(spacing: 1) {
                        Icon(name: tab.icon, size: 20)
                        Text(tab.label.uppercased())
                            .font(.system(size: 9, design: .monospaced))
                            .tracking(0.4)
                    }
                    .foregroundStyle(active == tab ? theme.accentPrimary : theme.fgSubtle)
                    .frame(maxWidth: .infinity)
                    .padding(.top, 6)
                    .padding(.bottom, 4)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.bottom, 22) // home-indicator inset
        .background(theme.bg2)
        .overlay(alignment: .top) {
            Rectangle()
                .fill(theme.line)
                .frame(height: 1)
        }
    }
}
