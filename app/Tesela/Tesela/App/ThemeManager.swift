import SwiftUI
import Observation

// MARK: - ThemeManager
// Manages app-wide appearance: color scheme (dark/light/auto) and accent color.

@Observable
@MainActor
final class ThemeManager {
    static let shared = ThemeManager()

    var colorScheme: AppColorScheme {
        didSet { Persistence.saveColorScheme(colorScheme.rawValue) }
    }
    var accentColor: AppAccentColor {
        didSet { Persistence.saveAccentColor(accentColor.rawValue) }
    }

    private init() {
        colorScheme = AppColorScheme(rawValue: Persistence.loadColorScheme()) ?? .auto
        accentColor = AppAccentColor(rawValue: Persistence.loadAccentColor()) ?? .blue
    }

    var preferredColorScheme: ColorScheme? {
        switch colorScheme {
        case .auto: nil
        case .dark: .dark
        case .light: .light
        }
    }

    var tintColor: Color {
        accentColor.color
    }
}

// MARK: - Color Scheme
enum AppColorScheme: String, CaseIterable {
    case auto = "auto"
    case dark = "dark"
    case light = "light"

    var label: String {
        switch self {
        case .auto: "Auto"
        case .dark: "Dark"
        case .light: "Light"
        }
    }

    var icon: String {
        switch self {
        case .auto: "circle.lefthalf.filled"
        case .dark: "moon.fill"
        case .light: "sun.max.fill"
        }
    }
}

// MARK: - Accent Color
enum AppAccentColor: String, CaseIterable {
    case blue = "blue"
    case purple = "purple"
    case indigo = "indigo"
    case pink = "pink"
    case red = "red"
    case orange = "orange"
    case yellow = "yellow"
    case green = "green"
    case mint = "mint"
    case teal = "teal"
    case cyan = "cyan"

    var color: Color {
        switch self {
        case .blue: .blue
        case .purple: .purple
        case .indigo: .indigo
        case .pink: .pink
        case .red: .red
        case .orange: .orange
        case .yellow: .yellow
        case .green: .green
        case .mint: .mint
        case .teal: .teal
        case .cyan: .cyan
        }
    }

    var label: String { rawValue.capitalized }
}
