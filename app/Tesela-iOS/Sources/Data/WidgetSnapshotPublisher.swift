import Foundation
import WidgetKit

enum WidgetSnapshotProjection {
    static func agendaItems(from rows: [AgendaRow], limit: Int = 6) -> [TeselaAgendaWidgetItem] {
        rows.prefix(limit).map { row in
            TeselaAgendaWidgetItem(
                id: row.id,
                text: compact(row.text),
                occurrenceDate: row.occurrence_date,
                occurrenceTime: row.occurrence_time,
                kind: row.kind.rawValue,
                overdue: row.overdue
            )
        }
    }

    static func inboxItems(from result: QueryResult, limit: Int = 6) -> [TeselaInboxWidgetItem] {
        var items: [TeselaInboxWidgetItem] = []
        for group in result.groups {
            for row in group.items where row.kind == .block {
                let text = compact(row.text)
                guard !text.isEmpty else { continue }
                items.append(
                    TeselaInboxWidgetItem(id: row.id, text: text, sourceTitle: row.title)
                )
                if items.count == limit { return items }
            }
        }
        return items
    }

    private static func compact(_ text: String) -> String {
        text.split(whereSeparator: \Character.isNewline)
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
            .joined(separator: " ")
    }
}

@MainActor
enum WidgetSnapshotPublisher {
    static func publish(
        from mosaic: MockMosaicService,
        generatedAt: Date = Date(),
        store: TeselaWidgetSnapshotStore? = nil
    ) async {
        guard mosaic.backendMutationAdmissionIsOpen else { return }
        switch mosaic.connection {
        case .connecting, .switching: return
        case .idle, .ready, .failed: break
        }
        guard let resolvedStore = store ?? TeselaWidgetSnapshotStore.shared() else { return }

        let formatter = DateFormatter()
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd"
        let through = Calendar.current.date(byAdding: .day, value: 7, to: generatedAt) ?? generatedAt

        do {
            let agenda = try await mosaic.fetchDashboardAgenda(
                from: formatter.string(from: generatedAt),
                to: formatter.string(from: through),
                includeDone: false
            )
            let inbox = try await mosaic.executeDashboardQuery(
                dsl: SavedView.fallbackInbox.dsl,
                group: nil,
                sort: nil
            )
            let snapshot = TeselaWidgetSnapshot(
                generatedAt: generatedAt,
                agenda: WidgetSnapshotProjection.agendaItems(from: agenda),
                inbox: WidgetSnapshotProjection.inboxItems(from: inbox)
            )
            try resolvedStore.save(snapshot)
            WidgetCenter.shared.reloadTimelines(ofKind: TeselaWidgetConstants.agendaKind)
            WidgetCenter.shared.reloadTimelines(ofKind: TeselaWidgetConstants.inboxKind)
        } catch is CancellationError {
            return
        } catch {
            return
        }
    }
}
