import SwiftUI

/// The mosaic-identity / reachability indicator that lives in every
/// screen's TopBar. Replaces the old standalone sync dot — the icon
/// itself communicates *which* mosaic, and its color communicates
/// reachability (green / yellow / red).
///
/// Tap → opens the mosaic switcher sheet.
struct MosaicChromeButton: View {
    @ObservedObject var registry: MosaicRegistry
    /// Current sync state — drives the color halo around the icon.
    let syncStatus: TabHeader.SyncDotState
    let onTap: () -> Void

    @Environment(\.theme) private var theme

    private var icon: String {
        registry.activeProfile?.iconSymbol ?? "questionmark.circle"
    }

    private var color: Color {
        switch syncStatus {
        case .ok:   return theme.typeQuery
        case .warn: return theme.typeNote
        case .err:  return theme.typeTask
        }
    }

    private var accessibilityState: String {
        switch syncStatus {
        case .ok:   return "connected"
        case .warn: return "connecting"
        case .err:  return "disconnected"
        }
    }

    var body: some View {
        Button(action: onTap) {
            ZStack {
                // Subtle ring in the reachability color so the chip
                // reads as "status + identity" at a glance.
                Circle()
                    .fill(color.opacity(0.22))
                    .frame(width: 28, height: 28)
                Image(systemName: icon)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(color)
            }
            .frame(width: 36, height: 36)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(accessibilityLabel)
    }

    private var accessibilityLabel: String {
        let name = registry.activeProfile?.name ?? "No mosaic"
        return "Mosaic \(name), \(accessibilityState). Tap to switch."
    }
}
