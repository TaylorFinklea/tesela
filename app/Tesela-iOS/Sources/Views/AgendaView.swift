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
    var relayTicker: RelayTicker? = nil
    var transcription: TranscriptionStore? = nil

    @Environment(\.theme) private var theme
    @EnvironmentObject private var mosaicRegistry: MosaicRegistry

    @State private var rows: [AgendaRow] = []
    @State private var loading = false
    @State private var includeDone = false
    @State private var rescheduleTarget: AgendaRow? = nil
    @State private var bulkTarget: BulkRescheduleTarget? = nil
    @State private var showSettings = false
    @State private var showMosaicSwitcher = false
    @State private var navigationPath = NavigationPath()
    /// Cached bucketing — recomputing `forwardBuckets` on every SwiftUI
    /// render (which fires when the context menu mounts) is O(N × 60+)
    /// and was freezing the UI ~25s on long-press. We materialize the
    /// buckets once when `rows` / `includeDone` change.
    @State private var cachedOverdueDeadlines: [AgendaRow] = []
    @State private var cachedOverdueScheduled: [AgendaRow] = []
    @State private var cachedForwardBuckets: [DayBucket] = []

    struct DayBucket: Identifiable, Equatable {
        let iso: String
        let label: String
        let rows: [AgendaRow]
        var id: String { iso }
    }

    /// Payload for a bulk-reschedule sheet — which property to write
    /// and the rows it'll apply to. `Identifiable` so `.sheet(item:)`
    /// can drive its presentation.
    struct BulkRescheduleTarget: Identifiable, Equatable {
        let id: String        // stable per invocation: "deadline-N" / "scheduled-N"
        let field: AgendaField
        let rows: [AgendaRow]
    }

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
            .sheet(item: $bulkTarget) { target in
                BulkRescheduleSheet(target: target) { iso, time in
                    Task { await applyBulkReschedule(target: target, iso: iso, time: time) }
                    bulkTarget = nil
                } onCancel: {
                    bulkTarget = nil
                }
            }
            .sheet(isPresented: $showMosaicSwitcher) {
                MosaicSwitcherSheet(registry: mosaicRegistry)
                    .environment(\.theme, theme)
            }
            .sheet(isPresented: $showSettings) {
                if let appearance, let syncState, let relayTicker {
                    SettingsView(
                        appearance: appearance,
                        mosaic: mosaic,
                        syncState: syncState,
                        backend: backend,
                        relayTicker: relayTicker,
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
        .onChange(of: rows) { _, _ in rebucket() }
        // Re-query when a refresh pass lands (relay tick / WS event) —
        // same signal that freshens Daily.
        .onChange(of: mosaic.refreshTick) { _, _ in Task { await load() } }
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
                if !cachedOverdueDeadlines.isEmpty {
                    overdueSection(
                        label: "⚑ OVERDUE DEADLINES",
                        rows: cachedOverdueDeadlines,
                        field: .deadline
                    )
                }
                if !cachedOverdueScheduled.isEmpty {
                    overdueSection(
                        label: "🕒 OVERDUE SCHEDULED",
                        rows: cachedOverdueScheduled,
                        field: .scheduled
                    )
                }
                ForEach(cachedForwardBuckets) { bucket in
                    daySection(label: bucket.label, rows: bucket.rows, accent: theme.fgFaint)
                }
            }
            .listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
            .background(theme.bg)
        }
    }

    /// Overdue sub-section: a normal day-section plus a header-trailing
    /// "Reschedule all" button that opens the bulk sheet for every row
    /// in the bucket, writing to the bucket's field on commit.
    @ViewBuilder
    private func overdueSection(label: String, rows: [AgendaRow], field: AgendaField) -> some View {
        Section {
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
        } header: {
            HStack {
                Text(label)
                    .font(.system(size: 10, design: .monospaced))
                    .tracking(1.2)
                    .foregroundStyle(theme.accentPrimary)
                Text("\(rows.count)")
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
                Spacer()
                Button {
                    bulkTarget = BulkRescheduleTarget(
                        id: "\(field.rawValue)-\(Date().timeIntervalSince1970)",
                        field: field,
                        rows: rows
                    )
                } label: {
                    Text("Reschedule all →")
                        .font(.system(size: 11))
                        .foregroundStyle(theme.fgDefault)
                }
                .buttonStyle(.plain)
            }
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

    /// Recompute `cachedOverdue` + `cachedForwardBuckets` from the
    /// current `rows`. Runs once when rows change (via `.onChange`)
    /// instead of on every render. Long-press freeze repro: the
    /// previous computed-property form re-evaluated the 60-day bucket
    /// walk + dictionary-grouping on every SwiftUI render cycle,
    /// including the spurious re-renders SwiftUI fires when a
    /// `.contextMenu` mounts. With a few hundred rows that locked the
    /// UI for ~25s on every long-press.
    private func rebucket() {
        let overdueAll = rows.filter { $0.overdue }
        cachedOverdueDeadlines = overdueAll.filter { $0.field == .deadline }
        cachedOverdueScheduled = overdueAll.filter { $0.field == .scheduled }
        let byDay = Dictionary(grouping: rows.filter { !$0.overdue }) { $0.occurrence_date }
        var out: [DayBucket] = []
        let cal = Calendar.current
        let today = cal.startOfDay(for: Date())
        for offset in 0...lookforwardDays {
            guard let d = cal.date(byAdding: .day, value: offset, to: today) else { continue }
            let iso = isoFormat(d)
            let dayRows = byDay[iso] ?? []
            // Empty days past the next two weeks are dropped — planning
            // wants Today/Tomorrow/this-week visible even when empty,
            // but a 60-row run of `EMPTY` past then is visual noise.
            if dayRows.isEmpty && offset > 14 { continue }
            out.append(DayBucket(iso: iso, label: dayLabel(d, offset: offset), rows: dayRows))
        }
        cachedForwardBuckets = out
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

    private func applyBulkReschedule(target: BulkRescheduleTarget, iso: String, time: String?) async {
        let value = time.map { "\(iso) \($0)" } ?? iso
        let key = target.field.rawValue  // "deadline" | "scheduled"
        // Fire setBlockProperty for each row in parallel — no batch
        // endpoint, but the row counts here are tens at most so a
        // TaskGroup over them is fine.
        await withTaskGroup(of: Void.self) { group in
            for row in target.rows {
                let bid = row.block_id
                group.addTask {
                    try? await mosaic.setBlockProperty(blockId: bid, key: key, value: value)
                }
            }
        }
        await load()
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
                TaskStatusMarker(
                    status: row.status,
                    priority: row.priority,
                    size: 16,
                    onTap: onToggleDone
                )
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

// ── Bulk reschedule sheet ──────────────────────────────────────────────

private struct BulkRescheduleSheet: View {
    let target: AgendaView.BulkRescheduleTarget
    let onCommit: (_ iso: String, _ time: String?) -> Void
    let onCancel: () -> Void

    var body: some View {
        // Reuse the DateInputSheet but ignore the field-picker (we
        // already know which field — the bucket that spawned the
        // sheet) and recurrence (bulk-set of recurrence doesn't make
        // sense; you'd be saying "every Tuesday" to a heterogeneous
        // set of tasks). The sheet's bareDateFieldDefault is set to
        // the target's field so the chip pre-selects correctly even
        // though the user can't change it.
        DateInputSheet(
            initialScheduled: nil,
            initialDeadline: nil,
            initialRecurrence: nil,
            canSkip: false,
            bareDateFieldDefault: target.field.rawValue,
            onCommit: { _field, iso, time, _recurrence in
                onCommit(iso, time)
            },
            onSkip: { onCancel() },
            onCancel: onCancel
        )
    }
}
