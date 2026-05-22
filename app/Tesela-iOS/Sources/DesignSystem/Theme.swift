import SwiftUI

/// Identifiers for every theme the app ships. The raw value matches the
/// `data-theme="…"` attribute used by `web/src/themes.css` so iOS and web
/// stay one-to-one mappable. `prism` is the cross-platform default
/// (matches `web/src/app.css` root variables) per the iOS Tile decisions
/// (`.docs/designs/2026-05-18-ios-design-followup.md` #1); `prismLight`
/// is its light-mode variant.
enum ThemeID: String, CaseIterable, Identifiable, Codable {
    case prism             = "prism"
    case prismSpark        = "prism-spark"
    case prismLight        = "prism-light"
    case tokyoNight        = "tokyo-night"
    case tokyoNightStorm   = "tokyo-night-storm"
    case catppuccinMocha   = "catppuccin-mocha"
    case catppuccinMacchiato = "catppuccin-macchiato"
    case rosePine          = "rose-pine"
    case rosePineMoon      = "rose-pine-moon"
    case kanagawaWave      = "kanagawa-wave"
    case kanagawaDragon    = "kanagawa-dragon"
    case everforestDark    = "everforest-dark"
    case gruvboxMaterial   = "gruvbox-material"
    case nord              = "nord"
    case dracula           = "dracula"
    case carbonfox         = "carbonfox"
    case ayuDark           = "ayu-dark"
    case monokaiPro        = "monokai-pro"
    case palenight         = "palenight"

    var id: String { rawValue }

    /// Human-readable name shown in the theme picker.
    var displayName: String {
        switch self {
        case .prism:              return "Prism"
        case .prismSpark:         return "Prism Spark"
        case .prismLight:         return "Prism Light"
        case .tokyoNight:         return "Tokyo Night"
        case .tokyoNightStorm:    return "Tokyo Night · Storm"
        case .catppuccinMocha:    return "Catppuccin · Mocha"
        case .catppuccinMacchiato: return "Catppuccin · Macchiato"
        case .rosePine:           return "Rosé Pine"
        case .rosePineMoon:       return "Rosé Pine · Moon"
        case .kanagawaWave:       return "Kanagawa · Wave"
        case .kanagawaDragon:     return "Kanagawa · Dragon"
        case .everforestDark:     return "Everforest"
        case .gruvboxMaterial:    return "Gruvbox · Material"
        case .nord:               return "Nord"
        case .dracula:            return "Dracula"
        case .carbonfox:          return "Carbonfox"
        case .ayuDark:            return "Ayu · Dark"
        case .monokaiPro:         return "Monokai Pro"
        case .palenight:          return "Material · Palenight"
        }
    }
}

/// A snapshot of every color role the app needs. Mirrors the role tokens
/// defined in `web/src/app.css` (and overridden per theme in
/// `web/src/themes.css`).
///
/// Components read these tokens via `@Environment(\.theme)` rather than
/// hard-coding colors, so swapping the active theme repaints the whole
/// app.
struct Theme: Equatable, Identifiable {
    let id: ThemeID

    // Backgrounds, canvas → chrome.
    let bg: Color
    let bg2: Color
    let bg3: Color
    let bg4: Color

    // Lines / hairlines.
    let line: Color
    let lineSoft: Color

    // Foreground scale.
    let fgDefault: Color
    let fgMuted: Color
    let fgSubtle: Color
    let fgFaint: Color

    // Accents — primary is the brand color (today indicator, primary
    // verbs, active selection); secondary is reserved for navigational
    // moments that shouldn't pull from primary.
    let accentPrimary: Color
    let accentSecondary: Color

    // Semantic type-* colors (block bullets, kind badges, tag chips).
    let typeTask: Color
    let typeEvent: Color
    let typeNote: Color
    let typeProject: Color
    let typePerson: Color
    let typeQuery: Color
    let typeTemplate: Color

    /// Convenience: tints `color` with `pct%` opacity over transparent.
    /// Matches `color-mix(in srgb, <color> <pct>%, transparent)` in CSS.
    func tint(_ color: Color, _ pct: Double) -> Color {
        color.opacity(pct / 100.0)
    }

    /// Returns the type-* color for a kind label. Falls back to fgMuted
    /// for unknown kinds.
    func typeColor(forKind kind: String) -> Color {
        switch kind.lowercased() {
        case "task":     return typeTask
        case "event":    return typeEvent
        case "note":     return typeNote
        case "project":  return typeProject
        case "person":   return typePerson
        case "query":    return typeQuery
        case "template": return typeTemplate
        case "tag":      return accentSecondary
        case "daily":    return accentPrimary
        default:         return fgMuted
        }
    }

    /// Light-mode themes paint on a pale surface. Drives the app's
    /// `preferredColorScheme` so system chrome (status bar, sheets)
    /// matches; the theme's own role-tokens always paint the UI itself.
    var isLight: Bool { id == .prismLight }

    /// The brand "spark" — a hotter coral than `accentPrimary`, for the
    /// few focus spots (today marker, active tab). Only the opt-in
    /// "Prism Spark" theme lights it up; every other theme reuses its
    /// primary accent, so the spark stays invisible.
    var accentSpark: Color {
        id == .prismSpark ? Color(hex: 0xFB5950) : accentPrimary
    }
}

// MARK: - Hex-tuple constructor

/// Compact builder so the per-theme literals below stay short enough for
/// the Swift type-checker. Long inline `Color(hex: 0x…)` chains in a
/// `Theme(...)` init blow past the type-checker's complexity budget.
private struct ThemeHexes {
    let bg, bg2, bg3, bg4: UInt32
    let line, lineSoft: UInt32
    let fgDefault, fgMuted, fgSubtle, fgFaint: UInt32
    let accentPrimary, accentSecondary: UInt32
    let typeTask, typeEvent, typeNote: UInt32
    let typeProject, typePerson, typeQuery, typeTemplate: UInt32
}

private extension Theme {
    init(id: ThemeID, hex: ThemeHexes) {
        // Pre-build every Color as a local so the call to the memberwise
        // init stays small enough for the Swift type-checker. Without this
        // split, the type-checker complexity budget blows up.
        let cBg        = Color(hex: hex.bg)
        let cBg2       = Color(hex: hex.bg2)
        let cBg3       = Color(hex: hex.bg3)
        let cBg4       = Color(hex: hex.bg4)
        let cLine      = Color(hex: hex.line)
        let cLineSoft  = Color(hex: hex.lineSoft)
        let cFgDefault = Color(hex: hex.fgDefault)
        let cFgMuted   = Color(hex: hex.fgMuted)
        let cFgSubtle  = Color(hex: hex.fgSubtle)
        let cFgFaint   = Color(hex: hex.fgFaint)
        let cAccentP   = Color(hex: hex.accentPrimary)
        let cAccentS   = Color(hex: hex.accentSecondary)
        let cTask      = Color(hex: hex.typeTask)
        let cEvent     = Color(hex: hex.typeEvent)
        let cNote      = Color(hex: hex.typeNote)
        let cProject   = Color(hex: hex.typeProject)
        let cPerson    = Color(hex: hex.typePerson)
        let cQuery     = Color(hex: hex.typeQuery)
        let cTemplate  = Color(hex: hex.typeTemplate)

        self.init(
            id: id,
            bg: cBg, bg2: cBg2, bg3: cBg3, bg4: cBg4,
            line: cLine, lineSoft: cLineSoft,
            fgDefault: cFgDefault, fgMuted: cFgMuted,
            fgSubtle: cFgSubtle, fgFaint: cFgFaint,
            accentPrimary: cAccentP, accentSecondary: cAccentS,
            typeTask: cTask, typeEvent: cEvent, typeNote: cNote,
            typeProject: cProject, typePerson: cPerson,
            typeQuery: cQuery, typeTemplate: cTemplate
        )
    }
}

// MARK: - Theme palettes

extension Theme {
    /// **Default theme.** The Prism brand theme — a warm-dark palette
    /// derived from the app logo (slate #3D405B, coral #FB5950, cream
    /// #F4F1DE). Mirrors the web client's `:root` defaults in
    /// `web/src/app.css`. First-launch iPhone looks like first-launch
    /// web. Per decision #1.
    static let prism = Theme(id: .prism, hex: ThemeHexes(
        bg: 0x23252F, bg2: 0x2C2E3E, bg3: 0x34374C, bg4: 0x3D405B,
        line: 0x454963, lineSoft: 0x383B52,
        fgDefault: 0xF4F1DE, fgMuted: 0xC9C6B4,
        fgSubtle: 0x928F7E, fgFaint: 0x6E6B60,
        accentPrimary: 0xE07A5F, accentSecondary: 0x81B29A,
        typeTask: 0xDB6C83, typeEvent: 0x6DBACC, typeNote: 0xE8B86B,
        typeProject: 0x6A8FDC, typePerson: 0xA98BE0,
        typeQuery: 0x88B85E, typeTemplate: 0xC79B58))

    /// **Prism Spark.** Identical to Prism, but `accentSpark` lights up
    /// to the hot logo coral — an opt-in theme for users who want the
    /// neon focus accent (active tab, today marker).
    static let prismSpark = Theme(id: .prismSpark, hex: ThemeHexes(
        bg: 0x23252F, bg2: 0x2C2E3E, bg3: 0x34374C, bg4: 0x3D405B,
        line: 0x454963, lineSoft: 0x383B52,
        fgDefault: 0xF4F1DE, fgMuted: 0xC9C6B4,
        fgSubtle: 0x928F7E, fgFaint: 0x6E6B60,
        accentPrimary: 0xE07A5F, accentSecondary: 0x81B29A,
        typeTask: 0xDB6C83, typeEvent: 0x6DBACC, typeNote: 0xE8B86B,
        typeProject: 0x6A8FDC, typePerson: 0xA98BE0,
        typeQuery: 0x88B85E, typeTemplate: 0xC79B58))

    /// **Prism Light.** The light variant of the brand theme — cream
    /// surface, slate ink, coral accent. Mirrors
    /// `[data-theme="prism-light"]` in `web/src/themes.css`. The coral is
    /// deepened from the logo's #FB5950 so it stays legible as a text /
    /// selection color on the cream surface.
    static let prismLight = Theme(id: .prismLight, hex: ThemeHexes(
        bg: 0xF4F1DE, bg2: 0xECE8D0, bg3: 0xE3DEC2, bg4: 0xD6D0B4,
        line: 0xD9D3B7, lineSoft: 0xE6E1C8,
        fgDefault: 0x3D405B, fgMuted: 0x5C5E76,
        fgSubtle: 0x8A8B86, fgFaint: 0xB0AD9A,
        accentPrimary: 0xBD5E40, accentSecondary: 0x5C9078,
        typeTask: 0xC2403F, typeEvent: 0x3C7E91, typeNote: 0x9A7430,
        typeProject: 0x3D6FC0, typePerson: 0x7E5BC0,
        typeQuery: 0x5E8438, typeTemplate: 0x8C6B36))

    static let tokyoNight = Theme(id: .tokyoNight, hex: ThemeHexes(
        bg: 0x1A1B26, bg2: 0x1F2335, bg3: 0x24283B, bg4: 0x2A2E42,
        line: 0x2F334D, lineSoft: 0x292E42,
        fgDefault: 0xC0CAF5, fgMuted: 0xA9B1D6,
        fgSubtle: 0x737AA2, fgFaint: 0x545C7E,
        accentPrimary: 0xFF9E64, accentSecondary: 0xBB9AF7,
        typeTask: 0xDB6C83, typeEvent: 0x6DBACC, typeNote: 0xE8B86B,
        typeProject: 0x6A8FDC, typePerson: 0xA98BE0,
        typeQuery: 0x88B85E, typeTemplate: 0xC79B58))

    static let tokyoNightStorm = Theme(id: .tokyoNightStorm, hex: ThemeHexes(
        bg: 0x24283B, bg2: 0x1F2335, bg3: 0x292E42, bg4: 0x313650,
        line: 0x3B4261, lineSoft: 0x2F334D,
        fgDefault: 0xC0CAF5, fgMuted: 0xA9B1D6,
        fgSubtle: 0x7982A9, fgFaint: 0x565F89,
        accentPrimary: 0xFF9E64, accentSecondary: 0xBB9AF7,
        typeTask: 0xDB6C83, typeEvent: 0x6DBACC, typeNote: 0xE8B86B,
        typeProject: 0x6A8FDC, typePerson: 0xA98BE0,
        typeQuery: 0x88B85E, typeTemplate: 0xC79B58))

    static let catppuccinMocha = Theme(id: .catppuccinMocha, hex: ThemeHexes(
        bg: 0x1E1E2E, bg2: 0x181825, bg3: 0x313244, bg4: 0x45475A,
        line: 0x313244, lineSoft: 0x292C3C,
        fgDefault: 0xCDD6F4, fgMuted: 0xBAC2DE,
        fgSubtle: 0x7F849C, fgFaint: 0x585B70,
        accentPrimary: 0xFAB387, accentSecondary: 0xCBA6F7,
        typeTask: 0xE08097, typeEvent: 0x74C7EC, typeNote: 0xE5B572,
        typeProject: 0x89B4FA, typePerson: 0xB4A5E6,
        typeQuery: 0x97C97A, typeTemplate: 0xC5A373))

    static let catppuccinMacchiato = Theme(id: .catppuccinMacchiato, hex: ThemeHexes(
        bg: 0x24273A, bg2: 0x1E2030, bg3: 0x363A4F, bg4: 0x494D64,
        line: 0x363A4F, lineSoft: 0x2A2D3F,
        fgDefault: 0xCAD3F5, fgMuted: 0xB8C0E0,
        fgSubtle: 0x8087A2, fgFaint: 0x5B6078,
        accentPrimary: 0xF5A97F, accentSecondary: 0xC6A0F6,
        typeTask: 0xE09199, typeEvent: 0x7DC4E4, typeNote: 0xE3B572,
        typeProject: 0x8AADF4, typePerson: 0xB3A4E8,
        typeQuery: 0x98C67C, typeTemplate: 0xC4A373))

    static let rosePine = Theme(id: .rosePine, hex: ThemeHexes(
        bg: 0x191724, bg2: 0x1F1D2E, bg3: 0x26233A, bg4: 0x2A283E,
        line: 0x26233A, lineSoft: 0x21202E,
        fgDefault: 0xE0DEF4, fgMuted: 0xC8C2EB,
        fgSubtle: 0x908CAA, fgFaint: 0x6E6A86,
        accentPrimary: 0xEBBCBA, accentSecondary: 0xC4A7E7,
        typeTask: 0xEB6F92, typeEvent: 0x9CCFD8, typeNote: 0xF6C177,
        typeProject: 0x31748F, typePerson: 0xC4A7E7,
        typeQuery: 0x80A37A, typeTemplate: 0xC08A5A))

    static let rosePineMoon = Theme(id: .rosePineMoon, hex: ThemeHexes(
        bg: 0x232136, bg2: 0x2A273F, bg3: 0x393552, bg4: 0x44415A,
        line: 0x393552, lineSoft: 0x2F2B43,
        fgDefault: 0xE0DEF4, fgMuted: 0xC8C2EB,
        fgSubtle: 0x908CAA, fgFaint: 0x6E6A86,
        accentPrimary: 0xEA9A97, accentSecondary: 0xC4A7E7,
        typeTask: 0xEB6F92, typeEvent: 0x9CCFD8, typeNote: 0xF6C177,
        typeProject: 0x3E8FB0, typePerson: 0xC4A7E7,
        typeQuery: 0x80A37A, typeTemplate: 0xC08A5A))

    static let kanagawaWave = Theme(id: .kanagawaWave, hex: ThemeHexes(
        bg: 0x1F1F28, bg2: 0x16161D, bg3: 0x2A2A37, bg4: 0x363646,
        line: 0x2A2A37, lineSoft: 0x232330,
        fgDefault: 0xDCD7BA, fgMuted: 0xC8C093,
        fgSubtle: 0x727169, fgFaint: 0x54546D,
        accentPrimary: 0xFFA066, accentSecondary: 0x957FB8,
        typeTask: 0xC34043, typeEvent: 0x7E9CD8, typeNote: 0xDCA561,
        typeProject: 0x7E9CD8, typePerson: 0x957FB8,
        typeQuery: 0x76946A, typeTemplate: 0xC0A36E))

    static let kanagawaDragon = Theme(id: .kanagawaDragon, hex: ThemeHexes(
        bg: 0x181616, bg2: 0x0D0C0C, bg3: 0x282727, bg4: 0x393836,
        line: 0x282727, lineSoft: 0x1F1E1E,
        fgDefault: 0xC5C9C5, fgMuted: 0xA6A69C,
        fgSubtle: 0x737070, fgFaint: 0x4D4D4D,
        accentPrimary: 0xB6927B, accentSecondary: 0xA292A3,
        typeTask: 0xC4746E, typeEvent: 0x8BA4B0, typeNote: 0xC4B28A,
        typeProject: 0x8BA4B0, typePerson: 0xA292A3,
        typeQuery: 0x87A987, typeTemplate: 0xB28A66))

    static let everforestDark = Theme(id: .everforestDark, hex: ThemeHexes(
        bg: 0x2D353B, bg2: 0x232A2E, bg3: 0x343F44, bg4: 0x3D484D,
        line: 0x3D484D, lineSoft: 0x323D42,
        fgDefault: 0xD3C6AA, fgMuted: 0xC8B98E,
        fgSubtle: 0x859289, fgFaint: 0x55676C,
        accentPrimary: 0xE69875, accentSecondary: 0xD699B6,
        typeTask: 0xE67E80, typeEvent: 0x7FBBB3, typeNote: 0xDBBC7F,
        typeProject: 0x7FBBB3, typePerson: 0xD699B6,
        typeQuery: 0xA7C080, typeTemplate: 0xC79B58))

    static let gruvboxMaterial = Theme(id: .gruvboxMaterial, hex: ThemeHexes(
        bg: 0x1D2021, bg2: 0x282828, bg3: 0x32302F, bg4: 0x3C3836,
        line: 0x45403D, lineSoft: 0x3C3836,
        fgDefault: 0xD4BE98, fgMuted: 0xDDC7A1,
        fgSubtle: 0x928374, fgFaint: 0x5A524C,
        accentPrimary: 0xE78A4E, accentSecondary: 0xD3869B,
        typeTask: 0xEA6962, typeEvent: 0x89B482, typeNote: 0xD8A657,
        typeProject: 0x7DAEA3, typePerson: 0xC084C2,
        typeQuery: 0xA9B665, typeTemplate: 0xB48A4A))

    static let nord = Theme(id: .nord, hex: ThemeHexes(
        bg: 0x2E3440, bg2: 0x3B4252, bg3: 0x434C5E, bg4: 0x4C566A,
        line: 0x434C5E, lineSoft: 0x3B4252,
        fgDefault: 0xECEFF4, fgMuted: 0xD8DEE9,
        fgSubtle: 0x88909D, fgFaint: 0x5E6779,
        accentPrimary: 0x88C0D0, accentSecondary: 0xB48EAD,
        typeTask: 0xBF616A, typeEvent: 0x81A1C1, typeNote: 0xEBCB8B,
        typeProject: 0x5E81AC, typePerson: 0xB48EAD,
        typeQuery: 0xA3BE8C, typeTemplate: 0xD08770))

    static let dracula = Theme(id: .dracula, hex: ThemeHexes(
        bg: 0x282A36, bg2: 0x1E1F29, bg3: 0x44475A, bg4: 0x4E5067,
        line: 0x44475A, lineSoft: 0x363846,
        fgDefault: 0xF8F8F2, fgMuted: 0xE6E6E0,
        fgSubtle: 0x9095A6, fgFaint: 0x6272A4,
        accentPrimary: 0xFF79C6, accentSecondary: 0xBD93F9,
        typeTask: 0xFF5555, typeEvent: 0x8BE9FD, typeNote: 0xF1FA8C,
        typeProject: 0xBD93F9, typePerson: 0xFFB86C,
        typeQuery: 0x50FA7B, typeTemplate: 0xFFB86C))

    static let carbonfox = Theme(id: .carbonfox, hex: ThemeHexes(
        bg: 0x161616, bg2: 0x252525, bg3: 0x353535, bg4: 0x484848,
        line: 0x353535, lineSoft: 0x2A2A2A,
        fgDefault: 0xF2F4F8, fgMuted: 0xDDE1E6,
        fgSubtle: 0x878D96, fgFaint: 0x525252,
        accentPrimary: 0xFF7EB6, accentSecondary: 0xBE95FF,
        typeTask: 0xEE5396, typeEvent: 0x33B1FF, typeNote: 0xFFB454,
        typeProject: 0x78A9FF, typePerson: 0xBE95FF,
        typeQuery: 0x42BE65, typeTemplate: 0xC79B58))

    static let ayuDark = Theme(id: .ayuDark, hex: ThemeHexes(
        bg: 0x0B0E14, bg2: 0x0D1017, bg3: 0x131721, bg4: 0x1B1F2B,
        line: 0x131721, lineSoft: 0x0F141C,
        fgDefault: 0xBFBDB6, fgMuted: 0xACA9A0,
        fgSubtle: 0x565B66, fgFaint: 0x3D424D,
        accentPrimary: 0xFF8F40, accentSecondary: 0xD2A6FF,
        typeTask: 0xF07178, typeEvent: 0x39BAE6, typeNote: 0xFFB454,
        typeProject: 0x59C2FF, typePerson: 0xD2A6FF,
        typeQuery: 0xAAD94C, typeTemplate: 0xCC9966))

    static let monokaiPro = Theme(id: .monokaiPro, hex: ThemeHexes(
        bg: 0x2D2A2E, bg2: 0x221F22, bg3: 0x403E41, bg4: 0x5B595C,
        line: 0x403E41, lineSoft: 0x352E33,
        fgDefault: 0xFCFCFA, fgMuted: 0xE3E1DC,
        fgSubtle: 0x939293, fgFaint: 0x5B595C,
        accentPrimary: 0xFC9867, accentSecondary: 0xAB9DF2,
        typeTask: 0xFF6188, typeEvent: 0x78DCE8, typeNote: 0xFFD866,
        typeProject: 0xAB9DF2, typePerson: 0xAB9DF2,
        typeQuery: 0xA9DC76, typeTemplate: 0xFFD866))

    static let palenight = Theme(id: .palenight, hex: ThemeHexes(
        bg: 0x292D3E, bg2: 0x202331, bg3: 0x32374D, bg4: 0x3F4862,
        line: 0x32374D, lineSoft: 0x262B3F,
        fgDefault: 0xA6ACCD, fgMuted: 0x959DC6,
        fgSubtle: 0x676E95, fgFaint: 0x4F557A,
        accentPrimary: 0xF78C6C, accentSecondary: 0xC792EA,
        typeTask: 0xFF5370, typeEvent: 0x82AAFF, typeNote: 0xFFCB6B,
        typeProject: 0x82AAFF, typePerson: 0xC792EA,
        typeQuery: 0xC3E88D, typeTemplate: 0xCC9966))

    /// All themes in picker order.
    static let all: [Theme] = [
        .prism, .prismSpark, .prismLight,
        .tokyoNight, .tokyoNightStorm,
        .catppuccinMocha, .catppuccinMacchiato,
        .rosePine, .rosePineMoon,
        .kanagawaWave, .kanagawaDragon,
        .everforestDark, .gruvboxMaterial,
        .nord, .dracula,
        .carbonfox, .ayuDark,
        .monokaiPro, .palenight,
    ]

    static func byId(_ id: ThemeID) -> Theme {
        all.first(where: { $0.id == id }) ?? .prism
    }
}
