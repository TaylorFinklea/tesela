import SwiftUI

/// The 4-card grid revealed when the Workspace filter chip is active
/// in Library. Each card opens its ambient buffer in a pushed
/// destination. Per decision #5.
struct WorkspaceGridView: View {
    @ObservedObject var mosaic: MockMosaicService
    @Environment(\.theme) private var theme

    var body: some View {
        ScrollView {
            LazyVGrid(columns: [
                GridItem(.flexible(), spacing: 12),
                GridItem(.flexible(), spacing: 12)
            ], spacing: 12) {
                NavigationLink(value: AmbientRoute.calendar) {
                    AmbientCard(
                        icon: .cal,
                        title: "Calendar",
                        hint: "tap a day → daily",
                        tint: theme.typeEvent
                    )
                }
                .buttonStyle(.plain)
                NavigationLink(value: AmbientRoute.inProgress) {
                    AmbientCard(
                        icon: .check,
                        title: "In Progress",
                        hint: "open tasks across the mosaic",
                        tint: theme.typeQuery
                    )
                }
                .buttonStyle(.plain)
                NavigationLink(value: AmbientRoute.dashboard) {
                    AmbientCard(
                        icon: .archive,
                        title: "Dashboard",
                        hint: "pinned widgets",
                        tint: theme.typeProject
                    )
                }
                .buttonStyle(.plain)
                NavigationLink(value: AmbientRoute.ai) {
                    AmbientCard(
                        icon: .sparkles,
                        title: "AI",
                        hint: "coming later",
                        tint: theme.accentSecondary,
                        comingSoon: true
                    )
                }
                .buttonStyle(.plain)
            }
            .padding(16)
        }
        .background(theme.bg)
        .navigationDestination(for: AmbientRoute.self) { route in
            switch route {
            case .calendar:   CalendarAmbientView(mosaic: mosaic)
            case .inProgress: InProgressAmbientView(mosaic: mosaic)
            case .dashboard:  DashboardAmbientView(mosaic: mosaic)
            case .ai:         AIAmbientView()
            }
        }
    }
}

enum AmbientRoute: Hashable {
    case calendar, inProgress, dashboard, ai
}

// MARK: - Card

struct AmbientCard: View {
    let icon: IconName
    let title: String
    let hint: String
    let tint: Color
    var comingSoon: Bool = false

    @Environment(\.theme) private var theme

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                ZStack {
                    RoundedRectangle(cornerRadius: 8)
                        .fill(tint.opacity(0.18))
                        .frame(width: 36, height: 36)
                    Icon(name: icon, size: 18)
                        .foregroundStyle(tint)
                }
                Spacer()
                if comingSoon {
                    Text("soon")
                        .font(.system(size: 9, weight: .semibold, design: .monospaced))
                        .padding(.horizontal, 6)
                        .padding(.vertical, 1)
                        .foregroundStyle(theme.fgFaint)
                        .background(theme.bg3)
                        .clipShape(Capsule())
                }
            }
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(theme.fgDefault)
                Text(hint)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
                    .lineLimit(2)
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, minHeight: 120, alignment: .topLeading)
        .background(theme.bg2)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(theme.line, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }
}

// MARK: - Ambient destinations

struct CalendarAmbientView: View {
    @ObservedObject var mosaic: MockMosaicService
    @Environment(\.theme) private var theme

    private let calendar = Calendar.current
    @State private var month = Date()

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            monthHeader
            weekdayHeader
            daysGrid
            Spacer()
        }
        .padding(16)
        .background(theme.bg)
        .navigationTitle("Calendar")
        .navigationBarTitleDisplayMode(.inline)
    }

    private var monthHeader: some View {
        let f = DateFormatter()
        f.dateFormat = "MMMM yyyy"
        return HStack {
            Button {
                if let prev = calendar.date(byAdding: .month, value: -1, to: month) {
                    month = prev
                }
            } label: {
                Icon(name: .chevLeft, size: 18).foregroundStyle(theme.fgMuted)
                    .frame(width: 44, height: 44)
            }
            .buttonStyle(.plain)
            Spacer()
            Text(f.string(from: month))
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(theme.fgDefault)
            Spacer()
            Button {
                if let next = calendar.date(byAdding: .month, value: 1, to: month) {
                    month = next
                }
            } label: {
                Icon(name: .chevRight, size: 18).foregroundStyle(theme.fgMuted)
                    .frame(width: 44, height: 44)
            }
            .buttonStyle(.plain)
        }
    }

    private var weekdayHeader: some View {
        HStack {
            ForEach(calendar.shortWeekdaySymbols, id: \.self) { day in
                Text(day)
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
                    .frame(maxWidth: .infinity)
            }
        }
    }

    private var daysGrid: some View {
        let days = monthDays()
        return LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 2), count: 7), spacing: 2) {
            ForEach(0..<days.count, id: \.self) { i in
                let date = days[i]
                Button {
                    // Phase 15 wires tap → push the daily for that date
                } label: {
                    let day = calendar.component(.day, from: date)
                    let inMonth = calendar.isDate(date, equalTo: month, toGranularity: .month)
                    Text(String(day))
                        .font(.system(size: 13, design: .monospaced))
                        .foregroundStyle(inMonth ? theme.fgDefault : theme.fgFaint)
                        .frame(maxWidth: .infinity, minHeight: 36)
                        .background(theme.bg2.opacity(inMonth ? 1 : 0.4))
                        .clipShape(RoundedRectangle(cornerRadius: 4))
                }
                .buttonStyle(.plain)
            }
        }
    }

    private func monthDays() -> [Date] {
        guard
            let interval = calendar.dateInterval(of: .month, for: month),
            let start = calendar.dateInterval(of: .weekOfYear, for: interval.start)?.start
        else { return [] }
        var days: [Date] = []
        var cursor = start
        for _ in 0..<42 {
            days.append(cursor)
            guard let next = calendar.date(byAdding: .day, value: 1, to: cursor) else { break }
            cursor = next
        }
        return days
    }
}

struct InProgressAmbientView: View {
    @ObservedObject var mosaic: MockMosaicService
    @Environment(\.theme) private var theme

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                ForEach(mosaic.todayBlocks.filter { $0.kind == .task && !$0.done }) { block in
                    BlockRow(
                        id: block.id,
                        kind: block.kind,
                        text: block.displayText,
                        isDone: block.done,
                        tags: block.tags,
                        onToggleTask: { mosaic.toggleTask(id: block.id) }
                    )
                }
                ForEach(mosaic.yesterdayBlocks.filter { $0.kind == .task && !$0.done }) { block in
                    BlockRow(
                        id: block.id,
                        kind: block.kind,
                        text: block.displayText,
                        isDone: block.done,
                        tags: block.tags,
                        onToggleTask: { mosaic.toggleTask(id: block.id) }
                    )
                    .opacity(0.7)
                }
            }
            .padding(.top, 12)
        }
        .background(theme.bg)
        .navigationTitle("In Progress")
        .navigationBarTitleDisplayMode(.inline)
    }
}

struct AIAmbientView: View {
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 14) {
            Icon(name: .sparkles, size: 36)
                .foregroundStyle(theme.accentSecondary)
            Text("AI workspace")
                .font(.system(size: 20, weight: .semibold))
                .foregroundStyle(theme.fgDefault)
            Text("Coming in a later phase. The Tile vision integrates Parakeet voice transcription and on-device summarization here once the chrome stabilizes.")
                .font(.system(size: 13))
                .multilineTextAlignment(.center)
                .foregroundStyle(theme.fgMuted)
                .lineSpacing(2)
                .padding(.horizontal, 28)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(theme.bg)
        .navigationTitle("AI")
        .navigationBarTitleDisplayMode(.inline)
    }
}
