import SwiftUI

/// Graphite button variants (mirrors the web `GrButton`).
enum GrButtonVariant {
    /// Default: raised fill, hairline border, muted text.
    case ghost
    /// Primary call-to-action: coral fill, near-black text, semibold.
    case cta
}

/// Graphite button primitive. Mirrors the web `.gr-headbtn` spec:
/// ghost = `bg3` fill / `line` border / `fgMuted` text, height 28, radius 8,
/// h-pad 11, 12pt; cta = `accentPrimary` fill / `#10110F` text / semibold.
/// An icon-only button (label-less, `icon` set) renders the `.gr-ic` shape:
/// 30×30, transparent, `fgSubtle` glyph.
///
/// Reads role colors from `@Environment(\.theme)` like the existing
/// `Sources/Components/` views — no hardcoded hex except the CTA's
/// near-black ink, which matches the web token.
struct GrButton: View {
    var variant: GrButtonVariant = .ghost
    var icon: String? = nil
    var label: String? = nil
    var action: () -> Void = {}

    @Environment(\.theme) private var theme

    /// Icon-only when an icon is set but no label text.
    private var iconOnly: Bool { icon != nil && (label?.isEmpty ?? true) }

    var body: some View {
        Button(action: action) {
            if iconOnly {
                GrIcon(name: icon!, size: 17)
            } else {
                HStack(spacing: 6) {
                    if let icon { GrIcon(name: icon, size: 15) }
                    if let label { Text(label) }
                }
            }
        }
        .buttonStyle(GrButtonStyle(variant: variant, iconOnly: iconOnly, theme: theme))
    }
}

private struct GrButtonStyle: ButtonStyle {
    let variant: GrButtonVariant
    let iconOnly: Bool
    let theme: Theme

    private var ctaInk: Color { Color(hex: 0x10110F) }

    func makeBody(configuration: Configuration) -> some View {
        let pressed = configuration.isPressed
        return Group {
            if iconOnly {
                configuration.label
                    .font(.system(size: 17, weight: .regular))
                    .foregroundStyle(pressed ? theme.fgDefault : theme.fgSubtle)
                    .frame(width: 30, height: 30)
                    .background(pressed ? theme.bg3 : .clear)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
            } else {
                configuration.label
                    .font(.system(size: 12, weight: variant == .cta ? .semibold : .regular))
                    .foregroundStyle(foreground)
                    .frame(height: 28)
                    .padding(.horizontal, 11)
                    .background(background(pressed: pressed))
                    .overlay(
                        RoundedRectangle(cornerRadius: 8)
                            .stroke(variant == .cta ? .clear : theme.line, lineWidth: 1)
                    )
                    .clipShape(RoundedRectangle(cornerRadius: 8))
            }
        }
        .opacity(pressed && variant == .cta ? 0.9 : 1)
    }

    private var foreground: Color {
        switch variant {
        case .ghost: return theme.fgMuted
        case .cta:   return ctaInk
        }
    }

    private func background(pressed: Bool) -> Color {
        switch variant {
        case .ghost: return pressed ? theme.bg4 : theme.bg3
        case .cta:   return theme.accentPrimary
        }
    }
}
