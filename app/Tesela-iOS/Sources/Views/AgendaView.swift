import SwiftUI

/// Agenda — the time-projection surface. Lists every dated task / event
/// in a forward-walking window so the user can see what's coming up.
/// Overdue items (anchor < today) get their own bucket at the top, then
/// today, tomorrow, and on through the window. Recurring blocks emit one
/// row per projected occurrence inside the window; only the anchor row
/// accepts mutations (the server gates `is_anchor`).
///
/// Mirrors the web `web/src/lib/ambients/agenda/Agenda.svelte`. The
/// underlying data is identical (`POST /agenda`); presentation is
/// iOS-native — tap a row to open the source page, tap the checkbox to
/// mark done, long-press for reschedule / skip.
struct AgendaView: View {
    @ObservedObject var mosaic: MockMosaicService
    @ObservedObject var backend: BackendSettings
    var appearance: AppearanceController? = nil
    var syncState: SyncState? = nil
    var transcription: TranscriptionStore? = nil

    @Environment(\.theme) private var theme
    @EnvironmentObject private var mosaicRegistry: MosaicRegistry

    @State private var rows: [AgendaRow] = []
    @State private var loading = false
    @State private var includeDone = false
    @State private var rescheduleTarget: AgendaRow? = nil
    @State private var showSettings = false
    @State private var showMosaicSwitcher = false
    @State private var navigationPath = NavigationPath()

    /// Lookback so overdue rows surface — mirrors the web client. The
    /// server gates `date >= from`, so without lookback the Overdue
    /// bucket is unreachable.
    private let lookbackDays = 90
    private let lookforwardDays = 60

    var body: some View {
        NavigationStack(path: $navigationPath) {
            VStack(spacing: 0) {
                TabHeader(
                    title: "Agenda",
                    syncStatus: TabHeader.SyncDotState(mosaic.connection),
                    onTapSettings: { showSettings = true },
                    onTapMosaic: { showMosaicSwitcher = true }
                )
                ConnectionBanner(connection: mosaic.connection) {
                    Task { await mosaic.refresh(from: backend.backend) }
                }
                content
                    .refreshable { await load() }
            }
            .background(theme.bg)
            .navigationDestination(for: DailyPageRoute.self) { route in
                if let page = route.resolvedPage(mosaic) {
                    PageViewWrapper(
                        page: page,
                        mosaic: mosaic,
                        syncState: SyncState()
                    )
                    .environment(\.theme, theme)
                } else {
                    placeholderPage(slug: route.slug)
                }
            }
            .sheet(item: $rescheduleTarget) { target in
                RescheduleSheet(row: target) { iso, time, recurrence in
                    Task { await applyReschedule(blockId: target.block_id, iso: iso, time: time, recurrence: recurrence) }
                    rescheduleTarget = nil
                } onCancel: {
                    rescheduleTarget = nil
                }
            }
            .sheet(isPresented: $showMosaicSwitcher) {
                MosaicSwitcherSheet(registry: mosaicRegistry)
                    .environment(\.theme, theme)
            }
            .sheet(isPresented: $showSettings) {
                if let appearance, let syncState {
                    SettingsView(
                        appearance: appearance,
                        mosaic: mosaic,
                        syncState: syncState,
                        backend: backend,
                        transcription: transcription
                    )
                    .environment(\.theme, theme)
                    .environment(\.density, appearance.density)
                } else {
                    Text("Settings unavailable.")
                        .padding()
                }
            }
        }
        .task { await load() }
        .onChange(of: includeDone) { _, _ in
            Task { await load() }
        }
    }

    // MARK: - Content

    @ViewBuilder
    private var content: some View {
        if loading && rows.isEmpty {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(theme.bg)
        } else if rows.isEmpty {
            ContentUnavailableView {
                Label("Nothing on the agenda", systemImage: "calendar")
            } description: {
                Text("Schedule a task or set a deadline and it'll appear here.")
            }
            .background(theme.bg)
        } else {
            List {
                Section {
                    Toggle("Show done", isOn: $includeDone)
                        .font(.system(size: 13))
                        .listRowBackground(theme.bg2)
                }
                if !overdueRows.isEmpty {
                    daySection(label: "OVERDUE", rows: overdueRows, accent: theme.accentPrimary)
                }
                ForEach(forwardBuckets, id: \.iso) { bucket in
                    daySection(label: bucket.label, rows: bucket.rows, accent: theme.fgFaint)
                }
            }
            .listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
            .background(theme.bg)
        }
    }

    @ViewBuilder
    private func daySection(label: String, rows: [AgendaRow], accent: Color) -> some View {
        Section {
            if rows.isEmpty {
                Text("(empty)")
                    .font(.system(size: 12))
                    .foregroundStyle(theme.fgFaint.opacity(0.5))
                    .listRowBackground(theme.bg2.opacity(0.3))
            } else {
                ForEach(rows) { row in
                    AgendaRowView(
                        row: row,
                        onToggleDone: { Task { await applyMarkDone(row) } },
                        onReschedule: { rescheduleTarget = row },
                        onSkip: { Task { await applySkip(row) } },
                        onOpenSource: { navigationPath.append(DailyPageRoute(slug: row.source_note_id)) }
                    )
                    .listRowBackground(theme.bg2)
                }
            }
        } header: {
            HStack {
                Text(label)
                    .font(.system(size: 10, design: .monospaced))
                    .tracking(1.2)
                    .foregroundStyle(accent)
                Spacer()
                if !rows.isEmpty {
                    Text("\(rows.count)")
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                }
            }
        }
    }

    // MARK: - Buckets

    private var overdueRows: [AgendaRow] {
        rows.filter { $0.overdue }
    }

    /// Today through `today + lookforwardDays`. Each day gets a bucket
    /// even when empty so the user has a sense of "what does Tuesday
    /// look like?" while scrolling.
    private var forwardBuckets: [(iso: String, label: String, rows: [AgendaRow])] {
        let byDay = Dictionary(grouping: rows.filter { !$0.overdue }) { $0.occurrence_date }
        var out: [(String, String, [AgendaRow])] = []
        let cal = Calendar.current
        let today = cal.startOfDay(for: Date())
        let fmt = DateFormatter()
        fmt.dateFormat = "yyyy-MM-DD"
        for offset in 0...lookforwardDays {
            guard let d = cal.date(byAdding: .day, value: offset, to: today) else { continue }
            let iso = isoFormat(d)
            let dayRows = byDay[iso] ?? []
            // Skip empty days beyond the next 2 weeks to keep the list
            // breathable — Today/Tomorrow/this-week stay visible empty
            // because that's planning-useful; "EMPTY" rows past day 14
            // are visual noise.
            if dayRows.isEmpty && offset > 14 { continue }
            out.append((iso, dayLabel(d, offset: offset), dayRows))
        }
        return out
    }

    private func dayLabel(_ d: Date, offset: Int) -> String {
        let fmt = DateFormatter()
        fmt.dateFormat = "EEE · MMM d"
        let core = fmt.string(from: d).uppercased()
        if offset == 0 { return "TODAY · \(core)" }
        if offset == 1 { return "TOMORROW · \(core)" }
        return core
    }

    private func isoFormat(_ d: Date) -> String {
        let fmt = DateFormatter()
        fmt.dateFormat = "yyyy-MM-dd"
        return fmt.string(from: d)
    }

    // MARK: - Data load + actions

    private func windowBounds() -> (from: String, to: String) {
        let cal = Calendar.current
        let today = cal.startOfDay(for: Date())
        let from = cal.date(byAdding: .day, value: -lookbackDays, to: today) ?? today
        let to = cal.date(byAdding: .day, value: lookforwardDays, to: today) ?? today
        return (isoFormat(from), isoFormat(to))
    }

    private func load() async {
        loading = true
        defer { loading = false }
        let (from, to) = windowBounds()
        let result = await mosaic.fetchAgenda(from: from, to: to, includeDone: includeDone)
        // Server already sorts by (date, time, block_id); preserve that.
        rows = result
    }

    private func applyMarkDone(_ row: AgendaRow) async {
        guard row.is_anchor else { return }
        do {
            try await mosaic.setBlockProperty(blockId: row.block_id, key: "status", value: "done")
            await load()
        } catch {
            // Silent — connection banner surfaces server failures.
        }
    }

    private func applyReschedule(blockId: String, iso: String, time: String?, recurrence: String?) async {
        let value = time.map { "\(iso) \($0)" } ?? iso
        do {
            try await mosaic.setBlockProperty(blockId: blockId, key: "scheduled", value: value)
            if let recurrence {
                try await mosaic.setBlockProperty(blockId: blockId, key: "recurring", value: recurrence)
            }
            await load()
        } catch {
            // Silent.
        }
    }

    private func applySkip(_ row: AgendaRow) async {
        guard row.is_anchor, row.recurrence != nil else { return }
        do {
            try await mosaic.recurBump(blockId: row.block_id, mode: .skip)
            await load()
        } catch {
            // Silent.
        }
    }

    private func placeholderPage(slug: String) -> some View {
        VStack(spacing: 12) {
            Text("Page not found")
                .font(.system(size: 20, weight: .semibold))
                .foregroundStyle(theme.fgDefault)
            Text("Nothing in the mosaic has the id `\(slug)`.")
                .font(.system(size: 13, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 24)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(theme.bg)
    }
}

// ── Row view ──────────────────────────────────────────────────────────

private struct AgendaRowView: View {
    let row: AgendaRow
    let onToggleDone: () -> Void
    let onReschedule: () -> Void
    let onSkip: () -> Void
    let onOpenSource: () -> Void

    @Environment(\.theme) private var theme

    private var isTask: Bool { row.kind == .task }
    private var isOverdue: Bool { row.overdue }
    /// Anchor rows accept mutations; projected future occurrences are
    /// read-only previews so don't show interactive affordances.
    private var showCheckbox: Bool { isTask && row.is_anchor }
    private var icon: String { isOverdue && isTask ? "⚑" : "🕒" }

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            if showCheckbox {
                Button(action: onToggleDone) {
                    ZStack {
                        RoundedRectangle(cornerRadius: 3)
                            .stroke(theme.typeTask, lineWidth: 1.5)
                            .frame(width: 16, height: 16)
                        if row.status == "done" {
                            RoundedRectangle(cornerRadius: 3)
                                .fill(theme.typeTask)
                                .frame(width: 16, height: 16)
                            Icon(name: .check, size: 11, lineWidth: 2.5)
                                .foregroundStyle(theme.bg)
                        }
                    }
                }
                .buttonStyle(.plain)
                .padding(.top, 2)
            } else if isTask {
                Spacer().frame(width: 16, height: 16)
            } else {
                Text("·")
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
                    .frame(width: 16, alignment: .center)
                    .padding(.top, 2)
            }

            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 6) {
                    Text("\(icon) \(timeOrDate)")
                        .font(.system(size: 11, weight: .medium))
                        .foregroundStyle(isOverdue ? theme.accentPrimary : theme.fgFaint)
                    if let rec = row.recurrence {
                        Text("↻ \(RecurrenceFormat.human(rec))")
                            .font(.system(size: 10))
                            .foregroundStyle(theme.fgFaint.opacity(0.7))
                    }
                    Spacer()
                    Text("in \(row.source_note_id)")
                        .font(.system(size: 10))
                        .foregroundStyle(theme.fgFaint.opacity(0.7))
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
                BlockText(text: row.text)
                    .font(.system(size: 14))
                    .foregroundStyle(row.status == "done" ? theme.fgSubtle : theme.fgDefault)
                    .strikethrough(row.status == "done", color: theme.fgSubtle)
            }
            Spacer(minLength: 0)
        }
        .contentShape(Rectangle())
        .onTapGesture { onOpenSource() }
        .contextMenu {
            if row.is_anchor {
                Button {
                    onReschedule()
                } label: {
                    Label("Reschedule", systemImage: "calendar")
                }
                if row.recurrence != nil {
                    Button {
                        onSkip()
                    } label: {
                        Label("Skip to next occurrence", systemImage: "forward.end")
                    }
                }
                Button {
                    onToggleDone()
                } label: {
                    Label(row.status == "done" ? "Mark not done" : "Mark done", systemImage: "checkmark.circle")
                }
            }
            Button {
                onOpenSource()
            } label: {
                Label("Open source", systemImage: "arrow.up.forward.square")
            }
        }
    }

    private var timeOrDate: String {
        if let t = row.occurrence_time { return t }
        return DateFormat.humanMonthDay(row.occurrence_date)
    }
}

// ── Reschedule sheet ──────────────────────────────────────────────────

private struct RescheduleSheet: View {
    let row: AgendaRow
    let onCommit: (_ iso: String, _ time: String?, _ recurrence: String?) -> Void
    let onCancel: () -> Void

    var body: some View {
        DateInputSheet(
            initialScheduled: row.occurrence_date + (row.occurrence_time.map { " \($0)" } ?? ""),
            initialDeadline: nil,
            initialRecurrence: row.recurrence,
            canSkip: false,
            bareDateFieldDefault: "scheduled",
            onCommit: { _field, iso, time, recurrence in
                onCommit(iso, time, recurrence)
            },
            onSkip: { onCancel() },
            onCancel: onCancel
        )
    }
}

// Make AgendaRow Identifiable-as-sheet-item — the `id` property
// (block_id:date) is already Hashable so SwiftUI's `.sheet(item:)`
// is happy.
