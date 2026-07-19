import SwiftUI
import WidgetKit

struct TeselaWidgetEntry: TimelineEntry {
    let date: Date
    let snapshot: TeselaWidgetSnapshot?
}

struct TeselaWidgetProvider: TimelineProvider {
    func placeholder(in context: Context) -> TeselaWidgetEntry {
        TeselaWidgetEntry(date: Date(), snapshot: .preview)
    }

    func getSnapshot(in context: Context, completion: @escaping (TeselaWidgetEntry) -> Void) {
        let snapshot = loadSnapshot() ?? (context.isPreview ? .preview : nil)
        completion(TeselaWidgetEntry(date: Date(), snapshot: snapshot))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<TeselaWidgetEntry>) -> Void) {
        let now = Date()
        let entry = TeselaWidgetEntry(date: now, snapshot: loadSnapshot())
        let refresh = Calendar.current.date(byAdding: .minute, value: 30, to: now) ?? now.addingTimeInterval(1_800)
        completion(Timeline(entries: [entry], policy: .after(refresh)))
    }

    private func loadSnapshot() -> TeselaWidgetSnapshot? {
        TeselaWidgetSnapshotStore.shared()?.load()
    }
}

private extension TeselaWidgetSnapshot {
    static var preview: TeselaWidgetSnapshot {
        let now = Date()
        let formatter = DateFormatter()
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd"
        let tomorrow = Calendar.current.date(byAdding: .day, value: 1, to: now) ?? now
        return TeselaWidgetSnapshot(
            generatedAt: now,
            agenda: [
                TeselaAgendaWidgetItem(
                    id: "preview-agenda-1",
                    text: "Review the project brief",
                    occurrenceDate: formatter.string(from: now),
                    occurrenceTime: "09:30",
                    kind: "task",
                    overdue: false
                ),
                TeselaAgendaWidgetItem(
                    id: "preview-agenda-2",
                    text: "Weekly planning",
                    occurrenceDate: formatter.string(from: tomorrow),
                    occurrenceTime: nil,
                    kind: "event",
                    overdue: false
                ),
            ],
            inbox: [
                TeselaInboxWidgetItem(id: "preview-inbox-1", text: "Follow up on the design notes", sourceTitle: "Today"),
                TeselaInboxWidgetItem(id: "preview-inbox-2", text: "Triage the open questions", sourceTitle: "Project"),
            ]
        )
    }
}
