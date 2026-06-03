import SwiftUI

@main
struct TeselaApp: App {
    /// Shell selection. The Graphite redesign is now the DEFAULT — it owns
    /// the daily-driver views and the collaborative-editing path, which the
    /// legacy `AppShell` does not have. The legacy shell is kept as an escape
    /// hatch behind the `-legacy` launch argument (Xcode scheme / `simctl
    /// launch … -legacy`) or the `tesela.useLegacyShell` default, until it's
    /// removed at the redesign cutover.
    private var useLegacyShell: Bool {
        ProcessInfo.processInfo.arguments.contains("-legacy")
            || UserDefaults.standard.bool(forKey: "tesela.useLegacyShell")
    }

    var body: some Scene {
        WindowGroup {
            if useLegacyShell {
                AppShell()
            } else {
                GrAppShell()
            }
        }
    }
}
