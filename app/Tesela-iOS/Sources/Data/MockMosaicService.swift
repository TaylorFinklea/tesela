import Foundation
import Combine

/// Mock mosaic backed by a realistic in-memory snapshot. Lets every
/// view render with believable content before the Rust FFI surface
/// expands to expose page/block/tag operations (Phase 15).
///
/// Mirrors the design canvas's `data.jsx`. Same shape so swapping in
/// the eventual `FFIMosaicService` is mechanical.
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

    init() {
        let today = Date()
        self.todayDate = today

        self.todayBlocks = [
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

        self.yesterdayBlocks = [
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

        self.pages = [
            Page(id: "prism-v5-chrome", title: "Prism v5 chrome",
                 slug: "prism-v5-chrome", type: "note", edited: "today",
                 blocks: 38, refs: 12,
                 body: [
                    "The chrome replaces v4's five-pane grab-bag with a tightly-typed buffer set: **page**, **derived**, **ambient**.",
                    "**Invariant.** Page buffers render exactly one filesystem-backed page. Derived buffers are pure functions of a reference. Ambient buffers are workspace singletons.",
                    "Renderers are host-agnostic — Peek and pane mount the same renderer with no host knowledge.",
                    "On iOS the binary pane tree collapses to one focused page at a time. Peek does the lifting.",
                 ]),
            Page(id: "tag-system", title: "Tag system",
                 slug: "tag-system", type: "note", edited: "yesterday",
                 blocks: 27, refs: 8),
            Page(id: "ios-design-brief", title: "iPhone design brief",
                 slug: "ios-design-brief", type: "note", edited: "2d",
                 blocks: 19, refs: 5),
            Page(id: "maya-conversations", title: "Maya · conversations",
                 slug: "maya-conversations", type: "person", edited: "3d",
                 blocks: 64, refs: 22),
            Page(id: "tesela-ios", title: "Tesela iOS",
                 slug: "tesela-ios", type: "project", edited: "today",
                 blocks: 41, refs: 18),
            Page(id: "open-tasks", title: "Open tasks",
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
            Page(id: "kc-meetup", title: "KC meetup · May",
                 slug: "kc-meetup", type: "event", edited: "1w",
                 blocks: 6, refs: 2),
        ]

        self.tags = [
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

        self.recent = [
            RecentEntry(id: "prism-v5-chrome",   title: "Prism v5 chrome",       at: "12m"),
            RecentEntry(id: "tag-system",        title: "Tag system",            at: "1h"),
            RecentEntry(id: "tesela-ios",        title: "Tesela iOS",            at: "3h"),
            RecentEntry(id: "maya-conversations", title: "Maya · conversations", at: "yesterday"),
        ]

        self.pinned = [
            PinnedEntry(id: "tesela-ios",       title: "Tesela iOS"),
            PinnedEntry(id: "open-tasks",       title: "Open tasks"),
            PinnedEntry(id: "ios-design-brief", title: "iPhone design brief"),
        ]

        self.palette = [
            PaletteVerb(id: ":daily",          hint: "Open today's daily"),
            PaletteVerb(id: ":scratch",        hint: "Start a scratch page"),
            PaletteVerb(id: ":promote",        hint: "Promote scratch → note"),
            PaletteVerb(id: ":rename-slug",    hint: "Rename current page's slug"),
            PaletteVerb(id: ":convert-to-tag", hint: "Convert current note → tag"),
            PaletteVerb(id: ":sync now",       hint: "Sync once with reachable peers"),
            PaletteVerb(id: ":graph",          hint: "Open workspace graph"),
        ]

        self.searchResults = [
            SearchResult(id: "r0", kind: .page, title: "Prism v5 chrome",
                snippet: "...the chrome replaces v4's five-pane grab-bag with a tightly-typed **buffer** set..."),
            SearchResult(id: "r1", kind: .block, title: "Today",
                snippet: "Read [[Prism v5 chrome]] sections on derived **buffer**s..."),
            SearchResult(id: "r2", kind: .page, title: "Tesela iOS",
                snippet: "...the iPhone collapses the binary pane tree into one focused **buffer** at a time..."),
            SearchResult(id: "r3", kind: .tag, title: "prism",
                snippet: "...22 references across 14 pages — most-recent ones cluster around the v5 **buffer** cutover..."),
        ]

        self.backlinks = [
            Backlink(id: UUID(), from: "Tesela iOS",
                snippet: "...the Peek surface lifts directly from [[Prism v5 chrome]]'s host-agnostic renderer..."),
            Backlink(id: UUID(), from: "iPhone design brief",
                snippet: "...read [[Prism v5 chrome]] end-to-end. It locks the platform/interaction decisions..."),
            Backlink(id: UUID(), from: "2026-05-17",
                snippet: "Read [[Prism v5 chrome]] sections on derived buffers..."),
            Backlink(id: UUID(), from: "Maya · conversations",
                snippet: "Maya: 'the [[Prism v5 chrome]] doc reads like a contract'..."),
        ]

        self.outline = [
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

    func toggleTask(id: String) {
        if let idx = todayBlocks.firstIndex(where: { $0.id == id }), todayBlocks[idx].kind == .task {
            todayBlocks[idx].done.toggle()
        } else if let idx = yesterdayBlocks.firstIndex(where: { $0.id == id }), yesterdayBlocks[idx].kind == .task {
            yesterdayBlocks[idx].done.toggle()
        }
    }

    func capture(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        let block = Block(
            id: "captured-\(UUID().uuidString.prefix(6))",
            kind: .note,
            text: trimmed
        )
        // Prepend to today (matches the web/decision: capture lands at
        // the top of today's daily).
        todayBlocks.insert(block, at: 0)
    }

    func search(_ query: String) -> [SearchResult] {
        let q = query.lowercased().trimmingCharacters(in: .whitespaces)
        guard !q.isEmpty else { return searchResults }
        return searchResults.filter {
            $0.title.lowercased().contains(q) || $0.snippet.lowercased().contains(q)
        }
    }
}
