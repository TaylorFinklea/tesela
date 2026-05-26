import SwiftUI

/// The mosaic-identity / reachability indicator that lives in every
/// screen's TopBar. Replaces the old standalone sync dot — the icon
/// itself communicates *which* mosaic, and its color communicates
/// reachability (green / yellow / red).
///
/// Tap shows a small native menu: "Switch mosaic" + "Sync settings".
/// Both used to be separate gestures — Daisy asked for them in one
/// place because they're both about "where am I + how am I talking
/// to it." Reachability anomalies and mosaic identity are the two
/// reasons you'd care about the dot in the first place.
struct MosaicChromeButton: View {
    @ObservedObject var registry: MosaicRegistry
    /// Current sync state — drives the color halo around the icon.
    let syncStatus: TabHeader.SyncDotState
    /// "Switch which mosaic this app is talking to."
    let onTapMosaic: () -> Void
    /// "Show the sync diagnostics + manual controls." Optional so
    /// hosts that don't have a Settings entry point can omit it; the
    /// menu just shows mosaic actions then.
    var onTapSync: (() -> Void)? = nil

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
        Menu {
            Button {
                onTapMosaic()
            } label: {
                Label("Switch mosaic…", systemImage: "square.stack.3d.up")
            }
            if let onTapSync {
                Button {
                    onTapSync()
                } label: {
                    Label("Sync settings…", systemImage: "arrow.triangle.2.circlepath")
                }
            }
        } label: {
            ZStack {
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
        .accessibilityLabel(accessibilityLabel)
    }

    private var accessibilityLabel: String {
        let name = registry.activeProfile?.name ?? "No mosaic"
        return "Mosaic \(name), \(accessibilityState). Tap to switch mosaic or open sync settings."
    }
}
