import SwiftUI

/// The Daily front door. Mirrors the canvas's Tile-A screen:
/// top bar with "Today" + date + sync pill, vertical scrolling stack
/// of today's blocks, yesterday section underneath. Capture bar +
/// bottom tab bar live in `AppShell` so they share chrome with Library
/// and Search.
struct DailyView: View {
    @ObservedObject var mosaic: MockMosaicService

    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 0) {
            DailyTopBar(
                title: mosaic.todayLongLabel,
                dateLabel: mosaic.todayLabel
            )
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    Spacer().frame(height: 12)

                    ForEach(mosaic.todayBlocks) { block in
                        BlockRow(
                            id: block.id,
                            kind: block.kind,
                            text: block.text,
                            indent: block.indent,
                            isDone: block.done,
                            tags: block.tags,
                            onToggleTask: { mosaic.toggleTask(id: block.id) }
                        )
                    }

                    SectionEyebrow(title: "Yesterday")

                    ForEach(mosaic.yesterdayBlocks) { block in
                        BlockRow(
                            id: block.id,
                            kind: block.kind,
                            text: block.text,
                            indent: block.indent,
                            isDone: block.done,
                            tags: block.tags,
                            onToggleTask: { mosaic.toggleTask(id: block.id) }
                        )
                        .opacity(0.7)
                    }

                    Spacer().frame(height: 24)
                }
            }
        }
        .background(theme.bg)
    }
}
