import SwiftUI

/// Top bar used at the head of the Daily tab — big "Today" label, dated
/// subtitle in mono, calendar button on the right, plus a sync dot.
/// Mirrors the canvas's `P2_TopBar` chrome.
struct DailyTopBar: View {
    /// Sync indicator state. `ok` = connected to backend; `warn` =
    /// connecting / mid-refresh; `err` = backend unreachable.
    enum SyncDotState { case ok, warn, err }

    let title: String
    let dateLabel: String
    var syncStatus: SyncDotState = .ok
    var onTapCalendar: () -> Void = {}

    @Environment(\.theme) private var theme

    private var dotColor: Color {
        switch syncStatus {
        case .ok:   return theme.typeQuery
        case .warn: return theme.typeNote
        case .err:  return theme.typeTask
        }
    }

    var body: some View {
        HStack(alignment: .center) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.system(size: 22, weight: .semibold))
                    .tracking(-0.2)
                    .foregroundStyle(theme.fgDefault)
                Text(dateLabel)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgSubtle)
            }
            Spacer()
            HStack(spacing: 4) {
                IconButton(name: .cal, action: onTapCalendar)
                    .accessibilityLabel("Calendar")
                ZStack {
                    Circle()
                        .fill(dotColor.opacity(0.24))
                        .frame(width: 14, height: 14)
                    Circle()
                        .fill(dotColor)
                        .frame(width: 8, height: 8)
                }
                .frame(width: 36, height: 36)
            }
        }
        .padding(.horizontal, 18)
        .padding(.top, 8)
        .padding(.bottom, 12)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(theme.lineSoft)
                .frame(height: 1)
        }
    }
}

/// Generic page top bar — back chevron, optional pin / more trailing
/// buttons. Used for non-Daily screens.
struct PageTopBar: View {
    let backLabel: String
    var onBack: () -> Void = {}
    var pinAction: (() -> Void)? = nil
    var moreAction: (() -> Void)? = nil

    @Environment(\.theme) private var theme

    var body: some View {
        HStack {
            Button(action: onBack) {
                HStack(spacing: -2) {
                    Icon(name: .chevLeft, size: 20)
                    Text(backLabel)
                        .font(.system(size: 15))
                }
                .foregroundStyle(theme.accentPrimary)
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            Spacer()
            HStack(spacing: 4) {
                if let pinAction {
                    IconButton(name: .pin, action: pinAction)
                }
                if let moreAction {
                    IconButton(name: .more, action: moreAction)
                }
            }
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 6)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(theme.lineSoft)
                .frame(height: 1)
        }
    }
}
