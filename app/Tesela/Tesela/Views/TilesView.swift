import SwiftUI

// MARK: - TilesView
// Scrollable timeline of daily notes — like Logseq's journal page.
// Each "tile" is a day with its note content shown inline.

struct TilesView: View {
    @Environment(AppState.self) private var appState
    @State private var dailyNotes: [Page] = []
    @State private var isLoading = false

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                // Today tile — always present
                TodayHeader(dailyNotes: dailyNotes)
                    .padding(.horizontal, 24)
                    .padding(.top, 16)

                // Past daily notes
                ForEach(dailyNotes) { page in
                    TileCard(page: page)
                        .onTapGesture { appState.open(page) }
                }
            }
            .padding(.bottom, 24)
        }
        .task { await loadDailyNotes() }
        .refreshable { await loadDailyNotes() }
    }

    private func loadDailyNotes() async {
        isLoading = true
        defer { isLoading = false }
        let notes = (try? await appState.api.listNotes(tag: "daily", limit: 100)) ?? []
        // Sort by ID descending — IDs are "2026-03-21" format so lexicographic = reverse chronological
        dailyNotes = notes.sorted { $0.id > $1.id }
    }
}

// MARK: - TodayHeader
private struct TodayHeader: View {
    let dailyNotes: [Page]
    @Environment(AppState.self) private var appState

    private var todayID: String {
        let fmt = DateFormatter()
        fmt.dateFormat = "yyyy-MM-dd"
        return fmt.string(from: Date())
    }

    private var hasTodayTile: Bool {
        dailyNotes.contains { $0.id == todayID }
    }

    var body: some View {
        if !hasTodayTile {
            Button {
                Task {
                    if let page = try? await appState.api.getDailyNote() {
                        appState.open(page)
                    }
                }
            } label: {
                HStack {
                    Image(systemName: "plus.square.dashed")
                    Text("Lay today's tile")
                        .font(.headline)
                }
                .foregroundStyle(Color.accentColor)
            }
            .buttonStyle(.plain)
            .padding(.bottom, 12)
        }
    }
}

// MARK: - TileCard
private struct TileCard: View {
    let page: Page

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Divider()
                .padding(.horizontal, 24)

            VStack(alignment: .leading, spacing: 8) {
                // Date header
                Text(formattedDate)
                    .font(.title3)
                    .bold()
                    .foregroundStyle(.primary)

                // Block content preview
                let blocks = BlockParser.flatten(blocks: BlockParser.parse(markdown: page.body))
                if blocks.isEmpty {
                    Text("Empty tile")
                        .font(.body)
                        .foregroundStyle(.tertiary)
                        .italic()
                } else {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(Array(blocks.prefix(10).enumerated()), id: \.offset) { _, block in
                            HStack(alignment: .top, spacing: 6) {
                                Text("•")
                                    .foregroundStyle(.tertiary)
                                    .padding(.leading, CGFloat(block.indentLevel) * 16)
                                Text(block.text)
                                    .font(.body)
                                    .foregroundStyle(.primary)
                            }
                        }
                        if blocks.count > 10 {
                            Text("…\(blocks.count - 10) more")
                                .font(.caption)
                                .foregroundStyle(.tertiary)
                        }
                    }
                }
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 16)
            .contentShape(Rectangle())
        }
    }

    private var formattedDate: String {
        let inputFmt = DateFormatter()
        inputFmt.dateFormat = "yyyy-MM-dd"
        guard let date = inputFmt.date(from: page.id) else { return page.title }

        let outputFmt = DateFormatter()
        outputFmt.dateFormat = "EEEE, MMMM d, yyyy"
        return outputFmt.string(from: date)
    }
}
