import SwiftUI

/// Sync status pill — small monospace badge with a colored dot.
/// Matches `.tx .pill` from the design tokens.
struct Pill: View {
    enum Status {
        case synced(at: String)   // green dot, "synced · 12:14"
        case syncing              // amber dot, "syncing…"
        case offline(since: String) // red dot, "offline · 2h ago"
        case custom(text: String, dotColor: Color?)
    }

    let status: Status

    @Environment(\.theme) private var theme

    private var dotColor: Color {
        switch status {
        case .synced:   return theme.typeQuery
        case .syncing:  return theme.typeNote
        case .offline:  return theme.typeTask
        case .custom(_, let c): return c ?? theme.typeQuery
        }
    }

    private var label: String {
        switch status {
        case .synced(let at):   return "synced · \(at)"
        case .syncing:          return "syncing…"
        case .offline(let s):   return "offline · \(s)"
        case .custom(let t, _): return t
        }
    }

    var body: some View {
        HStack(spacing: 6) {
            Circle()
                .fill(dotColor)
                .frame(width: 6, height: 6)
            Text(label)
                .font(.system(size: 10.5, design: .monospaced))
                .foregroundStyle(theme.fgSubtle)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 3)
        .background(theme.bg2)
        .overlay(
            RoundedRectangle(cornerRadius: 999)
                .stroke(theme.line, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 999))
    }
}
