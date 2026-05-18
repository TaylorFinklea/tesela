import SwiftUI

/// Kind badge — small uppercase monospace pill identifying a page's
/// `type:` frontmatter. Matches `.tx .kind` from the design tokens.
/// Type-colored background tint with type-colored foreground.
struct KindBadge: View {
    /// Page kind label (`note`, `tag`, `query`, `task`, `event`, `project`,
    /// `person`, `template`, `daily`, `scratch`, …). Case-insensitive.
    let kind: String

    @Environment(\.theme) private var theme

    private var color: Color {
        theme.typeColor(forKind: kind)
    }

    var body: some View {
        Text(kind.uppercased())
            .font(.system(size: 9.5, weight: .semibold, design: .monospaced))
            .tracking(0.4)
            .foregroundStyle(color)
            .padding(.horizontal, 6)
            .padding(.vertical, 1)
            .background(color.opacity(0.14))
            .overlay(
                RoundedRectangle(cornerRadius: 3)
                    .stroke(color.opacity(0.30), lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 3))
    }
}
