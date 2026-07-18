import SwiftUI

/// Current Graphite entry point for the configurable Dashboard. Layout state
/// is device-local; Query notes and saved views remain synced definitions.
struct GrDashboardView: View {
    @ObservedObject var mosaic: MockMosaicService
    @Binding var path: NavigationPath
    @Environment(\.theme) private var theme

    var body: some View {
        DashboardWidgetCollection(
            mosaic: mosaic,
            relayTicker: .shared,
            onOpenPage: { path.append(GrPageRoute(slug: $0)) }
        )
        .background(theme.bg)
        .navigationTitle("Dashboard")
        .navigationBarTitleDisplayMode(.inline)
    }
}

/// Legacy Workspace ambient entry point. It intentionally shares the exact
/// same layout/catalog implementation as Graphite so the two iOS shells cannot
/// drift back into separate widget systems.
struct DashboardAmbientView: View {
    @ObservedObject var mosaic: MockMosaicService
    @Environment(\.theme) private var theme

    var body: some View {
        DashboardWidgetCollection(
            mosaic: mosaic,
            relayTicker: .shared,
            onOpenPage: nil
        )
        .background(theme.bg)
        .navigationTitle("Dashboard")
        .navigationBarTitleDisplayMode(.inline)
    }
}

private struct DashboardWidgetCollection: View {
    @ObservedObject var mosaic: MockMosaicService
    @ObservedObject var relayTicker: RelayTicker
    let onOpenPage: ((String) -> Void)?

    @Environment(\.theme) private var theme
    @State private var layout = DashboardWidgetLayout.load()
    @State private var projections: [String: DashboardQueryProjection] = [:]
    @State private var candidates = DashboardWidgetCandidate.builtins
    @State private var catalogLoaded = false
    @State private var catalogLoading = false
    @State private var catalogError: String?
    @State private var pickerOpen = false

    private var catalogRevision: String {
        "\(mosaic.refreshTick):\(mosaic.viewsTick)"
    }

    private var availableCandidates: [DashboardWidgetCandidate] {
        let placed = Set(layout.placements.map(\.id))
        return candidates.filter { !placed.contains($0.id) }
    }

    var body: some View {
        ScrollView {
            LazyVStack(spacing: 12) {
                ForEach(Array(layout.placements.enumerated()), id: \.element.id) { index, placement in
                    widget(placement, at: index)
                }

                Button {
                    pickerOpen = true
                } label: {
                    Label("Add widget", systemImage: "plus")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(theme.fgMuted)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 12)
                        .background(theme.bg2)
                        .overlay(
                            RoundedRectangle(cornerRadius: 11)
                                .stroke(theme.line, style: StrokeStyle(lineWidth: 1, dash: [5, 4]))
                        )
                        .clipShape(RoundedRectangle(cornerRadius: 11))
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("dashboard-add-widget")

                if catalogLoading, !catalogLoaded {
                    Label("Loading widget sources…", systemImage: "arrow.triangle.2.circlepath")
                        .dashboardStatusStyle(color: theme.fgFaint)
                }
                if let catalogError {
                    VStack(spacing: 8) {
                        Text("Some widget sources are unavailable: \(catalogError)")
                            .dashboardStatusStyle(color: theme.typeTask)
                        Button("Retry") { Task { await loadCatalog() } }
                            .font(.system(size: 12, weight: .semibold))
                    }
                }
            }
            .padding(16)
        }
        .task(id: catalogRevision) { await loadCatalog() }
        .sheet(isPresented: $pickerOpen) { picker }
    }

    @ViewBuilder
    private func widget(_ placement: DashboardWidgetPlacement, at index: Int) -> some View {
        let projection = projections[placement.id]
        DashboardWidgetFrame(
            title: projection?.title ?? candidate(placement.id)?.title ?? placement.fallbackTitle,
            icon: projection?.icon ?? candidate(placement.id)?.icon ?? "search",
            placement: placement,
            index: index,
            count: layout.placements.count,
            onMove: { move in update(layout.moving(placement.id, by: move)) },
            onRemove: { update(layout.removing(placement.id)) },
            onToggle: { update(layout.toggling(placement.id)) }
        ) {
            if placement.id == "builtin:agenda" {
                DashboardAgendaWidget(mosaic: mosaic, onOpenPage: onOpenPage)
            } else if placement.id == "builtin:sync-health" {
                DashboardSyncHealthWidget(relayTicker: relayTicker)
            } else if let projection {
                DashboardQueryWidget(
                    projection: projection,
                    corpusRevision: catalogRevision,
                    mosaic: mosaic,
                    onOpenPage: onOpenPage
                )
            } else if catalogLoading, !catalogLoaded {
                Text("Loading widget definition…")
                    .dashboardStatusStyle(color: theme.fgFaint)
            } else {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Source “\(placement.sourceID)” is unavailable.")
                        .dashboardStatusStyle(color: theme.typeTask)
                    Text("Remove this widget or restore its Query note/saved view.")
                        .dashboardStatusStyle(color: theme.fgFaint)
                }
            }
        }
        .accessibilityIdentifier("dashboard-widget-\(placement.id)")
    }

    private var picker: some View {
        NavigationStack {
            Group {
                if catalogLoading, !catalogLoaded {
                    ProgressView("Loading widget sources…")
                } else if availableCandidates.isEmpty {
                    ContentUnavailableView(
                        "All widgets added",
                        systemImage: "rectangle.stack.badge.checkmark",
                        description: Text("Every available Query note and saved view is already on this device's Dashboard.")
                    )
                } else {
                    List(availableCandidates) { candidate in
                        Button {
                            update(layout.adding(candidate))
                            pickerOpen = false
                        } label: {
                            HStack(spacing: 12) {
                                GrIcon(name: candidate.icon, size: 17)
                                    .foregroundStyle(theme.accentPrimary)
                                    .frame(width: 24)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(candidate.title)
                                        .foregroundStyle(theme.fgDefault)
                                    Text(candidate.subtitle)
                                        .font(.system(size: 11, design: .monospaced))
                                        .foregroundStyle(theme.fgFaint)
                                }
                            }
                        }
                        .accessibilityIdentifier("dashboard-add-\(candidate.id)")
                    }
                    .listStyle(.plain)
                }
            }
            .navigationTitle("Add widget")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { pickerOpen = false }
                }
            }
        }
        .environment(\.theme, theme)
    }

    private func candidate(_ id: String) -> DashboardWidgetCandidate? {
        candidates.first { $0.id == id }
    }

    private func update(_ next: DashboardWidgetLayout) {
        layout = next
        next.save()
    }

    @MainActor
    private func loadCatalog() async {
        catalogLoading = true
        defer { catalogLoading = false }
        do {
            async let queryDefinitions = mosaic.fetchDashboardQueryDefinitions()
            async let savedViews = mosaic.fetchViews()
            let (queries, views) = try await (queryDefinitions, savedViews)

            var nextProjections: [String: DashboardQueryProjection] = [:]
            for query in queries {
                let projection = DashboardQueryProjection(query: query)
                nextProjections[projection.id] = projection
            }
            for view in views {
                let projection = DashboardQueryProjection(view: view)
                nextProjections[projection.id] = projection
            }

            projections = nextProjections
            candidates = DashboardWidgetCandidate.builtins
                + queries.map(\.candidate)
                + views.filter { $0.id != SavedView.builtinInboxId }.map(\.dashboardCandidate)
            catalogLoaded = true
            catalogError = nil
        } catch is CancellationError {
            return
        } catch {
            catalogLoaded = true
            catalogError = error.localizedDescription
        }
    }
}

private struct DashboardWidgetFrame<Content: View>: View {
    let title: String
    let icon: String
    let placement: DashboardWidgetPlacement
    let index: Int
    let count: Int
    let onMove: (Int) -> Void
    let onRemove: () -> Void
    let onToggle: () -> Void
    @ViewBuilder let content: () -> Content

    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 7) {
                GrIcon(name: icon, size: 14)
                    .foregroundStyle(theme.fgSubtle)
                Text(title.uppercased())
                    .font(.system(size: 11, weight: .semibold))
                    .tracking(0.4)
                    .foregroundStyle(theme.fgMuted)
                    .lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                managementButton("arrow.up", label: "Move \(title) up", disabled: index == 0) {
                    onMove(-1)
                }
                managementButton("arrow.down", label: "Move \(title) down", disabled: index == count - 1) {
                    onMove(1)
                }
                managementButton("xmark", label: "Remove \(title)", disabled: false, role: .destructive) {
                    onRemove()
                }
                Button(action: onToggle) {
                    Image(systemName: "chevron.down")
                        .font(.system(size: 11, weight: .semibold))
                        .rotationEffect(.degrees(placement.collapsed ? -90 : 0))
                        .frame(width: 24, height: 24)
                }
                .buttonStyle(.plain)
                .foregroundStyle(theme.fgFaint)
                .accessibilityLabel("\(placement.collapsed ? "Expand" : "Collapse") \(title)")
            }
            .padding(.horizontal, 11)
            .padding(.vertical, 8)

            if !placement.collapsed {
                VStack(alignment: .leading, spacing: 7) { content() }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 11)
                    .padding(.bottom, 11)
            }
        }
        .background(theme.bg3)
        .overlay(RoundedRectangle(cornerRadius: 11).stroke(theme.line, lineWidth: 1))
        .clipShape(RoundedRectangle(cornerRadius: 11))
    }

    private func managementButton(
        _ symbol: String,
        label: String,
        disabled: Bool,
        role: ButtonRole? = nil,
        action: @escaping () -> Void
    ) -> some View {
        Button(role: role, action: action) {
            Image(systemName: symbol)
                .font(.system(size: 10, weight: .semibold))
                .frame(width: 24, height: 24)
        }
        .buttonStyle(.plain)
        .foregroundStyle(role == .destructive ? theme.typeTask : theme.fgFaint)
        .disabled(disabled)
        .opacity(disabled ? 0.25 : 1)
        .accessibilityLabel(label)
    }
}

private struct DashboardQueryWidget: View {
    let projection: DashboardQueryProjection
    let corpusRevision: String
    @ObservedObject var mosaic: MockMosaicService
    let onOpenPage: ((String) -> Void)?

    @Environment(\.theme) private var theme
    @State private var result: QueryResult?
    @State private var loading = false
    @State private var errorMessage: String?
    @State private var retryRevision = 0

    private var taskRevision: String {
        "\(projection.definitionRevision):\(corpusRevision):\(retryRevision)"
    }

    private var rows: [QueryItem] {
        Array((result?.groups.flatMap(\.items) ?? []).prefix(6))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            if loading, result == nil {
                ProgressView("Running query…")
                    .controlSize(.small)
            } else if let errorMessage, result == nil {
                Text("Query unavailable: \(errorMessage)")
                    .dashboardStatusStyle(color: theme.typeTask)
                retryButton
            } else if rows.isEmpty {
                Text("No matches")
                    .dashboardStatusStyle(color: theme.fgFaint)
            } else {
                if loading {
                    Text("Refreshing…")
                        .dashboardStatusStyle(color: theme.fgFaint)
                } else if let errorMessage {
                    Text("Showing stale results · \(errorMessage)")
                        .dashboardStatusStyle(color: theme.typeTask)
                    retryButton
                }
                ForEach(rows) { row in
                    DashboardResultRow(row: row, onOpenPage: onOpenPage)
                }
            }
        }
        .task(id: taskRevision) { await run() }
    }

    private var retryButton: some View {
        Button("Retry") { retryRevision &+= 1 }
            .font(.system(size: 11, weight: .semibold))
    }

    @MainActor
    private func run() async {
        loading = true
        defer { loading = false }
        do {
            result = try await mosaic.executeDashboardQuery(
                dsl: projection.dsl,
                group: projection.group,
                sort: projection.sort
            )
            errorMessage = nil
        } catch is CancellationError {
            return
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

private struct DashboardResultRow: View {
    let row: QueryItem
    let onOpenPage: ((String) -> Void)?
    @Environment(\.theme) private var theme

    var body: some View {
        Button {
            onOpenPage?(row.page_id)
        } label: {
            VStack(alignment: .leading, spacing: 3) {
                Text(row.kind == .page ? (row.title.isEmpty ? row.text : row.title) : row.text)
                    .font(.system(size: 13.5))
                    .foregroundStyle(theme.fgDefault)
                    .lineLimit(2)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Text(row.title.isEmpty ? row.page_id : row.title)
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
                    .lineLimit(1)
            }
            .padding(.vertical, 4)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(onOpenPage == nil)
        .accessibilityLabel("Open \(row.title.isEmpty ? row.page_id : row.title)")
    }
}

private struct DashboardAgendaWidget: View {
    @ObservedObject var mosaic: MockMosaicService
    let onOpenPage: ((String) -> Void)?

    @Environment(\.theme) private var theme
    @State private var rows: [AgendaRow] = []
    @State private var loading = false
    @State private var loaded = false
    @State private var errorMessage: String?
    @State private var retryRevision = 0

    private var taskRevision: String { "\(mosaic.refreshTick):\(retryRevision)" }

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            if loading, !loaded {
                ProgressView("Loading agenda…").controlSize(.small)
            } else if let errorMessage, !loaded {
                Text("Agenda unavailable: \(errorMessage)")
                    .dashboardStatusStyle(color: theme.typeTask)
                retryButton
            } else if rows.isEmpty {
                Text("No open tasks in the next seven days")
                    .dashboardStatusStyle(color: theme.fgFaint)
            } else {
                if loading {
                    Text("Refreshing…").dashboardStatusStyle(color: theme.fgFaint)
                } else if let errorMessage {
                    Text("Showing stale agenda · \(errorMessage)")
                        .dashboardStatusStyle(color: theme.typeTask)
                    retryButton
                }
                ForEach(rows.prefix(6)) { row in
                    Button {
                        onOpenPage?(row.source_note_id)
                    } label: {
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            Image(systemName: row.kind == .event ? "calendar" : "checkmark.circle")
                                .font(.system(size: 11))
                                .foregroundStyle(row.overdue ? theme.typeTask : theme.accentPrimary)
                            Text(row.text)
                                .font(.system(size: 13.5))
                                .foregroundStyle(theme.fgDefault)
                                .lineLimit(2)
                            Spacer(minLength: 8)
                            Text(row.occurrence_date)
                                .font(.system(size: 9.5, design: .monospaced))
                                .foregroundStyle(theme.fgFaint)
                        }
                        .padding(.vertical, 4)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .disabled(onOpenPage == nil)
                }
            }
        }
        .task(id: taskRevision) { await load() }
    }

    private var retryButton: some View {
        Button("Retry") { retryRevision &+= 1 }
            .font(.system(size: 11, weight: .semibold))
    }

    @MainActor
    private func load() async {
        loading = true
        defer { loading = false }
        let formatter = DateFormatter()
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd"
        let from = formatter.string(from: Date())
        let through = Calendar.current.date(byAdding: .day, value: 7, to: Date()) ?? Date()
        do {
            rows = try await mosaic.fetchDashboardAgenda(
                from: from,
                to: formatter.string(from: through),
                includeDone: false
            )
            loaded = true
            errorMessage = nil
        } catch is CancellationError {
            return
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

private struct DashboardSyncHealthWidget: View {
    @ObservedObject var relayTicker: RelayTicker
    @Environment(\.theme) private var theme

    private var relativeTick: String {
        guard let tick = relayTicker.lastTickAt else { return "never" }
        return RelativeDateTimeFormatter().localizedString(for: tick, relativeTo: Date())
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 7) {
                Circle()
                    .fill(relayTicker.lastError == nil
                        ? (relayTicker.isRunning ? theme.typeEvent : theme.fgFaint)
                        : theme.typeTask)
                    .frame(width: 7, height: 7)
                Text(relayTicker.lastError == nil
                    ? (relayTicker.isRunning ? "Relay active" : "Relay paused")
                    : "Relay error")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(theme.fgDefault)
                Spacer()
                Button("Refresh") { relayTicker.wake() }
                    .font(.system(size: 10.5, weight: .semibold))
            }
            if let error = relayTicker.lastError {
                Text(error).dashboardStatusStyle(color: theme.typeTask)
            }
            Text("Last tick \(relativeTick) · cursor \(relayTicker.inboundCursorSeq)")
                .dashboardStatusStyle(color: theme.fgFaint)
            Text("Last pass: \(relayTicker.lastApplied) applied · \(relayTicker.lastSent) sent")
                .dashboardStatusStyle(color: theme.fgFaint)
        }
    }
}

private extension View {
    func dashboardStatusStyle(color: Color) -> some View {
        font(.system(size: 11, design: .monospaced))
            .foregroundStyle(color)
            .frame(maxWidth: .infinity, alignment: .leading)
            .fixedSize(horizontal: false, vertical: true)
    }
}
