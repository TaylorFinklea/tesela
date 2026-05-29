import SwiftUI

/// Graphite row primitive. Mirrors the web `.gr-row`: an `HStack` of an
/// optional leading icon (`fgSubtle`), a flexible label (`fgMuted`, 12.5pt,
/// single-line), and an optional trailing mono meta (`fgFaint`, 10.5pt;
/// `accentPrimary` when `urgent`). Padding 6/8, radius 7; an `active`
/// (or pressed) row fills `bg4`.
struct GrRow: View {
    var icon: String? = nil
    let label: String
    var meta: String? = nil
    var urgent: Bool = false
    var active: Bool = false
    var action: () -> Void = {}

    @Environment(\.theme) private var theme

    var body: some View {
        Button(action: action) {
            HStack(spacing: 9) {
                if let icon {
                    GrIcon(name: icon, size: 15)
                        .foregroundStyle(theme.fgSubtle)
                }
                Text(label)
                    .font(.system(size: 12.5))
                    .foregroundStyle(active ? theme.fgDefault : theme.fgMuted)
                    .lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                if let meta {
                    Text(meta)
                        .font(.system(size: 10.5, design: .monospaced))
                        .monospacedDigit()
                        .foregroundStyle(urgent ? theme.accentPrimary : theme.fgFaint)
                        .lineLimit(1)
                }
            }
            .padding(.vertical, 6)
            .padding(.horizontal, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(active ? theme.bg4 : .clear)
            .clipShape(RoundedRectangle(cornerRadius: 7))
            .contentShape(Rectangle())
        }
        .buttonStyle(GrRowButtonStyle())
    }
}

/// Press feedback for `GrRow` — paints the `bg4` highlight while held.
private struct GrRowButtonStyle: ButtonStyle {
    @Environment(\.theme) private var theme

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .background(
                configuration.isPressed
                    ? theme.bg4.clipShape(RoundedRectangle(cornerRadius: 7))
                    : Color.clear.clipShape(RoundedRectangle(cornerRadius: 7))
            )
    }
}
