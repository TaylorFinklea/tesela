import SwiftUI
import Observation
import os.log

private let logger = Logger(subsystem: "com.tesela.AppState", category: "AppState")

@Observable
@MainActor
final class AppState {
    // MARK: - Connection
    var connectionStatus: ConnectionStatus = .disconnected
    var initialDataError: String?

    // MARK: - Navigation
    var selectedNavItem: NavItem = .tiles {
        didSet { Persistence.saveSelectedNavItem(selectedNavItem.persistenceKey) }
    }
    var currentPage: Page?
    var pages: [Page] = []
    var tags: [String] = []
    var typeRegistry: [TypeDefinition] = []
    var propertyRegistry: [PropertyDef] = []

    // MARK: - Navigation Stack (back/forward)
    struct NavEntry: Equatable {
        let pageId: String
        let blockIndex: Int?  // flat index into the page's block list, nil = full page
    }
    private(set) var navigationStack: [NavEntry] = []
    private(set) var navigationIndex: Int = -1

    var canGoBack: Bool { navigationIndex > 0 }
    var canGoForward: Bool { navigationIndex < navigationStack.count - 1 }
    var zoomedBlockIndex: Int? {
        guard navigationIndex >= 0, navigationIndex < navigationStack.count else { return nil }
        return navigationStack[navigationIndex].blockIndex
    }

    // MARK: - Focused Block (for right sidebar properties panel)
    struct FocusedBlockInfo {
        let text: String
        let tags: [String]
        let properties: [String: String]
    }
    var focusedBlockInfo: FocusedBlockInfo?

    // MARK: - Recent & Favorites
    var recentPageIds: [String] = []
    var favoritePageIds: [String] = []

    // MARK: - UI State
    var isLeftSidebarVisible = true {
        didSet { Persistence.saveLeftSidebarVisible(isLeftSidebarVisible) }
    }
    var isRightSidebarVisible = false {
        didSet { Persistence.saveRightSidebarVisible(isRightSidebarVisible) }
    }
    var isCommandPaletteVisible = false
    var isSlashMenuVisible = false
    var isSpaceMenuVisible = false
    var isShowingNewPageSheet = false
    var isSearchVisible = false
    var lastError: String?
    var searchQuery = ""

    // MARK: - Services
    let api: APIClient
    let wsClient = WebSocketClient()

    init() {
        // Restore persisted state
        let serverURL = Persistence.loadServerURL()
        api = APIClient(baseURL: URL(string: serverURL) ?? URL(string: "http://localhost:7474")!)
        isLeftSidebarVisible = Persistence.loadLeftSidebarVisible()
        isRightSidebarVisible = Persistence.loadRightSidebarVisible()
        selectedNavItem = NavItem.from(persistenceKey: Persistence.loadSelectedNavItem())
    }

    // MARK: - Startup
    func launch() async {
        await checkHealth()
        guard connectionStatus == .connected else { return }
        await loadInitialData()
        wireWebSocketCallbacks()
        await wsClient.connect()
    }

    private func wireWebSocketCallbacks() {
        wsClient.onNoteCreated = { [weak self] note in
            guard let self else { return }
            if !pages.contains(where: { $0.id == note.id }) {
                pages.append(note)
            }
        }
        wsClient.onNoteUpdated = { [weak self] note in
            guard let self else { return }
            if let idx = pages.firstIndex(where: { $0.id == note.id }) {
                pages[idx] = note
            }
            if currentPage?.id == note.id { currentPage = note }
        }
        wsClient.onNoteDeleted = { [weak self] id in
            guard let self else { return }
            pages.removeAll { $0.id == id }
            if currentPage?.id == id { currentPage = nil }
            recentPageIds.removeAll { $0 == id }
            favoritePageIds.removeAll { $0 == id }
        }
        wsClient.onConnectionStateChanged = { [weak self] connected in
            guard let self else { return }
            connectionStatus = connected ? .connected : .error("Server disconnected")
        }
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
        async let typesTask = api.getTypes()
        do {
            let (fetchedPages, fetchedTags) = try await (pagesTask, tagsTask)
            pages = fetchedPages
            tags = fetchedTags
        } catch {
            logger.error("loadInitialData failed: \(error.localizedDescription)")
            initialDataError = "Failed to load notes and tags: \(error.localizedDescription)"
        }
        do {
            typeRegistry = try await typesTask
        } catch {
            logger.debug("Failed to load type registry: \(error.localizedDescription)")
            typeRegistry = []
            if initialDataError == nil {
                initialDataError = "Failed to load type registry"
            }
        }
        do {
            propertyRegistry = try await api.getProperties()
        } catch {
            logger.debug("Failed to load property registry: \(error.localizedDescription)")
            propertyRegistry = []
            if initialDataError == nil {
                initialDataError = "Failed to load property registry"
            }
        }
    }

    // MARK: - Navigation helpers
    func open(_ page: Page) {
        // Truncate forward history and push
        navigationStack = Array(navigationStack.prefix(navigationIndex + 1))
        navigationStack.append(NavEntry(pageId: page.id, blockIndex: nil))
        navigationIndex = navigationStack.count - 1
        currentPage = page
        addToRecents(page.id)
    }

    func openBlockZoom(blockIndex: Int) {
        guard let page = currentPage else { return }
        navigationStack = Array(navigationStack.prefix(navigationIndex + 1))
        navigationStack.append(NavEntry(pageId: page.id, blockIndex: blockIndex))
        navigationIndex = navigationStack.count - 1
    }

    func exitBlockZoom() {
        guard zoomedBlockIndex != nil else { return }
        goBack()
    }

    func goBack() {
        guard canGoBack else { return }
        navigationIndex -= 1
        let entry = navigationStack[navigationIndex]
        if currentPage?.id != entry.pageId {
            currentPage = pages.first { $0.id == entry.pageId }
        }
    }

    func goForward() {
        guard canGoForward else { return }
        navigationIndex += 1
        let entry = navigationStack[navigationIndex]
        if currentPage?.id != entry.pageId {
            currentPage = pages.first { $0.id == entry.pageId }
        }
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

    // MARK: - CRUD

    func createPage(title: String) async {
        do {
            let page = try await api.createNote(title: title, content: "", tags: [])
            pages.append(page)
            open(page)
            await refreshPages()  // pick up server-generated frontmatter
        } catch {
            lastError = "Failed to create page: \(error.localizedDescription)"
        }
    }

    func updatePage(id: String, newBody: String) async {
        // Find the page from currentPage OR the pages array (tiles edit without setting currentPage)
        let page: Page? = if currentPage?.id == id {
            currentPage
        } else {
            pages.first { $0.id == id }
        }
        guard let page else { return }

        let fullContent = rebuildContent(from: page, newBody: newBody)
        do {
            let updated = try await api.updateNote(id: id, content: fullContent)
            if currentPage?.id == id { currentPage = updated }
            if let idx = pages.firstIndex(where: { $0.id == id }) {
                pages[idx] = updated
            }
        } catch {
            lastError = "Failed to save: \(error.localizedDescription)"
        }
    }

    /// Updates a page with complete content (frontmatter + body).
    /// Use this when the frontmatter itself changes (e.g. Property page schema edits).
    func updatePageContent(id: String, fullContent: String) async {
        do {
            let updated = try await api.updateNote(id: id, content: fullContent)
            if currentPage?.id == id { currentPage = updated }
            if let idx = pages.firstIndex(where: { $0.id == id }) {
                pages[idx] = updated
            }
            // Refresh type/property registries since schema may have changed
            do {
                typeRegistry = try await api.getTypes()
            } catch {
                logger.debug("Failed to refresh type registry: \(error.localizedDescription)")
            }
            do {
                propertyRegistry = try await api.getProperties()
            } catch {
                logger.debug("Failed to refresh property registry: \(error.localizedDescription)")
            }
        } catch {
            lastError = "Failed to save: \(error.localizedDescription)"
        }
    }

    func deletePage(_ page: Page) async {
        do {
            try await api.deleteNote(id: page.id)
            pages.removeAll { $0.id == page.id }
            if currentPage?.id == page.id {
                currentPage = nil
            }
            recentPageIds.removeAll { $0 == page.id }
            favoritePageIds.removeAll { $0 == page.id }
        } catch {
            lastError = "Failed to delete: \(error.localizedDescription)"
        }
    }

    func refreshPages() async {
        async let pagesTask = api.listNotes()
        async let tagsTask = api.listTags()
        do {
            let (fetchedPages, fetchedTags) = try await (pagesTask, tagsTask)
            pages = fetchedPages
            tags = fetchedTags
        } catch {
            logger.debug("Failed to refresh pages: \(error.localizedDescription)")
        }
    }

    // Reconstructs full file content (frontmatter + body) for a PUT request.
    // Walks lines to find the closing --- of the frontmatter block, then appends newBody.
    private func rebuildContent(from page: Page, newBody: String) -> String {
        let lines = page.content.components(separatedBy: "\n")
        var dashCount = 0
        var frontmatterEnd = 0
        for (i, line) in lines.enumerated() {
            if line.trimmingCharacters(in: .whitespaces) == "---" {
                dashCount += 1
                if dashCount == 2 { frontmatterEnd = i; break }
            }
        }
        guard dashCount >= 2 else { return newBody }
        return lines[0...frontmatterEnd].joined(separator: "\n") + "\n" + newBody
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
    case tiles
    case pages
    case graph

    var label: String {
        switch self {
        case .tiles: "Tiles"
        case .pages: "Pages"
        case .graph: "Graph"
        }
    }

    var systemImage: String {
        switch self {
        case .tiles: "calendar"
        case .pages: "doc.text"
        case .graph: "point.3.connected.trianglepath.dotted"
        }
    }

    var persistenceKey: String {
        switch self {
        case .tiles: "tiles"
        case .pages: "pages"
        case .graph: "graph"
        }
    }

    static func from(persistenceKey: String) -> NavItem {
        switch persistenceKey {
        case "pages": .pages
        case "graph": .graph
        default: .tiles
        }
    }
}
