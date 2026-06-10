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

    /// Daily notes older than yesterday, newest first — rendered as
    /// dimmed, display-only sections below Yesterday in the Daily feed.
    /// Capped to a recent window; days with no blocks are dropped.
    @Published private(set) var pastDailies: [DailyEntry] = []

    /// How many past-day sections (older than yesterday) the Daily feed
    /// currently shows. Starts at one week; `loadOlderDailies()` widens
    /// it by a week each time the user reaches the bottom of the feed.
    private var pastDailiesWindow: Int = 7

    /// Whether older dailies likely exist beyond the current window —
    /// drives the Daily feed's load-more sentinel. `false` once a load
    /// exhausts the local mosaic (relay) / the server's daily list.
    @Published private(set) var hasOlderDailies: Bool = true

    @Published private(set) var palette: [PaletteVerb]
    @Published private(set) var searchResults: [SearchResult]

    /// Bumped after every non-mock `refresh(from:)` pass — including the
    /// relay-tick path (`onAppliedChanges → applyRemoteChange → refresh`).
    /// Views whose data lives in their own query-backed `@State` (Agenda,
    /// Inbox) observe this to re-run their load when new ops land; the
    /// Daily doesn't need it because it renders `todayBlocks` directly.
    @Published private(set) var refreshTick: Int = 0

    /// Bumped when the saved-views registry changed server-side (`.http`
    /// mode: the `views_changed` WS event, wired in the shell). The Inbox
    /// tab observes this to re-read the view switcher without a full note
    /// refresh. `.relay` doesn't need it — the registry doc arrives via
    /// the relay tick, whose apply already bumps `refreshTick`.
    @Published private(set) var viewsTick: Int = 0

    /// Signal a server-side views-registry change (the `views_changed`
    /// WS event). Main-actor; just bumps `viewsTick`.
    func noteViewsChanged() {
        viewsTick += 1
    }

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
        /// Local-first relay mode: read the on-device engine's relay-synced
        /// materialized notes (`localMosaicRoot()/notes`) with NO Mac HTTP; the
        /// RelayTicker syncs in the background. Reads reuse the same local
        /// helpers `.http` uses for its local-first render
        /// (`applyLocalRefreshFallback` / `readLocalNote` / `localSearch`).
        case relay
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
    /// Cached frontmatter for today's daily, kept in sync with whatever
    /// the last successful HTTP refresh (or local-hydration pass) saw.
    /// Reused by `pushTodayBlocks` so an offline writeback doesn't need
    /// to fetch the existing frontmatter over HTTP (5-second timeout)
    /// before the engine path can fire. Without this cache, the engine
    /// write was being blocked behind the HTTP probe — meaning if the
    /// user kept typing or force-closed the app within the timeout
    /// window, the edit never reached SQLite.
    private var loadedDailyFrontmatter: String? = nil

    /// Wall-clock of the last successful in-memory hydration per note,
    /// used by the local-fallback path to decide whether the on-disk
    /// sandbox copy is newer than what we're rendering. Without this,
    /// "keep in-memory on HTTP fail" would mask Mac edits that the
    /// RelayTicker has already applied to the local file.
    private var inMemoryLoadedAt: [String: Date] = [:]
    /// Same idea for today's daily, which has its own field.
    private var todayLoadedAt: Date? = nil

    /// Callback fired whenever an iOS-authored edit needs to be
    /// durably persisted to the sync layer. Wired in AppShell to
    /// `relayTicker.recordAndPush(...)` so the edit goes into the
    /// local sync engine AND gets pushed through the relay alongside
    /// the existing HTTP PUT path. Without this, an iOS edit on
    /// cellular (where Mac is unreachable for direct HTTP) would
    /// be stuck in iOS memory until Mac came back into HTTP reach.
    ///
    /// Args: (slug, title, fullContent, createdAtMillis).
    var onLocalWrite: ((String, String, String, Int64) -> Void)? = nil

    /// Collab editing C1 outbound: fired for a single in-block CHARACTER
    /// SPLICE (one keystroke) instead of a whole-text re-author. Wired in
    /// `GrAppShell` to `relayTicker.spliceAndPush(...)`, which applies the
    /// splice to the block's per-block Loro `LoroText` (`text_seq`) and
    /// pushes the resulting delta. Because the CRDT merges splices, a
    /// peer's concurrent same-block edit is no longer clobbered (the old
    /// whole-text path emitted DELETEs of the peer's characters).
    /// Args: (slug, blockIdHex, utf16Offset, utf16DeleteLen, insert).
    var onLocalSplice: ((String, String, Int, Int, String) -> Void)? = nil

    /// P1.11 relay-mode property write: fired when a `.relay` triage /
    /// mark-done / reschedule needs to reach the on-device engine. Wired in
    /// both shells to `relayTicker.setBlockPropertyAndPush(...)` (the FFI
    /// `setBlockProperty` → typed `BlockPropertySet` op → materialize +
    /// relay/WS push). ASYNC + result-bearing — unlike `onLocalWrite` — so
    /// `setBlockProperty` can read the just-materialized file after the
    /// write and THROW on a not-found bid instead of letting the row
    /// optimistically vanish over a silent no-op.
    /// Args: (slug, bidHex, key, value) → true when the engine recorded it.
    var onLocalPropertySet:
        ((_ slug: String, _ bidHex: String, _ key: String, _ value: String) async -> Bool)? = nil

    /// Relay-mode whole-note write used by `saveInboxDsl` (the saved-filter
    /// Query note). Same `recordAndPush` + live-WS tail as `onLocalWrite`,
    /// but AWAITABLE so the caller's immediate reload (`load()` right after
    /// the save) reads the re-materialized file instead of racing it.
    /// Args: (slug, title, fullContent, createdAtMillis).
    var onLocalNoteWrite:
        ((_ slug: String, _ title: String, _ content: String, _ createdAtMillis: Int64) async -> Void)? = nil

    /// Saved-views registry seams (saved-views spec, 2026-06-10) — the
    /// `.relay` analog of the server's `/views` routes. Wired in
    /// `GrAppShell` to `RelayTicker.viewsList()` /
    /// `viewsUpsertAndPush(_:)` / `viewsDeleteAndPush(viewId:)` so the
    /// Inbox tab's view switcher reads/writes the engine's synced
    /// registry doc. List returns nil when the engine can't open (the
    /// caller falls back to the builtin Inbox); the writes THROW on
    /// rejection so the editor never pretends a save landed.
    var onViewsList: (() async -> [SavedView]?)? = nil
    var onViewsUpsert: ((_ view: SavedView) async throws -> Void)? = nil
    var onViewsDelete: ((_ viewId: String) async throws -> Void)? = nil

    /// Callback fired whenever a note becomes visible/loaded (the daily
    /// on refresh, any opened page). Wired in both shells to
    /// `relayTicker.bootstrapNoteIfNeeded(slug:)` so a device that's only
    /// RECEIVING imports the server's note doc as a current base — not
    /// just when it first AUTHORS an edit (delivery-layer redesign
    /// 2026-05-31, T2). Without this, a receive-only device never holds
    /// the base, so partial WS deltas can't materialize (they stay
    /// pending) and the device never produces converging pushes.
    /// `bootstrapNoteIfNeeded` is idempotent (resident-check), so firing
    /// on every open is safe-but-cheap. Arg: the note slug.
    var onNoteOpened: ((String) -> Void)? = nil

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
            let done = todayBlocks[idx].done
            // Keep the in-memory property mirror consistent with the flip
            // so a later whole-note writeback renders the same status.
            upsert(&todayBlocks[idx].properties, key: "status", value: Self.taskStatusValue(done: done))
            let slug = serverDailyId.isEmpty ? dailyId(daysAgo: 0) : serverDailyId
            persistTaskToggle(noteId: slug, bid: id, done: done) { [weak self] in
                self?.scheduleWriteback()
            }
        } else if let idx = yesterdayBlocks.firstIndex(where: { $0.id == id }), yesterdayBlocks[idx].kind == .task {
            yesterdayBlocks[idx].done.toggle()
            let done = yesterdayBlocks[idx].done
            upsert(&yesterdayBlocks[idx].properties, key: "status", value: Self.taskStatusValue(done: done))
            persistTaskToggle(noteId: yesterdayId, bid: id, done: done) { [weak self] in
                self?.scheduleYesterdayWriteback()
            }
        }
    }

    /// Canonical `status::` value for a task's done flag.
    static func taskStatusValue(done: Bool) -> String { done ? "done" : "todo" }

    /// Persist a checkbox toggle as a TYPED `status::` property write —
    /// the engine container op in `.relay`, `POST /blocks/set-property`
    /// in `.http` — NOT the whole-note writeback.
    ///
    /// The writeback path silently REVERTED toggles (2026-06-10 product
    /// test): when a block's `status` lives in the engine's property
    /// CONTAINER (any task touched by web triage / NL-lift / iOS triage),
    /// the whole-note diff records the flipped `status:: done` line into
    /// the block's TEXT, but the materializer's container-wins dedup
    /// (A4) drops any in-text line whose key matches a container prop —
    /// so the file re-renders `status:: todo` from the untouched
    /// container and the next refresh flips the checkbox back. Only a
    /// property op updates the container. The writeback `fallback` runs
    /// when the bid isn't in the engine yet (a fresh, not-yet-
    /// materialized block — which also has no container prop to shadow
    /// the in-text line, so the legacy path is correct there).
    private func persistTaskToggle(
        noteId: String,
        bid: String,
        done: Bool,
        fallback: @escaping @MainActor () -> Void
    ) {
        let status = Self.taskStatusValue(done: done)
        switch currentBackend {
        case .mock:
            return
        case .http(let baseURL):
            beginLocalWriteSuppression()
            Task {
                let body = APISetBlockPropertyBody(
                    block_id: "\(noteId):\(bid)", key: "status", value: status
                )
                do {
                    try await httpPostNoResponse("/blocks/set-property", baseURL: baseURL, body: body)
                } catch {
                    fallback()
                }
            }
        case .relay:
            beginLocalWriteSuppression()
            Task {
                let applied = await onLocalPropertySet?(noteId, bid, "status", status) ?? false
                if applied {
                    // The engine re-materialized the file before the seam
                    // returned — re-read so the UI reflects the durable
                    // state (and refreshTick nudges Agenda/Inbox).
                    await refresh(from: currentBackend)
                } else {
                    fallback()
                }
            }
        }
    }

    // MARK: - Yesterday edits (Phase 2.2)
    //
    // Yesterday's daily was display-only until 2026-05-27 — Daisy
    // reported clicks did nothing. The note IS just a regular markdown
    // note on disk (`YYYY-MM-DD.md` for yesterday's date), so we can
    // edit it via the same engine path that powers `pushPage`. The only
    // local-state distinction from today is which array holds the
    // blocks (`yesterdayBlocks` vs `todayBlocks`); both flush via the
    // shared engine.

    /// Slug of yesterday's daily — `YYYY-MM-DD` of (today - 1 day).
    /// Computed each call from `todayDate`, so this naturally rolls
    /// over at midnight.
    private var yesterdayId: String { dailyId(daysAgo: 1) }

    /// Mirror of `editTodayBlock` for yesterday's daily.
    func editYesterdayBlock(id: String, text: String) {
        guard let idx = yesterdayBlocks.firstIndex(where: { $0.id == id }) else { return }
        let (body, tags) = Self.splitInlineTags(text)
        yesterdayBlocks[idx].text = body.components(separatedBy: "\n").first ?? body
        yesterdayBlocks[idx].rawText = body
        yesterdayBlocks[idx].tags = tags
        scheduleYesterdayWriteback()
    }

    /// Append a fresh empty block to yesterday's daily. Returns the new
    /// block's id so the caller can flip it into edit mode.
    @discardableResult
    func appendYesterdayBlock(kind: BlockKind = .note) -> String {
        let id = UUID().uuidString.lowercased()
        yesterdayBlocks.append(Block(id: id, kind: kind, text: ""))
        scheduleYesterdayWriteback()
        return id
    }

    /// Delete a block from yesterday's daily.
    func deleteYesterdayBlock(id: String) {
        yesterdayBlocks.removeAll { $0.id == id }
        scheduleYesterdayWriteback()
    }

    /// Indent / outdent a yesterday block; clamps to the structural
    /// invariant (`[0, prev + 1]`) the today path enforces.
    func indentYesterdayBlock(id: String, by delta: Int) {
        guard let idx = yesterdayBlocks.firstIndex(where: { $0.id == id }) else { return }
        let maxIndent = idx > 0 ? yesterdayBlocks[idx - 1].indent + 1 : 0
        yesterdayBlocks[idx].indent = max(0, min(maxIndent, yesterdayBlocks[idx].indent + delta))
        scheduleYesterdayWriteback()
    }

    /// Mirror of `scheduleWriteback` for yesterday's daily. Routes
    /// through `pushPage` since yesterday isn't tracked under
    /// `serverDailyId`.
    private func scheduleYesterdayWriteback() {
        let slug = yesterdayId
        guard !slug.isEmpty else { return }
        let snapshot = yesterdayBlocks
        Task { await pushPage(id: slug, blocks: snapshot) }
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
        // `text` is the first line (used by previews/grep); `rawText`
        // carries the full multi-line body so continuation lines
        // survive the next writeback. They're equal for the common
        // single-line case.
        todayBlocks[idx].text = body.components(separatedBy: "\n").first ?? body
        todayBlocks[idx].rawText = body
        todayBlocks[idx].tags = tags
        scheduleWriteback()
    }

    /// Collab editing C1 outbound: apply ONE character splice (the user's
    /// actual keystroke) to a today block, routed to the engine's
    /// per-block `LoroText` (`text_seq`) so a concurrent same-block edit
    /// merges instead of being clobbered. Does NOT run the whole-text
    /// writeback (`scheduleWriteback`/`editTodayBlock`) — that path
    /// re-authors the block and emits DELETEs of a peer's chars, and its
    /// space-collapsing / tag-restructuring normalization would diverge
    /// the engine text from the editor and misalign future splice offsets.
    ///
    /// The in-memory model is updated by splicing the ENGINE-EXACT block
    /// text (body with inline tags — the materialized line's visible
    /// content) and re-deriving `text`/`rawText`/`tags` for display. The
    /// re-derivation is a display projection only, NOT a storage
    /// transformation: the engine receives the raw UTF-16 splice.
    ///
    /// `utf16Offset` / `utf16DeleteLen` are UTF-16 code units (the
    /// editor's native `NSRange`), exactly what `spliceBlockText` wants.
    func spliceTodayBlock(id: String, utf16Offset: Int, utf16DeleteLen: Int, insert: String) {
        guard let idx = todayBlocks.firstIndex(where: { $0.id == id }) else { return }
        // A local keystroke changes the editor text; bump so an inbound
        // live-reconcile whose async read overlaps this edit can detect the
        // race and skip its now-stale merged text (C1-inbound).
        localSpliceSeq &+= 1
        // Reconstruct the engine-exact string (the editor's loaded value),
        // apply the SAME UTF-16 splice the editor just applied to it on an
        // NSString (UTF-16 semantics, matching the engine + NSRange), and
        // keep the result FAITHFUL — no space-collapsing or tag-stripping
        // normalization, which would diverge the in-memory text from the
        // editor + engine and misalign future splice offsets. Tags are
        // re-DERIVED for the chip display only (a projection, not a
        // storage transform).
        let before = Self.engineExactText(of: todayBlocks[idx])
        let ns = NSMutableString(string: before)
        let len = ns.length
        let clampedOffset = max(0, min(utf16Offset, len))
        let clampedDelete = max(0, min(utf16DeleteLen, len - clampedOffset))
        ns.replaceCharacters(
            in: NSRange(location: clampedOffset, length: clampedDelete),
            with: insert
        )
        let after = ns as String
        // Faithful display update: split off ONLY the trailing tag cluster
        // (matching `renderBody`/the parser's trailing-tag convention)
        // without collapsing interior spacing, so the next reconstruction
        // round-trips to the same string.
        let (body, tags) = Self.splitTrailingTags(after)
        todayBlocks[idx].text = body.components(separatedBy: "\n").first ?? body
        todayBlocks[idx].rawText = body
        todayBlocks[idx].tags = tags
        // Route the raw splice to the engine with the ORIGINAL editor
        // offset (the authoritative UTF-16 position from the UITextView).
        // Open the local-write suppression window so the echo of our own
        // delta can't revert the splice (mirrors `scheduleWriteback`).
        // `.relay` writes through the same engine seam (audit A6) — the
        // splice is purely local; the RelayTicker drains it via the tick.
        // Only `.mock` (the fake snapshot) drops the write.
        let dailySlug: String
        switch currentBackend {
        case .mock:
            return
        case .http:
            guard !serverDailyId.isEmpty else { return }
            dailySlug = serverDailyId
        case .relay:
            // `applyLocalRefreshFallback` sets `serverDailyId` once today's
            // file exists locally; before the first materialization (fresh
            // pair) fall back to the date-derived daily slug, which is
            // identical on every device.
            dailySlug = serverDailyId.isEmpty ? dailyId(daysAgo: 0) : serverDailyId
        }
        // Pre-materialization window (2026-06-10): until today's daily
        // exists as a local file, the engine may not hold this block at
        // all — the placeholder gate in `pushTodayBlocks` defers authoring
        // until content exists, and `spliceBlockText` cannot CREATE a
        // block (an engine-miss splice is a silent no-op → lost
        // keystroke). Route the edit through the whole-content writeback
        // instead: the engine-side diff creates the block carrying this
        // keystroke's text. Safe from collab clobber — a block that exists
        // on no other device has no concurrent editors. Once the file
        // materializes (one cheap stat, memoized) splices flow normally.
        if !dailyMaterializedLocally(slug: dailySlug) {
            scheduleWriteback()
            return
        }
        beginLocalWriteSuppression()
        onLocalSplice?(dailySlug, id, clampedOffset, clampedDelete, insert)
    }

    /// The engine-exact stored text for a block: the materialized line's
    /// VISIBLE content — `displayText` (body, possibly multi-line) with
    /// the block's `tags` re-inlined as a trailing ` #tag …` cluster. This
    /// mirrors `renderBody`'s line construction (sans the `- ` bullet and
    /// the `<!-- bid:… -->` comment, neither of which is part of the Loro
    /// `text_seq`) and equals what `BlockRow.combinedEditableText()` loads
    /// into the editor — so splice offsets land 1:1 on the engine.
    static func engineExactText(of block: Block) -> String {
        let body = block.displayText
        guard !block.tags.isEmpty else { return body }
        let tagCluster = block.tags.joined(separator: " ")
        if body.isEmpty { return tagCluster }
        return body + " " + tagCluster
    }

    /// Split a faithful block string into (body, trailingTags) by peeling
    /// the trailing `#tag` cluster off the LAST line WITHOUT collapsing
    /// interior whitespace — the non-destructive counterpart of
    /// `splitInlineTags` (which collapses spaces + pulls tags from
    /// anywhere). Used by the splice path so the chip display stays in
    /// sync while the stored body remains a faithful 1:1 view of the
    /// engine's `text_seq` (offset alignment). Tags keep their `#` prefix.
    static func splitTrailingTags(_ raw: String) -> (body: String, tags: [String]) {
        var lines = raw.components(separatedBy: "\n")
        guard let last = lines.last else { return (raw, []) }
        var tokens = last.split(separator: " ", omittingEmptySubsequences: false).map(String.init)
        var tags: [String] = []
        let tagPattern = "^#[A-Za-z0-9_-]+$"
        while let token = tokens.last,
              token.range(of: tagPattern, options: .regularExpression) != nil {
            tags.insert(token, at: 0)
            tokens.removeLast()
            // Also drop a single separating space we implicitly consumed.
            if tokens.last == "" { tokens.removeLast() }
        }
        guard !tags.isEmpty else { return (raw, []) }
        lines[lines.count - 1] = tokens.joined(separator: " ")
        let body = lines.joined(separator: "\n")
        return (body, tags)
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
        // Canonical (36-char dashed) UUID so isCanonicalUUID(...) returns
        // true and the rendered `- text <!-- bid:UUID -->` carries the
        // id verbatim. The earlier "ios-<12char>" form was non-canonical,
        // so the bid marker was omitted; the server-side parser would
        // then generate a fresh random UUID on every parse, churning the
        // block's identity across pushes — and racing the HTTP-PUT and
        // relay paths to produce DUPLICATE blocks on the receiver
        // (each path assigned a different bid for the same intent).
        let id = UUID().uuidString.lowercased()
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
        blocks[idx].text = text.components(separatedBy: "\n").first ?? text
        blocks[idx].rawText = text
        Task { await pushPage(id: pageId, blocks: blocks) }
    }

    /// Append a new empty block on a non-daily page.
    @discardableResult
    func appendPageBlock(pageId: String, kind: BlockKind = .note) -> String {
        let id = UUID().uuidString.lowercased()
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
    ///
    /// Daily captures APPEND at the bottom (matching the web client),
    /// slotting in just before any trailing empty-placeholder run — the
    /// old `insert(at: 0)` put every capture ABOVE the day's existing
    /// notes (2026-06-10 product test).
    func capture(_ text: String, target: CaptureTarget) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        let id = UUID().uuidString.lowercased()
        switch target {
        case .today:
            todayBlocks.insert(
                Block(id: id, kind: .note, text: trimmed),
                at: Self.captureInsertIndex(in: todayBlocks)
            )
            scheduleWriteback()
        case .inbox:
            todayBlocks.insert(
                Block(id: id, kind: .note, text: trimmed, tags: ["#inbox"]),
                at: Self.captureInsertIndex(in: todayBlocks)
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

    /// Where a daily capture lands: after the LAST contentful block,
    /// but before any trailing run of bare placeholders (the editable
    /// "Add block" rows the user hasn't typed into) so the empty row
    /// stays the visual tail of the day.
    static func captureInsertIndex(in blocks: [Block]) -> Int {
        var idx = blocks.count
        while idx > 0 && isBarePlaceholder(blocks[idx - 1]) {
            idx -= 1
        }
        return idx
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
        case .relay:
            // Local sandbox scan over the relay-synced notes; no Mac.
            searchHits = localSearch(q)
            searchError = nil
            return
        case .http(let baseURL):
            searchInFlight = true
            // HTTP-first with 2s deadline. If Mac is unreachable
            // (cellular without Tailscale) fall back to a local scan
            // of the iOS sandbox — slower but works offline.
            let httpHits: [APISearchHit]? = await {
                do {
                    let encoded = q.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? q
                    return try await fetchOrTimeout("/search?q=\(encoded)", baseURL: baseURL, seconds: 2)
                } catch {
                    return nil
                }
            }()
            if let httpHits {
                searchHits = httpHits.map(mapSearchHit)
                searchError = nil
            } else {
                // Local scan over the iOS sandbox notes/ dir.
                searchHits = localSearch(q)
                searchError = nil
            }
            searchInFlight = false
        }
    }

    /// Walk the iOS sandbox notes/ directory + return matching hits.
    /// Match rules — same casing-insensitive substring shape the
    /// server's `/search` endpoint uses for v1 lexical search.
    /// Title hits beat body hits (sorted first), capped at 30 results
    /// so the UI doesn't grind on a query that matches "the".
    ///
    /// O(notes × body_size) per query; fine for a personal mosaic up
    /// to ~2-3k notes. Beyond that, build an in-memory inverted index
    /// in B.4 (Path B follow-up).
    private func localSearch(_ query: String) -> [SearchResult] {
        let notesDir = localMosaicRoot().appendingPathComponent("notes")
        guard let files = try? FileManager.default.contentsOfDirectory(atPath: notesDir.path) else {
            return []
        }
        let needle = query.lowercased()
        var titleHits: [SearchResult] = []
        var bodyHits: [SearchResult] = []
        for fname in files where fname.hasSuffix(".md") {
            let slug = String(fname.dropLast(3))
            let path = notesDir.appendingPathComponent(fname).path
            guard let raw = try? String(contentsOfFile: path, encoding: .utf8) else { continue }
            // Pull title from frontmatter if present; else fall back to slug.
            let frontmatter = extractFrontmatter(from: raw)
            let title = parseTitleFromFrontmatter(frontmatter) ?? slug

            if title.lowercased().contains(needle) {
                titleHits.append(SearchResult(id: slug, kind: .page, title: title, snippet: ""))
                continue
            }
            // Body match — find the first line containing the needle
            // (excluding the frontmatter block + bid comments).
            let body = stripFrontmatter(raw)
            for line in body.split(separator: "\n") {
                let cleaned = String(line)
                    .replacingOccurrences(of: #"<!--\s*bid:[^\s>]+\s*-->"#, with: "", options: .regularExpression)
                if cleaned.lowercased().contains(needle) {
                    let snippet = cleaned.trimmingCharacters(in: .whitespaces)
                        .replacingOccurrences(of: "^- ", with: "", options: .regularExpression)
                    bodyHits.append(SearchResult(id: slug, kind: .block, title: title, snippet: snippet))
                    break
                }
            }
        }
        return Array((titleHits + bodyHits).prefix(30))
    }

    /// Strip the leading YAML frontmatter block from a raw note,
    /// returning just the body. Mirror of what the body-extraction in
    /// `readLocalNote` does — same logic but without constructing an
    /// `APINote`.
    private func stripFrontmatter(_ raw: String) -> String {
        guard raw.hasPrefix("---"),
              let close = raw.range(of: "\n---", options: [], range: raw.index(raw.startIndex, offsetBy: 3)..<raw.endIndex)
        else { return raw }
        return String(raw[close.upperBound...]).trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// Local backlinks: scan every note in the iOS sandbox for
    /// `[[targetSlug]]` references and return rows pointing back to
    /// the source pages. Same shape as `mapBacklink` produces from
    /// the server's `/notes/<id>/backlinks` endpoint.
    ///
    /// Slug-match is exact (case-insensitive). Title-aliasing (the
    /// server's "linked references via title rather than slug") is
    /// deliberately not done here — would require knowing every
    /// note's title up front, which is what the indexer maintains.
    /// Good enough for v1: explicit `[[slug]]` wiki-links resolve.
    private func localBacklinks(for targetId: String) -> [Backlink] {
        let notesDir = localMosaicRoot().appendingPathComponent("notes")
        guard let files = try? FileManager.default.contentsOfDirectory(atPath: notesDir.path) else {
            return []
        }
        let needle = targetId.lowercased()
        var out: [Backlink] = []
        // Match `[[anything]]` then post-filter on the slug inside.
        let regex = try? NSRegularExpression(pattern: #"\[\[([^\]]+)\]\]"#, options: [])
        for fname in files where fname.hasSuffix(".md") {
            let sourceSlug = String(fname.dropLast(3))
            if sourceSlug.lowercased() == needle { continue }  // skip self
            let path = notesDir.appendingPathComponent(fname).path
            guard let raw = try? String(contentsOfFile: path, encoding: .utf8) else { continue }
            let body = stripFrontmatter(raw)
            let nsBody = body as NSString
            let matches = regex?.matches(in: body, range: NSRange(location: 0, length: nsBody.length)) ?? []
            for m in matches where m.numberOfRanges >= 2 {
                let inner = nsBody.substring(with: m.range(at: 1)).lowercased()
                if inner == needle {
                    // Use the first matching line as the snippet.
                    let absLoc = m.range.location
                    let beforeNL = (body as NSString).substring(to: absLoc).components(separatedBy: "\n").last ?? ""
                    let afterNL = (body as NSString).substring(from: absLoc).components(separatedBy: "\n").first ?? ""
                    var snippet = (beforeNL + afterNL)
                        .replacingOccurrences(of: #"<!--\s*bid:[^\s>]+\s*-->"#, with: "", options: .regularExpression)
                        .trimmingCharacters(in: .whitespaces)
                    if snippet.hasPrefix("- ") { snippet.removeFirst(2) }
                    let sourceTitle = parseTitleFromFrontmatter(extractFrontmatter(from: raw)) ?? sourceSlug
                    out.append(Backlink(id: UUID(), from: sourceTitle, snippet: snippet, pageId: sourceSlug))
                    break  // one snippet per source note
                }
            }
        }
        return out
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

    /// Fetch the server's full Loro snapshot for a note (raw bytes), so a
    /// fresh device can import it as a shared base before authoring
    /// locally (multi-device convergence — Part D). Returns `nil` when the
    /// server has no resident doc for the slug (404), so the caller treats
    /// a missing base as "nothing to bootstrap" rather than an error.
    func fetchLoroSnapshot(slug: String) async throws -> Data? {
        guard case .http(let baseURL) = currentBackend else {
            throw URLError(.badURL)
        }
        let req = {
            var r = URLRequest(url: endpoint("/loro/notes/\(slug)/snapshot", baseURL: baseURL))
            r.timeoutInterval = 8
            return r
        }()
        let (data, response) = try await session.data(for: req)
        if let http = response as? HTTPURLResponse, http.statusCode == 404 {
            return nil
        }
        try ensureOk(response, data: data)
        return data
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
        case .relay:
            // Render the engine's relay-synced local file; no Mac HTTP. Fire the
            // note-opened hook so the RelayTicker imports/catches-up this note's
            // base. Backlinks come from the local sandbox scan; outgoing links
            // are HTTP-only today (skip). Resolve the load state either way so
            // the page never hangs on "loading".
            onNoteOpened?(id)
            if let local = readLocalNote(id: id) {
                loadedPageBlocks[id] = parseBlocks(from: local.body, noteId: id)
                loadedPageFrontmatter[id] = extractFrontmatter(from: local.content)
                inMemoryLoadedAt[id] = localNoteMTime(id: id) ?? Date()
            } else {
                loadedPageBlocks[id] = []
            }
            loadedBacklinks[id] = localBacklinks(for: id)
            loadedLinks[id] = []
            pageLoadStates[id] = .ready
        case .http(let baseURL):
            // The page is becoming visible — import the server's note
            // doc as a base so live deltas for it materialize and our
            // pushes converge (T2). Idempotent; fires regardless of
            // whether the local-first render or the HTTP freshen wins.
            onNoteOpened?(id)
            // Local-first: if we have a materialized file, show it
            // immediately so the user sees the page right away,
            // even on a slow / down network. Then fire HTTP in the
            // background to freshen.
            if let local = readLocalNote(id: id) {
                loadedPageBlocks[id] = parseBlocks(from: local.body, noteId: id)
                loadedPageFrontmatter[id] = extractFrontmatter(from: local.content)
                inMemoryLoadedAt[id] = localNoteMTime(id: id) ?? Date()
                loadedBacklinks[id] = localBacklinks(for: id)
                pageLoadStates[id] = .ready
            }
            // HTTP freshen — best-effort. If we already rendered
            // from local, a failure here is silent; the user keeps
            // looking at the local copy. If we never had a local
            // copy and HTTP also fails, then we honestly surface
            // the "couldn't load" state.
            let httpResult: APINote? = await fetchNoteWithTimeout(
                id: id,
                baseURL: baseURL,
                seconds: 3
            )
            if let note = httpResult {
                // Engine-render: prefer the engine-materialized local file
                // (the merged Loro output) over the HTTP body for a resident
                // page; fall back to the server body only when the engine
                // hasn't materialized this note yet (first view). See the
                // daily path in `refresh(from:)` for the full rationale.
                let pageRender = readLocalNote(id: id) ?? note
                loadedPageBlocks[id] = parseBlocks(from: pageRender.body, noteId: id)
                loadedPageFrontmatter[id] = extractFrontmatter(from: pageRender.content)
                inMemoryLoadedAt[id] = Date()
                pageLoadStates[id] = .ready
            } else if pageLoadStates[id] != .ready {
                pageLoadStates[id] = .failed("Couldn't reach \(baseURL.host ?? "server")")
                return
            }
            // Backlinks: HTTP-first with a tight 1.5s deadline, then
            // a local-sandbox scan as the fallback. Local-only matches
            // explicit `[[slug]]` wiki-links (no title aliasing), which
            // is the dominant use case.
            let httpBacklinks: [APILink]? = try? await fetchOrTimeout(
                "/notes/\(id)/backlinks", baseURL: baseURL, seconds: 1.5
            )
            if let httpBacklinks {
                loadedBacklinks[id] = httpBacklinks.map(mapBacklink)
            } else {
                loadedBacklinks[id] = localBacklinks(for: id)
            }
            // Outgoing links: HTTP-only for now (computing locally would
            // require parsing this page's body, which we already have
            // in loadedPageBlocks but in a different shape). Cheap to
            // wire up later; not on the critical path.
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
        // `.http` AND `.relay` both write through the engine seam below —
        // there is no HTTP tail left here (the PUT was removed in Phase
        // 2.1), so the only mode that must NOT write is `.mock` (audit A6:
        // this gate used to be `.http`-only, silently dropping every
        // `.relay` edit).
        guard currentBackend != .mock else { return }
        beginLocalWriteSuppression()
        let body = renderBody(from: blocks)
        let frontmatter = loadedPageFrontmatter[id] ?? "---\ntitle: \(id)\n---"
        let content = "\(frontmatter)\n\n\(body)\n"
        // Single write path through the engine + relay. The HTTP PUT
        // path was removed in Phase 2.1 (sync redesign 2026-05-26)
        // because it raced the relay path on Mac:
        //   - HTTP PUT path emits server-side diff vs Mac's current
        //     file — but iOS's view doesn't include peer edits Mac
        //     already has, so the diff emits spurious BlockDeletes for
        //     those peer blocks AND tries to insert iOS-authored
        //     blocks under freshly-stamped server ids.
        //   - The relay path concurrently delivers iOS's own ops with
        //     DIFFERENT block ids (parse_note generates fresh UUIDs
        //     for bid-less iOS content).
        //   - When both apply on Mac, the result is duplicate blocks
        //     (each path's separate ids both got appended) AND
        //     overwritten peer blocks. Symptom: "web overwrites iOS"
        //     and "web shows the same block 3 times."
        // The engine path is the single, correct writer now. Relay
        // tick (~2 s) carries to Mac; APNs silent push (Phase 4)
        // will reduce that to sub-second.
        onLocalWrite?(id, id, content, Int64(Date().timeIntervalSince1970 * 1000))
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

    /// Hydrate the in-memory snapshot. **Local-first**: always render
    /// the engine-materialized files immediately so the UI is usable
    /// in <100ms even when offline, then fire HTTP in the background
    /// to pick up server-side changes. The HTTP path is best-effort:
    /// failures leave the local view in place silently. Pull-to-
    /// refresh sets `userInitiated: true` so an HTTP failure can
    /// surface its banner; the default (background refresh, app
    /// foreground, RelayTicker callback) stays silent.
    ///
    /// This pattern is what makes iOS feel like a local-first app
    /// (no "blank screen for 5 seconds" cold launches, no "offline
    /// edit gets clobbered on reconnect"). HTTP becomes a freshen
    /// pass on top of the local source of truth, not a gate that
    /// blocks the UI.
    func refresh(from backend: Backend, userInitiated: Bool = false) async {
        // Nudge query-backed views (Agenda/Inbox) on every real-backend
        // refresh — `defer` so the `.http` catch-path early returns
        // still signal (a failed freshen may still have hydrated from
        // the local sandbox).
        defer { if backend != .mock { refreshTick &+= 1 } }
        switch backend {
        case .mock:
            resetToSeed()
            connection = .idle
        case .relay:
            // Local-first, no Mac: render the relay-synced sandbox notes and
            // let the RelayTicker freshen in the background (it nudges us via
            // onAppliedChanges → another refresh when new ops land). Mirrors
            // the local-first STEP of the .http path, minus the HTTP freshen.
            onNoteOpened?(dailyId(daysAgo: 0))
            let hydrated = applyLocalRefreshFallback()
            if hydrated {
                todayLoadedAt = localNoteMTime(id: dailyId(daysAgo: 0)) ?? Date()
            }
            // The daily slug is purely date-derived in relay mode (no server
            // to mint an id), so set it even when today's file doesn't exist
            // yet — the daily write gates (`scheduleWriteback`/splice) need
            // it non-empty for the FIRST edit on a fresh pair (audit A6).
            // Re-derive on every refresh (not just when empty): an app left
            // foregrounded across midnight would otherwise keep writing
            // "today's" edits into yesterday's note (review finding).
            serverDailyId = dailyId(daysAgo: 0)
            // Older days (the feed below Yesterday) from the same local
            // sandbox. `.relay` previously never filled `pastDailies`, so
            // the Daily tab dead-ended at Yesterday (2026-06-10 product
            // test). Honors the progressive `pastDailiesWindow`.
            let past = localPastDailies(limit: pastDailiesWindow)
            pastDailies = past.entries
            hasOlderDailies = past.more
            // Always .ready — there's no server to wait on. Empty until the
            // relay delivers the first ops, then onAppliedChanges re-refreshes.
            connection = .ready
        case .http(let baseURL):
            // Today's daily is the always-visible note on launch +
            // every refresh — import the server's doc as a base so its
            // live deltas materialize and our pushes converge (T2).
            // Idempotent; cheap (resident-check) once bootstrapped.
            onNoteOpened?(dailyId(daysAgo: 0))
            // Step 1: render local immediately. Cheap (filesystem
            // walks the iOS sandbox), so we always do it before
            // touching the network. After this point the UI is
            // usable; the HTTP step that follows just freshens it.
            let hadDataBefore = !todayBlocks.isEmpty
            let hydratedFromLocal = applyLocalRefreshFallback()
            if hydratedFromLocal {
                let todayId = dailyId(daysAgo: 0)
                todayLoadedAt = localNoteMTime(id: todayId) ?? Date()
                connection = .ready
            }

            // Step 2: HTTP freshen. Don't block the UI on this — if
            // we have local data, the user can already see + edit
            // their notes. The freshen call below catches up to any
            // server-side changes that happened since the last
            // RelayTicker apply.
            connection = hydratedFromLocal ? .ready : .connecting
            do {
                let daily: APINote = try await fetchOrTimeout(
                    "/notes/daily", baseURL: baseURL, seconds: 3
                )
                let notes: [APINote] = try await httpGet("/notes?limit=200", baseURL: baseURL)
                let yesterdayNote: APINote? = (try? await fetchYesterdayDaily(baseURL: baseURL))
                // +2 covers today + yesterday, which the filter below drops.
                let dailyFetchLimit = pastDailiesWindow + 2
                let dailyNotes: [APINote] = (try? await httpGet("/notes?tag=daily&limit=\(dailyFetchLimit)", baseURL: baseURL)) ?? []
                let serverTagNames: [String] = (try? await httpGet("/tags", baseURL: baseURL)) ?? []

                serverDailyId = daily.id
                // Engine-render: the iOS engine materializes each note to
                // <sandbox>/notes/<id>.md on every apply, so the local file
                // IS the engine's merged output. If the engine has resident
                // (materialized) state for today, render its body from the
                // file — NOT the HTTP body — so block-level Loro merges win
                // and a whole-body mtime pick never defeats them. The local
                // file already holds any offline iOS edit (it's in the
                // engine), so this also preserves an unshipped edit across a
                // stale HTTP refresh without any mtime override. Only when
                // the engine has NOT yet materialized today's note (never
                // opened / first view) do we fall back to the server body;
                // `onNoteOpened` above fires the catch-up so the engine takes
                // over on the next pass. Server metadata (id, tags,
                // serverDailyId) still comes from `daily`.
                let dailyRender = readLocalNote(id: daily.id) ?? daily
                todayBlocks = parseBlocks(from: dailyRender.body, noteId: daily.id)
                todayLoadedAt = Date()
                loadedDailyFrontmatter = extractFrontmatter(from: dailyRender.content)
                pages = notes
                    .filter { $0.id != daily.id }
                    .map { mapPage($0) }
                yesterdayBlocks = yesterdayNote.map { parseBlocks(from: $0.body, noteId: $0.id) } ?? []
                let yesterdayId = dailyId(daysAgo: 1)
                let pastEntries = dailyNotes
                    .filter { $0.id != daily.id && $0.id != yesterdayId }
                    .sorted { $0.id > $1.id }
                    .map { DailyEntry(id: $0.id, blocks: parseBlocks(from: $0.body, noteId: $0.id)) }
                    .filter { !$0.blocks.isEmpty }
                pastDailies = Array(pastEntries.prefix(pastDailiesWindow))
                // A full server page means more days probably exist past
                // the window; an underfull one means we've seen them all.
                hasOlderDailies = pastEntries.count > pastDailiesWindow
                    || dailyNotes.count >= dailyFetchLimit
                tags = serverTagNames.map { name in
                    let parts = name.split(separator: "/")
                    let leaf = parts.last.map(String.init) ?? name
                    let parent = parts.count > 1 ? parts.dropLast().joined(separator: "/") : nil
                    return Tag(id: name, title: leaf, parent: parent, count: 0, recent: "today")
                }
                recent = pages.sorted(by: { $0.edited > $1.edited })
                    .prefix(8)
                    .map { RecentEntry(id: $0.id, title: $0.title, at: $0.edited) }
                // Snapshot Mac's notes to the iOS sandbox so offline
                // search + backlinks see the whole mosaic, not just
                // the subset the relay tick has delivered. This is
                // the "first refresh on a new device pulls a full
                // snapshot" behaviour Logseq has: after one
                // successful online refresh, the device is fully
                // usable offline. Subsequent refreshes only refresh
                // notes that are stale (cheaper than re-writing
                // everything every time).
                await snapshotNotesToSandbox(notes + [daily])
                connection = .ready
            } catch {
                // HTTP failed. If we already have local data on
                // screen, the user sees no disruption — we just
                // don't surface a banner. The only time we want
                // to surface it is on an explicit pull-to-refresh
                // (`userInitiated`) where the user actively asked
                // for the network round-trip and deserves to know
                // it failed. Even then, the existing local data
                // stays in place; the banner is a hint, not a wipe.
                if hadDataBefore || hydratedFromLocal {
                    connection = .ready
                    if userInitiated {
                        setConnectionFailedIfReal(error, host: baseURL.host)
                    }
                    return
                }
                // No local data either — surface the error so the
                // user knows the app couldn't find anything to show.
                setConnectionFailedIfReal(error, host: baseURL.host)
            }
        }
    }

    /// Read every `.md` file in the iOS sandbox notes/ directory and
    /// hydrate the in-memory `pages` + `todayBlocks` from them. Used
    /// as a fallback when HTTP refresh fails (cellular without
    /// Tailscale, Mac asleep, etc.). Returns true when at least one
    /// file was readable; false means there's nothing local to show
    /// and the caller should surface the original HTTP error.
    @discardableResult
    private func applyLocalRefreshFallback() -> Bool {
        let notesDir = localMosaicRoot().appendingPathComponent("notes")
        guard let files = try? FileManager.default.contentsOfDirectory(atPath: notesDir.path),
              !files.isEmpty
        else { return false }
        let mdFiles = files.filter { $0.hasSuffix(".md") }
        guard !mdFiles.isEmpty else { return false }

        var loadedNotes: [APINote] = []
        for fname in mdFiles {
            let id = String(fname.dropLast(3))  // strip ".md"
            if let note = readLocalNote(id: id) {
                loadedNotes.append(note)
            }
        }
        guard !loadedNotes.isEmpty else { return false }

        // Identify today's daily — `YYYY-MM-DD` of today as id.
        let todayId = dailyId(daysAgo: 0)
        if let daily = loadedNotes.first(where: { $0.id == todayId }) {
            serverDailyId = daily.id
            todayBlocks = parseBlocks(from: daily.body, noteId: daily.id)
            // Cache frontmatter from the local file so writebacks
            // composed before any HTTP refresh still preserve Mac's
            // tags / properties (the most recent state the engine
            // applied to disk is the most reliable source we have).
            loadedDailyFrontmatter = extractFrontmatter(from: daily.content)
        }
        // Yesterday's daily similarly.
        let yesterdayId = dailyId(daysAgo: 1)
        if let y = loadedNotes.first(where: { $0.id == yesterdayId }) {
            yesterdayBlocks = parseBlocks(from: y.body, noteId: y.id)
        }
        // All other notes become pages.
        pages = loadedNotes
            .filter { $0.id != todayId }
            .map { mapPage($0) }
        // Tags: union of every note's tags, no usage counts.
        let tagSet = Set(loadedNotes.flatMap { $0.metadata.tags })
        tags = tagSet.sorted().map { name in
            let parts = name.split(separator: "/")
            let leaf = parts.last.map(String.init) ?? name
            let parent = parts.count > 1 ? parts.dropLast().joined(separator: "/") : nil
            return Tag(id: name, title: leaf, parent: parent, count: 0, recent: "today")
        }
        // Recent: edited-date-sorted top 8.
        recent = pages.sorted(by: { $0.edited > $1.edited })
            .prefix(8)
            .map { RecentEntry(id: $0.id, title: $0.title, at: $0.edited) }
        return true
    }

    private var currentBackend: Backend = .mock

    func attach(backend: Backend) {
        currentBackend = backend
        if backend != .mock {
            // HTTP and relay modes must never render the built-in
            // `MockSeed`. Clearing here means a slow or failing connect
            // (or, in relay mode, a fresh pair before the first relay
            // delivery) shows an honest empty state instead of fake "old
            // mosaic" data: a successful `refresh` repopulates from the
            // server / local sandbox, and a failed one leaves the empty
            // snapshot in place rather than resetting to the seed.
            // (Audit A6: `.relay` previously kept the seed, presenting
            // fake notes as a healthy real backend.)
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
        pastDailies = []
        pastDailiesWindow = 7
        hasOlderDailies = true
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

    /// The block currently open in the editor (mirrors `GrDailyView`'s
    /// `editingBlockId`). The C1-inbound live-apply path uses it to know
    /// which block's merged text to reconcile when a remote splice lands
    /// mid-edit. `nil` when nothing is being edited.
    var editingBlockId: String? = nil
    /// The open editor's imperative inserter, registered by the focused
    /// `BlockRow`. Weak: the row's `@State` owns it; a stale ref (a
    /// torn-down text view) is a harmless no-op since the inserter holds
    /// the `UITextView` weakly and `reconcile` early-returns.
    weak var openBlockInserter: CollabTextInserter?
    /// Reads a block's current engine-exact text — `(slug, blockIdHex) →
    /// text`. Wired by `GrAppShell` to `RelayTicker.readBlockText` so the
    /// inbound live-apply path can fetch the MERGED block text after a
    /// remote splice (the engine is the source of truth).
    var readEngineBlockText: ((_ slug: String, _ blockIdHex: String) async -> String?)?
    /// Monotonic counter bumped on every local in-block splice. The inbound
    /// live-apply path captures it before its async engine read and bails if
    /// it changed — a keystroke landed during the read, so the merged text it
    /// got is already stale and applying it would fight the live edit.
    private var localSpliceSeq: UInt64 = 0
    /// Coalesced retry for a live reconcile that raced an in-flight keystroke,
    /// so the remote edit still lands once the typing settles (one pending
    /// task; fires on a brief idle).
    private var reconcileRetry: Task<Void, Never>?

    /// A local write echoes straight back as a WebSocket event. Remote
    /// refreshes are skipped for a short window after a local write so
    /// the echo of our own edit can't revert a change made right after
    /// it (and not yet pushed). A genuine remote change in that window
    /// is deferred, not dropped — `pendingRemoteRefresh` flushes it.
    private var suppressRemoteUntil: Date?
    private var suppressionFlush: Task<Void, Never>?

    /// Debounce handle for the inbound-refresh coalescing window. Each
    /// inbound WS event (`WsEvent::NoteUpdated` text + binary delta)
    /// calls `applyRemoteChange()`; a burst of web edits would otherwise
    /// fire one full-note HTTP `refresh()` + `refreshLoadedPages()` PER
    /// event, backing the main actor up until the UI freezes and the
    /// refreshes never visibly land. Instead each call cancels the prior
    /// pending refresh and schedules a fresh one ~300 ms out, so a burst
    /// of N events collapses to O(1) refresh when the burst settles
    /// (delivery-layer redesign 2026-05-31, T4).
    private var remoteRefreshDebounce: Task<Void, Never>?
    /// Coalescing window for the debounce above. Long enough to absorb a
    /// rapid burst of web edits, short enough that a single edit still
    /// shows on the device in well under a second.
    private static let remoteRefreshDebounceNanos: UInt64 = 300_000_000

    /// React to a server-side note change announced over the live-sync
    /// WebSocket. Coalesces a burst of inbound events into a single
    /// debounced refresh (~300 ms) so N rapid web edits cause O(1-few)
    /// full-note re-fetches instead of N — the per-event storm froze the
    /// app and the refreshes never visibly landed (delivery-layer
    /// redesign 2026-05-31, T4). The existing edit-suppression guards
    /// still apply: an event that arrives mid-edit or inside the
    /// post-local-write window is deferred (`pendingRemoteRefresh`) and
    /// flushed when the guard clears, never refreshing over a live edit.
    func applyRemoteChange() async {
        // `.relay` MUST pass: the relay tick's `onAppliedChanges` is the
        // ONLY automatic refresh seam in that mode (no WS, no reconnect
        // loop), and `refresh(from: .relay)` is a pure local read — the
        // edit/suppression guards below apply unchanged (audit A6: the
        // `.http`-only gate left `.relay` permanently stale in-session).
        guard currentBackend != .mock else { return }
        if isEditingBlock {
            pendingRemoteRefresh = true
            // C1-inbound: live-apply a remote splice into the OPEN editor so a
            // concurrent same-block edit appears under the cursor immediately,
            // instead of waiting for the blur-time full refresh (which is the
            // only time a FOCUSED field otherwise updates). The deferred
            // refresh above still reconciles every other block on blur.
            reconcileOpenBlockLive()
            return
        }
        if let until = suppressRemoteUntil, until > Date() {
            pendingRemoteRefresh = true
            scheduleSuppressionFlush(at: until)
            return
        }
        pendingRemoteRefresh = false
        scheduleRemoteRefresh()
    }

    /// C1-inbound live-apply: after a remote splice lands in the engine while
    /// the user is editing, read the MERGED block text and apply it to the
    /// open editor's `UITextView` (minimal diff + caret remap) — the only
    /// path that updates a FOCUSED field, since `CollabTextView.updateUIView`
    /// is gated on `!isFirstResponder`. Safe to call on every inbound apply:
    /// the reconcile is a no-op when the editor already matches the engine
    /// (the echo of our OWN splice). Also refreshes the in-memory mirror for
    /// the block so subsequent local splice offsets and the blur-commit stay
    /// aligned with the engine. Today-blocks only (the collab editor path);
    /// the open block's slug is `serverDailyId`.
    private func reconcileOpenBlockLive() {
        guard let bid = editingBlockId,
              let read = readEngineBlockText,
              let inserter = openBlockInserter,
              !serverDailyId.isEmpty
        else { return }
        let slug = serverDailyId
        let seqAtStart = localSpliceSeq
        Task { @MainActor [weak self] in
            guard let self, let merged = await read(slug, bid) else { return }
            // Stale-read guard: if the user typed a local splice, or
            // switched/closed the block (a different inserter is registered),
            // DURING the async read, this merged text predates their keystroke.
            // Applying it would fight the live edit and could momentarily drop
            // the just-typed char from the view. Skip and retry once typing
            // settles so the remote edit still lands shortly.
            guard self.editingBlockId == bid,
                  self.openBlockInserter === inserter,
                  self.localSpliceSeq == seqAtStart
            else {
                self.scheduleReconcileRetry()
                return
            }
            // Atomic on the main actor (no await between) so the in-memory
            // mirror and the UITextView are updated together and stay in
            // lockstep — the next local splice's offset (UITextView-relative)
            // therefore matches the mirror length the clamp uses. Same
            // projection `spliceTodayBlock` maintains, so a blur-commit/refresh
            // doesn't revert the merge.
            if let idx = self.todayBlocks.firstIndex(where: { $0.id == bid }) {
                let (body, tags) = Self.splitTrailingTags(merged)
                self.todayBlocks[idx].text = body.components(separatedBy: "\n").first ?? body
                self.todayBlocks[idx].rawText = body
                self.todayBlocks[idx].tags = tags
            }
            inserter.reconcile(toEngineText: merged)
        }
    }

    /// Re-run the live reconcile after a brief idle when a prior attempt raced
    /// an in-flight keystroke, so the remote edit still appears once the user
    /// pauses. Coalesced to a single pending retry so continuous typing can't
    /// pile up tasks; the `isEditingBlock` gate stops it after blur (the
    /// deferred full refresh then reconciles everything).
    private func scheduleReconcileRetry() {
        reconcileRetry?.cancel()
        reconcileRetry = Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: 200_000_000)
            guard let self, !Task.isCancelled else { return }
            self.reconcileRetry = nil
            if self.isEditingBlock { self.reconcileOpenBlockLive() }
        }
    }

    /// Cancel any pending debounced refresh and schedule a fresh one
    /// ~300 ms out. The actual `refresh(from:)` + `refreshLoadedPages()`
    /// runs once, when the inbound burst settles. Re-checks the
    /// edit/suppression guards at fire time: an edit (or a fresh
    /// post-local-write window) that began during the debounce window
    /// re-defers the refresh via `pendingRemoteRefresh` instead of
    /// clobbering the in-progress edit. `@MainActor` throughout — the
    /// scheduled `Task` inherits the service's main-actor isolation, so
    /// the mutations stay race-free.
    private func scheduleRemoteRefresh() {
        remoteRefreshDebounce?.cancel()
        remoteRefreshDebounce = Task { [weak self] in
            try? await Task.sleep(nanoseconds: Self.remoteRefreshDebounceNanos)
            guard let self, !Task.isCancelled else { return }
            self.remoteRefreshDebounce = nil
            // Re-check the guards: an edit may have started, or a local
            // write may have opened a new suppression window, while we
            // were debouncing. Defer rather than refresh over the edit.
            if self.isEditingBlock {
                self.pendingRemoteRefresh = true
                return
            }
            if let until = self.suppressRemoteUntil, until > Date() {
                self.pendingRemoteRefresh = true
                self.scheduleSuppressionFlush(at: until)
                return
            }
            // Single coalesced pass for the whole settled burst: the
            // daily (+ page list) plus every open page, folded into one
            // refresh rather than one per inbound event.
            await self.refresh(from: self.currentBackend)
            await self.refreshLoadedPages()
        }
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
            setConnectionFailedIfReal(error, host: URL(string: serverURL)?.host)
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
        // `.http` and `.relay` both flush through the engine seam in
        // `pushTodayBlocks` (the HTTP PUT was removed in Phase 2.1; the
        // old `baseURL` capture was vestigial). Only `.mock` drops the
        // write (audit A6: the `.http`-only gate silently discarded every
        // `.relay` daily edit — capture, toggle, delete, indent…).
        guard currentBackend != .mock, !serverDailyId.isEmpty else {
            return
        }
        beginLocalWriteSuppression()
        let snapshot = todayBlocks
        Task { await pushTodayBlocks(snapshot) }
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

    /// Body for `POST /agenda`. Matches the server's `AgendaQuery` deser.
    private struct APIAgendaRequest: Encodable {
        let from: String
        let to: String
        let include_done: Bool
    }

    /// Body for `POST /search/query`. The web client passes `group`
    /// and `sort` for some surfaces; iOS leaves them nil for now and
    /// post-filters / sorts client-side.
    private struct APIExecuteQueryBody: Encodable {
        let dsl: String
        let group: String?
        let sort: String?
    }

    /// Body for `POST /blocks/set-property`.
    private struct APISetBlockPropertyBody: Encodable {
        let block_id: String
        let key: String
        let value: String
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

    /// DELETE with no payload. Used by the views switcher
    /// (`DELETE /views/{id}`); non-2xx surfaces through `ensureOk` with
    /// the server's message snippet (e.g. the builtin-delete 400).
    private func httpDelete(_ path: String, baseURL: URL) async throws {
        var req = URLRequest(url: endpoint(path, baseURL: baseURL))
        req.httpMethod = "DELETE"
        req.timeoutInterval = 8
        let (data, response) = try await session.data(for: req)
        try ensureOk(response, data: data)
    }

    /// POST + decode JSON response. Used by the Agenda + Inbox surfaces
    /// to call `/agenda` and `/search/query` without having to
    /// hand-roll a URLRequest each time.
    private func httpPostJSON<Body: Encodable, T: Decodable>(
        _ path: String,
        baseURL: URL,
        body: Body,
    ) async throws -> T {
        let url = endpoint(path, baseURL: baseURL)
        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        req.timeoutInterval = 8
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try JSONEncoder().encode(body)
        let (data, response) = try await session.data(for: req)
        try ensureOk(response, data: data)
        return try JSONDecoder().decode(T.self, from: data)
    }

    /// POST that ignores the response body. Used by `setBlockProperty`
    /// where the server returns 200 OK with no payload of interest.
    private func httpPostNoResponse<Body: Encodable>(
        _ path: String,
        baseURL: URL,
        body: Body,
    ) async throws {
        let url = endpoint(path, baseURL: baseURL)
        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        req.timeoutInterval = 8
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try JSONEncoder().encode(body)
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

    /// Generic timeout wrapper around `httpGet`. Throws
    /// `URLError(.timedOut)` after `seconds` if the call hasn't
    /// returned yet; otherwise behaves identically to httpGet. The
    /// home-screen `refresh` path uses this so the catch block's
    /// existing local-fallback can fire after a short deadline
    /// instead of waiting URLSession's default 8s.
    private func fetchOrTimeout<T: Decodable>(_ path: String, baseURL: URL, seconds: Double) async throws -> T {
        try await withThrowingTaskGroup(of: T.self) { group in
            group.addTask { [weak self] in
                guard let self else { throw URLError(.unknown) }
                return try await self.httpGet(path, baseURL: baseURL)
            }
            group.addTask {
                try await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
                throw URLError(.timedOut)
            }
            guard let first = try await group.next() else {
                throw URLError(.unknown)
            }
            group.cancelAll()
            return first
        }
    }

    /// Race an HTTP fetch against a wall-clock timeout. Returns the
    /// note when HTTP returned first; nil when the deadline passed
    /// (or the request itself failed). Used by the load-page path so
    /// "Mac reachable on LAN" gets fresh content fast AND "Mac
    /// unreachable on cellular" doesn't sit on URLSession's 8-second
    /// default before falling back to local.
    ///
    /// Implementation: spawn the HTTP call as a Task, sleep for
    /// `seconds`, whichever finishes first wins. The losing Task is
    /// cancelled; the URLSession respects it.
    private func fetchNoteWithTimeout(id: String, baseURL: URL, seconds: Double) async -> APINote? {
        return await withTaskGroup(of: APINote?.self) { group in
            group.addTask { [weak self] in
                guard let self else { return nil }
                return try? await self.httpGet("/notes/\(id)", baseURL: baseURL) as APINote
            }
            group.addTask {
                try? await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
                return nil
            }
            // First child to finish wins. Cancel the rest.
            let first = await group.next() ?? nil
            group.cancelAll()
            return first
        }
    }

    // MARK: - Local-file fallback (B.3.4)

    /// Filesystem path of the iOS-side mosaic root the RelayTicker
    /// writes into. Matches `RelayTicker.ensureCoordinator`. Reads
    /// from this directory let the iOS UI render Mac-originated edits
    /// without ever reaching the Mac over HTTP — critical for
    /// cellular use, where the Mac isn't internet-routable.
    private func localMosaicRoot() -> URL {
        let docs = FileManager.default.urls(
            for: .documentDirectory,
            in: .userDomainMask
        )[0]
        return docs.appendingPathComponent("sync-ios-mosaic")
    }

    /// Write each server-side note to `<sandbox>/notes/<id>.md` so the
    /// offline read paths (search, backlinks, page load, daily render)
    /// see the full mosaic instead of only the subset the engine has
    /// applied via the relay. Skips a note when the local file already
    /// has the same content (cheap content compare) so we don't churn
    /// the indexer / file watchers on every refresh.
    ///
    /// **Local-precedence rule:** never overwrite a local file that
    /// is *newer* than the server's `modified_at`. That file
    /// represents an iOS-authored edit the server hasn't seen yet;
    /// stomping it here would cause exactly the offline-edit-loss bug
    /// this whole rewrite is fixing. The mtime guard protects the
    /// engine-materialized file (the merged Loro output) from being
    /// overwritten by a staler server snapshot.
    private func snapshotNotesToSandbox(_ notes: [APINote]) async {
        // Move the file I/O off the @MainActor — writing hundreds of
        // notes synchronously on the main actor froze the UI for 9 s+
        // on fresh-install cold launch (the "Tesela 9000+ ms fence
        // hang" Daisy saw in the Xcode HUD). Captured locals only, no
        // self access inside the detached task.
        let notesDir = localMosaicRoot().appendingPathComponent("notes")
        let snapshot = notes
        await Task.detached(priority: .utility) {
            try? FileManager.default.createDirectory(
                at: notesDir, withIntermediateDirectories: true
            )
            let serverFmt = ISO8601DateFormatter()
            for note in snapshot {
                let path = notesDir.appendingPathComponent("\(note.id).md")
                let serverMtime = serverFmt.date(from: note.modified_at) ?? .distantPast
                // Skip if local is newer (iOS edit pending push to server).
                if let attrs = try? FileManager.default.attributesOfItem(atPath: path.path),
                   let localMtime = attrs[.modificationDate] as? Date,
                   localMtime > serverMtime {
                    continue
                }
                // Skip if local file already has identical content
                // (avoids touching mtime + retriggering file-watchers
                // for unchanged notes).
                if let existing = try? String(contentsOf: path, encoding: .utf8),
                   existing == note.content {
                    continue
                }
                // Write — best-effort; we don't want a single bad note to
                // stop the whole snapshot.
                try? note.content.write(to: path, atomically: true, encoding: .utf8)
            }
        }.value
    }

    /// File mtime of the local sandbox copy of a note, or nil when
    /// missing. Used to decide whether to prefer local over in-memory
    /// state when an HTTP refresh fails — if the file is newer than
    /// the timestamp on the in-memory render, the relay has caught up
    /// to something the user hasn't seen yet.
    private func localNoteMTime(id: String) -> Date? {
        let path = localMosaicRoot()
            .appendingPathComponent("notes")
            .appendingPathComponent("\(id).md")
        return (try? FileManager.default.attributesOfItem(atPath: path.path)[.modificationDate]) as? Date
    }

    /// Read a materialized note from the local sandbox. Returns nil
    /// when the file is missing, unreadable, or unparseable — caller
    /// then falls back to its prior behaviour. Constructs an `APINote`
    /// shape identical to what the Mac's HTTP `/notes/<id>` returns,
    /// so the parse-and-render code path downstream is unchanged.
    private func readLocalNote(id: String) -> APINote? {
        let path = localMosaicRoot()
            .appendingPathComponent("notes")
            .appendingPathComponent("\(id).md")
        guard let raw = try? String(contentsOf: path, encoding: .utf8) else {
            return nil
        }
        let frontmatter = extractFrontmatter(from: raw)
        let body: String = {
            // Body is everything after the closing `---\n` of the
            // frontmatter. If there's no frontmatter, body = full raw.
            guard raw.hasPrefix("---"),
                  let close = raw.range(of: "\n---", options: [], range: raw.index(raw.startIndex, offsetBy: 3)..<raw.endIndex)
            else { return raw }
            let after = raw.index(close.upperBound, offsetBy: 0)
            return String(raw[after...]).trimmingCharacters(in: .whitespacesAndNewlines)
        }()
        let title = parseTitleFromFrontmatter(frontmatter) ?? id
        let tags = parseTagsFromFrontmatter(frontmatter)
        let noteType = parseNoteTypeFromFrontmatter(frontmatter)
        let mtime = (try? FileManager.default.attributesOfItem(atPath: path.path)[.modificationDate] as? Date)
            ?? Date()
        let mtimeISO = ISO8601DateFormatter().string(from: mtime)
        return APINote(
            id: id,
            title: title,
            content: raw,
            body: body,
            metadata: APINoteMetadata(
                title: title,
                tags: tags,
                note_type: noteType,
                created: nil,
                modified: mtimeISO
            ),
            modified_at: mtimeISO
        )
    }

    /// True iff `id` is a daily slug — `YYYY-MM-DD`.
    static func isDailySlug(_ id: String) -> Bool {
        id.range(of: #"^\d{4}-\d{2}-\d{2}$"#, options: .regularExpression) != nil
    }

    /// Enumerate the local sandbox for daily notes OLDER than yesterday,
    /// newest first, parse up to `limit` non-empty days, and report
    /// whether more candidate days remain beyond the window. ISO daily
    /// slugs sort lexicographically by date, so plain string compares
    /// order the feed. Powers the `.relay` Daily feed (and the `.http`
    /// offline fallback) — cheap: reads at most `limit` files plus the
    /// directory listing.
    private func localPastDailies(limit: Int) -> (entries: [DailyEntry], more: Bool) {
        let notesDir = localMosaicRoot().appendingPathComponent("notes")
        guard let files = try? FileManager.default.contentsOfDirectory(atPath: notesDir.path) else {
            return ([], false)
        }
        let yesterdayCutoff = dailyId(daysAgo: 1)
        let candidateIds = files
            .filter { $0.hasSuffix(".md") }
            .map { String($0.dropLast(3)) }
            .filter { Self.isDailySlug($0) && $0 < yesterdayCutoff }
            .sorted(by: >)
        var entries: [DailyEntry] = []
        var scanned = 0
        for id in candidateIds {
            scanned += 1
            guard let note = readLocalNote(id: id) else { continue }
            let blocks = parseBlocks(from: note.body, noteId: id)
            guard !blocks.isEmpty else { continue }
            entries.append(DailyEntry(id: id, blocks: blocks))
            if entries.count >= limit { break }
        }
        return (entries, scanned < candidateIds.count)
    }

    /// Widen the Daily feed's past-days window by a week and reload it —
    /// called when the user scrolls to the feed's bottom sentinel.
    /// `.relay` reads the local sandbox; `.http` re-fetches the server's
    /// daily list with the larger limit (one cheap GET), falling back to
    /// the local snapshot mirror when the server is unreachable.
    func loadOlderDailies() async {
        switch currentBackend {
        case .mock:
            hasOlderDailies = false
        case .relay:
            pastDailiesWindow += 7
            let past = localPastDailies(limit: pastDailiesWindow)
            pastDailies = past.entries
            hasOlderDailies = past.more
        case .http(let baseURL):
            pastDailiesWindow += 7
            let limit = pastDailiesWindow + 2  // today + yesterday get filtered out
            guard let dailyNotes: [APINote] = try? await httpGet(
                "/notes?tag=daily&limit=\(limit)", baseURL: baseURL
            ) else {
                let past = localPastDailies(limit: pastDailiesWindow)
                pastDailies = past.entries
                hasOlderDailies = past.more
                return
            }
            let todayId = serverDailyId.isEmpty ? dailyId(daysAgo: 0) : serverDailyId
            let yesterdayId = dailyId(daysAgo: 1)
            let entries = dailyNotes
                .filter { $0.id != todayId && $0.id != yesterdayId }
                .sorted { $0.id > $1.id }
                .map { DailyEntry(id: $0.id, blocks: parseBlocks(from: $0.body, noteId: $0.id)) }
                .filter { !$0.blocks.isEmpty }
            pastDailies = Array(entries.prefix(pastDailiesWindow))
            hasOlderDailies = entries.count > pastDailiesWindow
                || dailyNotes.count >= limit
        }
    }

    /// Read every materialized note in the local sandbox, sorted by
    /// slug for deterministic output. The whole-mosaic walk the `.relay`
    /// Agenda + Inbox local queries run over — same corpus and cost
    /// shape as `localSearch` / `applyLocalRefreshFallback`.
    private func loadAllLocalNotes() -> [APINote] {
        let notesDir = localMosaicRoot().appendingPathComponent("notes")
        guard let files = try? FileManager.default.contentsOfDirectory(atPath: notesDir.path) else {
            return []
        }
        return files
            .filter { $0.hasSuffix(".md") }
            .sorted()
            .compactMap { readLocalNote(id: String($0.dropLast(3))) }
    }

    /// Pull `title: "..."` out of a YAML frontmatter block. Returns
    /// nil when not found. Quick + dirty — doesn't handle multi-line
    /// titles or escaped quotes, but the Mac's writer never produces
    /// either.
    private func parseTitleFromFrontmatter(_ fm: String) -> String? {
        for line in fm.split(separator: "\n") {
            if line.hasPrefix("title:") {
                let val = line.dropFirst("title:".count).trimmingCharacters(in: .whitespaces)
                return val.trimmingCharacters(in: CharacterSet(charactersIn: "\""))
            }
        }
        return nil
    }

    /// Pull `type: "..."` (the note_type discriminator) out of a YAML
    /// frontmatter block. Same quick-and-dirty single-line treatment as
    /// `parseTitleFromFrontmatter` — the Mac writer emits `type: "Query"`
    /// style lines only. Needed locally so the `.relay` Inbox can apply
    /// the server's `on:system-pages` semantics (Tag/Property/Query/
    /// Template) without HTTP.
    private func parseNoteTypeFromFrontmatter(_ fm: String) -> String? {
        for line in fm.split(separator: "\n") {
            if line.hasPrefix("type:") {
                let val = line.dropFirst("type:".count).trimmingCharacters(in: .whitespaces)
                let unquoted = val.trimmingCharacters(in: CharacterSet(charactersIn: "\""))
                return unquoted.isEmpty ? nil : unquoted
            }
        }
        return nil
    }

    /// Pull the `tags: [a, b, c]` flow-style array out of frontmatter.
    /// Doesn't handle the block-style `tags:\n  - a\n  - b` form;
    /// fine for now since the Mac writer always emits flow.
    private func parseTagsFromFrontmatter(_ fm: String) -> [String] {
        for line in fm.split(separator: "\n") {
            if line.hasPrefix("tags:") {
                let rest = line.dropFirst("tags:".count).trimmingCharacters(in: .whitespaces)
                guard rest.hasPrefix("["), rest.hasSuffix("]") else { return [] }
                let inner = rest.dropFirst().dropLast()
                return inner.split(separator: ",").map {
                    $0.trimmingCharacters(in: .whitespaces)
                        .trimmingCharacters(in: CharacterSet(charactersIn: "\""))
                }.filter { !$0.isEmpty }
            }
        }
        return []
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

    /// Update `connection` to `.failed(...)` UNLESS the error is one of
    /// the "benign in-flight" cases — `URLError.cancelled` (the request
    /// was superseded or the network changed mid-flight) and friends.
    ///
    /// Why: the iOS app fires a flurry of HTTP requests on foreground
    /// transitions and on background→cellular handover; those requests
    /// often get cancelled by URLSession itself and surface as
    /// `URLError.cancelled`. Promoting those to a red banner is wrong —
    /// the next refresh will succeed. Once iOS reads notes from the
    /// local engine instead of HTTP (B.3.4/B.3.5) this whole class of
    /// noise disappears; until then, swallow the benign cases here.
    private func setConnectionFailedIfReal(_ error: Error, host: String?) {
        if let url = error as? URLError {
            switch url.code {
            case .cancelled, .networkConnectionLost:
                // Drop silently — the running RelayTicker keeps real
                // sync going, and the next mosaic refresh will retry.
                return
            default:
                break
            }
        }
        // Swift task cancellation — fires when `fetchOrTimeout`
        // cancels the losing branch of its TaskGroup race, or when
        // SwiftUI tears down a `.task` modifier mid-flight. Neither
        // is a real connection failure; the next tick / next refresh
        // will retry without the user needing to know.
        if error is CancellationError {
            return
        }
        // NSError wrapper around the above — happens when URLSession's
        // cancellation surfaces through a Decodable error path that
        // bridges Swift errors into NSError. Same treatment.
        let ns = error as NSError
        if ns.domain == NSURLErrorDomain && ns.code == NSURLErrorCancelled {
            return
        }
        connection = .failed(humanizeError(error, host: host))
    }

    /// `YYYY-MM-DD` slug of today's daily — the note that's always
    /// visible on launch. Exposed so the shells can explicitly bootstrap
    /// its base on first connect (the initial `refresh` runs before
    /// `onNoteOpened` is wired, so a receive-only device that never
    /// edits or backgrounds would otherwise miss the daily bootstrap;
    /// delivery-layer redesign 2026-05-31, T2).
    var todayDailySlug: String { dailyId(daysAgo: 0) }

    /// `YYYY-MM-DD` id of the daily note `daysAgo` days before today.
    private func dailyId(daysAgo: Int) -> String {
        let cal = Calendar.current
        guard let date = cal.date(byAdding: .day, value: -daysAgo, to: todayDate) else { return "" }
        return Self.dailySlug(for: date)
    }

    /// Pure date→slug derivation for daily notes (`yyyy-MM-dd`). This is
    /// the convention every device derives independently — the `.relay`
    /// splice path falls back to it before the first local materialization
    /// sets `serverDailyId` — so the format is load-bearing for sync:
    /// devices converge on the same daily note only because this string
    /// matches across platforms.
    static func dailySlug(for date: Date) -> String {
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd"
        return f.string(from: date)
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
    ///
    /// `noteId` is stored on each `Block` so `recurBump` can build the
    /// server's `<noteId>:<line>` composite id without a separate lookup.
    private func parseBlocks(from body: String, noteId: String = "") -> [Block] {
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

            // Record the 0-based line index of this bullet for recur-bump.
            let blockLineNumber = i

            let rawIndent = leadingSpaces(line) / 2
            // Structural invariant: a block is at most one level deeper
            // than the block before it — every block has an immediate
            // parent. Clamping here both rejects malformed input and
            // *repairs* existing files: a blank sub-block that crept
            // many levels deep collapses back to parent + 1 on load.
            let indent = min(rawIndent, previousIndent + 1)
            previousIndent = indent
            let parsed = parseBlockLine(line, indent: indent)

            // Collect the property sub-lines AND continuation text that
            // follow this block (physically indented deeper than the
            // bullet, no `- `). Properties feed `properties`; everything
            // else is a continuation line and is appended to the body so
            // multi-line blocks render and round-trip intact.
            var properties: [BlockProperty] = []
            var continuationLines: [String] = []
            i += 1
            while i < lines.count {
                let next = lines[i]
                let nextIndent = leadingSpaces(next) / 2
                let nextTrim = next.trimmingCharacters(in: .whitespaces)
                if nextTrim.hasPrefix("- ") { break }
                if nextIndent <= rawIndent && !nextTrim.isEmpty { break }
                if nextTrim.isEmpty {
                    // Blank sub-line — skip silently, matching the web
                    // parser which drops empty lines outright.
                    i += 1
                    continue
                }
                if let prop = parseProperty(nextTrim) {
                    properties.append(prop)
                } else {
                    continuationLines.append(nextTrim)
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

            let rawText: String
            if continuationLines.isEmpty {
                rawText = parsed.text
            } else {
                rawText = ([parsed.text] + continuationLines).joined(separator: "\n")
            }

            blocks.append(Block(
                id: parsed.bid,
                kind: kind,
                text: parsed.text,
                rawText: rawText,
                done: done,
                indent: indent,
                tags: parsed.tags,
                properties: properties,
                lineNumber: blockLineNumber,
                noteId: noteId
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

    /// True when `block` carries nothing a peer would care about — no
    /// text, tags, properties, or task state. The "Add block" editable-row
    /// placeholder shape.
    static func isBarePlaceholder(_ block: Block) -> Bool {
        block.kind != .task
            && block.tags.isEmpty
            && block.properties.isEmpty
            && block.displayText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    /// Pure half of the placeholder-authoring gate (2026-06-10 product
    /// test): a daily writeback consisting ONLY of bare empty blocks, for
    /// a daily that has never been materialized locally, is the editable-
    /// row placeholder — not user content. AUTHORING it as the note's
    /// first synced state put a stray empty bullet ABOVE the peer's real
    /// content after the fresh-day union (iOS showed [empty, dude, empty];
    /// desktop [dude, empty]). Once a local file exists the gate is off:
    /// an all-bare state is then a REAL edit (the user deleted the last
    /// contentful block) and must flow.
    static func shouldSuppressPlaceholderAuthoring(
        blocks: [Block],
        dailyFileExists: Bool
    ) -> Bool {
        !dailyFileExists && blocks.allSatisfy(isBarePlaceholder)
    }

    /// Memo of the last daily slug confirmed materialized in the local
    /// sandbox, so the per-keystroke check below is one string compare
    /// after the file first appears. A new day (new slug) re-probes.
    private var materializedDailySlugCache: String?

    /// True once `<sandbox>/notes/<slug>.md` exists — i.e. the engine has
    /// materialized today's daily at least once (an inbound peer apply or
    /// our own first contentful writeback).
    private func dailyMaterializedLocally(slug: String) -> Bool {
        if materializedDailySlugCache == slug { return true }
        let path = localMosaicRoot()
            .appendingPathComponent("notes")
            .appendingPathComponent("\(slug).md")
            .path
        if FileManager.default.fileExists(atPath: path) {
            materializedDailySlugCache = slug
            return true
        }
        return false
    }

    private func pushTodayBlocks(_ blocks: [Block]) async {
        guard !serverDailyId.isEmpty else { return }
        // Placeholder gate — see `shouldSuppressPlaceholderAuthoring`.
        if Self.shouldSuppressPlaceholderAuthoring(
            blocks: blocks,
            dailyFileExists: dailyMaterializedLocally(slug: serverDailyId)
        ) {
            return
        }
        // Build the new content from cached frontmatter rather than
        // re-fetching it over HTTP — the old "fetch existing
        // frontmatter, then write" sequence introduced a 5-second
        // HTTP timeout BEFORE the engine path could fire, which
        // meant offline edits never persisted if the user kept
        // typing or backgrounded the app inside that window.
        //
        // `loadedDailyFrontmatter` is populated by every successful
        // refresh + every local-hydration pass, so by the time the
        // user can edit the daily we already have its frontmatter
        // cached. The "first edit on a brand-new install" case
        // falls through to a minimal `title: <id>` block, which
        // Mac will accept; subsequent edits pick up Mac's enriched
        // frontmatter on the next refresh tick.
        let existingFrontmatter = loadedDailyFrontmatter ?? "---\ntitle: \(serverDailyId)\n---"
        let newBody = renderBody(from: blocks)
        let content = combine(frontmatter: existingFrontmatter, body: newBody)
        // Engine path — durable, cellular-tolerant, no network
        // dependency. Fires synchronously (no await) so the SQLite
        // + materialized file write happens before anything else.
        let titleGuess = serverDailyId  // YYYY-MM-DD for dailies
        // Single write path through engine + relay. See `pushPage`
        // for the full reasoning — duplicate writes via HTTP PUT
        // raced the relay path and produced duplicate blocks +
        // overwrote peer edits on Mac. Relay tick carries within
        // ~2 s; APNs (Phase 4) will drop that further.
        onLocalWrite?(serverDailyId, titleGuess, content, Int64(Date().timeIntervalSince1970 * 1000))
    }

    /// Drop blocks that carry nothing — empty text, no tags, no task
    /// state, no properties, and no indented children. `appendTodayBlock`
    /// writes back the instant a block is added, so without this every
    /// abandoned "Add block" tap (or block split the user never typed
    /// into) would leave a permanent blank `- ` bullet on disk, which
    /// then round-trips back as a real empty block forever. The block
    /// still lives in the in-memory list so the user can type into it —
    /// it just isn't persisted until it actually has content.
    ///
    /// Walks back-to-front so dropping a child re-exposes its parent as
    /// a leaf within the same pass.
    private func droppingBareLeafBlocks(_ blocks: [Block]) -> [Block] {
        var kept: [Block] = []
        for block in blocks.reversed() {
            let isBare = block.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                && block.tags.isEmpty
                && block.properties.isEmpty
                && block.kind != .task
            // `kept` holds everything that renders after this block;
            // its last entry is the immediate successor. A deeper
            // successor means this block parents it — keep it then.
            let hasChild = (kept.last?.indent ?? block.indent) > block.indent
            if isBare && !hasChild { continue }
            kept.append(block)
        }
        return kept.reversed()
    }

    private func renderBody(from blocks: [Block]) -> String {
        var out: [String] = []
        // Phase 2.2 (2026-05-27): no longer strips blank-leaf blocks
        // at render time. Web preserves blank blocks (empty bullets
        // sit on disk until the user types into them or explicitly
        // deletes them); iOS's earlier silent prune produced an
        // asymmetry Daisy reported as confusing. If we ever want
        // abandoned-block cleanup back, it should be a deliberate
        // user-facing action (e.g. "tidy blanks") rather than a
        // silent renderer-side filter that disagrees with the web
        // client's behaviour.
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
            //
            // Multi-line blocks emit the first line after the bullet,
            // then each continuation line indented two spaces deeper
            // than the bullet — matching what `parseBlocks` expects so
            // the body round-trips losslessly.
            let bodyLines = block.displayText.components(separatedBy: "\n")
            let firstLine = bodyLines.first ?? ""
            out.append("\(indent)- \(firstLine)\(trailingTags)\(bidSuffix)")
            let continuationIndent = "\(indent)  "
            for line in bodyLines.dropFirst() {
                out.append("\(continuationIndent)\(line)")
            }

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
        pastDailies = []
        pastDailiesWindow = 7
        hasOlderDailies = false
        palette = MockSeed.palette
        searchResults = MockSeed.searchResults
        serverDailyId = ""
    }

    // MARK: - Property writes

    /// Upsert the full `properties` list on the block identified by `id`.
    /// Mirrors `editTodayBlock` / `editPageBlock` for block location and
    /// the same write-back path.
    func setBlockProperties(id: String, properties: [BlockProperty]) {
        if let idx = todayBlocks.firstIndex(where: { $0.id == id }) {
            todayBlocks[idx].properties = properties
            scheduleWriteback()
            return
        }
        for pageId in loadedPageBlocks.keys {
            var blocks = loadedPageBlocks[pageId] ?? []
            if let bidx = blocks.firstIndex(where: { $0.id == id }) {
                blocks[bidx].properties = properties
                Task { await pushPage(id: pageId, blocks: blocks) }
                return
            }
        }
        // Block not found in any loaded context — silently no-op.
    }

    // MARK: - Recurrence bump

    enum RecurBumpMode: String {
        case complete
        case skip
    }

    /// POST /blocks/recur-bump on the server to advance a recurring block
    /// to its next occurrence. In mock mode this is a no-op. After a
    /// successful request the daily is refreshed so the updated dates
    /// appear immediately.
    ///
    /// The server's route parses `block_id` as `<noteId>:<line>` (see
    /// `crates/tesela-server/src/routes/notes.rs`). iOS blocks carry
    /// `noteId` and `lineNumber` from `parseBlocks` so we can build the
    /// composite id here. If the block can't be located we bail early
    /// rather than sending a malformed request.
    func recurBump(blockId: String, mode: RecurBumpMode) async throws {
        guard case .http(let baseURL) = currentBackend else {
            // Mock mode — no server to call; silently succeed.
            return
        }

        // Locate the block to derive its noteId + lineNumber.
        let block: Block?
        if let b = todayBlocks.first(where: { $0.id == blockId }) {
            block = b
        } else {
            block = loadedPageBlocks.values.lazy
                .compactMap { $0.first(where: { $0.id == blockId }) }
                .first
        }
        guard let found = block, !found.noteId.isEmpty else {
            // Block not found or missing noteId — log and bail.
            print("[recurBump] block \(blockId) not found or has no noteId; skipping server call")
            return
        }
        // Build the server-expected composite id: "<noteId>:<lineNumber>".
        let compositeId = "\(found.noteId):\(found.lineNumber)"

        let body: [String: Any] = ["block_id": compositeId, "mode": mode.rawValue]
        let url = endpoint("/blocks/recur-bump", baseURL: baseURL)
        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        req.timeoutInterval = 8
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try JSONSerialization.data(withJSONObject: body)
        let (data, response) = try await session.data(for: req)
        try ensureOk(response, data: data)
        // Refresh so the bumped block's new scheduled/deadline dates appear.
        await refresh(from: currentBackend)
    }

    // MARK: - Agenda + Inbox queries

    /// Fetch the agenda window from `POST /agenda`. Bare ISO date strings
    /// (`YYYY-MM-DD`) for `from` / `to`, inclusive both ends. Returns an
    /// empty list on mock mode or HTTP failure rather than throwing, so
    /// the calling view can render an empty state without ceremony.
    ///
    /// `.relay` serves the same window from the relay-synced sandbox
    /// notes via `LocalQueryEngine` (the local mirror of the server's
    /// `agenda_blocks`) — the Agenda was empty in relay mode because
    /// this method was `.http`-gated (2026-06-10 product test).
    func fetchAgenda(from: String, to: String, includeDone: Bool) async -> [AgendaRow] {
        switch currentBackend {
        case .mock:
            return []
        case .relay:
            return localAgenda(from: from, to: to, includeDone: includeDone)
        case .http(let baseURL):
            let body = APIAgendaRequest(from: from, to: to, include_done: includeDone)
            do {
                return try await httpPostJSON("/agenda", baseURL: baseURL, body: body)
            } catch {
                return []
            }
        }
    }

    /// Local-sandbox mirror of `POST /agenda`: walk every materialized
    /// note, parse blocks, and let `LocalQueryEngine` apply the server's
    /// `agenda_blocks` semantics (scheduled/deadline anchors, done
    /// filtering, recurrence projection, the canonical sort).
    private func localAgenda(from: String, to: String, includeDone: Bool) -> [AgendaRow] {
        let today = Self.dailySlug(for: Date())
        var rows: [AgendaRow] = []
        for note in loadAllLocalNotes() {
            let blocks = parseBlocks(from: note.body, noteId: note.id)
            rows += LocalQueryEngine.agendaRows(
                blocks: blocks,
                from: from,
                to: to,
                includeDone: includeDone,
                today: today
            )
        }
        LocalQueryEngine.sortAgendaRows(&rows)
        return rows
    }

    /// Fetch the active Inbox-style saved filter's DSL. The Inbox surface
    /// is backed by a `note_type: Query` note whose body carries a
    /// `query:: <dsl>` line — same shape the web client uses. `slug`
    /// is normally `"inbox"` (the canonical default) but the user can
    /// save additional filters at `inbox-work`, `inbox-personal`, etc.;
    /// the active slug is persisted client-side and passed in.
    ///
    /// Returns `nil` when:
    ///   - we're on the mock backend
    ///   - the note doesn't exist yet (first-run mosaic)
    ///   - the note exists but has no `query::` line
    ///
    /// Callers fall back to `defaultInboxDsl()` in those cases.
    /// `.relay` reads the saved-filter note from the local sandbox
    /// instead of HTTP — same body scan either way.
    func fetchInboxDsl(slug: String) async -> String? {
        let note: APINote
        switch currentBackend {
        case .mock:
            return nil
        case .relay:
            guard let local = readLocalNote(id: slug) else { return nil }
            note = local
        case .http(let baseURL):
            do {
                note = try await httpGet("/notes/\(slug)", baseURL: baseURL)
            } catch {
                return nil
            }
        }
        // Match `^query::\s*(.+)$` line-by-line; mirrors the web's
        // `readQueryFromNote` (`Inbox.svelte`).
        for line in note.body.split(separator: "\n", omittingEmptySubsequences: false) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("query::") {
                let dsl = trimmed.dropFirst("query::".count).trimmingCharacters(in: .whitespaces)
                return dsl.isEmpty ? nil : dsl
            }
        }
        return nil
    }

    /// Canonical default DSL for the Inbox when no saved Query note
    /// exists yet. Delegates to the free `defaultInboxDsl()` in
    /// `InboxChips.swift` so the chip registry is the single source of
    /// truth (a change to the registry's `defaultOn` flags
    /// automatically updates this output). Mirrors the web's
    /// `defaultInboxDsl()` so iOS + web converge on the same first-run
    /// experience.
    static func defaultInboxDsl() -> String {
        Tesela.defaultInboxDsl()
    }

    /// Lightweight reference to a saved Inbox-style filter. Returned
    /// by `listInboxFilters` to drive the switcher menu without
    /// dragging the full note body through every refresh.
    struct InboxFilterRef: Hashable, Identifiable {
        let slug: String
        let title: String
        var id: String { slug }
    }

    /// List every Inbox-style saved filter — every `note_type: Query`
    /// note whose slug is `inbox` or starts with `inbox-`. Drives the
    /// switcher menu so the user can flip between saved filters
    /// without leaving the inbox surface. Filters non-Inbox Query
    /// notes (e.g. `calendar`, `tasks`, system widgets that are also
    /// Query-typed but aren't inbox alternatives) so the menu stays
    /// focused. Mirrors `availableFilters` in the web's
    /// `Inbox.svelte`.
    func listInboxFilters() async -> [InboxFilterRef] {
        let notes: [APINote]
        switch currentBackend {
        case .mock:
            return []
        case .relay:
            // Local sandbox scan; `note_type` comes from the frontmatter
            // `type:` line via `readLocalNote`.
            notes = loadAllLocalNotes()
        case .http(let baseURL):
            do {
                notes = try await httpGet("/notes?limit=500", baseURL: baseURL)
            } catch {
                return []
            }
        }
        return notes
            .filter { $0.metadata.note_type == "Query" }
            .filter { $0.id == "inbox" || $0.id.hasPrefix("inbox-") }
            .map { InboxFilterRef(slug: $0.id, title: $0.title.isEmpty ? $0.id : $0.title) }
            .sorted { $0.title.localizedCaseInsensitiveCompare($1.title) == .orderedAscending }
    }

    /// Slugify a user-entered filter name into a canonical id. Lowercases,
    /// replaces whitespace with `-`, drops everything that isn't
    /// `[a-z0-9-]`. Mirrors `slugify` in the web's `Inbox.svelte`.
    static func slugifyInboxFilterName(_ name: String) -> String {
        let lowered = name.trimmingCharacters(in: .whitespaces).lowercased()
        let withDashes = lowered.replacingOccurrences(
            of: #"\s+"#,
            with: "-",
            options: .regularExpression
        )
        return withDashes.replacingOccurrences(
            of: #"[^a-z0-9-]"#,
            with: "",
            options: .regularExpression
        )
    }

    /// Namespace a user-entered filter slug under `inbox-` unless the
    /// caller already did. Mirrors the web's Save-as composition step.
    static func namespacedInboxFilterSlug(_ baseSlug: String) -> String {
        if baseSlug == "inbox" { return baseSlug }
        if baseSlug.hasPrefix("inbox-") { return baseSlug }
        return "inbox-\(baseSlug)"
    }

    /// Persist a new DSL for the given saved-filter slug. Mirrors the
    /// web's `Inbox.svelte` flow: if the note already exists, splice
    /// the new `query::` line into its existing body (preserving
    /// frontmatter, icon, color); if it doesn't exist yet, create a
    /// fresh `note_type: Query` note with canonical frontmatter and
    /// the new DSL baked in.
    func saveInboxDsl(slug: String, dsl: String) async throws {
        // `.relay` persists the saved-filter Query note through the engine
        // — the relay analog of the HTTP read-splice-PUT below, with
        // `readLocalNote` playing the GET (preserve existing frontmatter).
        // Awaiting `onLocalNoteWrite` means the engine has re-materialized
        // the local file before the caller's reload reads the DSL back
        // (the views call `load()` immediately after saving). Previously
        // this was `.http`-gated: a relay-mode chip toggle / save-filter
        // silently did nothing. `.mock` still drops the write.
        if case .relay = currentBackend {
            let title = Self.titleForInboxFilterSlug(slug)
            let newContent: String
            if let existing = readLocalNote(id: slug) {
                newContent = Self.spliceInboxDsl(into: existing.content, dsl: dsl, title: title)
            } else {
                newContent = Self.freshInboxNoteContent(title: title, dsl: dsl)
            }
            await onLocalNoteWrite?(
                slug, title, newContent, Int64(Date().timeIntervalSince1970 * 1000)
            )
            return
        }
        guard case .http(let baseURL) = currentBackend else { return }
        // First try to read the existing note so we can preserve its
        // frontmatter (icon, color, section, etc.). 404 → first-write
        // path, fall through to create.
        let existing: APINote? = try? await httpGet("/notes/\(slug)", baseURL: baseURL)
        let title = Self.titleForInboxFilterSlug(slug)
        let newContent: String
        if let existing {
            newContent = Self.spliceInboxDsl(into: existing.content, dsl: dsl, title: title)
            try await httpPut("/notes/\(slug)", baseURL: baseURL, body: ["content": newContent])
            return
        }
        // First-write: build canonical fresh content + POST /notes.
        newContent = Self.freshInboxNoteContent(title: title, dsl: dsl)
        struct CreateNoteReq: Encodable {
            let title: String
            let content: String
            let tags: [String]
        }
        let req = CreateNoteReq(title: title, content: newContent, tags: [])
        // Best-effort: if create races with the server's lazy-seeder
        // the dup-id throw is recoverable — fall back to PUT.
        do {
            let _: APINote = try await httpPostJSON("/notes", baseURL: baseURL, body: req)
        } catch {
            try await httpPut("/notes/\(slug)", baseURL: baseURL, body: ["content": newContent])
        }
    }

    /// Human-readable title derived from a saved-filter slug. `inbox`
    /// is the canonical default and reads as "Inbox"; everything else
    /// title-cases the slug with hyphens turned into spaces
    /// (`inbox-work` → "Inbox Work"). Mirrors the web's
    /// `titleForNewFilter` in `Inbox.svelte`.
    static func titleForInboxFilterSlug(_ slug: String) -> String {
        if slug == "inbox" { return "Inbox" }
        return slug.split(separator: "-")
            .filter { !$0.isEmpty }
            .map { word -> String in
                let first = word.prefix(1).uppercased()
                let rest = word.dropFirst()
                return first + rest
            }
            .joined(separator: " ")
    }

    /// Replace (or insert) the `query::` line in an existing Query
    /// note's content, preserving everything else. Used by
    /// `saveInboxDsl` when the note already exists.
    static func spliceInboxDsl(into content: String, dsl: String, title: String) -> String {
        // Split off frontmatter so we only touch the body.
        let parts = content.components(separatedBy: "---")
        // Expected shape: ["", "frontmatter content", "body content"...]
        guard content.hasPrefix("---") && parts.count >= 3 else {
            // No frontmatter — fall through to a fresh-body write,
            // discarding whatever was there. Shouldn't happen for
            // Query notes the server writes, but keeps the function
            // total.
            return freshInboxNoteContent(title: title, dsl: dsl)
        }
        let frontmatter = "---" + parts[1] + "---"
        // Re-join everything after the closing `---` so a `---` inside
        // the body doesn't get destructively split.
        let body = parts[2...].joined(separator: "---")
        var bodyLines = body.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        var found = false
        for i in 0..<bodyLines.count {
            let trimmed = bodyLines[i].trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("query::") {
                bodyLines[i] = "query:: \(dsl)"
                found = true
                break
            }
        }
        if !found {
            // No existing query:: line — append after the leading
            // blank line (or at the top if the body is empty).
            bodyLines.insert("query:: \(dsl)", at: bodyLines.firstIndex(where: { !$0.isEmpty }) ?? 0)
        }
        return frontmatter + bodyLines.joined(separator: "\n")
    }

    /// Build the full canonical content for a brand-new Inbox-style
    /// Query note. Used on first-write when no note exists yet.
    static func freshInboxNoteContent(title: String, dsl: String) -> String {
        """
        ---
        title: "\(title)"
        type: "Query"
        icon: "inbox"
        color: "teal"
        section: "saved"
        ---

        query:: \(dsl)

        """
    }

    /// Read-only relay status from the configured Mac server. iOS
    /// itself isn't a sync peer yet (UniFFI track is the deferred
    /// multi-week work), so this surfaces the picture from the Mac's
    /// perspective: "your Mac is paired with relay X, last poll N
    /// seconds ago." Returns `nil` when the server isn't reachable
    /// or when there's no `/sync/relay/status` endpoint on the other
    /// end (i.e. older server version).
    func fetchRelayStatus() async -> RelayStatusInfo? {
        guard case .http(let baseURL) = currentBackend else { return nil }
        do {
            return try await httpGet("/sync/relay/status", baseURL: baseURL)
        } catch {
            return nil
        }
    }

    /// Run an arbitrary query DSL via `POST /search/query`. The Inbox
    /// surface uses a saved filter's DSL (fetched via `fetchInboxDsl`)
    /// or `defaultInboxDsl()` for first-run. Returns an empty result on
    /// failure so the view renders the empty state instead of crashing.
    ///
    /// `.relay` evaluates the chip-registry token subset of the DSL
    /// locally over the sandbox notes (`LocalQueryEngine`, mirroring
    /// `block_matches`) — the Inbox was empty in relay mode because this
    /// method was `.http`-gated (2026-06-10 product test). JQL-grammar
    /// clauses beyond that subset are skipped locally (match-all).
    func executeQuery(_ dsl: String) async -> QueryResult {
        switch currentBackend {
        case .mock:
            return QueryResult(groups: [])
        case .relay:
            return localExecuteQuery(dsl)
        case .http(let baseURL):
            let body = APIExecuteQueryBody(dsl: dsl, group: nil, sort: nil)
            do {
                return try await httpPostJSON("/search/query", baseURL: baseURL, body: body)
            } catch {
                return QueryResult(groups: [])
            }
        }
    }

    /// Local-sandbox mirror of `POST /search/query` for block-kind
    /// queries. Page-kind queries (`kind:page`) aren't served locally —
    /// no iOS surface issues them today. Group/sort follow the iOS
    /// caller convention (both nil), which server-side yields a single
    /// ungrouped bucket with an empty key (`apply_group(None)`).
    private func localExecuteQuery(_ dsl: String) -> QueryResult {
        let parsed = LocalQueryEngine.parseSimpleDsl(dsl)
        guard parsed.kind == .block else { return QueryResult(groups: []) }
        var items: [QueryItem] = []
        for note in loadAllLocalNotes() {
            let blocks = parseBlocks(from: note.body, noteId: note.id)
            items += LocalQueryEngine.queryItems(
                blocks: blocks,
                noteId: note.id,
                noteTitle: note.title,
                pageNoteType: note.metadata.note_type,
                dsl: parsed
            )
        }
        return QueryResult(groups: [QueryGroup(key: "", items: items)])
    }

    // MARK: - Saved views (saved-views spec, 2026-06-10)

    /// All saved views, sorted `(order, id)` — the Inbox tab's switcher
    /// chips. `.relay` reads the engine's synced registry doc through the
    /// shell-wired seam; `.http` hits `GET /views`; `.mock` (and every
    /// failure path) serves the canonical builtin Inbox so the triage
    /// surface always has a working default. Never returns empty.
    func fetchViews() async -> [SavedView] {
        switch currentBackend {
        case .mock:
            return [SavedView.fallbackInbox]
        case .relay:
            guard let views = await onViewsList?(), !views.isEmpty else {
                return [SavedView.fallbackInbox]
            }
            return SavedViewLogic.sorted(views)
        case .http(let baseURL):
            do {
                let views: [SavedView] = try await httpGet("/views", baseURL: baseURL)
                return views.isEmpty ? [SavedView.fallbackInbox] : views
            } catch {
                return [SavedView.fallbackInbox]
            }
        }
    }

    /// Create (`isNew`) or update a saved view. `.relay` routes through
    /// the engine seam (record + relay push, so other devices converge);
    /// `.http` POSTs/PUTs the server's `/views` routes (which validate
    /// the DSL again and fan out `views_changed`). Throws on rejection —
    /// the editor sheet surfaces the message. `.mock` stays inert.
    func saveView(_ view: SavedView, isNew: Bool) async throws {
        switch currentBackend {
        case .mock:
            return
        case .relay:
            guard let upsert = onViewsUpsert else {
                throw URLError(
                    .cannotWriteToFile,
                    userInfo: [
                        NSLocalizedDescriptionKey:
                            "views engine seam not wired — view not saved"
                    ]
                )
            }
            try await upsert(view)
        case .http(let baseURL):
            if isNew {
                struct CreateViewReq: Encodable {
                    let id: String
                    let name: String
                    let dsl: String
                    let order: Int64
                    let display_mode: String
                    let display_group_by: String?
                    let display_show_done: Bool?
                }
                let req = CreateViewReq(
                    id: view.id,
                    name: view.name,
                    dsl: view.dsl,
                    order: view.order,
                    display_mode: view.displayMode,
                    display_group_by: view.displayGroupBy,
                    display_show_done: view.displayShowDone
                )
                let _: SavedView = try await httpPostJSON("/views", baseURL: baseURL, body: req)
            } else {
                var body: [String: Any] = [
                    "name": view.name,
                    "dsl": view.dsl,
                    "order": view.order,
                    "display_mode": view.displayMode,
                ]
                // Empty string clears the grouping key server-side; nil in
                // our record means "no grouping".
                body["display_group_by"] = view.displayGroupBy ?? ""
                if let showDone = view.displayShowDone {
                    body["display_show_done"] = showDone
                }
                try await httpPut("/views/\(view.id)", baseURL: baseURL, body: body)
            }
        }
    }

    /// Delete a saved view. Builtins are editable but never deletable —
    /// the UI hides the affordance, the engine and the server both
    /// enforce the guard, and this client-side pre-check turns a bypass
    /// into a clear local error instead of a backend round-trip.
    func deleteView(id: String) async throws {
        guard id != SavedView.builtinInboxId else {
            throw URLError(
                .cannotWriteToFile,
                userInfo: [
                    NSLocalizedDescriptionKey:
                        "the built-in Inbox cannot be deleted"
                ]
            )
        }
        switch currentBackend {
        case .mock:
            return
        case .relay:
            guard let delete = onViewsDelete else {
                throw URLError(
                    .cannotWriteToFile,
                    userInfo: [
                        NSLocalizedDescriptionKey:
                            "views engine seam not wired — view not deleted"
                    ]
                )
            }
            try await delete(id)
        case .http(let baseURL):
            try await httpDelete("/views/\(id)", baseURL: baseURL)
        }
    }

    /// Persist a new switcher order: each view's `order` becomes
    /// `(index + 1) * 10` (the server's reorder rule). `.http` posts the
    /// bare id array to `/views/reorder`; `.relay` upserts only the views
    /// whose order actually changed through the engine seam.
    func reorderViews(_ orderedViews: [SavedView]) async throws {
        switch currentBackend {
        case .mock:
            return
        case .relay:
            guard let upsert = onViewsUpsert else {
                throw URLError(
                    .cannotWriteToFile,
                    userInfo: [
                        NSLocalizedDescriptionKey:
                            "views engine seam not wired — order not saved"
                    ]
                )
            }
            for (idx, view) in orderedViews.enumerated() {
                let newOrder = Int64(idx + 1) * 10
                guard view.order != newOrder else { continue }
                var updated = view
                updated.order = newOrder
                try await upsert(updated)
            }
        case .http(let baseURL):
            try await httpPostNoResponse(
                "/views/reorder",
                baseURL: baseURL,
                body: orderedViews.map(\.id)
            )
        }
    }

    /// Direct write to `/blocks/set-property`. Used by Agenda + Inbox
    /// triage paths that already have the canonical server-side block
    /// id (`noteId:lineNumber`) from a query response — no need to look
    /// up via in-memory caches the way `setBlockProperties(id:)` does.
    ///
    /// `.relay` routes through the on-device engine instead (P1.11): the
    /// row address's suffix is resolved to the block's canonical bid
    /// against the local sandbox copy (the client mirror of the server's
    /// `block_bid_from_suffix`), `onLocalPropertySet` records the typed
    /// `BlockPropertySet` op via the FFI, and the engine re-materializes
    /// the file before the awaited seam returns — then a local refresh
    /// re-renders + bumps `refreshTick` (the `recurBump` pattern) so the
    /// Daily and the query-backed Agenda/Inbox freshen. Previously this
    /// was `.http`-gated: a relay-mode triage swipe / mark-done silently
    /// did NOTHING while the row optimistically vanished.
    func setBlockProperty(blockId: String, key: String, value: String) async throws {
        switch currentBackend {
        case .mock:
            return
        case .http(let baseURL):
            let body = APISetBlockPropertyBody(block_id: blockId, key: key, value: value)
            try await httpPostNoResponse("/blocks/set-property", baseURL: baseURL, body: body)
        case .relay:
            guard let (noteId, bid) = resolveLocalBlockBid(blockId) else {
                throw URLError(
                    .fileDoesNotExist,
                    userInfo: [
                        NSLocalizedDescriptionKey:
                            "block \(blockId) not found in the local mosaic"
                    ]
                )
            }
            let applied = await onLocalPropertySet?(noteId, bid, key, value) ?? false
            guard applied else {
                throw URLError(
                    .cannotWriteToFile,
                    userInfo: [
                        NSLocalizedDescriptionKey:
                            "engine property write for \(blockId) did not apply"
                    ]
                )
            }
            await refresh(from: currentBackend)
        }
    }

    /// Split a query-row block address (`<noteId>:<line>` or
    /// `<noteId>:<bid>`) into note id + suffix on the LAST colon —
    /// `rsplit_once(':')`, matching the server route's parsing. `nil`
    /// when either half would be empty.
    static func splitBlockAddress(_ blockId: String) -> (noteId: String, suffix: String)? {
        guard let sep = blockId.lastIndex(of: ":"), sep != blockId.startIndex else {
            return nil
        }
        let noteId = String(blockId[..<sep])
        let suffix = String(blockId[blockId.index(after: sep)...])
        guard !suffix.isEmpty else { return nil }
        return (noteId, suffix)
    }

    /// Resolve an address suffix to the block's canonical bid against a
    /// parsed block list — the client mirror of the server's
    /// `block_bid_from_suffix`: a numeric suffix is a 0-based body line
    /// matched on `lineNumber`; anything else is a bid passed through.
    /// `nil` when a numeric line matches no block. (A bid-less parsed
    /// line carries a parser-MINTED UUID — that resolves here but the
    /// engine reports it not-found downstream, which `setBlockProperty`
    /// surfaces as a throw instead of a silent drop.)
    static func blockBid(in blocks: [Block], suffix: String) -> String? {
        guard let line = Int(suffix) else { return suffix }
        return blocks.first(where: { $0.lineNumber == line })?.id
    }

    /// `<noteId>:<line|bid>` → `(noteId, canonical bid)` resolved against
    /// the local sandbox copy of the note (the same materialized file the
    /// relay-mode queries were answered from). `nil` when the address is
    /// malformed, the note has no local file, or no block sits at that
    /// line.
    private func resolveLocalBlockBid(_ blockId: String) -> (noteId: String, bid: String)? {
        guard let (noteId, suffix) = Self.splitBlockAddress(blockId) else { return nil }
        if Int(suffix) == nil { return (noteId, suffix) }
        guard let note = readLocalNote(id: noteId) else { return nil }
        let blocks = parseBlocks(from: note.body, noteId: noteId)
        guard let bid = Self.blockBid(in: blocks, suffix: suffix) else { return nil }
        return (noteId, bid)
    }

    // MARK: - Internal test hooks

    /// Exposes `parseBlocks(from:noteId:)` to the `@testable` unit-test
    /// target. Not part of the public service contract.
    func testableParseBlocks(from body: String, noteId: String) -> [Block] {
        parseBlocks(from: body, noteId: noteId)
    }

    /// Exposes `renderBody(from:)` to the `@testable` unit-test target so
    /// we can verify round-tripping (including multi-line continuation
    /// lines) through parse → render without going over HTTP.
    func testableRenderBody(from blocks: [Block]) -> String {
        renderBody(from: blocks)
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
