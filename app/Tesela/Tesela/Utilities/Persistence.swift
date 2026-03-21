import Foundation

// MARK: - Persistence
// Lightweight UserDefaults wrappers for favorites and recents

enum Persistence {
    nonisolated(unsafe) private static let defaults = UserDefaults.standard

    // MARK: - Keys
    private enum Keys {
        static let recents = "tesela.recentPageIds"
        static let favorites = "tesela.favoritePageIds"
        static let serverURL = "tesela.serverURL"
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
}
