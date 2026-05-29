import SwiftUI

@main
struct TeselaApp: App {
    /// Graphite-redesign shell opt-in. Default is the shipping `AppShell`;
    /// pass the `-graphite` launch argument (Xcode scheme / `simctl launch`)
    /// or set the `tesela.useGraphiteShell` default to preview the new
    /// Graphite shell. Reversible until the redesign cutover makes it the
    /// sole entry.
    private var useGraphiteShell: Bool {
        ProcessInfo.processInfo.arguments.contains("-graphite")
            || UserDefaults.standard.bool(forKey: "tesela.useGraphiteShell")
    }

    var body: some Scene {
        WindowGroup {
            if useGraphiteShell {
                GrAppShell()
            } else {
                AppShell()
            }
        }
    }
}
