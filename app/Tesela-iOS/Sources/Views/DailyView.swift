import SwiftUI

/// The Daily front door. Mirrors the canvas's Tile-A screen:
/// top bar with "Today" + date + sync pill, vertical scrolling stack
/// of today's blocks, yesterday section underneath. Capture bar +
/// bottom tab bar live in `AppShell` so they share chrome with Library
/// and Search.
struct DailyView: View {
    @ObservedObject var mosaic: MockMosaicService
    /// Optional — when provided, drives pull-to-refresh and the
    /// dynamic sync dot in the top bar.
    var backend: BackendSettings? = nil

    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 0) {
            DailyTopBar(
                title: mosaic.todayLongLabel,
                dateLabel: mosaic.todayLabel,
                syncStatus: syncStatus
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
            .refreshable {
                if let backend {
                    await mosaic.refresh(from: backend.backend)
                }
            }
        }
        .background(theme.bg)
    }

    /// Maps the mosaic's HTTP connection state to a dot color.
    private var syncStatus: DailyTopBar.SyncDotState {
        switch mosaic.connection {
        case .ready, .idle:    return .ok
        case .connecting:      return .warn
        case .failed:          return .err
        }
    }
}
