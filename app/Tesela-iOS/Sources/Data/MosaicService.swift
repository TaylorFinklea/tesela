import Foundation
import Combine

/// Read/write contract the views call into. The concrete
/// `MockMosaicService` implements it against either an in-memory mock
/// snapshot or a live `tesela-server` over HTTP.
///
/// Per-page derived data (backlinks, outline) is not on this protocol —
/// it is keyed by note id on the concrete service, since the views that
/// consume it always know which page they are rendering.
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

    var todayDate: Date { get }
    var todayLabel: String { get }      // "Sun, May 17"
    var todayLongLabel: String { get }  // "Today"

    func toggleTask(id: String)
    func capture(_ text: String)
    func search(_ query: String) -> [SearchResult]
}
