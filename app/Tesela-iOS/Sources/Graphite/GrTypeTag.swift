import SwiftUI

/// Graphite type tag. Mirrors the web `.gr-typetag`: rounded rect
/// (radius 6, height 21, h-pad 9) with a leading 6×6 rounded swatch and a
/// mono 10.5pt label, both in the type-semantic color. `bg3` fill, `line`
/// border. The color comes from `Theme.typeColor(forKind:)`.
struct GrTypeTag: View {
    /// Type/kind label rendered as the tag text and swatch color.
    let kind: String

    @Environment(\.theme) private var theme

    private var color: Color { theme.typeColor(forKind: kind) }

    var body: some View {
        HStack(spacing: 6) {
            RoundedRectangle(cornerRadius: 2)
                .fill(color)
                .frame(width: 6, height: 6)
            Text(kind)
                .font(.system(size: 10.5, design: .monospaced))
                .tracking(0.2)
                .foregroundStyle(color)
        }
        .frame(height: 21)
        .padding(.horizontal, 9)
        .background(theme.bg3)
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(theme.line, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 6))
    }
}
