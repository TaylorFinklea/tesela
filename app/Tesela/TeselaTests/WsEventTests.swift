import XCTest
@testable import Tesela

final class WsEventTests: XCTestCase {
    private let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.dateDecodingStrategy = .iso8601
        return d
    }()

    // Minimal valid Page JSON matching the Rust server response shape
    private func pageJSON(id: String = "abc123", title: String = "Test Note", event: String) -> String {
        """
        {
          "event": "\(event)",
          "note": {
            "id": "\(id)",
            "title": "\(title)",
            "content": "---\\ntitle: \(title)\\n---\\n- Hello",
            "body": "- Hello",
            "metadata": {"title": "\(title)", "tags": [], "aliases": [], "custom": {}},
            "path": "/notes/test.md",
            "checksum": "deadbeef",
            "created_at": "2025-01-15T10:30:00Z",
            "modified_at": "2025-01-15T10:30:00Z",
            "attachments": []
          }
        }
        """
    }

    func testDecodesNoteCreated() throws {
        let event = try decoder.decode(WsEvent.self, from: Data(pageJSON(event: "note_created").utf8))
        guard case .noteCreated(let page) = event else {
            return XCTFail("Expected .noteCreated, got \(event)")
        }
        XCTAssertEqual(page.id, "abc123")
        XCTAssertEqual(page.title, "Test Note")
    }

    func testDecodesNoteUpdated() throws {
        let event = try decoder.decode(
            WsEvent.self,
            from: Data(pageJSON(title: "Updated Note", event: "note_updated").utf8)
        )
        guard case .noteUpdated(let page) = event else {
            return XCTFail("Expected .noteUpdated, got \(event)")
        }
        XCTAssertEqual(page.title, "Updated Note")
    }

    func testDecodesNoteDeleted() throws {
        let json = #"{"event": "note_deleted", "id": "abc123"}"#
        let event = try decoder.decode(WsEvent.self, from: Data(json.utf8))
        guard case .noteDeleted(let id) = event else {
            return XCTFail("Expected .noteDeleted, got \(event)")
        }
        XCTAssertEqual(id, "abc123")
    }

    func testUnknownEventThrows() {
        let json = #"{"event": "unknown_event", "data": {}}"#
        XCTAssertThrowsError(try decoder.decode(WsEvent.self, from: Data(json.utf8)))
    }
}
