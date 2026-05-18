import SwiftUI
import Combine

/// The live "what theme + density should the app render in?" decision.
/// Persists via `@AppStorage` so it survives relaunches. Reading code
/// should prefer the environment values (`@Environment(\.theme)`) over
/// reaching into the controller directly — that keeps views theme-blind
/// and makes previews trivial.
@MainActor
final class AppearanceController: ObservableObject {
    @AppStorage("appearance.themeID") private var themeIDRaw: String = ThemeID.prismIndigo.rawValue
    @AppStorage("appearance.density") private var densityRaw: String = DensityTier.comfortable.rawValue

    var themeID: ThemeID {
        get { ThemeID(rawValue: themeIDRaw) ?? .prismIndigo }
        set {
            themeIDRaw = newValue.rawValue
            objectWillChange.send()
        }
    }

    var density: DensityTier {
        get { DensityTier(rawValue: densityRaw) ?? .comfortable }
        set {
            densityRaw = newValue.rawValue
            objectWillChange.send()
        }
    }

    var theme: Theme {
        Theme.byId(themeID)
    }
}

/// Top-level wrapper that injects the active theme + density into the
/// environment for every descendant view. Use once at the app root.
struct TeselaAppearance<Content: View>: View {
    @ObservedObject var controller: AppearanceController
    let content: () -> Content

    init(controller: AppearanceController, @ViewBuilder content: @escaping () -> Content) {
        self.controller = controller
        self.content = content
    }

    var body: some View {
        content()
            .environment(\.theme, controller.theme)
            .environment(\.density, controller.density)
            // Force-dark for v0.4 per decision #10 (light themes ship later).
            .preferredColorScheme(.dark)
    }
}
