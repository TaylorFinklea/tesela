import SwiftUI

/// Placeholder body for a Graphite shell tab. Wraps `GrHeader` with the
/// tab's label and a faint "view lands next phase" notice in the canvas.
///
/// The real Daily / Agenda / Inbox / Library views are the next plan —
/// this phase delivers the chrome (tab bar + header + capture sheet)
/// with placeholder content. Presentation only; reads `@Environment(\.theme)`.
struct GrTabPlaceholder: View {
    let tab: AppTab

    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 0) {
            GrHeader(title: tab.label, subtitle: subtitle)
            Spacer(minLength: 0)
            VStack(spacing: 12) {
                GrIcon(name: railIcon, size: 34)
                    .foregroundStyle(theme.fgFaint)
                Text("\(tab.label) view lands next phase")
                    .font(.system(size: 13))
                    .foregroundStyle(theme.fgFaint)
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 16)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .background(theme.bg)
    }

    /// Mono subtitle eyebrow per the mobile header treatment.
    private var subtitle: String {
        switch tab {
        case .daily:   return "JOURNAL"
        case .agenda:  return "PLANNING"
        case .inbox:   return "TRIAGE"
        case .library: return "REFERENCE"
        case .search:  return "FIND"
        }
    }

    /// Kebab Graphite icon name (`GrIcon` map) per tab.
    private var railIcon: String {
        switch tab {
        case .daily:   return "calendar"
        case .agenda:  return "file-text"
        case .inbox:   return "inbox"
        case .library: return "folder"
        case .search:  return "search"
        }
    }
}
