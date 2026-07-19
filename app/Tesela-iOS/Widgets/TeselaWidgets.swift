import SwiftUI
import WidgetKit

private enum WidgetPalette {
    static let background = Color(red: 0.055, green: 0.059, blue: 0.075)
    static let surface = Color(red: 0.09, green: 0.098, blue: 0.122)
    static let primary = Color(red: 0.91, green: 0.92, blue: 0.95)
    static let secondary = Color(red: 0.52, green: 0.55, blue: 0.62)
    static let accent = Color(red: 0.95, green: 0.40, blue: 0.33)
    static let agenda = Color(red: 0.39, green: 0.73, blue: 0.66)
}

struct TeselaAgendaWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: TeselaWidgetConstants.agendaKind, provider: TeselaWidgetProvider()) { entry in
            TeselaAgendaWidgetView(entry: entry)
                .containerBackground(for: .widget) { WidgetPalette.background }
                .widgetURL(TeselaWidgetConstants.agendaURL)
        }
        .configurationDisplayName("Tesela Agenda")
        .description("Your next open tasks and events.")
        .supportedFamilies([.systemSmall, .systemMedium])
    }
}

struct TeselaInboxWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: TeselaWidgetConstants.inboxKind, provider: TeselaWidgetProvider()) { entry in
            TeselaInboxWidgetView(entry: entry)
                .containerBackground(for: .widget) { WidgetPalette.background }
                .widgetURL(TeselaWidgetConstants.inboxURL)
        }
        .configurationDisplayName("Tesela Inbox")
        .description("A glance at items waiting in your Inbox.")
        .supportedFamilies([.systemSmall, .systemMedium])
    }
}

private struct TeselaAgendaWidgetView: View {
    @Environment(\.widgetFamily) private var family
    let entry: TeselaWidgetEntry

    private var limit: Int { family == .systemSmall ? 3 : 5 }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            WidgetHeader(title: "AGENDA", symbol: "calendar", updatedAt: entry.snapshot?.generatedAt)
            if let snapshot = entry.snapshot {
                if snapshot.agenda.isEmpty {
                    WidgetEmptyState(text: "No open tasks in the next seven days")
                } else {
                    ForEach(snapshot.agenda.prefix(limit)) { item in
                        HStack(alignment: .firstTextBaseline, spacing: 7) {
                            Image(systemName: item.kind == "event" ? "calendar" : "circle")
                                .font(.system(size: 9, weight: .semibold))
                                .foregroundStyle(item.overdue ? WidgetPalette.accent : WidgetPalette.agenda)
                            Text(item.text)
                                .font(.system(size: 12, weight: .medium))
                                .foregroundStyle(WidgetPalette.primary)
                                .lineLimit(1)
                                .privacySensitive()
                            Spacer(minLength: 4)
                            Text(compactDate(item))
                                .font(.system(size: 9, design: .monospaced))
                                .foregroundStyle(item.overdue ? WidgetPalette.accent : WidgetPalette.secondary)
                        }
                    }
                }
            } else {
                WidgetEmptyState(text: "Open Tesela to load your agenda")
            }
            Spacer(minLength: 0)
        }
    }

    private func compactDate(_ item: TeselaAgendaWidgetItem) -> String {
        let formatter = DateFormatter()
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd"
        guard let date = formatter.date(from: item.occurrenceDate) else { return item.occurrenceDate }
        if Calendar.current.isDateInToday(date) { return item.occurrenceTime ?? "TODAY" }
        if Calendar.current.isDateInTomorrow(date) { return "TOM" }
        formatter.dateFormat = "EEE"
        return formatter.string(from: date).uppercased()
    }
}

private struct TeselaInboxWidgetView: View {
    @Environment(\.widgetFamily) private var family
    let entry: TeselaWidgetEntry

    private var limit: Int { family == .systemSmall ? 3 : 5 }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            WidgetHeader(title: "INBOX", symbol: "tray", updatedAt: entry.snapshot?.generatedAt)
            if let snapshot = entry.snapshot {
                if snapshot.inbox.isEmpty {
                    WidgetEmptyState(text: "Inbox is clear")
                } else {
                    ForEach(snapshot.inbox.prefix(limit)) { item in
                        VStack(alignment: .leading, spacing: 1) {
                            Text(item.text)
                                .font(.system(size: 12, weight: .medium))
                                .foregroundStyle(WidgetPalette.primary)
                                .lineLimit(1)
                                .privacySensitive()
                            if family == .systemMedium {
                                Text(item.sourceTitle.uppercased())
                                    .font(.system(size: 8, weight: .semibold))
                                    .foregroundStyle(WidgetPalette.secondary)
                                    .lineLimit(1)
                                    .privacySensitive()
                            }
                        }
                    }
                }
            } else {
                WidgetEmptyState(text: "Open Tesela to load your inbox")
            }
            Spacer(minLength: 0)
        }
    }
}

private struct WidgetHeader: View {
    let title: String
    let symbol: String
    let updatedAt: Date?

    var body: some View {
        HStack(spacing: 5) {
            Image(systemName: symbol)
                .font(.system(size: 10, weight: .bold))
                .foregroundStyle(WidgetPalette.accent)
                .widgetAccentable()
            Text(title)
                .font(.system(size: 10, weight: .bold))
                .tracking(1.1)
                .foregroundStyle(WidgetPalette.secondary)
            Spacer(minLength: 4)
            if let updatedAt {
                Text(updatedAt, style: .relative)
                    .font(.system(size: 8))
                    .foregroundStyle(WidgetPalette.secondary)
            }
        }
    }
}

private struct WidgetEmptyState: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(size: 12, weight: .medium))
            .foregroundStyle(WidgetPalette.secondary)
            .lineLimit(3)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(8)
            .background(WidgetPalette.surface)
            .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}
