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
    var onTapSettings: () -> Void = {}
    /// Tap-target for the mosaic chrome button (replaces the old sync
    /// dot). Opens the mosaic switcher sheet.
    var onTapMosaic: () -> Void = {}

    @EnvironmentObject private var mosaicRegistry: MosaicRegistry

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
                IconButton(name: .settings, action: onTapSettings)
                    .accessibilityLabel("Settings")
                MosaicChromeButton(
                    registry: mosaicRegistry,
                    syncStatus: syncStatus,
                    onTap: onTapMosaic
                )
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

    private var syncStatusAccessibilityLabel: String {
        switch syncStatus {
        case .ok:   return "Connected. Tap for backend settings."
        case .warn: return "Connecting. Tap for backend settings."
        case .err:  return "Disconnected. Tap to open backend settings."
        }
    }
}

extension DailyTopBar.SyncDotState {
    /// Derive the sync-dot color from the mosaic's real HTTP connection
    /// state. Every tab's `MosaicChromeButton` routes through this so
    /// the indicator reflects actual server reachability instead of a
    /// per-screen guess (the Inbox/Library dot previously read a debug
    /// `SyncState` toggle, which could disagree with the live backend).
    init(_ connection: MockMosaicService.ConnectionState) {
        switch connection {
        case .ready, .idle:           self = .ok
        case .connecting, .switching: self = .warn
        case .failed:                 self = .err
        }
    }
}

/// A thin status strip shown when the mosaic can't reach its server.
/// Without it, a failed connect leaves an empty screen that looks
/// identical to an empty mosaic — the strip names the reason and gives
/// a one-tap retry. Renders nothing (zero height) unless disconnected.
struct ConnectionBanner: View {
    let connection: MockMosaicService.ConnectionState
    var onRetry: () -> Void = {}

    @Environment(\.theme) private var theme

    var body: some View {
        if case .failed(let message) = connection {
            Button(action: onRetry) {
                HStack(spacing: 8) {
                    Image(systemName: "wifi.exclamationmark")
                        .font(.system(size: 13, weight: .semibold))
                    Text(message)
                        .font(.system(size: 12, weight: .medium))
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Spacer(minLength: 8)
                    Text("Retry")
                        .font(.system(size: 12, weight: .semibold))
                }
                .foregroundStyle(theme.typeTask)
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .frame(maxWidth: .infinity)
                .background(theme.typeTask.opacity(0.12))
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Disconnected: \(message). Tap to retry.")
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
