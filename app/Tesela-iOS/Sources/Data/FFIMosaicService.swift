import Foundation
import Combine

/// Real mosaic backed by the Rust core via UniFFI. **Stub for now.**
///
/// Phase 15 plan: extend `crates/tesela-sync-ffi/src/lib.rs` to expose:
///   - `mosaic_list_pages(root: String) -> Vec<PageRecord>`
///   - `mosaic_get_page(root: String, slug: String) -> PageRecord`
///   - `mosaic_upsert_block(root, page_slug, block: BlockRecord)`
///   - `mosaic_get_backlinks(root, slug) -> Vec<BacklinkRecord>`
///   - `mosaic_search(root, q: String) -> Vec<SearchResultRecord>`
///
/// Each delegates to `tesela-core` (the same core the web client and
/// CLI use). Regenerate the Swift bindings via:
///   `cargo uniffi-bindgen generate --library libtesela_sync_ffi.dylib --language swift`
///
/// Then swap `MockMosaicService` for this implementation behind a
/// settings toggle. Until then, this class throws on every call.
@MainActor
final class FFIMosaicService: ObservableObject, MosaicService {
    @Published private(set) var pages: [Page] = []
    @Published private(set) var tags: [Tag] = []
    @Published private(set) var recent: [RecentEntry] = []
    @Published private(set) var pinned: [PinnedEntry] = []
    @Published private(set) var todayBlocks: [Block] = []
    @Published private(set) var yesterdayBlocks: [Block] = []
    @Published private(set) var palette: [PaletteVerb] = []
    @Published private(set) var searchResults: [SearchResult] = []
    @Published private(set) var backlinks: [Backlink] = []
    @Published private(set) var outline: [OutlineEntry] = []

    let todayDate: Date

    var todayLabel: String {
        let f = DateFormatter()
        f.dateFormat = "EEE, MMM d"
        return f.string(from: todayDate)
    }

    var todayLongLabel: String { "Today" }

    /// Path to the mosaic root on disk. Defaults to a per-user folder
    /// under `Documents/`; will be configurable in Settings → Mosaic.
    private let mosaicRoot: URL

    init(mosaicRoot: URL? = nil) {
        self.todayDate = Date()
        self.mosaicRoot = mosaicRoot ?? FileManager.default
            .urls(for: .documentDirectory, in: .userDomainMask)
            .first!
            .appendingPathComponent("Mosaic", isDirectory: true)
        // Phase 15: hydrate the @Published properties from the Rust
        // core. For now we leave them empty so any view rendered
        // against this service shows the empty state.
    }

    func toggleTask(id: String) {
        // Phase 15: route through mosaic_upsert_block FFI.
        guard let idx = todayBlocks.firstIndex(where: { $0.id == id }), todayBlocks[idx].kind == .task else {
            return
        }
        todayBlocks[idx].done.toggle()
    }

    func capture(_ text: String) {
        // Phase 15: route through mosaic_upsert_block FFI to actually
        // write to disk + replay through the sync layer.
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        let block = Block(
            id: "ffi-\(UUID().uuidString.prefix(6))",
            kind: .note,
            text: trimmed
        )
        todayBlocks.insert(block, at: 0)
    }

    func search(_ query: String) -> [SearchResult] {
        // Phase 15: route through mosaic_search FFI.
        []
    }
}
