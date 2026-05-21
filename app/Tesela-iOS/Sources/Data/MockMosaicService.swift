import Foundation
import Combine

/// The mosaic the SwiftUI views consume. Despite the historical
/// `Mock` prefix, this service can now load from two sources:
///
/// 1. **Built-in mock** — a realistic in-memory snapshot mirroring the
///    design canvas's `data.jsx`. Default; works offline.
/// 2. **HTTP** — hits the local `tesela-server` REST API on a
///    user-configurable URL (Settings → Backend). Same endpoints the
///    web client uses. Lets the iOS simulator share a mosaic with the
///    desktop side without a UniFFI expansion.
///
/// Phase 17 — connects the simulator to your local server for the
/// dual-client smoke test.
@MainActor
final class MockMosaicService: ObservableObject, MosaicService {
    @Published private(set) var pages: [Page]
    @Published private(set) var tags: [Tag]
    @Published private(set) var recent: [RecentEntry]
    @Published private(set) var pinned: [PinnedEntry]

    @Published private(set) var todayBlocks: [Block]
    @Published private(set) var yesterdayBlocks: [Block]

    @Published private(set) var palette: [PaletteVerb]
    @Published private(set) var searchResults: [SearchResult]

    /// Backlinks for each page the user has opened, keyed by note id.
    /// Filled by `loadPage(id:)` from `GET /notes/{id}/backlinks`. The
    /// page outline is derived on demand from `loadedPageBlocks`, so it
    /// needs no field of its own.
    @Published private(set) var loadedBacklinks: [String: [Backlink]] = [:]

    /// Outgoing wiki-links for each opened page, keyed by note id.
    /// Filled by `loadPage(id:)` from `GET /notes/{id}/links` — drives
    /// the Peek graph lens.
    @Published private(set) var loadedLinks: [String: [Backlink]] = [:]

    let todayDate: Date

    var todayLabel: String {
        let formatter = DateFormatter()
        formatter.dateFormat = "EEE, MMM d"
        return formatter.string(from: todayDate)
    }
    var todayLongLabel: String { "Today" }

    /// Backend mode + URL come from `@AppStorage` and are managed by
    /// `BackendSettings`. The Mosaic reads them once on `refresh()`.
    enum Backend: Equatable {
        case mock
        case http(URL)
    }

    enum ConnectionState: Equatable {
        case idle
        case connecting
        /// The server is restarting to swap which mosaic it serves.
        case switching
        case ready
        case failed(String)
    }
    @Published var connection: ConnectionState = .idle {
        didSet {
            // Auto-reconnect on every fresh transition into .failed.
            // The user shouldn't have to manually pull-to-refresh after
            // a transient WiFi blip or server restart; we keep retrying
            // with exponential backoff (2s, 4s, 8s, ... capped at 60s)
            // until the request succeeds and the daemon flips back to
            // .ready, at which point the loop self-cancels.
            switch (oldValue, connection) {
            case (.failed, .failed): break // already retrying
            case (_, .failed): startReconnectLoop()
            case (_, .ready), (_, .idle): cancelReconnectLoop()
            default: break
            }
        }
    }

    /// In-flight reconnect task. Cancelled when we leave the .failed
    /// state for any reason (success, mock-mode swap, explicit refresh).
    private var reconnectTask: Task<Void, Never>?

    /// Per-page load state, keyed by note id. PageView reads this to
    /// show a loading skeleton while the body is being fetched.
    enum PageLoadState: Equatable {
        case idle
        case loading
        case ready
        case failed(String)
    }
    @Published private(set) var pageLoadStates: [String: PageLoadState] = [:]

    /// Parsed blocks for pages opened from Library/Search. Filled by
    /// `loadPage(id:)`. The Daily tab has its own `todayBlocks` field;
    /// this dictionary holds the bodies of every other page the user
    /// has navigated into so they re-open instantly.
    @Published private(set) var loadedPageBlocks: [String: [Block]] = [:]

    /// Raw frontmatter for each loaded page, so writeback can splice
    /// new body content without stomping `tags:`, `title:`, etc.
    private var loadedPageFrontmatter: [String: String] = [:]

    private let session = URLSession(configuration: .default)
    private let iso = ISO8601DateFormatter()
    private var serverDailyId: String = ""

    init() {
        let today = Date()
        self.todayDate = today
        self.pages = MockSeed.pages
        self.tags = MockSeed.tags
        self.recent = MockSeed.recent
        self.pinned = Self.loadPinned()
        self.todayBlocks = MockSeed.todayBlocks
        self.yesterdayBlocks = MockSeed.yesterdayBlocks
        self.palette = MockSeed.palette
        self.searchResults = MockSeed.searchResults
    }

    // MARK: - Mutating API

    func toggleTask(id: String) {
        if let idx = todayBlocks.firstIndex(where: { $0.id == id }), todayBlocks[idx].kind == .task {
            todayBlocks[idx].done.toggle()
            scheduleWriteback()
        } else if let idx = yesterdayBlocks.firstIndex(where: { $0.id == id }), yesterdayBlocks[idx].kind == .task {
            yesterdayBlocks[idx].done.toggle()
        }
    }

    /// Cycle a block's status: note → open task → done task → note.
    /// Used by the keyboard accessory toolbar so the user can convert
    /// between note and task without leaving the keyboard.
    func cycleBlockStatus(id: String, pageSlug: String? = nil) {
        if let slug = pageSlug {
            var blocks = loadedPageBlocks[slug] ?? []
            guard let idx = blocks.firstIndex(where: { $0.id == id }) else { return }
            blocks[idx] = nextStatus(blocks[idx])
            Task { await pushPage(id: slug, blocks: blocks) }
        } else if let idx = todayBlocks.firstIndex(where: { $0.id == id }) {
            todayBlocks[idx] = nextStatus(todayBlocks[idx])
            scheduleWriteback()
        }
    }

    private func nextStatus(_ block: Block) -> Block {
        var next = block
        switch (block.kind, block.done) {
        case (.note, _):       next.kind = .task; next.done = false
        case (.task, false):   next.done = true
        case (.task, true):    next.kind = .note; next.done = false
        default:               next.kind = .note; next.done = false
        }
        return next
    }

    /// Replace the body text of a block on today's daily, then push.
    /// The raw text may contain inline `#tag` hashtags; we split those
    /// out into `block.tags` and store only the body in `block.text`.
    func editTodayBlock(id: String, text: String) {
        guard let idx = todayBlocks.firstIndex(where: { $0.id == id }) else { return }
        let (body, tags) = Self.splitInlineTags(text)
        todayBlocks[idx].text = body
        todayBlocks[idx].tags = tags
        scheduleWriteback()
    }

    /// Pull `#tag` tokens out of `raw` and return (bodyWithoutTags, tags).
    /// Hashtags are recognized as `#` followed by 1+ characters from
    /// `[A-Za-z0-9_-]`. Tags preserve their `#` prefix to match the web
    /// client's storage convention.
    static func splitInlineTags(_ raw: String) -> (body: String, tags: [String]) {
        let pattern = "#[A-Za-z0-9_-]+"
        guard let re = try? NSRegularExpression(pattern: pattern) else {
            return (raw.trimmingCharacters(in: .whitespacesAndNewlines), [])
        }
        let ns = raw as NSString
        let range = NSRange(location: 0, length: ns.length)
        var tags: [String] = []
        re.enumerateMatches(in: raw, range: range) { match, _, _ in
            guard let r = match?.range else { return }
            tags.append(ns.substring(with: r))
        }
        // Remove all matches from the body in a single pass.
        let body = re.stringByReplacingMatches(in: raw, range: range, withTemplate: "")
        let trimmedBody = body
            .replacingOccurrences(of: "  +", with: " ", options: .regularExpression)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return (trimmedBody, tags)
    }

    /// Append a fresh empty block to today and push. Returns the new
    /// block's id so the caller can flip it into edit mode immediately.
    @discardableResult
    func appendTodayBlock(kind: BlockKind = .note) -> String {
        let id = "ios-\(UUID().uuidString.prefix(12).lowercased())"
        todayBlocks.append(Block(id: id, kind: kind, text: ""))
        scheduleWriteback()
        return id
    }

    /// Delete a block from today and push.
    func deleteTodayBlock(id: String) {
        todayBlocks.removeAll { $0.id == id }
        scheduleWriteback()
    }

    /// Indent (or outdent) a block on today. Pass a positive `by`
    /// for indent, negative for outdent. A block may be at most one
    /// level deeper than the block above it (every block has an
    /// immediate parent), so the depth is clamped to `[0, prev + 1]`.
    func indentTodayBlock(id: String, by delta: Int) {
        guard let idx = todayBlocks.firstIndex(where: { $0.id == id }) else { return }
        let maxIndent = idx > 0 ? todayBlocks[idx - 1].indent + 1 : 0
        todayBlocks[idx].indent = max(0, min(maxIndent, todayBlocks[idx].indent + delta))
        scheduleWriteback()
    }

    /// Same for a non-daily page — clamped to `[0, prev + 1]`.
    func indentPageBlock(pageId: String, blockId: String, by delta: Int) {
        var blocks = loadedPageBlocks[pageId] ?? []
        guard let idx = blocks.firstIndex(where: { $0.id == blockId }) else { return }
        let maxIndent = idx > 0 ? blocks[idx - 1].indent + 1 : 0
        blocks[idx].indent = max(0, min(maxIndent, blocks[idx].indent + delta))
        Task { await pushPage(id: pageId, blocks: blocks) }
    }

    /// Same shape as editTodayBlock, but for any opened page.
    func editPageBlock(pageId: String, blockId: String, text: String) {
        var blocks = loadedPageBlocks[pageId] ?? []
        guard let idx = blocks.firstIndex(where: { $0.id == blockId }) else { return }
        blocks[idx].text = text
        Task { await pushPage(id: pageId, blocks: blocks) }
    }

    /// Append a new empty block on a non-daily page.
    @discardableResult
    func appendPageBlock(pageId: String, kind: BlockKind = .note) -> String {
        let id = "ios-\(UUID().uuidString.prefix(12).lowercased())"
        var blocks = loadedPageBlocks[pageId] ?? []
        blocks.append(Block(id: id, kind: kind, text: ""))
        Task { await pushPage(id: pageId, blocks: blocks) }
        return id
    }

    /// Delete a block from a page.
    func deletePageBlock(pageId: String, blockId: String) {
        var blocks = loadedPageBlocks[pageId] ?? []
        blocks.removeAll { $0.id == blockId }
        Task { await pushPage(id: pageId, blocks: blocks) }
    }

    func capture(_ text: String) {
        capture(text, target: .today)
    }

    /// Route a capture to the chosen target. `.today` and `.inbox` both
    /// land in today's daily — `.inbox` adds a `#inbox` tag so the
    /// block surfaces in `InboxView`'s tagged section. `.page` appends
    /// to the named page via the existing per-page push.
    func capture(_ text: String, target: CaptureTarget) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        let id = "captured-\(UUID().uuidString.prefix(12).lowercased())"
        switch target {
        case .today:
            todayBlocks.insert(Block(id: id, kind: .note, text: trimmed), at: 0)
            scheduleWriteback()
        case .inbox:
            todayBlocks.insert(
                Block(id: id, kind: .note, text: trimmed, tags: ["#inbox"]),
                at: 0
            )
            scheduleWriteback()
        case .page(let slug, _):
            var blocks = loadedPageBlocks[slug] ?? []
            blocks.append(Block(id: id, kind: .note, text: trimmed))
            Task { await pushPage(id: slug, blocks: blocks) }
        case .childOf(let parentId, _, let pageSlug):
            insertChildBlock(
                parentId: parentId,
                pageSlug: pageSlug,
                newId: id,
                text: trimmed
            )
        }
    }

    /// Insert a new block directly after `parentId` with indent one
    /// deeper than the parent. `pageSlug == nil` means today's daily.
    private func insertChildBlock(
        parentId: String,
        pageSlug: String?,
        newId: String,
        text: String
    ) {
        if let slug = pageSlug {
            var blocks = loadedPageBlocks[slug] ?? []
            guard let idx = blocks.firstIndex(where: { $0.id == parentId }) else { return }
            let childIndent = blocks[idx].indent + 1
            blocks.insert(
                Block(id: newId, kind: .note, text: text, indent: childIndent),
                at: idx + 1
            )
            Task { await pushPage(id: slug, blocks: blocks) }
        } else {
            guard let idx = todayBlocks.firstIndex(where: { $0.id == parentId }) else { return }
            let childIndent = todayBlocks[idx].indent + 1
            todayBlocks.insert(
                Block(id: newId, kind: .note, text: text, indent: childIndent),
                at: idx + 1
            )
            scheduleWriteback()
        }
    }

    func search(_ query: String) -> [SearchResult] {
        let q = query.lowercased().trimmingCharacters(in: .whitespaces)
        guard !q.isEmpty else { return searchResults }
        return searchResults.filter {
            $0.title.lowercased().contains(q) || $0.snippet.lowercased().contains(q)
        }
    }

    /// Live search hits + state for the SearchView. Filled by
    /// `runSearch`; the existing `searchResults` snapshot stays as
    /// the mock fallback.
    @Published private(set) var searchHits: [SearchResult] = []
    @Published private(set) var searchError: String? = nil
    @Published private(set) var searchInFlight: Bool = false

    /// Hit GET /search?q={query} on the backend. Stores parsed hits
    /// in `searchHits`. The web client's snippets contain `<b>...</b>`
    /// for highlighted matches; we rewrite those to `**...**` so the
    /// existing bold-span rendering picks them up.
    func runSearch(_ query: String) async {
        let q = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !q.isEmpty else {
            searchHits = []
            searchError = nil
            searchInFlight = false
            return
        }
        switch currentBackend {
        case .mock:
            searchHits = search(q)
            searchError = nil
            return
        case .http(let baseURL):
            searchInFlight = true
            do {
                let encoded = q.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? q
                let hits: [APISearchHit] = try await httpGet("/search?q=\(encoded)", baseURL: baseURL)
                searchHits = hits.map(mapSearchHit)
                searchError = nil
            } catch {
                searchError = humanizeError(error, host: baseURL.host)
                searchHits = []
            }
            searchInFlight = false
        }
    }

    // MARK: - Pairing-code fetch

    /// Server-issued pairing code. The iPhone is the thin HTTP client,
    /// so the "QR you show to a third device" is the *server's* code,
    /// not anything iPhone-local. This wraps GET /sync/peer/pairing-code
    /// so PairDeviceView can render real handshake material.
    struct ServerPairingCode: Decodable, Equatable {
        let code: String                       // long base64url payload
        let display_name: String
        let device_id_hex: String
        let url: String
        let short_code: String                 // 6-char verifier
        let short_code_expires_in_secs: Int
    }

    func fetchPairingCode() async throws -> ServerPairingCode {
        guard case .http(let baseURL) = currentBackend else {
            throw URLError(.badURL)
        }
        return try await httpGet("/sync/peer/pairing-code", baseURL: baseURL)
    }

    // MARK: - Voice transcription

    /// Upload a WAV file to the server's /transcription/transcribe
    /// endpoint and return the transcribed text. The server holds
    /// the active model selection in `<mosaic>/.tesela/models/ACTIVE`.
    func transcribe(audio fileURL: URL) async throws -> String {
        guard case .http(let baseURL) = currentBackend else {
            throw URLError(.badURL)
        }
        let endpoint = endpoint("/transcription/transcribe", baseURL: baseURL)
        var req = URLRequest(url: endpoint)
        req.httpMethod = "POST"
        let boundary = "Boundary-\(UUID().uuidString)"
        req.setValue("multipart/form-data; boundary=\(boundary)", forHTTPHeaderField: "Content-Type")
        req.timeoutInterval = 120  // larger model + audio can take a while

        let data = try Data(contentsOf: fileURL)
        var body = Data()
        body.append("--\(boundary)\r\n".data(using: .utf8)!)
        body.append("Content-Disposition: form-data; name=\"audio\"; filename=\"recording.wav\"\r\n".data(using: .utf8)!)
        body.append("Content-Type: audio/wav\r\n\r\n".data(using: .utf8)!)
        body.append(data)
        body.append("\r\n--\(boundary)--\r\n".data(using: .utf8)!)
        let (responseData, response) = try await session.upload(for: req, from: body)
        try ensureOk(response, data: responseData)
        let decoded = try JSONDecoder().decode(APITranscribeResponse.self, from: responseData)
        return decoded.text
    }

    private struct APITranscribeResponse: Decodable {
        let text: String
        let model_id: String
        let duration_ms: Int
    }

    /// Convenience: take a recording, transcribe it, and append the
    /// resulting text as a new block on today's daily. Returns the
    /// transcript so the caller can surface it for confirmation.
    func captureVoiceNote(audio fileURL: URL) async throws -> String {
        let text = try await transcribe(audio: fileURL)
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty {
            capture(trimmed)
        }
        return trimmed
    }

    private struct APISearchHit: Decodable {
        let note_id: String
        let title: String
        let snippet: String
        let rank: Double
        let tags: [String]
        let path: String
    }

    private func mapSearchHit(_ hit: APISearchHit) -> SearchResult {
        // The server marks matches with <b>…</b>. Rewrite to **…**
        // so the existing markdown bold span rendering applies, and
        // strip bid comments while we're at it.
        var snippet = hit.snippet
            .replacingOccurrences(of: "<b>", with: "**")
            .replacingOccurrences(of: "</b>", with: "**")
        snippet = snippet.replacingOccurrences(
            of: #"<!--\s*bid:[^\s>]+\s*-->"#,
            with: "",
            options: .regularExpression
        )
        // Single-line snippets read better in the result list.
        snippet = snippet
            .replacingOccurrences(of: "\n", with: " ")
            .trimmingCharacters(in: .whitespaces)

        let kind: SearchResult.Kind = hit.tags.contains("daily") ? .page : .page
        return SearchResult(
            id: hit.note_id,
            kind: kind,
            title: hit.title,
            snippet: snippet
        )
    }

    /// Map a server `Link` into the iOS `Backlink` row model. Resolves
    /// the source note id to its page title when that page is known,
    /// and strips `<!-- bid:… -->` comments from the context line.
    private func mapBacklink(_ link: APILink) -> Backlink {
        let title = pages.first(where: { $0.id == link.target })?.title ?? link.target
        var snippet = link.text
            .replacingOccurrences(
                of: #"<!--\s*bid:[^\s>]+\s*-->"#,
                with: "",
                options: .regularExpression
            )
            .trimmingCharacters(in: .whitespaces)
        // The context line is a raw block — drop the leading `- ` bullet
        // so the snippet reads as prose.
        if snippet.hasPrefix("- ") {
            snippet.removeFirst(2)
        }
        return Backlink(id: UUID(), from: title, snippet: snippet, pageId: link.target)
    }

    /// Fetch a page's real body content and populate the cache.
    /// Idempotent — calling again while loading or ready is a no-op.
    /// Pass `force: true` to bust the cache (used on app foreground).
    func loadPage(id: String, force: Bool = false) async {
        if !force {
            switch pageLoadStates[id] {
            case .loading, .ready: return
            default: break
            }
        }
        pageLoadStates[id] = .loading
        switch currentBackend {
        case .mock:
            // Use the in-memory mock body if available; otherwise an
            // empty placeholder so the load state still resolves.
            if let page = pages.first(where: { $0.id == id }) {
                let blocks = page.body.enumerated().map { idx, line in
                    Block(id: "mock-\(id)-\(idx)", kind: .note, text: line)
                }
                loadedPageBlocks[id] = blocks
            } else {
                loadedPageBlocks[id] = []
            }
            loadedBacklinks[id] = MockSeed.backlinks
            loadedLinks[id] = []
            pageLoadStates[id] = .ready
        case .http(let baseURL):
            do {
                let note: APINote = try await httpGet("/notes/\(id)", baseURL: baseURL)
                loadedPageBlocks[id] = parseBlocks(from: note.body)
                loadedPageFrontmatter[id] = extractFrontmatter(from: note.content)
                pageLoadStates[id] = .ready
            } catch {
                pageLoadStates[id] = .failed(humanizeError(error, host: baseURL.host))
                return
            }
            // Backlinks load independently of the body — a fetch
            // failure here leaves an empty list rather than failing
            // the whole page.
            let links: [APILink] = (try? await httpGet("/notes/\(id)/backlinks", baseURL: baseURL)) ?? []
            loadedBacklinks[id] = links.map(mapBacklink)
            let outgoing: [APILink] = (try? await httpGet("/notes/\(id)/links", baseURL: baseURL)) ?? []
            loadedLinks[id] = outgoing.map(mapBacklink)
        }
    }

    /// Refresh every currently-loaded page from the server. Cheap when
    /// the cache is small; safe to call on app foreground.
    func refreshLoadedPages() async {
        let ids = Array(loadedPageBlocks.keys)
        for id in ids {
            await loadPage(id: id, force: true)
        }
    }

    /// Write a page's block list back to the server. Preserves the
    /// page's existing frontmatter from `loadedPageFrontmatter` so we
    /// don't stomp tags / title / status.
    func pushPage(id: String, blocks: [Block]) async {
        loadedPageBlocks[id] = blocks
        guard case .http(let baseURL) = currentBackend else { return }
        beginLocalWriteSuppression()
        let body = renderBody(from: blocks)
        let frontmatter = loadedPageFrontmatter[id] ?? "---\ntitle: \(id)\n---"
        let content = "\(frontmatter)\n\n\(body)\n"
        do {
            try await httpPut("/notes/\(id)", baseURL: baseURL, body: ["content": content])
        } catch {
            connection = .failed(humanizeError(error, host: baseURL.host))
        }
    }

    private func extractFrontmatter(from content: String) -> String {
        guard content.hasPrefix("---") else { return "---\n---" }
        let afterOpen = content.index(content.startIndex, offsetBy: 3)
        guard let closeRange = content.range(of: "\n---", options: [], range: afterOpen..<content.endIndex) else {
            return "---\n---"
        }
        return String(content[content.startIndex..<closeRange.upperBound])
    }

    // MARK: - HTTP refresh

    /// Pull pages, daily, and tags from a `tesela-server` and replace
    /// the in-memory snapshot. Resets to the built-in mock if the URL
    /// can't be reached.
    func refresh(from backend: Backend) async {
        switch backend {
        case .mock:
            // Reset to the seed mosaic so swapping back from HTTP is
            // clean.
            resetToSeed()
            connection = .idle
        case .http(let baseURL):
            connection = .connecting
            do {
                let daily: APINote = try await httpGet("/notes/daily", baseURL: baseURL)
                let notes: [APINote] = try await httpGet("/notes?limit=200", baseURL: baseURL)
                let yesterdayNote: APINote? = (try? await fetchYesterdayDaily(baseURL: baseURL))
                let serverTagNames: [String] = (try? await httpGet("/tags", baseURL: baseURL)) ?? []

                serverDailyId = daily.id
                todayBlocks = parseBlocks(from: daily.body)
                pages = notes
                    .filter { $0.id != daily.id }
                    .map { mapPage($0) }
                yesterdayBlocks = yesterdayNote.map { parseBlocks(from: $0.body) } ?? []
                tags = serverTagNames.map { name in
                    let parts = name.split(separator: "/")
                    let leaf = parts.last.map(String.init) ?? name
                    let parent = parts.count > 1 ? parts.dropLast().joined(separator: "/") : nil
                    return Tag(id: name, title: leaf, parent: parent, count: 0, recent: "today")
                }
                recent = pages.sorted(by: { $0.edited > $1.edited })
                    .prefix(8)
                    .map { RecentEntry(id: $0.id, title: $0.title, at: $0.edited) }
                connection = .ready
            } catch {
                connection = .failed(humanizeError(error, host: baseURL.host))
            }
        }
    }

    private var currentBackend: Backend = .mock

    func attach(backend: Backend) {
        currentBackend = backend
        if case .http = backend {
            // HTTP mode must never render the built-in `MockSeed`.
            // Clearing here means a slow or failing connect shows an
            // honest empty state instead of fake "old mosaic" data: a
            // successful `refresh` repopulates from the server, and a
            // failed one leaves the empty snapshot in place rather than
            // resetting to the seed.
            clearToEmpty()
        }
    }

    /// Drop every in-memory mosaic snapshot. Used when switching to an
    /// HTTP backend so the design-time `MockSeed` can't leak into a
    /// real-server session. Device-local `pinned` favorites survive —
    /// they are not server data. A foreground `refresh` that fails
    /// after a successful load keeps the real data (offline tolerance);
    /// only this explicit attach-time call wipes the snapshot.
    private func clearToEmpty() {
        pages = []
        tags = []
        recent = []
        todayBlocks = []
        yesterdayBlocks = []
        searchResults = []
        searchHits = []
        searchError = nil
        loadedBacklinks = [:]
        loadedLinks = [:]
        loadedPageBlocks = [:]
        loadedPageFrontmatter = [:]
        pageLoadStates = [:]
        serverDailyId = ""
    }

    // MARK: - Live sync (incoming)

    /// True while the user is actively editing a block's text. A live
    /// remote refresh is deferred while this holds so an incoming
    /// WebSocket event can't replace `todayBlocks` mid-edit; the
    /// deferred refresh runs the moment editing ends.
    var isEditingBlock: Bool = false {
        didSet {
            guard oldValue && !isEditingBlock, pendingRemoteRefresh else { return }
            pendingRemoteRefresh = false
            Task { await applyRemoteChange() }
        }
    }
    private var pendingRemoteRefresh = false

    /// A local write echoes straight back as a WebSocket event. Remote
    /// refreshes are skipped for a short window after a local write so
    /// the echo of our own edit can't revert a change made right after
    /// it (and not yet pushed). A genuine remote change in that window
    /// is deferred, not dropped — `pendingRemoteRefresh` flushes it.
    private var suppressRemoteUntil: Date?
    private var suppressionFlush: Task<Void, Never>?

    /// React to a server-side note change announced over the live-sync
    /// WebSocket: re-fetch the daily, the page list, and any open page.
    func applyRemoteChange() async {
        guard case .http = currentBackend else { return }
        if isEditingBlock {
            pendingRemoteRefresh = true
            return
        }
        if let until = suppressRemoteUntil, until > Date() {
            pendingRemoteRefresh = true
            scheduleSuppressionFlush(at: until)
            return
        }
        pendingRemoteRefresh = false
        await refresh(from: currentBackend)
        await refreshLoadedPages()
    }

    /// Open a 2s window during which remote refreshes are deferred —
    /// long enough to outlast the echo of a local write. Called from
    /// every local-write path.
    private func beginLocalWriteSuppression() {
        suppressRemoteUntil = Date().addingTimeInterval(2)
    }

    /// Ensure a single trailing refresh runs once the suppression
    /// window closes, so a remote change that arrived mid-window isn't
    /// lost.
    private func scheduleSuppressionFlush(at deadline: Date) {
        guard suppressionFlush == nil else { return }
        suppressionFlush = Task { [weak self] in
            let wait = deadline.timeIntervalSinceNow
            if wait > 0 {
                try? await Task.sleep(nanoseconds: UInt64(wait * 1_000_000_000))
            }
            guard let self else { return }
            self.suppressionFlush = nil
            if self.pendingRemoteRefresh {
                await self.applyRemoteChange()
            }
        }
    }

    // MARK: - Mosaic switching

    /// Make the server actually serve the mosaic at `path`. A no-op when
    /// it already is. Otherwise: persist the switch, restart the server,
    /// and hold `.switching` while it reboots (~2-3s) so the swap reads
    /// as intentional rather than a connection failure. The caller's
    /// `refresh()` then loads the new mosaic.
    func ensureServerMosaic(path: String, serverURL: String) async {
        let serving: String
        do {
            serving = try await MosaicServerClient.currentPath(serverURL: serverURL)
        } catch {
            // Can't read the current mosaic — leave the switch alone and
            // let the normal refresh surface any real connectivity issue.
            return
        }
        guard serving != path else { return }

        connection = .switching
        do {
            try await MosaicServerClient.switchMosaic(serverURL: serverURL, path: path)
        } catch {
            connection = .failed(humanizeError(error, host: URL(string: serverURL)?.host))
            return
        }
        // Best-effort: the server schedules its own SIGTERM, so a dropped
        // connection on this call still means it is restarting.
        try? await MosaicServerClient.restart(serverURL: serverURL)

        // Poll until the server is back on the new mosaic, holding
        // `.switching` throughout so the caller's refresh lands cleanly.
        for delay in [2.0, 2.0, 3.0, 4.0] {
            try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
            if (try? await MosaicServerClient.currentPath(serverURL: serverURL)) != nil {
                return
            }
        }
        // Gave up waiting — the caller's refresh will fail and the
        // standard auto-reconnect loop takes over from there.
    }

    // MARK: - Auto-reconnect

    /// Spin up a background loop that retries `refresh(from:)` on
    /// exponentially growing delays. Subsequent calls cancel the
    /// previous loop. The loop only runs while `connection == .failed`
    /// for an `.http` backend — mock mode never retries.
    private func startReconnectLoop() {
        guard case .http = currentBackend else { return }
        reconnectTask?.cancel()
        reconnectTask = Task { [weak self] in
            var delaySecs: UInt64 = 2
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: delaySecs * 1_000_000_000)
                if Task.isCancelled { return }
                guard let self else { return }
                // Don't fight a manual refresh that's already in-flight
                // or recovered.
                if case .failed = await self.connection {
                    await self.refresh(from: self.currentBackend)
                } else {
                    return
                }
                // Cap backoff at 60s. Each tick doubles up to that.
                delaySecs = min(delaySecs * 2, 60)
            }
        }
    }

    private func cancelReconnectLoop() {
        reconnectTask?.cancel()
        reconnectTask = nil
    }

    private func scheduleWriteback() {
        guard case .http(let baseURL) = currentBackend, !serverDailyId.isEmpty else {
            return
        }
        beginLocalWriteSuppression()
        let snapshot = todayBlocks
        Task { await pushTodayBlocks(snapshot, baseURL: baseURL) }
    }

    // MARK: - HTTP plumbing

    private struct APINote: Decodable {
        let id: String
        let title: String
        let content: String
        let body: String
        let metadata: APINoteMetadata
        let modified_at: String
    }

    private struct APINoteMetadata: Decodable {
        let title: String?
        let tags: [String]
        let note_type: String?
        let created: String?
        let modified: String?
    }

    /// `Link` JSON from `GET /notes/{id}/backlinks`. For backlinks the
    /// server sets `target` to the *source* note's id and `text` to the
    /// line of context; the other `Link` fields are unused here.
    private struct APILink: Decodable {
        let target: String
        let text: String
    }

    /// Build a request URL by concatenating `baseURL` and `path` as
    /// strings rather than `URL.appendingPathComponent`, which mangles
    /// query strings (`?` gets URL-encoded to `%3F`). Server endpoints
    /// like `/notes?limit=200` or `/notes/daily?date=...` need the raw
    /// concatenation behavior.
    private func endpoint(_ path: String, baseURL: URL) -> URL {
        let baseStr = baseURL.absoluteString
        let trimmedBase = baseStr.hasSuffix("/") ? String(baseStr.dropLast()) : baseStr
        let trimmedPath = path.hasPrefix("/") ? path : "/\(path)"
        return URL(string: trimmedBase + trimmedPath) ?? baseURL
    }

    private func httpGet<T: Decodable>(_ path: String, baseURL: URL) async throws -> T {
        var req = URLRequest(url: endpoint(path, baseURL: baseURL))
        req.timeoutInterval = 8
        let (data, response) = try await session.data(for: req)
        try ensureOk(response, data: data)
        return try JSONDecoder().decode(T.self, from: data)
    }

    private func httpPut(_ path: String, baseURL: URL, body: [String: Any]) async throws {
        let url = endpoint(path, baseURL: baseURL)
        var req = URLRequest(url: url)
        req.httpMethod = "PUT"
        req.timeoutInterval = 8
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try JSONSerialization.data(withJSONObject: body)
        let (data, response) = try await session.data(for: req)
        try ensureOk(response, data: data)
    }

    private func ensureOk(_ response: URLResponse, data: Data) throws {
        guard let http = response as? HTTPURLResponse else { return }
        guard (200..<300).contains(http.statusCode) else {
            let snippet = String(data: data.prefix(160), encoding: .utf8) ?? ""
            throw URLError(.badServerResponse, userInfo: [NSLocalizedDescriptionKey: "HTTP \(http.statusCode): \(snippet)"])
        }
    }

    private func humanizeError(_ error: Error, host: String?) -> String {
        let url = error as? URLError
        switch url?.code {
        case .some(.cannotConnectToHost): return "Couldn't reach \(host ?? "server")"
        case .some(.timedOut):            return "Server timed out"
        case .some(.notConnectedToInternet): return "Device is offline"
        default:                          return error.localizedDescription
        }
    }

    private func fetchYesterdayDaily(baseURL: URL) async throws -> APINote {
        let cal = Calendar.current
        guard let yesterday = cal.date(byAdding: .day, value: -1, to: todayDate) else {
            throw URLError(.badURL)
        }
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd"
        let dateStr = f.string(from: yesterday)
        return try await httpGet("/notes/daily?date=\(dateStr)", baseURL: baseURL)
    }

    // MARK: - Mapping

    private func mapPage(_ note: APINote) -> Page {
        Page(
            id: note.id,
            title: note.title,
            slug: note.id,
            type: inferType(from: note),
            edited: relativeEdit(from: note.modified_at),
            created: note.metadata.created.map { String($0.prefix(10)) } ?? "",
            blocks: countBlockLines(note.body),
            refs: 0,
            hidden: note.metadata.tags.contains("scratch"),
            body: previewLines(from: note.body)
        )
    }

    private func inferType(from note: APINote) -> String {
        if let raw = note.metadata.note_type, !raw.isEmpty {
            return raw.lowercased()
        }
        if note.metadata.tags.contains("daily") { return "daily" }
        return "note"
    }

    private func relativeEdit(from iso: String) -> String {
        guard let date = self.iso.date(from: iso) else { return iso }
        let interval = Date().timeIntervalSince(date)
        if interval < 60 { return "now" }
        if interval < 3600 { return "\(Int(interval / 60))m" }
        if interval < 86400 {
            let cal = Calendar.current
            return cal.isDateInToday(date) ? "today" : "\(Int(interval / 3600))h"
        }
        if Calendar.current.isDateInYesterday(date) { return "yesterday" }
        let days = Int(interval / 86400)
        return days < 7 ? "\(days)d" : "\(days / 7)w"
    }

    private func countBlockLines(_ body: String) -> Int {
        body.split(separator: "\n").filter { $0.trimmingCharacters(in: .whitespaces).hasPrefix("- ") }.count
    }

    private func previewLines(from body: String) -> [String] {
        body.split(separator: "\n", omittingEmptySubsequences: true)
            .prefix(4)
            .map { strip(blockMarkdown: String($0)) }
    }

    // MARK: - Block parsing and writeback

    /// Parses the body of a daily into iOS Blocks. Recognizes two task
    /// formats:
    ///   1. **Property-based** (web canonical) — `- text` followed by
    ///      indented `  status:: todo|done|…` and/or `  tags:: Task`
    ///      sub-lines.
    ///   2. **Markdown checkbox** (legacy / hand-typed) — `- [ ]` or
    ///      `- [x]`. Read for tolerance; writeback always uses format 1.
    ///
    /// Properties attached to a block are preserved so non-task keys
    /// (priority, due, etc.) round-trip cleanly.
    private func parseBlocks(from body: String) -> [Block] {
        let lines = body.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        var blocks: [Block] = []
        var i = 0
        // Normalized indent of the block parsed just before this one,
        // used to enforce the structural invariant below. -1 so the
        // first block clamps to 0.
        var previousIndent = -1
        while i < lines.count {
            let line = lines[i]
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard trimmed.hasPrefix("- ") else { i += 1; continue }

            let rawIndent = leadingSpaces(line) / 2
            // Structural invariant: a block is at most one level deeper
            // than the block before it — every block has an immediate
            // parent. Clamping here both rejects malformed input and
            // *repairs* existing files: a blank sub-block that crept
            // many levels deep collapses back to parent + 1 on load.
            let indent = min(rawIndent, previousIndent + 1)
            previousIndent = indent
            let parsed = parseBlockLine(line, indent: indent)

            // Collect the property sub-lines that follow this block
            // (physically indented deeper than the bullet, no `- `).
            var properties: [BlockProperty] = []
            i += 1
            while i < lines.count {
                let next = lines[i]
                let nextIndent = leadingSpaces(next) / 2
                let nextTrim = next.trimmingCharacters(in: .whitespaces)
                if nextTrim.hasPrefix("- ") { break }
                if nextIndent <= rawIndent && !nextTrim.isEmpty { break }
                if let prop = parseProperty(nextTrim) {
                    properties.append(prop)
                }
                i += 1
            }

            // Resolve task kind / done from properties + markdown.
            var kind = parsed.kind
            var done = parsed.done
            if let tagsProp = properties.first(where: { $0.key.lowercased() == "tags" }) {
                if tagsProp.value.lowercased().split(separator: ",").map({ $0.trimmingCharacters(in: .whitespaces).lowercased() }).contains("task") {
                    kind = .task
                }
            }
            if let statusProp = properties.first(where: { $0.key.lowercased() == "status" }) {
                let v = statusProp.value.lowercased()
                if v == "done" || v == "completed" {
                    kind = .task
                    done = true
                } else if v == "todo" || v == "doing" || v == "backlog" || v == "blocked" {
                    kind = .task
                }
            }

            blocks.append(Block(
                id: parsed.bid,
                kind: kind,
                text: parsed.text,
                done: done,
                indent: indent,
                tags: parsed.tags,
                properties: properties
            ))
        }
        return blocks
    }

    /// Parses one `- ` bullet line into (bid, text, tags, base kind).
    /// Kind detection here only looks at markdown checkbox syntax;
    /// property-based task detection happens in `parseBlocks` after
    /// sub-lines are collected.
    private func parseBlockLine(_ line: String, indent: Int) -> (bid: String, text: String, kind: BlockKind, done: Bool, tags: [String]) {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        let withoutBullet = String(trimmed.dropFirst(2))

        // Pull the bid out of the trailing HTML comment if present.
        let bidPattern = #"<!--\s*bid:([^\s>]+)\s*-->"#
        let visible: String
        let bid: String
        if let re = try? NSRegularExpression(pattern: bidPattern),
           let match = re.firstMatch(in: line, range: NSRange(location: 0, length: (line as NSString).length))
        {
            let ns = line as NSString
            bid = ns.substring(with: match.range(at: 1))
            let stripped = re.stringByReplacingMatches(
                in: withoutBullet,
                options: [],
                range: NSRange(location: 0, length: (withoutBullet as NSString).length),
                withTemplate: ""
            )
            visible = stripped
        } else {
            bid = UUID().uuidString
            visible = withoutBullet
        }

        let cleaned = visible.trimmingCharacters(in: .whitespaces)
        var kind: BlockKind = .note
        var done = false
        var text = cleaned
        if cleaned.hasPrefix("[ ]") {
            kind = .task
            text = String(cleaned.dropFirst(3)).trimmingCharacters(in: .whitespaces)
        } else if cleaned.hasPrefix("[x]") || cleaned.hasPrefix("[X]") {
            kind = .task
            done = true
            text = String(cleaned.dropFirst(3)).trimmingCharacters(in: .whitespaces)
        }

        let tags = trailingTagCluster(in: text)
        let bodyText = strip(trailingTags: tags, from: text)
        return (bid, bodyText, kind, done, tags)
    }

    /// Parses a `key:: value` line (already trimmed) into a property.
    /// Returns nil if the line doesn't match the property shape.
    private func parseProperty(_ line: String) -> BlockProperty? {
        guard let sep = line.range(of: "::") else { return nil }
        let key = String(line[..<sep.lowerBound]).trimmingCharacters(in: .whitespaces)
        let value = String(line[sep.upperBound...]).trimmingCharacters(in: .whitespaces)
        guard !key.isEmpty else { return nil }
        return BlockProperty(key: key, value: value)
    }

    /// Count of leading space characters. Used to derive a block's
    /// indent depth (`leadingSpaces / 2`). Must count *only* the
    /// leading run — an earlier version also folded in every other
    /// space on the line, so each parse→render round-trip inflated an
    /// indented block's depth, making blank sub-blocks visibly creep.
    private func leadingSpaces(_ s: String) -> Int {
        s.prefix(while: { $0 == " " }).count
    }

    private func trailingTagCluster(in text: String) -> [String] {
        var tokens = text.split(separator: " ")
        var tags: [String] = []
        while let last = tokens.last, last.hasPrefix("#") {
            tags.insert(String(last), at: 0)
            tokens.removeLast()
        }
        return tags
    }

    private func strip(trailingTags tags: [String], from text: String) -> String {
        guard !tags.isEmpty else { return text }
        var stripped = text
        for tag in tags.reversed() {
            if let r = stripped.range(of: tag, options: .backwards) {
                stripped.removeSubrange(r)
            }
        }
        return stripped.trimmingCharacters(in: .whitespaces)
    }

    private func strip(blockMarkdown line: String) -> String {
        var s = line.trimmingCharacters(in: .whitespaces)
        if s.hasPrefix("- ") { s.removeFirst(2) }
        if s.hasPrefix("[ ]") || s.hasPrefix("[x]") || s.hasPrefix("[X]") {
            s.removeFirst(3)
            s = s.trimmingCharacters(in: .whitespaces)
        }
        s = s.replacingOccurrences(
            of: #"<!--\s*bid:[^\s>]+\s*-->"#,
            with: "",
            options: .regularExpression
        )
        return s.trimmingCharacters(in: .whitespaces)
    }

    private func pushTodayBlocks(_ blocks: [Block], baseURL: URL) async {
        guard !serverDailyId.isEmpty else { return }
        do {
            let existing: APINote = try await httpGet("/notes/\(serverDailyId)", baseURL: baseURL)
            let newBody = renderBody(from: blocks)
            let content = combine(frontmatter: existing.content, body: newBody)
            try await httpPut("/notes/\(serverDailyId)", baseURL: baseURL, body: ["content": content])
        } catch {
            connection = .failed(humanizeError(error, host: baseURL.host))
        }
    }

    private func renderBody(from blocks: [Block]) -> String {
        var out: [String] = []
        for block in blocks {
            let indent = String(repeating: "  ", count: block.indent)
            let trailingTags = block.tags.isEmpty ? "" : " " + block.tags.joined(separator: " ")
            // Only emit a bid comment for ids that look like real UUIDs.
            // The Rust core's `stamp_block_ids` appends *new* bids
            // without removing existing ones, so emitting placeholder
            // bids (like `ios-…` or `captured-…`) leaves a stale
            // comment on disk that the web client renders verbatim.
            // Letting the server stamp these on save keeps the on-disk
            // form canonical and avoids the duplicate-bid leak.
            let bidSuffix = isCanonicalUUID(block.id) ? " <!-- bid:\(block.id) -->" : ""

            // Always use a plain `- ` bullet. Task state is expressed
            // via the canonical property format the web client expects:
            //   - text
            //     status:: todo|done
            //     tags:: Task
            // The legacy `- [ ]` / `- [x]` markdown is read for input
            // tolerance but never written back.
            out.append("\(indent)- \(block.text)\(trailingTags)\(bidSuffix)")

            let propLines = renderProperties(for: block, indent: indent)
            out.append(contentsOf: propLines)
        }
        return out.joined(separator: "\n")
    }

    /// Build the `key:: value` sub-lines for a block. Merges the
    /// block's existing `properties` with task-derived properties
    /// (`status::`, `tags::`) so a toggle updates the canonical
    /// representation without dropping user-set keys (priority, due,
    /// etc.).
    private func renderProperties(for block: Block, indent: String) -> [String] {
        var merged = block.properties

        // Update or insert status:: and tags:: for task blocks.
        if block.kind == .task {
            upsert(&merged, key: "status", value: block.done ? "done" : "todo")
            if !merged.contains(where: { $0.key.lowercased() == "tags" }) {
                merged.append(BlockProperty(key: "tags", value: "Task"))
            }
        } else {
            // Non-task block: strip any inherited task-state properties
            // so converting a task → note doesn't leave stale state.
            merged.removeAll {
                $0.key.lowercased() == "status" ||
                ($0.key.lowercased() == "tags" && $0.value.lowercased() == "task")
            }
        }

        return merged.map { "\(indent)  \($0.key):: \($0.value)" }
    }

    private func upsert(_ props: inout [BlockProperty], key: String, value: String) {
        if let idx = props.firstIndex(where: { $0.key.lowercased() == key.lowercased() }) {
            props[idx].value = value
        } else {
            props.append(BlockProperty(key: key, value: value))
        }
    }

    /// True iff `id` matches the 8-4-4-4-12 hex UUID shape (v4 or v7).
    private func isCanonicalUUID(_ id: String) -> Bool {
        let pattern = #"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"#
        return id.range(of: pattern, options: .regularExpression) != nil
    }

    private func combine(frontmatter original: String, body: String) -> String {
        guard original.hasPrefix("---") else { return body }
        let openLen = original.distance(from: original.startIndex, to: original.index(original.startIndex, offsetBy: 3))
        let afterOpen = original.index(original.startIndex, offsetBy: openLen)
        guard let closeRange = original.range(of: "\n---", options: [], range: afterOpen..<original.endIndex) else {
            return body
        }
        let frontmatter = original[original.startIndex..<closeRange.upperBound]
        return "\(frontmatter)\n\n\(body)\n"
    }

    // MARK: - Pinned pages (local favorites)

    /// Pinned pages are device-local favorites — there is no server
    /// "favorites" concept — persisted to `UserDefaults` as JSON so the
    /// list survives relaunch and backend swaps.
    private static let pinnedDefaultsKey = "pinned.pages"

    private static func loadPinned() -> [PinnedEntry] {
        guard let data = UserDefaults.standard.data(forKey: pinnedDefaultsKey),
              let decoded = try? JSONDecoder().decode([PinnedEntry].self, from: data)
        else { return [] }
        return decoded
    }

    private func persistPinned() {
        guard let data = try? JSONEncoder().encode(pinned) else { return }
        UserDefaults.standard.set(data, forKey: Self.pinnedDefaultsKey)
    }

    func isPinned(_ id: String) -> Bool {
        pinned.contains { $0.id == id }
    }

    /// Pin or unpin a page. Idempotent per page id.
    func togglePin(page: Page) {
        if let idx = pinned.firstIndex(where: { $0.id == page.id }) {
            pinned.remove(at: idx)
        } else {
            pinned.append(PinnedEntry(id: page.id, title: page.title))
        }
        persistPinned()
    }

    private func resetToSeed() {
        pages = MockSeed.pages
        tags = MockSeed.tags
        recent = MockSeed.recent
        todayBlocks = MockSeed.todayBlocks
        yesterdayBlocks = MockSeed.yesterdayBlocks
        palette = MockSeed.palette
        searchResults = MockSeed.searchResults
        serverDailyId = ""
    }
}

// MARK: - Seed snapshot

/// Realistic in-memory mosaic mirroring the design canvas's `data.jsx`.
/// Used when the backend is set to `.mock` or when HTTP refresh fails.
enum MockSeed {
    static let todayBlocks: [Block] = [
        Block(id: "t0", kind: .task,
              text: "Sketch the iPhone front door — what does the daily look like when you launch?",
              done: false, tags: ["#design", "#tesela/ios"]),
        Block(id: "t1", kind: .task,
              text: "Decide tab structure with Taylor",
              done: true, tags: ["#tesela/ios"]),
        Block(id: "t2", kind: .note,
              text: "Idea: peek-as-segmented sits flush under the page title — keeps backlinks one tap away without a sheet.",
              tags: []),
        Block(id: "t3", kind: .note,
              text: "Read [[Prism v5 chrome]] sections on derived buffers — the host-agnostic renderer contract is what makes iOS Peek even feasible.",
              tags: ["#prism"]),
        Block(id: "t4", kind: .note,
              text: "Cold mornings, hot espresso. 11° at the kitchen window.",
              tags: ["#weather"]),
        Block(id: "t5", kind: .task,
              text: "Reply to Maya re: tag chip rendering",
              done: false, tags: ["#followup"]),
        Block(id: "t6", kind: .note,
              text: "Trailing-cluster rule is so good — markdown stays portable; chip-ness is *positional*, not metadata.",
              tags: ["#tags"]),
    ]

    static let yesterdayBlocks: [Block] = [
        Block(id: "y0", kind: .task,
              text: "Print and tape the Tokyo Night swatches above the monitor",
              done: true, tags: []),
        Block(id: "y1", kind: .note,
              text: "[[Maya]] — voice memo on Logseq vs Tesela density. She wants block-density not chrome-density. Agreed.",
              tags: ["#followup"]),
        Block(id: "y2", kind: .note,
              text: "Bird at the feeder around 7:14 — looks like a cardinal but the back is muddier.",
              tags: ["#nature/birds"]),
    ]

    static let pages: [Page] = [
        Page(id: "prism-v5-chrome", title: "Prism v5 chrome",
             slug: "prism-v5-chrome", type: "note", edited: "today",
             blocks: 38, refs: 12,
             body: [
                "The chrome replaces v4's five-pane grab-bag with a tightly-typed buffer set: **page**, **derived**, **ambient**.",
                "**Invariant.** Page buffers render exactly one filesystem-backed page. Derived buffers are pure functions of a reference. Ambient buffers are workspace singletons.",
                "Renderers are host-agnostic — Peek and pane mount the same renderer with no host knowledge.",
                "On iOS the binary pane tree collapses to one focused page at a time. Peek does the lifting.",
             ]),
        Page(id: "tag-system",         title: "Tag system",
             slug: "tag-system", type: "note", edited: "yesterday",
             blocks: 27, refs: 8),
        Page(id: "ios-design-brief",   title: "iPhone design brief",
             slug: "ios-design-brief", type: "note", edited: "2d",
             blocks: 19, refs: 5),
        Page(id: "maya-conversations", title: "Maya · conversations",
             slug: "maya-conversations", type: "person", edited: "3d",
             blocks: 64, refs: 22),
        Page(id: "tesela-ios",         title: "Tesela iOS",
             slug: "tesela-ios", type: "project", edited: "today",
             blocks: 41, refs: 18),
        Page(id: "open-tasks",         title: "Open tasks",
             slug: "open-tasks", type: "query", edited: "live",
             blocks: 12, refs: 0,
             query: "type:task AND status:open AND assignee:me"),
        Page(id: "scratch-2026-05-15-1423",
             title: "scratch · 2026-05-15-1423",
             slug: "scratch-2026-05-15-1423", type: "scratch",
             edited: "2d", blocks: 4, refs: 0, hidden: true),
        Page(id: "cold-press-recipes", title: "Cold press recipes",
             slug: "cold-press-recipes", type: "note", edited: "1w",
             blocks: 11, refs: 3),
        Page(id: "weekly-review-template", title: "Weekly review",
             slug: "weekly-review-template", type: "template",
             edited: "1w", blocks: 15, refs: 4),
        Page(id: "kc-meetup",          title: "KC meetup · May",
             slug: "kc-meetup", type: "event", edited: "1w",
             blocks: 6, refs: 2),
    ]

    static let tags: [Tag] = [
        Tag(id: "design",       title: "design",   parent: nil,      count: 47, recent: "today"),
        Tag(id: "tesela",       title: "tesela",   parent: nil,      count: 124, recent: "today"),
        Tag(id: "tesela-ios",   title: "ios",      parent: "tesela", count: 31, recent: "today"),
        Tag(id: "tesela-sync",  title: "sync",     parent: "tesela", count: 18, recent: "yesterday"),
        Tag(id: "prism",        title: "prism",    parent: nil,      count: 22, recent: "today"),
        Tag(id: "tags",         title: "tags",     parent: nil,      count: 14, recent: "today"),
        Tag(id: "nature",       title: "nature",   parent: nil,      count:  9, recent: "yesterday"),
        Tag(id: "nature-birds", title: "birds",    parent: "nature", count:  6, recent: "yesterday"),
        Tag(id: "followup",     title: "followup", parent: nil,      count: 11, recent: "today"),
        Tag(id: "weather",      title: "weather",  parent: nil,      count: 31, recent: "today"),
    ]

    static let recent: [RecentEntry] = [
        RecentEntry(id: "prism-v5-chrome",   title: "Prism v5 chrome",       at: "12m"),
        RecentEntry(id: "tag-system",        title: "Tag system",            at: "1h"),
        RecentEntry(id: "tesela-ios",        title: "Tesela iOS",            at: "3h"),
        RecentEntry(id: "maya-conversations", title: "Maya · conversations", at: "yesterday"),
    ]

    static let palette: [PaletteVerb] = [
        PaletteVerb(id: ":daily",          hint: "Open today's daily"),
        PaletteVerb(id: ":scratch",        hint: "Start a scratch page"),
        PaletteVerb(id: ":promote",        hint: "Promote scratch → note"),
        PaletteVerb(id: ":rename-slug",    hint: "Rename current page's slug"),
        PaletteVerb(id: ":convert-to-tag", hint: "Convert current note → tag"),
        PaletteVerb(id: ":sync now",       hint: "Sync once with reachable peers"),
        PaletteVerb(id: ":graph",          hint: "Open workspace graph"),
    ]

    static let searchResults: [SearchResult] = [
        SearchResult(id: "r0", kind: .page, title: "Prism v5 chrome",
            snippet: "...the chrome replaces v4's five-pane grab-bag with a tightly-typed **buffer** set..."),
        SearchResult(id: "r1", kind: .block, title: "Today",
            snippet: "Read [[Prism v5 chrome]] sections on derived **buffer**s..."),
        SearchResult(id: "r2", kind: .page, title: "Tesela iOS",
            snippet: "...the iPhone collapses the binary pane tree into one focused **buffer** at a time..."),
        SearchResult(id: "r3", kind: .tag, title: "prism",
            snippet: "...22 references across 14 pages — most-recent ones cluster around the v5 **buffer** cutover..."),
    ]

    static let backlinks: [Backlink] = [
        Backlink(id: UUID(), from: "Tesela iOS",
            snippet: "...the Peek surface lifts directly from [[Prism v5 chrome]]'s host-agnostic renderer..."),
        Backlink(id: UUID(), from: "iPhone design brief",
            snippet: "...read [[Prism v5 chrome]] end-to-end. It locks the platform/interaction decisions..."),
        Backlink(id: UUID(), from: "2026-05-17",
            snippet: "Read [[Prism v5 chrome]] sections on derived buffers..."),
        Backlink(id: UUID(), from: "Maya · conversations",
            snippet: "Maya: 'the [[Prism v5 chrome]] doc reads like a contract'..."),
    ]

}
