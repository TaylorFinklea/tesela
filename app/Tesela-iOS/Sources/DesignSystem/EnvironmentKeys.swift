import SwiftUI

/// Environment key for the active theme. Views read it via
/// `@Environment(\.theme)` to get role-token colors that automatically
/// repaint when the theme changes.
private struct ThemeKey: EnvironmentKey {
    static let defaultValue: Theme = .prism
}

/// Environment key for the active density tier. Affects every type-scale
/// role; views read it via `@Environment(\.density)`.
private struct DensityKey: EnvironmentKey {
    static let defaultValue: DensityTier = .comfortable
}

extension EnvironmentValues {
    var theme: Theme {
        get { self[ThemeKey.self] }
        set { self[ThemeKey.self] = newValue }
    }

    var density: DensityTier {
        get { self[DensityKey.self] }
        set { self[DensityKey.self] = newValue }
    }
}
