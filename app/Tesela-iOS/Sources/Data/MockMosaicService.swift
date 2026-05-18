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
    @Published private(set) var backlinks: [Backlink]
    @Published private(set) var outline: [OutlineEntry]

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
        case ready
        case failed(String)
    }
    @Published var connection: ConnectionState = .idle

    private let session = URLSession(configuration: .default)
    private let iso = ISO8601DateFormatter()
    private var serverDailyId: String = ""

    init() {
        let today = Date()
        self.todayDate = today
        self.pages = MockSeed.pages
        self.tags = MockSeed.tags
        self.recent = MockSeed.recent
        self.pinned = MockSeed.pinned
        self.todayBlocks = MockSeed.todayBlocks
        self.yesterdayBlocks = MockSeed.yesterdayBlocks
        self.palette = MockSeed.palette
        self.searchResults = MockSeed.searchResults
        self.backlinks = MockSeed.backlinks
        self.outline = MockSeed.outline
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

    func capture(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        let block = Block(
            id: "captured-\(UUID().uuidString.prefix(12).lowercased())",
            kind: .note,
            text: trimmed
        )
        todayBlocks.insert(block, at: 0)
        scheduleWriteback()
    }

    func search(_ query: String) -> [SearchResult] {
        let q = query.lowercased().trimmingCharacters(in: .whitespaces)
        guard !q.isEmpty else { return searchResults }
        return searchResults.filter {
            $0.title.lowercased().contains(q) || $0.snippet.lowercased().contains(q)
        }
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
    }

    private func scheduleWriteback() {
        guard case .http(let baseURL) = currentBackend, !serverDailyId.isEmpty else {
            return
        }
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

    private func parseBlocks(from body: String) -> [Block] {
        var blocks: [Block] = []
        for rawLine in body.split(separator: "\n", omittingEmptySubsequences: false) {
            let line = String(rawLine)
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard trimmed.hasPrefix("- ") else { continue }

            let indent = leadingSpaces(line) / 2
            let withoutBullet = String(trimmed.dropFirst(2))

            // Pull the bid out of the trailing HTML comment if it's there
            // UUIDs have dashes, so the capture group must allow them.
            // The previous `[^\s-]+` exclusion truncated bid values at
            // the first hyphen.
            // Accept any non-whitespace, non-`>` bid value so blocks
            // captured before they hit the server (with a placeholder
            // bid) parse cleanly. The server rewrites placeholder bids
            // to UUIDv7 on next save anyway.
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

            blocks.append(Block(
                id: bid,
                kind: kind,
                text: bodyText,
                done: done,
                indent: indent,
                tags: tags
            ))
        }
        return blocks
    }

    private func leadingSpaces(_ s: String) -> Int {
        var n = 0
        for ch in s where ch == " " { n += 1 }
        return n - (s.drop(while: { $0 == " " }).count == s.count ? n : 0) + s.prefix(while: { $0 == " " }).count
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
        blocks.map { block in
            let indent = String(repeating: "  ", count: block.indent)
            let bullet: String
            switch block.kind {
            case .task: bullet = block.done ? "- [x]" : "- [ ]"
            default:    bullet = "-"
            }
            let trailingTags = block.tags.isEmpty ? "" : " " + block.tags.joined(separator: " ")
            return "\(indent)\(bullet) \(block.text)\(trailingTags) <!-- bid:\(block.id) -->"
        }
        .joined(separator: "\n")
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

    private func resetToSeed() {
        pages = MockSeed.pages
        tags = MockSeed.tags
        recent = MockSeed.recent
        pinned = MockSeed.pinned
        todayBlocks = MockSeed.todayBlocks
        yesterdayBlocks = MockSeed.yesterdayBlocks
        palette = MockSeed.palette
        searchResults = MockSeed.searchResults
        backlinks = MockSeed.backlinks
        outline = MockSeed.outline
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

    static let pinned: [PinnedEntry] = [
        PinnedEntry(id: "tesela-ios",       title: "Tesela iOS"),
        PinnedEntry(id: "open-tasks",       title: "Open tasks"),
        PinnedEntry(id: "ios-design-brief", title: "iPhone design brief"),
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

    static let outline: [OutlineEntry] = [
        OutlineEntry(id: UUID(), depth: 0, text: "Context"),
        OutlineEntry(id: UUID(), depth: 0, text: "Regions"),
        OutlineEntry(id: UUID(), depth: 1, text: "Top bar"),
        OutlineEntry(id: UUID(), depth: 1, text: "Left sidebar"),
        OutlineEntry(id: UUID(), depth: 1, text: "Main pane tree"),
        OutlineEntry(id: UUID(), depth: 0, text: "Three buffer kinds"),
        OutlineEntry(id: UUID(), depth: 1, text: "Page buffer"),
        OutlineEntry(id: UUID(), depth: 1, text: "Derived buffer"),
        OutlineEntry(id: UUID(), depth: 1, text: "Ambient buffer"),
        OutlineEntry(id: UUID(), depth: 0, text: "Renderer protocol"),
        OutlineEntry(id: UUID(), depth: 0, text: "Focus rules"),
    ]
}
