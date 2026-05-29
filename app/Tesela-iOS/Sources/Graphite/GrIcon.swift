import SwiftUI

/// Graphite icon layer. Takes the same kebab-case string names as the web
/// `GrIcon` (so call sites stay cross-platform aligned) and maps each to
/// the closest SF Symbol, rendered via `Image(systemName:)`.
///
/// SF Symbols are the parity baseline; bundling the exact Tabler SVGs for
/// pixel-parity is an iterate-phase option. The glyph is tinted by the
/// caller's `foregroundStyle`, like every other Graphite primitive.
struct GrIcon: View {
    /// Kebab name matching the web `GrIcon` map (e.g. `"settings"`,
    /// `"square-check"`, `"circle-dot"`).
    let name: String
    var size: CGFloat = 16
    var weight: Font.Weight = .regular

    /// Kebab name → SF Symbol. Unknown names fall back to `questionmark`.
    private static let symbols: [String: String] = [
        "settings": "gearshape",
        "sun": "sun.max",
        "square-check": "checkmark.square",
        "microphone": "mic",
        "pin": "pin",
        "bolt": "bolt",
        "graph": "point.3.connected.trianglepath.dotted",
        "inbox": "tray",
        "calendar": "calendar",
        "search": "magnifyingglass",
        "plus": "plus",
        "chevron-down": "chevron.down",
        "chevron-right": "chevron.right",
        "flame": "flame",
        "circle-dot": "smallcircle.filled.circle",
        "folder": "folder",
        "hash": "number",
        "clock": "clock",
        "link": "link",
        "file-text": "doc.text",
        "user": "person",
        "dots-vertical": "ellipsis",
        "arrow-left": "chevron.left",
        "corner-down-right": "arrow.turn.down.right",
        "layout-sidebar": "sidebar.left",
        "adjustments": "slider.horizontal.3",
    ]

    private var symbol: String {
        GrIcon.symbols[name] ?? "questionmark"
    }

    var body: some View {
        Image(systemName: symbol)
            .font(.system(size: size, weight: weight))
            .accessibilityHidden(true)
    }
}
