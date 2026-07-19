import XCTest
@testable import Tesela

final class WidgetSnapshotTests: XCTestCase {
    func testAppGroupContainerIsAvailableToTheSimulatorHost() {
        XCTAssertNotNil(TeselaWidgetSnapshotStore.shared())
    }

    func testSnapshotStoreRoundTripsAndRejectsUnknownVersion() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("WidgetSnapshotTests-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = TeselaWidgetSnapshotStore(directoryURL: directory)
        let snapshot = TeselaWidgetSnapshot(
            generatedAt: Date(timeIntervalSince1970: 123),
            agenda: [
                TeselaAgendaWidgetItem(
                    id: "a", text: "Agenda row", occurrenceDate: "2026-07-19",
                    occurrenceTime: "09:00", kind: "task", overdue: false
                ),
            ],
            inbox: [TeselaInboxWidgetItem(id: "i", text: "Inbox row", sourceTitle: "Today")]
        )

        try store.save(snapshot)
        XCTAssertEqual(store.load(), snapshot)

        let unsupported = TeselaWidgetSnapshot(
            version: 99, generatedAt: snapshot.generatedAt, agenda: [], inbox: []
        )
        try store.save(unsupported)
        XCTAssertNil(store.load())
    }

    @MainActor
    func testPublisherWritesAnHonestEmptyMockSnapshot() async throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("WidgetSnapshotPublisherTests-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = TeselaWidgetSnapshotStore(directoryURL: directory)

        await WidgetSnapshotPublisher.publish(
            from: MockMosaicService(),
            generatedAt: Date(timeIntervalSince1970: 456),
            store: store
        )

        let snapshot = try XCTUnwrap(store.load())
        XCTAssertEqual(snapshot.generatedAt, Date(timeIntervalSince1970: 456))
        XCTAssertTrue(snapshot.agenda.isEmpty)
        XCTAssertTrue(snapshot.inbox.isEmpty)
    }

    func testProjectionIsCompactLimitedAndBlockOnly() {
        let agendaRows = (0..<8).map { index in
            AgendaRow(
                block_id: "block-\(index)", source_note_id: "note", occurrence_date: "2026-07-19",
                occurrence_time: nil, kind: .task, overdue: index == 0, recurrence: nil,
                is_anchor: true, text: "Line one\n  Line two", status: "todo", priority: nil,
                field: .scheduled
            )
        }
        XCTAssertEqual(WidgetSnapshotProjection.agendaItems(from: agendaRows).count, 6)
        XCTAssertEqual(WidgetSnapshotProjection.agendaItems(from: agendaRows)[0].text, "Line one Line two")

        let page = QueryItem(
            block_id: nil, page_id: "page", title: "Page", text: "Page result",
            parent_breadcrumb: [], kind: .page, primary_tag: nil, properties: [:], page_note_type: nil
        )
        let block = QueryItem(
            block_id: "block", page_id: "page", title: "Page", text: "  Inbox item  ",
            parent_breadcrumb: [], kind: .block, primary_tag: nil, properties: [:], page_note_type: nil
        )
        let result = QueryResult(groups: [QueryGroup(key: "all", items: [page, block])])

        XCTAssertEqual(
            WidgetSnapshotProjection.inboxItems(from: result),
            [TeselaInboxWidgetItem(id: "block", text: "Inbox item", sourceTitle: "Page")]
        )
    }

    func testDeepLinksResolveOnlySupportedTeselaDestinations() {
        XCTAssertEqual(
            TeselaDeepLink.destination(for: TeselaWidgetConstants.agendaURL),
            .agenda
        )
        XCTAssertEqual(
            TeselaDeepLink.destination(for: TeselaWidgetConstants.inboxURL),
            .views
        )
        XCTAssertEqual(
            TeselaDeepLink.destination(for: URL(string: "tesela://inbox")!),
            .views
        )
        XCTAssertNil(TeselaDeepLink.destination(for: URL(string: "https://tesela.app/agenda")!))
        XCTAssertNil(TeselaDeepLink.destination(for: URL(string: "tesela://settings")!))
    }
}
