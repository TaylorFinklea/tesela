import SwiftUI
import Observation

@Observable
@MainActor
final class AppState {
    // MARK: - Connection
    var connectionStatus: ConnectionStatus = .disconnected

    // MARK: - Navigation
    var selectedNavItem: NavItem = .journals
    var currentPage: Page?
    var pages: [Page] = []
    var tags: [String] = []

    // MARK: - Recent & Favorites
    var recentPageIds: [String] = []
    var favoritePageIds: [String] = []

    // MARK: - UI State
    var isLeftSidebarVisible = true
    var isRightSidebarVisible = false
    var isCommandPaletteVisible = false
    var isSearchVisible = false
    var searchQuery = ""

    // MARK: - Services
    let api = APIClient()
    let wsClient = WebSocketClient()

    // MARK: - Startup
    func launch() async {
        await checkHealth()
        guard connectionStatus == .connected else { return }
        await loadInitialData()
        await wsClient.connect()
    }

    private func checkHealth() async {
        connectionStatus = .connecting
        do {
            let healthy = try await api.health()
            connectionStatus = healthy ? .connected : .error("Server unhealthy")
        } catch {
            connectionStatus = .error(error.localizedDescription)
        }
    }

    private func loadInitialData() async {
        async let pagesTask = api.listNotes()
        async let tagsTask = api.listTags()
        do {
            let (fetchedPages, fetchedTags) = try await (pagesTask, tagsTask)
            pages = fetchedPages
            tags = fetchedTags
        } catch {
            // Non-fatal — show empty state
        }
    }

    // MARK: - Navigation helpers
    func open(_ page: Page) {
        currentPage = page
        addToRecents(page.id)
    }

    func addToRecents(_ id: String) {
        recentPageIds.removeAll { $0 == id }
        recentPageIds.insert(id, at: 0)
        if recentPageIds.count > 10 {
            recentPageIds = Array(recentPageIds.prefix(10))
        }
        Persistence.saveRecents(recentPageIds)
    }

    func toggleFavorite(_ id: String) {
        if favoritePageIds.contains(id) {
            favoritePageIds.removeAll { $0 == id }
        } else {
            favoritePageIds.append(id)
        }
        Persistence.saveFavorites(favoritePageIds)
    }

    var recentPages: [Page] {
        recentPageIds.compactMap { id in pages.first { $0.id == id } }
    }

    var favoritePages: [Page] {
        favoritePageIds.compactMap { id in pages.first { $0.id == id } }
    }
}

// MARK: - ConnectionStatus
enum ConnectionStatus: Equatable {
    case disconnected
    case connecting
    case connected
    case error(String)

    var isConnected: Bool { self == .connected }

    var displayLabel: String {
        switch self {
        case .disconnected: "Disconnected"
        case .connecting: "Connecting…"
        case .connected: "Connected"
        case .error(let msg): "Error: \(msg)"
        }
    }

    var color: Color {
        switch self {
        case .connected: .green
        case .connecting: .yellow
        case .disconnected, .error: .red
        }
    }
}

// MARK: - NavItem
enum NavItem: Hashable, CaseIterable {
    case journals
    case pages
    case graph

    var label: String {
        switch self {
        case .journals: "Journals"
        case .pages: "Pages"
        case .graph: "Graph"
        }
    }

    var systemImage: String {
        switch self {
        case .journals: "calendar"
        case .pages: "doc.text"
        case .graph: "point.3.connected.trianglepath.dotted"
        }
    }
}
