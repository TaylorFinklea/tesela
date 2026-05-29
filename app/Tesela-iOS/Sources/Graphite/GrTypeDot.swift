import SwiftUI

/// Graphite type dot — a small filled circle in the type-semantic color.
/// Mirrors the web `.gr-dot` (6×6). The color comes from the theme's
/// `typeColor(forKind:)` helper so it tracks the active palette.
struct GrTypeDot: View {
    /// Type/kind label (`task`, `event`, `note`, `project`, `person`,
    /// `query`, …). Case-insensitive; reuses `Theme.typeColor(forKind:)`.
    let kind: String
    var size: CGFloat = 6

    @Environment(\.theme) private var theme

    var body: some View {
        Circle()
            .fill(theme.typeColor(forKind: kind))
            .frame(width: size, height: size)
    }
}
