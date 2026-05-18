import SwiftUI

/// Semantic font roles defined in the iOS design memo. Each role takes
/// a `DensityTier` so the whole scale shifts coherently when the user
/// changes Settings → Appearance → Density.
///
/// Two font families are in play:
/// - **Inter Tight** (sans) for body / titles / general UI text
/// - **JetBrains Mono** (mono) for captions, chips, slugs, status text
///
/// Fall back to system fonts if the families aren't bundled — keeps
/// previews working before the assets land.
enum TypeRole {
    case pageTitle
    case sectionTitle
    case heading
    case body
    case bodyCompact
    case caption
    case chip
    case statusLine

    /// Base size in points before density adjustment.
    var baseSize: CGFloat {
        switch self {
        case .pageTitle:    return 28
        case .sectionTitle: return 22
        case .heading:      return 17
        case .body:         return 15
        case .bodyCompact:  return 13
        case .caption:      return 11
        case .chip:         return 11.5
        case .statusLine:   return 10.5
        }
    }

    var weight: Font.Weight {
        switch self {
        case .pageTitle, .sectionTitle, .heading, .chip: return .semibold
        default: return .regular
        }
    }

    var design: Font.Design {
        switch self {
        case .caption, .chip, .statusLine: return .monospaced
        default: return .default
        }
    }

    var tracking: CGFloat {
        switch self {
        case .pageTitle, .sectionTitle, .heading: return -0.20
        default: return 0
        }
    }
}

extension Font {
    /// Build the font for a `TypeRole` honoring the current density tier.
    /// The `body` role uses the tier's own `bodySize`; all other roles
    /// scale proportionally via the tier's `scale` multiplier.
    static func tesela(_ role: TypeRole, density: DensityTier = .comfortable) -> Font {
        let size: CGFloat
        switch role {
        case .body:
            size = density.bodySize
        default:
            size = role.baseSize * density.scale
        }
        return .system(size: size, weight: role.weight, design: role.design)
    }
}

/// Convenience modifier so views can call `.teselaFont(.body)` without
/// reaching into the environment manually.
struct TeselaFontModifier: ViewModifier {
    let role: TypeRole
    @Environment(\.density) private var density

    func body(content: Content) -> some View {
        content
            .font(.tesela(role, density: density))
            .tracking(role.tracking)
    }
}

extension View {
    func teselaFont(_ role: TypeRole) -> some View {
        modifier(TeselaFontModifier(role: role))
    }
}
