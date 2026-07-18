import SwiftUI

/// Graphite Library — the reference surface, re-themed over the SAME
/// `MockMosaicService` collections the legacy `LibraryView` reads
/// (`pages`, `tags`, `recent`, `pinned`). No data layer is rebuilt: the
/// segmented control filters `mosaic.pages` exactly like LibraryView's
/// `LibraryFilter`, and tapping any page/tag pushes `GrPageView` on the
/// shared `NavigationStack` (mirroring the daily/page nav).
///
/// Only the chrome is new: a `GrHeader`, the mobile `.grm-seg` segmented
/// control, the `.grm-grid` workspace-ambient card grid, and `GrWidget`
/// Pinned/Recent lists. Reads `@Environment(\.theme)` (forced to
/// `.graphite` by `GrAppShell`).
struct GrLibraryView: View {
    @ObservedObject var mosaic: MockMosaicService
    var backend: BackendSettings? = nil

    @Environment(\.theme) private var theme

    @State private var segment: Segment = .workspace
    @State private var navigationPath = NavigationPath()

    enum Segment: String, CaseIterable, Identifiable {
        case workspace, pages, tags
        var id: String { rawValue }
        var label: String {
            switch self {
            case .workspace: return "Workspace"
            case .pages:     return "Pages"
            case .tags:      return "Tags"
            }
        }
    }

    var body: some View {
        NavigationStack(path: $navigationPath) {
            VStack(spacing: 0) {
                GrHeader(title: "Library", subtitle: "REFERENCE")
                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        Spacer().frame(height: 10)
                        segmentBar
                        content
                        Spacer().frame(height: 96)
                    }
                    .padding(.horizontal, 14)
                }
                .refreshable {
                    _ = await mosaic.refreshAttachedBackend()
                }
            }
            .background(theme.bg)
            .navigationDestination(for: GrPageRoute.self) { route in
                GrPageView(slug: route.slug, mosaic: mosaic, path: $navigationPath)
                    .environment(\.theme, theme)
            }
        }
    }

    // ── Segmented control (.grm-seg) ────────────────────────────────────

    private var segmentBar: some View {
        HStack(spacing: 4) {
            ForEach(Segment.allCases) { seg in
                let on = (segment == seg)
                Button {
                    segment = seg
                } label: {
                    Text(seg.label)
                        .font(.system(size: 12.5, weight: on ? .semibold : .regular))
                        .foregroundStyle(on ? theme.fgDefault : theme.fgMuted)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 7)
                        .background(on ? theme.bg3 : .clear)
                        .clipShape(RoundedRectangle(cornerRadius: 7))
                }
                .buttonStyle(.plain)
            }
        }
        .padding(3)
        .background(theme.bg2)
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(theme.lineSoft, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 10))
    }

    @ViewBuilder
    private var content: some View {
        switch segment {
        case .workspace: workspace
        case .pages:     pagesList
        case .tags:      tagsList
        }
    }

    // ── Workspace ambient grid (.grm-grid + .grm-acard) ─────────────────

    private struct Ambient: Identifiable {
        let id = UUID()
        let icon: String
        let title: String
        let hint: String
        let tintKind: String
        var soon: Bool = false
    }

    private let ambients: [Ambient] = [
        Ambient(icon: "calendar", title: "Calendar", hint: "tap a day → daily", tintKind: "event"),
        Ambient(icon: "square-check", title: "In Progress", hint: "open tasks across the mosaic", tintKind: "query"),
        Ambient(icon: "folder", title: "Dashboard", hint: "pinned widgets", tintKind: "project"),
        Ambient(icon: "flame", title: "AI", hint: "coming later", tintKind: "person", soon: true),
    ]

    private var workspace: some View {
        VStack(alignment: .leading, spacing: 16) {
            LazyVGrid(
                columns: [GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)],
                spacing: 12
            ) {
                ForEach(ambients) { a in
                    if a.title == "Dashboard" {
                        NavigationLink {
                            GrDashboardView(mosaic: mosaic, path: $navigationPath)
                                .environment(\.theme, theme)
                        } label: {
                            ambientCard(a)
                        }
                        .buttonStyle(.plain)
                    } else {
                        ambientCard(a)
                    }
                }
            }
            pinnedWidget
            recentWidget
        }
    }

    private func ambientCard(_ a: Ambient) -> some View {
        let tint = theme.typeColor(forKind: a.tintKind)
        return VStack(alignment: .leading, spacing: 10) {
            HStack {
                ZStack {
                    RoundedRectangle(cornerRadius: 9)
                        .fill(tint.opacity(0.18))
                        .frame(width: 38, height: 38)
                    GrIcon(name: a.icon, size: 19)
                        .foregroundStyle(tint)
                }
                Spacer()
                if a.soon {
                    Text("soon")
                        .font(.system(size: 9.5, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(theme.bg)
                        .clipShape(Capsule())
                }
            }
            Text(a.title)
                .font(.system(size: 14.5, weight: .semibold))
                .foregroundStyle(theme.fgDefault)
            Text(a.hint)
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
                .lineLimit(2)
        }
        .frame(maxWidth: .infinity, minHeight: 112, alignment: .topLeading)
        .padding(13)
        .background(theme.bg3)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(theme.line, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    private var pinnedWidget: some View {
        Group {
            if !mosaic.pinned.isEmpty {
                GrWidget(title: "Pinned", icon: "pin", badge: "\(mosaic.pinned.count)") {
                    ForEach(mosaic.pinned) { entry in
                        GrRow(icon: "file-text", label: entry.title) {
                            navigationPath.append(GrPageRoute(slug: entry.id))
                        }
                    }
                }
            }
        }
    }

    private var recentWidget: some View {
        Group {
            if !mosaic.recent.isEmpty {
                GrWidget(title: "Recent", icon: "clock", badge: "\(mosaic.recent.count)") {
                    ForEach(mosaic.recent) { entry in
                        GrRow(icon: "file-text", label: entry.title, meta: entry.at) {
                            navigationPath.append(GrPageRoute(slug: entry.id))
                        }
                    }
                }
            }
        }
    }

    // ── Pages list ──────────────────────────────────────────────────────

    private var pagesList: some View {
        let visible = mosaic.pages.filter { !$0.hidden }
        return VStack(spacing: 0) {
            if visible.isEmpty {
                emptyState(title: "No pages yet", hint: "Capture something — it lands here.")
            } else {
                ForEach(visible) { page in
                    Button {
                        navigationPath.append(GrPageRoute(slug: page.id))
                    } label: {
                        pageRow(page)
                    }
                    .buttonStyle(.plain)
                    Divider().overlay(theme.lineSoft)
                }
            }
        }
        .background(theme.bg2)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(theme.lineSoft, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    private func pageRow(_ page: Page) -> some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(page.title)
                    .font(.system(size: 14.5))
                    .foregroundStyle(theme.fgDefault)
                    .lineLimit(1)
                Text("notes/\(page.slug).md")
                    .font(.system(size: 10.5, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
                    .lineLimit(1)
            }
            Spacer(minLength: 8)
            VStack(alignment: .trailing, spacing: 5) {
                GrTypeTag(kind: page.type)
                if !page.edited.isEmpty {
                    Text(page.edited)
                        .font(.system(size: 10.5, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                }
            }
        }
        .padding(.horizontal, 13)
        .padding(.vertical, 11)
        .contentShape(Rectangle())
    }

    // ── Tags list ─────────────────────────────────────────────────────

    private var tagsList: some View {
        VStack(spacing: 0) {
            if mosaic.tags.isEmpty {
                emptyState(title: "No tags yet", hint: "Add #tags to blocks — they collect here.")
            } else {
                ForEach(mosaic.tags) { tag in
                    Button {
                        navigationPath.append(GrPageRoute(slug: tag.slug))
                    } label: {
                        tagRow(tag)
                    }
                    .buttonStyle(.plain)
                    Divider().overlay(theme.lineSoft)
                }
            }
        }
        .background(theme.bg2)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(theme.lineSoft, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    private func tagRow(_ tag: Tag) -> some View {
        HStack(spacing: 12) {
            HStack(spacing: 0) {
                Text("#")
                    .foregroundStyle(theme.fgFaint)
                if let parent = tag.parent {
                    Text("\(parent)/")
                        .foregroundStyle(theme.fgFaint)
                }
                Text(tag.title)
                    .foregroundStyle(theme.fgDefault)
            }
            .font(.system(size: 13.5, design: .monospaced))
            .lineLimit(1)
            Spacer(minLength: 8)
            Text("\(tag.count) refs")
                .font(.system(size: 10.5, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
        }
        .padding(.horizontal, 13)
        .padding(.vertical, 11)
        .contentShape(Rectangle())
    }

    private func emptyState(title: String, hint: String) -> some View {
        VStack(spacing: 8) {
            Text(title)
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(theme.fgDefault)
            Text(hint)
                .font(.system(size: 11.5, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 40)
        .padding(.horizontal, 24)
    }
}
