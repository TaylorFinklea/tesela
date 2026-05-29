import SwiftUI

/// Graphite widget shell — the rail/library widget host. Mirrors the web
/// `.gr-w`: a card (`bg3` fill, `line` border, radius 11, clipped) with a
/// header row (optional leading icon `fgSubtle`; uppercased 11pt-semibold
/// `fgMuted` title with tracking; optional mono badge pill = `bg` fill +
/// `line` border; trailing chevron `fgFaint`) above a body slot.
///
/// This is the host SHELL only — parity ships a fixed widget set;
/// configurability is an iterate-phase concern per the spec.
struct GrWidget<Content: View>: View {
    let title: String
    var icon: String? = nil
    var badge: String? = nil
    @ViewBuilder var content: () -> Content

    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 0) {
            header
            VStack(spacing: 0) {
                content()
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 7)
            .padding(.top, 2)
            .padding(.bottom, 9)
        }
        .background(theme.bg3)
        .overlay(
            RoundedRectangle(cornerRadius: 11)
                .stroke(theme.line, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 11))
    }

    private var header: some View {
        HStack(spacing: 8) {
            if let icon {
                GrIcon(name: icon, size: 14)
                    .foregroundStyle(theme.fgSubtle)
            }
            Text(title.uppercased())
                .font(.system(size: 11, weight: .semibold))
                .tracking(0.4)
                .foregroundStyle(theme.fgMuted)
                .frame(maxWidth: .infinity, alignment: .leading)
            if let badge {
                Text(badge)
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundStyle(theme.fgSubtle)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 1)
                    .background(theme.bg)
                    .overlay(
                        RoundedRectangle(cornerRadius: 5)
                            .stroke(theme.line, lineWidth: 1)
                    )
                    .clipShape(RoundedRectangle(cornerRadius: 5))
            }
            GrIcon(name: "chevron-down", size: 14)
                .foregroundStyle(theme.fgFaint)
        }
        .padding(.horizontal, 11)
        .padding(.top, 9)
        .padding(.bottom, 7)
    }
}
