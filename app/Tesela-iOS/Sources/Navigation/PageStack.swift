import SwiftUI

/// Workspace-level stack of currently-open pages. Mirrors Safari's
/// tab carousel: each opened page is a "card"; the most recent card
/// is on top. Swipe-up from the bottom edge reveals the carousel of
/// open cards (Phase 13a stretch goal — for now we trigger the
/// overlay via a button on the page's top bar).
///
/// Persisted via @AppStorage so the stack survives app relaunches.
@MainActor
final class PageStack: ObservableObject {
    @Published private(set) var openPages: [Page] = []

    private let storageKey = "pageStack.openPageIds"

    init() {
        loadFromStorage()
    }

    /// Push a page onto the stack. If it's already open, move it to the top.
    func open(_ page: Page) {
        openPages.removeAll { $0.id == page.id }
        openPages.insert(page, at: 0)
        persist()
    }

    /// Remove a page from the stack.
    func close(_ pageId: String) {
        openPages.removeAll { $0.id == pageId }
        persist()
    }

    /// Clear the stack.
    func closeAll() {
        openPages.removeAll()
        persist()
    }

    /// Rehydrate from `@AppStorage` if available; resolve page ids back
    /// to full `Page` records via the mosaic.
    func rehydrate(from mosaic: MockMosaicService) {
        guard openPages.isEmpty else { return }
        let storedIds = UserDefaults.standard.array(forKey: storageKey) as? [String] ?? []
        openPages = storedIds.compactMap { id in
            mosaic.pages.first(where: { $0.id == id })
        }
    }

    private func persist() {
        let ids = openPages.map { $0.id }
        UserDefaults.standard.set(ids, forKey: storageKey)
    }

    private func loadFromStorage() {
        // Defer to `rehydrate(from:)` once mosaic is available.
    }
}
