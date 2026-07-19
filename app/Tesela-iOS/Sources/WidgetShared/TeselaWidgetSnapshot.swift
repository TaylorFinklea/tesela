import Foundation

enum TeselaWidgetConstants {
    static let appGroupIdentifier = "group.app.tesela.shared"
    static let snapshotFilename = "widget-snapshot-v1.json"
    static let agendaKind = "app.tesela.ios.widget.agenda"
    static let inboxKind = "app.tesela.ios.widget.inbox"

    static let agendaURL = URL(string: "tesela://agenda")!
    static let inboxURL = URL(string: "tesela://views")!
}

struct TeselaWidgetSnapshot: Codable, Equatable, Sendable {
    static let currentVersion = 1

    let version: Int
    let generatedAt: Date
    let agenda: [TeselaAgendaWidgetItem]
    let inbox: [TeselaInboxWidgetItem]

    init(
        version: Int = currentVersion,
        generatedAt: Date,
        agenda: [TeselaAgendaWidgetItem],
        inbox: [TeselaInboxWidgetItem]
    ) {
        self.version = version
        self.generatedAt = generatedAt
        self.agenda = agenda
        self.inbox = inbox
    }
}

struct TeselaAgendaWidgetItem: Codable, Equatable, Identifiable, Sendable {
    let id: String
    let text: String
    let occurrenceDate: String
    let occurrenceTime: String?
    let kind: String
    let overdue: Bool
}

struct TeselaInboxWidgetItem: Codable, Equatable, Identifiable, Sendable {
    let id: String
    let text: String
    let sourceTitle: String
}

struct TeselaWidgetSnapshotStore {
    private let directoryURL: URL

    init(directoryURL: URL) {
        self.directoryURL = directoryURL
    }

    static func shared(fileManager: FileManager = .default) -> TeselaWidgetSnapshotStore? {
        guard let directory = fileManager.containerURL(
            forSecurityApplicationGroupIdentifier: TeselaWidgetConstants.appGroupIdentifier
        ) else {
            return nil
        }
        return TeselaWidgetSnapshotStore(directoryURL: directory)
    }

    func load(fileManager: FileManager = .default) -> TeselaWidgetSnapshot? {
        let url = directoryURL.appendingPathComponent(TeselaWidgetConstants.snapshotFilename)
        guard let data = fileManager.contents(atPath: url.path),
              let snapshot = try? JSONDecoder().decode(TeselaWidgetSnapshot.self, from: data),
              snapshot.version == TeselaWidgetSnapshot.currentVersion else {
            return nil
        }
        return snapshot
    }

    func save(_ snapshot: TeselaWidgetSnapshot, fileManager: FileManager = .default) throws {
        try fileManager.createDirectory(at: directoryURL, withIntermediateDirectories: true)
        let data = try JSONEncoder().encode(snapshot)
        try data.write(
            to: directoryURL.appendingPathComponent(TeselaWidgetConstants.snapshotFilename),
            options: .atomic
        )
    }
}
