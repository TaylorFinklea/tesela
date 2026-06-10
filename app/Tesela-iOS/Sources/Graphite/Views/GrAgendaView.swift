import SwiftUI

/// Graphite Agenda — the planning surface, re-themed over the SAME
/// `MockMosaicService.fetchAgenda(from:to:includeDone:)` the legacy
/// `AgendaView` uses. No data layer is rebuilt: rows come from the same
/// `POST /agenda` call (90-day lookback / 60-day lookforward), and
/// mark-done / reschedule / skip route through the same mutations
/// (`setBlockProperty`, `recurBump`). Only the chrome is new: a
/// `GrHeader`, the mobile `.grm-agstrip` week strip + `.gr-dayhdr` day
/// divider, and type-colored Graphite rows.
///
/// The web client uses a 5-column time grid; iOS mirrors the mobile
/// mockup's day-focused list: a week strip selects a day, and the body
/// lists that day's rows (plus an always-visible Overdue bucket). Tapping
/// a row pushes the source page on the shared `NavigationStack`.
struct GrAgendaView: View {
    @ObservedObject var mosaic: MockMosaicService
    var backend: BackendSettings? = nil

    @Environment(\.theme) private var theme

    @State private var rows: [AgendaRow] = []
    @State private var loading = false
    @State private var includeDone = false
    @State private var selectedDay: Date = Calendar.current.startOfDay(for: Date())
    @State private var weekAnchor: Date = Calendar.current.startOfDay(for: Date())
    @State private var navigationPath = NavigationPath()
    @State private var rescheduleTarget: AgendaRow? = nil

    private let lookbackDays = 90
    private let lookforwardDays = 60

    var body: some View {
        NavigationStack(path: $navigationPath) {
            VStack(spacing: 0) {
                GrHeader(title: "Agenda", subtitle: weekSubtitle) {
                    GrButton(icon: "calendar") { jumpToToday() }
                }
                weekStrip
                dayDivider
                content
            }
            .background(theme.bg)
            .navigationDestination(for: GrPageRoute.self) { route in
                GrPageView(slug: route.slug, mosaic: mosaic, path: $navigationPath)
                    .environment(\.theme, theme)
            }
            .sheet(item: $rescheduleTarget) { target in
                rescheduleSheet(target)
            }
        }
        .task { await load() }
        .onChange(of: includeDone) { _, _ in Task { await load() } }
        // Re-query when a refresh pass lands (relay tick / WS event /
        // pull-to-refresh elsewhere) — same signal that freshens Daily.
        .onChange(of: mosaic.refreshTick) { _, _ in Task { await load() } }
    }

    // ── Week strip (.grm-agstrip) ───────────────────────────────────────

    private var weekDays: [Date] {
        let cal = Calendar.current
        // Monday-anchored week containing `weekAnchor`.
        let weekday = cal.component(.weekday, from: weekAnchor)
        let deltaToMonday = (weekday + 5) % 7
        guard let monday = cal.date(byAdding: .day, value: -deltaToMonday, to: weekAnchor) else {
            return []
        }
        return (0..<7).compactMap { cal.date(byAdding: .day, value: $0, to: monday) }
    }

    private var weekStrip: some View {
        HStack(spacing: 6) {
            ForEach(weekDays, id: \.self) { day in
                weekDayCell(day)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .overlay(alignment: .bottom) {
            Rectangle().fill(theme.lineSoft).frame(height: 1)
        }
        .gesture(
            DragGesture(minimumDistance: 30)
                .onEnded { value in
                    let cal = Calendar.current
                    if value.translation.width < 0 {
                        weekAnchor = cal.date(byAdding: .day, value: 7, to: weekAnchor) ?? weekAnchor
                    } else if value.translation.width > 0 {
                        weekAnchor = cal.date(byAdding: .day, value: -7, to: weekAnchor) ?? weekAnchor
                    }
                }
        )
    }

    private func weekDayCell(_ day: Date) -> some View {
        let cal = Calendar.current
        let isSelected = cal.isDate(day, inSameDayAs: selectedDay)
        let isToday = cal.isDateInToday(day)
        return Button {
            selectedDay = cal.startOfDay(for: day)
        } label: {
            VStack(spacing: 3) {
                Text(dowLetter(day))
                    .font(.system(size: 9.5, design: .monospaced))
                    .tracking(0.6)
                    .foregroundStyle(isToday ? theme.accentPrimary : theme.fgFaint)
                Text("\(cal.component(.day, from: day))")
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(isToday ? theme.accentPrimary : theme.fgDefault)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 6)
            .background(isSelected ? theme.bg3 : .clear)
            .overlay(
                RoundedRectangle(cornerRadius: 9)
                    .stroke(isSelected ? theme.accentPrimary.opacity(0.4) : .clear, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 9))
        }
        .buttonStyle(.plain)
    }

    // ── Day divider (.gr-dayhdr) ────────────────────────────────────────

    private var dayDivider: some View {
        HStack(spacing: 11) {
            Text(dayName(selectedDay))
                .font(.system(size: 15, weight: .semibold))
                .tracking(-0.15)
                .foregroundStyle(Calendar.current.isDateInToday(selectedDay) ? theme.accentPrimary : theme.fgDefault)
            Text(monthDay(selectedDay))
                .font(.system(size: 10.5, design: .monospaced))
                .tracking(0.8)
                .foregroundStyle(theme.fgFaint)
            Rectangle().fill(theme.lineSoft).frame(height: 1).frame(maxWidth: .infinity)
        }
        .padding(.horizontal, 18)
        .padding(.top, 14)
        .padding(.bottom, 8)
    }

    // ── Content ─────────────────────────────────────────────────────────

    @ViewBuilder
    private var content: some View {
        if loading && rows.isEmpty {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 8) {
                    Toggle("Show done", isOn: $includeDone)
                        .font(.system(size: 12))
                        .tint(theme.accentPrimary)
                        .padding(.horizontal, 18)
                        .padding(.bottom, 4)
                    if !overdueRows.isEmpty {
                        sectionHeader("OVERDUE", count: overdueRows.count, accent: theme.accentPrimary)
                        ForEach(overdueRows) { row in agendaRow(row, overdue: true) }
                    }
                    let day = dayRows
                    if day.isEmpty {
                        Text("Nothing scheduled.")
                            .font(.system(size: 12))
                            .foregroundStyle(theme.fgFaint)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 32)
                    } else {
                        ForEach(day) { row in agendaRow(row, overdue: false) }
                    }
                    Spacer().frame(height: 96)
                }
                .padding(.horizontal, 14)
                .padding(.top, 6)
            }
            .refreshable { await load() }
        }
    }

    private func sectionHeader(_ label: String, count: Int, accent: Color) -> some View {
        HStack {
            Text(label)
                .font(.system(size: 10, design: .monospaced))
                .tracking(1.2)
                .foregroundStyle(accent)
            Text("\(count)")
                .font(.system(size: 10, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
            Spacer()
        }
        .padding(.horizontal, 4)
        .padding(.top, 6)
    }

    // ── Row (.gr-icard-flavored, type-colored) ──────────────────────────

    private func agendaRow(_ row: AgendaRow, overdue: Bool) -> some View {
        let isTask = (row.kind == .task)
        let isDone = (row.status == "done")
        return HStack(alignment: .top, spacing: 12) {
            // Leading affordance: a tappable checkbox for anchor tasks,
            // else a type dot.
            if isTask && row.is_anchor {
                Button {
                    Task { await markDone(row) }
                } label: {
                    ZStack {
                        RoundedRectangle(cornerRadius: 4)
                            .stroke(theme.typeTask, lineWidth: 1.5)
                            .frame(width: 18, height: 18)
                        if isDone {
                            RoundedRectangle(cornerRadius: 4)
                                .fill(theme.typeTask)
                                .frame(width: 18, height: 18)
                            GrIcon(name: "square-check", size: 11, weight: .bold)
                                .foregroundStyle(theme.bg)
                        }
                    }
                }
                .buttonStyle(.plain)
                .padding(.top, 1)
            } else {
                GrTypeDot(kind: row.kind.rawValue, size: 8)
                    .padding(.top, 6)
                    .frame(width: 18)
            }

            // Row BODY navigates; the checkbox column stays its own tap
            // target. The old whole-card `.onTapGesture` swallowed the
            // checkbox button's taps, so EVERY tap opened the block
            // (2026-06-10 product test).
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    Text(timeOrDate(row))
                        .font(.system(size: 11, weight: .medium, design: .monospaced))
                        .foregroundStyle(overdue ? theme.accentPrimary : theme.fgFaint)
                    if let rec = row.recurrence {
                        Text("↻ \(rec)")
                            .font(.system(size: 10, design: .monospaced))
                            .foregroundStyle(theme.fgFaint.opacity(0.7))
                            .lineLimit(1)
                    }
                    Spacer(minLength: 6)
                    Text("in \(row.source_note_id)")
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundStyle(theme.fgFaint.opacity(0.7))
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
                Text(row.text)
                    .font(.system(size: 14))
                    .foregroundStyle(isDone ? theme.fgSubtle : theme.fgDefault)
                    .strikethrough(isDone, color: theme.fgSubtle)
                    .multilineTextAlignment(.leading)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .contentShape(Rectangle())
            .onTapGesture {
                guard !row.source_note_id.isEmpty else { return }
                navigationPath.append(GrPageRoute(slug: row.source_note_id))
            }
        }
        .padding(.horizontal, 13)
        .padding(.vertical, 12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(theme.bg2)
        .overlay(
            RoundedRectangle(cornerRadius: 11)
                .stroke(overdue ? theme.accentPrimary.opacity(0.34) : theme.lineSoft, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 11))
        .contextMenu {
            if row.is_anchor {
                Button { rescheduleTarget = row } label: {
                    Label("Reschedule", systemImage: "calendar")
                }
                if row.recurrence != nil {
                    Button { Task { await skip(row) } } label: {
                        Label("Skip to next occurrence", systemImage: "forward.end")
                    }
                }
                Button { Task { await markDone(row) } } label: {
                    Label(isDone ? "Mark not done" : "Mark done", systemImage: "checkmark.circle")
                }
            }
            Button {
                guard !row.source_note_id.isEmpty else { return }
                navigationPath.append(GrPageRoute(slug: row.source_note_id))
            } label: {
                Label("Open source", systemImage: "arrow.up.forward.square")
            }
        }
    }

    // ── Row bucketing ───────────────────────────────────────────────────

    private var overdueRows: [AgendaRow] {
        rows.filter { $0.overdue }
    }

    private var dayRows: [AgendaRow] {
        let iso = isoFormat(selectedDay)
        return rows.filter { !$0.overdue && $0.occurrence_date == iso }
    }

    // ── Data load + actions (same mutations as AgendaView) ──────────────

    private func load() async {
        loading = true
        defer { loading = false }
        let cal = Calendar.current
        let today = cal.startOfDay(for: Date())
        let from = cal.date(byAdding: .day, value: -lookbackDays, to: today) ?? today
        let to = cal.date(byAdding: .day, value: lookforwardDays, to: today) ?? today
        rows = await mosaic.fetchAgenda(from: isoFormat(from), to: isoFormat(to), includeDone: includeDone)
    }

    private func markDone(_ row: AgendaRow) async {
        guard row.is_anchor else { return }
        let next = (row.status == "done") ? "todo" : "done"
        // Optimistic restyle — strikethrough/un-strike immediately; the
        // post-write `load()` settles the row's final placement (done
        // rows drop out unless "Show done" is on). A failed write skips
        // straight to `load()`, which restores the server truth.
        if let idx = rows.firstIndex(of: row) {
            rows[idx] = AgendaRow(
                block_id: row.block_id,
                source_note_id: row.source_note_id,
                occurrence_date: row.occurrence_date,
                occurrence_time: row.occurrence_time,
                kind: row.kind,
                overdue: row.overdue,
                recurrence: row.recurrence,
                is_anchor: row.is_anchor,
                text: row.text,
                status: next,
                field: row.field
            )
        }
        try? await mosaic.setBlockProperty(blockId: row.block_id, key: "status", value: next)
        await load()
    }

    private func skip(_ row: AgendaRow) async {
        guard row.is_anchor, row.recurrence != nil else { return }
        try? await mosaic.recurBump(blockId: row.block_id, mode: .skip)
        await load()
    }

    private func applyReschedule(blockId: String, iso: String, time: String?, recurrence: String?) async {
        let value = time.map { "\(iso) \($0)" } ?? iso
        try? await mosaic.setBlockProperty(blockId: blockId, key: "scheduled", value: value)
        if let recurrence {
            try? await mosaic.setBlockProperty(blockId: blockId, key: "recurring", value: recurrence)
        }
        await load()
    }

    private func rescheduleSheet(_ target: AgendaRow) -> some View {
        DateInputSheet(
            initialScheduled: target.occurrence_date + (target.occurrence_time.map { " \($0)" } ?? ""),
            initialDeadline: nil,
            initialRecurrence: target.recurrence,
            canSkip: false,
            bareDateFieldDefault: "scheduled",
            onCommit: { _field, iso, time, recurrence in
                Task { await applyReschedule(blockId: target.block_id, iso: iso, time: time, recurrence: recurrence) }
                rescheduleTarget = nil
            },
            onSkip: { rescheduleTarget = nil },
            onCancel: { rescheduleTarget = nil }
        )
        .environment(\.theme, theme)
    }

    private func jumpToToday() {
        let today = Calendar.current.startOfDay(for: Date())
        selectedDay = today
        weekAnchor = today
    }

    // ── Date helpers ────────────────────────────────────────────────────

    private func timeOrDate(_ row: AgendaRow) -> String {
        if let t = row.occurrence_time { return t }
        if let date = Self.isoParser.date(from: row.occurrence_date) {
            return monthDay(date)
        }
        return row.occurrence_date
    }

    private func isoFormat(_ d: Date) -> String { Self.isoParser.string(from: d) }

    private func dowLetter(_ d: Date) -> String {
        let f = DateFormatter()
        f.dateFormat = "EEEEE"
        return f.string(from: d).uppercased()
    }

    private func dayName(_ d: Date) -> String {
        let f = DateFormatter()
        f.dateFormat = "EEEE"
        return f.string(from: d)
    }

    private func monthDay(_ d: Date) -> String {
        let f = DateFormatter()
        f.dateFormat = "MMM d"
        return f.string(from: d).uppercased()
    }

    private var weekSubtitle: String {
        let days = weekDays
        guard let first = days.first, let last = days.last else { return "PLANNING" }
        return "\(monthDay(first)) – \(monthDay(last))"
    }

    private static let isoParser: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd"
        return f
    }()
}
