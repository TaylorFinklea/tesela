import Foundation

// MARK: - Persistence
// Lightweight UserDefaults wrappers for app state

enum Persistence {
    nonisolated(unsafe) private static let defaults = UserDefaults.standard

    // MARK: - Keys
    private enum Keys {
        static let recents = "tesela.recentPageIds"
        static let favorites = "tesela.favoritePageIds"
        static let serverURL = "tesela.serverURL"
        static let leftSidebarVisible = "tesela.leftSidebarVisible"
        static let rightSidebarVisible = "tesela.rightSidebarVisible"
        static let selectedNavItem = "tesela.selectedNavItem"
        static let colorScheme = "tesela.colorScheme"
        static let accentColor = "tesela.accentColor"
    }

    // MARK: - Recents
    static func loadRecents() -> [String] {
        defaults.stringArray(forKey: Keys.recents) ?? []
    }

    static func saveRecents(_ ids: [String]) {
        defaults.set(ids, forKey: Keys.recents)
    }

    // MARK: - Favorites
    static func loadFavorites() -> [String] {
        defaults.stringArray(forKey: Keys.favorites) ?? []
    }

    static func saveFavorites(_ ids: [String]) {
        defaults.set(ids, forKey: Keys.favorites)
    }

    // MARK: - Server URL
    static func loadServerURL() -> String {
        defaults.string(forKey: Keys.serverURL) ?? "http://localhost:7474"
    }

    static func saveServerURL(_ url: String) {
        defaults.set(url, forKey: Keys.serverURL)
    }

    // MARK: - Sidebar State
    static func loadLeftSidebarVisible() -> Bool {
        defaults.object(forKey: Keys.leftSidebarVisible) == nil ? true : defaults.bool(forKey: Keys.leftSidebarVisible)
    }

    static func saveLeftSidebarVisible(_ visible: Bool) {
        defaults.set(visible, forKey: Keys.leftSidebarVisible)
    }

    static func loadRightSidebarVisible() -> Bool {
        defaults.bool(forKey: Keys.rightSidebarVisible)
    }

    static func saveRightSidebarVisible(_ visible: Bool) {
        defaults.set(visible, forKey: Keys.rightSidebarVisible)
    }

    // MARK: - Nav Item
    static func loadSelectedNavItem() -> String {
        defaults.string(forKey: Keys.selectedNavItem) ?? "tiles"
    }

    static func saveSelectedNavItem(_ item: String) {
        defaults.set(item, forKey: Keys.selectedNavItem)
    }

    // MARK: - Theme
    static func loadColorScheme() -> String {
        defaults.string(forKey: Keys.colorScheme) ?? "auto"
    }

    static func saveColorScheme(_ scheme: String) {
        defaults.set(scheme, forKey: Keys.colorScheme)
    }

    static func loadAccentColor() -> String {
        defaults.string(forKey: Keys.accentColor) ?? "blue"
    }

    static func saveAccentColor(_ color: String) {
        defaults.set(color, forKey: Keys.accentColor)
    }
}
