import Foundation
import Combine

/// Read/write contract the views call into. Real implementation comes
/// in Phase 15 (FFI-backed); for now the views run against
/// `MockMosaicService` so the UI is testable end-to-end without I/O.
@MainActor
protocol MosaicService: AnyObject, ObservableObject {
    var pages: [Page] { get }
    var tags: [Tag] { get }
    var recent: [RecentEntry] { get }
    var pinned: [PinnedEntry] { get }

    /// Today's blocks (mutable — toggling a task updates this list).
    var todayBlocks: [Block] { get }
    var yesterdayBlocks: [Block] { get }

    var palette: [PaletteVerb] { get }
    var searchResults: [SearchResult] { get }
    var backlinks: [Backlink] { get }
    var outline: [OutlineEntry] { get }

    var todayDate: Date { get }
    var todayLabel: String { get }      // "Sun, May 17"
    var todayLongLabel: String { get }  // "Today"

    func toggleTask(id: String)
    func capture(_ text: String)
    func search(_ query: String) -> [SearchResult]
}
