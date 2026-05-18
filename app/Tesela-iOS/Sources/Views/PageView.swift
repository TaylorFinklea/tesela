import SwiftUI

/// PageView — the page-renderer host. Mirrors the canvas's Tile-Page
/// screen with:
///   • Title chrome (kind badge · slug · title · meta)
///   • PageTagsChips strip directly under the title (decision #8)
///   • Page body (block list)
///   • Collapsible derived-only Peek segmented control (decision #7)
///
/// Pushed onto a NavigationStack from LibraryView or wiki-link taps.
struct PageView: View {
    let page: Page
    @ObservedObject var mosaic: MockMosaicService

    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss
    @State private var tags: [String] = []
    @State private var peekOpen: Bool = false
    @State private var peekSegment: PeekSegment = .backlinks

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                titleChrome
                PageTagsChips(
                    pageId: page.id,
                    tags: $tags,
                    knownTags: mosaic.tags.map { $0.title }
                )
                Divider().background(theme.lineSoft)
                pageBody
                peekSection
                Spacer().frame(height: 24)
            }
        }
        .background(theme.bg)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    // Phase 8 — pin
                } label: {
                    Icon(name: .pin, size: 20)
                        .foregroundStyle(theme.fgMuted)
                }
            }
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    // Phase 8 — properties sheet entry
                } label: {
                    Icon(name: .more, size: 20)
                        .foregroundStyle(theme.fgMuted)
                }
            }
        }
        .onAppear {
            // Mock: pre-populate with a couple of tags so the strip
            // demonstrates itself. Real data lands in Phase 15.
            if tags.isEmpty {
                tags = page.type == "note" ? ["prism", "tesela"] : []
            }
        }
    }

    // ── Title chrome ────────────────────────────────────────────────────

    private var titleChrome: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                KindBadge(kind: page.type)
                Text("notes/\(page.slug).md")
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgSubtle)
                Spacer()
            }
            Text(page.title)
                .font(.system(size: 26, weight: .semibold))
                .tracking(-0.26)
                .foregroundStyle(theme.fgDefault)
            Text("\(page.blocks) blocks · \(page.refs) refs · edited \(page.edited)")
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
        }
        .padding(.horizontal, 18)
        .padding(.top, 12)
    }

    // ── Body ────────────────────────────────────────────────────────────

    private var pageBody: some View {
        VStack(alignment: .leading, spacing: 0) {
            Spacer().frame(height: 12)
            ForEach(Array(page.body.enumerated()), id: \.offset) { _, line in
                BlockRow(
                    id: UUID().uuidString,
                    kind: .note,
                    text: line,
                    indent: 0,
                    isDone: false,
                    tags: []
                )
            }
            // A couple of stub blocks so a tag-light page still looks
            // like a real page in the mock. Real bodies come in Phase 15.
            if page.body.isEmpty {
                BlockRow(
                    id: "stub-1", kind: .note,
                    text: "Body content placeholder — Phase 15 wires the real markdown body through FFI.",
                    tags: []
                )
            }
        }
    }

    // ── Peek (derived-only) ─────────────────────────────────────────────

    private var peekSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Toggle row
            Button {
                withAnimation(.easeInOut(duration: 0.18)) { peekOpen.toggle() }
            } label: {
                HStack(spacing: 6) {
                    Icon(name: peekOpen ? .chevDown : .chevRight, size: 14)
                        .foregroundStyle(theme.fgSubtle)
                    Text("Peek")
                        .font(.system(size: 10.5, design: .monospaced))
                        .tracking(0.8)
                        .foregroundStyle(theme.fgSubtle)
                    Spacer()
                    Text("derived · 5 lenses")
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                }
                .padding(.horizontal, 18)
                .padding(.vertical, 10)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            if peekOpen {
                segmentedControl
                Divider().background(theme.lineSoft)
                segmentBody
            }
        }
        .background(
            peekOpen ? theme.bg2.opacity(0.5) : Color.clear
        )
        .overlay(alignment: .top) {
            Rectangle().fill(theme.lineSoft).frame(height: 1)
        }
        .padding(.top, 16)
    }

    private var segmentedControl: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 18) {
                ForEach(PeekSegment.allCases) { seg in
                    Button {
                        peekSegment = seg
                    } label: {
                        HStack(spacing: 4) {
                            Text(seg.label)
                            if let count = seg.count(mosaic) {
                                Text(String(count))
                                    .foregroundStyle(theme.fgFaint)
                            }
                        }
                        .font(.system(size: 11.5, weight: peekSegment == seg ? .semibold : .regular, design: .monospaced))
                        .foregroundStyle(peekSegment == seg ? theme.accentPrimary : theme.fgSubtle)
                        .padding(.vertical, 10)
                        .overlay(alignment: .bottom) {
                            Rectangle()
                                .fill(peekSegment == seg ? theme.accentPrimary : .clear)
                                .frame(height: 2)
                                .offset(y: 1)
                        }
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 18)
        }
    }

    @ViewBuilder
    private var segmentBody: some View {
        switch peekSegment {
        case .backlinks: BacklinksView(mosaic: mosaic)
        case .outline:   OutlineLensView(mosaic: mosaic)
        case .props:     PropsLensView(page: page, tags: tags)
        case .tasks:     TasksLensView()
        case .graph:     GraphLensView()
        }
    }
}

// MARK: - Peek segments

enum PeekSegment: String, CaseIterable, Identifiable {
    case backlinks, outline, props, tasks, graph

    var id: String { rawValue }
    var label: String { rawValue }

    @MainActor
    func count(_ mosaic: MockMosaicService) -> Int? {
        switch self {
        case .backlinks: return mosaic.backlinks.count
        case .outline:   return mosaic.outline.count
        case .tasks:     return mosaic.todayBlocks.filter { $0.kind == .task }.count
        case .props, .graph: return nil
        }
    }
}

// MARK: - Derived lens views

struct BacklinksView: View {
    @ObservedObject var mosaic: MockMosaicService
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(mosaic.backlinks) { b in
                VStack(alignment: .leading, spacing: 4) {
                    Text(b.from)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(theme.accentPrimary)
                    Text(b.snippet)
                        .font(.system(size: 13.5))
                        .foregroundStyle(theme.fgMuted)
                        .lineSpacing(2)
                }
                .padding(.horizontal, 18)
                .padding(.vertical, 10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .overlay(alignment: .bottom) {
                    Rectangle().fill(theme.lineSoft).frame(height: 1)
                }
            }
        }
    }
}

struct OutlineLensView: View {
    @ObservedObject var mosaic: MockMosaicService
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(mosaic.outline) { o in
                Text(o.text)
                    .font(.system(size: 14))
                    .foregroundStyle(o.depth == 0 ? theme.fgDefault : theme.fgMuted)
                    .padding(.leading, CGFloat(18 + o.depth * 18))
                    .padding(.trailing, 18)
                    .padding(.vertical, 8)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .overlay(alignment: .bottom) {
                        Rectangle().fill(theme.lineSoft).frame(height: 1)
                    }
            }
        }
    }
}

struct PropsLensView: View {
    let page: Page
    let tags: [String]
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            propRow(key: "type",    value: page.type)
            propRow(key: "slug",    value: page.slug)
            propRow(key: "created", value: "2026-05-15 09:24")
            propRow(key: "edited",  value: "2026-05-17 12:14")
            HStack(alignment: .top, spacing: 14) {
                Text("tags")
                    .frame(width: 80, alignment: .leading)
                    .foregroundStyle(theme.fgFaint)
                if tags.isEmpty {
                    Text("—").foregroundStyle(theme.fgFaint)
                } else {
                    HStack(spacing: 4) {
                        ForEach(tags, id: \.self) { TagChip(value: $0) }
                    }
                }
                Spacer()
            }
            propRow(key: "refs", value: "\(page.refs) in")
        }
        .font(.system(size: 12, design: .monospaced))
        .padding(.horizontal, 18)
        .padding(.vertical, 14)
    }

    private func propRow(key: String, value: String) -> some View {
        HStack(alignment: .top, spacing: 14) {
            Text(key)
                .frame(width: 80, alignment: .leading)
                .foregroundStyle(theme.fgFaint)
            Text(value)
                .foregroundStyle(theme.fgDefault)
            Spacer()
        }
    }
}

struct TasksLensView: View {
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            BlockRow(id: "tlv-1", kind: .task, text: "Decide iOS Peek surface — bottom sheet vs segmented", isDone: false, tags: ["#tesela/ios"])
            BlockRow(id: "tlv-2", kind: .task, text: "Lock buffer-kind invariant in v5", isDone: true, tags: [])
        }
    }
}

struct GraphLensView: View {
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("local-graph-of-page · 1-hop · 14 pages")
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(theme.fgSubtle)
            RoundedRectangle(cornerRadius: 8)
                .fill(theme.bg3)
                .overlay(
                    RoundedRectangle(cornerRadius: 8)
                        .stroke(theme.line, lineWidth: 1)
                )
                .frame(height: 220)
                .overlay {
                    Text("[ graph render ]")
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                }
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 14)
    }
}
