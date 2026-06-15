import SwiftUI

/// Renders an inline `query:: <dsl>` block as a live results widget — the iOS
/// parity for the web's QueryBlock. Runs the DSL through the (typed, L5-aware)
/// query engine via `MockMosaicService.executeQuery` and lists the matching
/// blocks. List-only: no table/kanban view switcher (that's web-only, same as
/// the Inbox surface), so a `view:: table` companion property is noted, not
/// laid out.
///
/// Plain `VStack`/`ForEach` (NOT a nested `List`/`ScrollView`) because it lives
/// inside `GrPageView`'s outer scroll view; nesting scrollers breaks sizing.
struct GrQueryWidget: View {
    let dsl: String
    @ObservedObject var mosaic: MockMosaicService
    @Binding var path: NavigationPath
    @Environment(\.theme) private var theme

    @State private var rows: [QueryItem] = []
    @State private var loading = true

    /// Cap rendered rows so a broad query can't blow up the page. Mirrors the
    /// Inbox surface's 200-row cap.
    private let rowCap = 200

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            header
            if loading {
                Text("running…")
                    .font(.system(size: 12))
                    .foregroundStyle(theme.fgFaint)
                    .padding(.vertical, 2)
            } else if rows.isEmpty {
                Text("No matches")
                    .font(.system(size: 12))
                    .foregroundStyle(theme.fgFaint)
                    .padding(.vertical, 2)
            } else {
                ForEach(rows) { row in
                    resultRow(row)
                }
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(theme.bg2)
        .overlay(
            RoundedRectangle(cornerRadius: 11)
                .stroke(theme.lineSoft, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 11))
        .task(id: dsl) { await runQuery() }
    }

    // ── Header (DSL + match count) ──────────────────────────────────────────

    private var header: some View {
        HStack(spacing: 8) {
            GrIcon(name: "search", size: 12)
                .foregroundStyle(theme.fgFaint)
            Text(dsl)
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(theme.fgSubtle)
                .lineLimit(1)
                .truncationMode(.middle)
            Spacer(minLength: 4)
            if !loading {
                Text("\(rows.count)")
                    .font(.system(size: 10.5, weight: .medium, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }
        }
    }

    // ── Result row (compact card) ───────────────────────────────────────────

    private func resultRow(_ row: QueryItem) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(row.text.isEmpty ? "(empty block)" : row.text)
                .font(.system(size: 13.5))
                .foregroundStyle(theme.fgDefault)
                .multilineTextAlignment(.leading)
            HStack(spacing: 8) {
                metaPill("in \(row.title.isEmpty ? row.page_id : row.title)")
                if let tag = row.primary_tag {
                    metaPill("#\(tag)")
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, 6)
        .contentShape(Rectangle())
        .onTapGesture {
            guard !row.page_id.isEmpty else { return }
            path.append(GrPageRoute(slug: row.page_id))
        }
    }

    private func metaPill(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 10.5, design: .monospaced))
            .foregroundStyle(theme.fgSubtle)
            .lineLimit(1)
            .truncationMode(.middle)
            .padding(.horizontal, 7)
            .padding(.vertical, 2)
            .background(theme.bg4)
            .clipShape(RoundedRectangle(cornerRadius: 5))
    }

    // ── Run ─────────────────────────────────────────────────────────────────

    private func runQuery() async {
        loading = true
        let result = await mosaic.executeQuery(dsl)
        var collected: [QueryItem] = []
        outer: for group in result.groups {
            for item in group.items where item.kind == .block {
                collected.append(item)
                if collected.count >= rowCap { break outer }
            }
        }
        rows = collected
        loading = false
    }
}
