import SwiftUI

/// Body-text density tier. Affects the base size of the `body` style; all
/// other scale roles step in proportion. Default is `.comfortable` per
/// decision #15 in the iOS design follow-up.
///
/// Mobile defaults to **less** dense than desktop, by design — fat fingers
/// vs. mouse clicks. The user can switch under Settings → Appearance →
/// Density.
enum DensityTier: String, CaseIterable, Identifiable, Codable {
    case comfortable
    case compact
    case compactPlus

    var id: String { rawValue }

    /// Body font size in points.
    var bodySize: CGFloat {
        switch self {
        case .comfortable: return 15
        case .compact:     return 13
        case .compactPlus: return 12
        }
    }

    /// Multiplier applied to non-body roles (pageTitle, sectionTitle, …)
    /// so the type scale stays proportional across tiers.
    var scale: CGFloat {
        switch self {
        case .comfortable: return 1.00
        case .compact:     return 0.87
        case .compactPlus: return 0.80
        }
    }

    /// Default line-height multiplier; tighter rows for denser tiers.
    var lineHeight: CGFloat {
        switch self {
        case .comfortable: return 1.50
        case .compact:     return 1.45
        case .compactPlus: return 1.40
        }
    }

    var displayName: String {
        switch self {
        case .comfortable: return "Comfortable"
        case .compact:     return "Compact"
        case .compactPlus: return "Compact+"
        }
    }
}
