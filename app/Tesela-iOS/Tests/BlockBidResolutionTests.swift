import XCTest
@testable import Tesela

/// P1.11 — relay-mode property writes. Two seams under test, both pure
/// logic (no engine, no filesystem beyond the real parser):
///
/// 1. Block-address resolution: relay query rows carry `<noteId>:<line>`
///    ids (and the editor seam may hand `<noteId>:<bid>`); the FFI wants
///    the canonical bid. `splitBlockAddress` + `blockBid(in:suffix:)`
///    are the client mirror of the server's `block_bid_from_suffix`.
/// 2. The `.relay` gate in `setBlockProperty` / `saveInboxDsl` now ROUTES
///    through the engine seams (`onLocalPropertySet` / `onLocalNoteWrite`)
///    instead of silently no-opping — and surfaces a rejected write as a
///    throw so triage rows don't optimistically vanish over nothing.
@MainActor
final class BlockBidResolutionTests: XCTestCase {

    // MARK: - Address splitting (rsplit_once(':') mirror)

    func testSplitBlockAddressLineForm() {
        let split = MockMosaicService.splitBlockAddress("2026-06-10:3")
        XCTAssertEqual(split?.noteId, "2026-06-10")
        XCTAssertEqual(split?.suffix, "3")
    }

    func testSplitBlockAddressBidForm() {
        let split = MockMosaicService.splitBlockAddress(
            "project-notes:22222222-2222-2222-2222-222222222222"
        )
        XCTAssertEqual(split?.noteId, "project-notes")
        XCTAssertEqual(split?.suffix, "22222222-2222-2222-2222-222222222222")
    }

    func testSplitBlockAddressMalformed() {
        XCTAssertNil(MockMosaicService.splitBlockAddress("no-colon-here"))
        XCTAssertNil(MockMosaicService.splitBlockAddress("trailing-colon:"))
        XCTAssertNil(MockMosaicService.splitBlockAddress(":0"))
    }

    // MARK: - Line → bid resolution against the real parser

    /// Fixture parsed by the REAL service parser (the same path the
    /// relay-mode queries use), bid markers exactly as the engine
    /// materializes them. Line numbers are 0-based body-line indices:
    /// block 1 at line 0, a property sub-line at 1, block 2 at line 2.
    private func parsedFixture() -> [Block] {
        let body = """
        - Triage me <!-- bid:11111111-1111-1111-1111-111111111111 -->
          priority:: p2
        - Second block <!-- bid:22222222-2222-2222-2222-222222222222 -->
        """
        return MockMosaicService().testableParseBlocks(from: body, noteId: "fixture-note")
    }

    func testBlockBidResolvesLineNumberToMarkerBid() {
        let blocks = parsedFixture()
        XCTAssertEqual(
            MockMosaicService.blockBid(in: blocks, suffix: "0"),
            "11111111-1111-1111-1111-111111111111"
        )
        XCTAssertEqual(
            MockMosaicService.blockBid(in: blocks, suffix: "2"),
            "22222222-2222-2222-2222-222222222222"
        )
    }

    func testBlockBidUnknownLineIsNil() {
        XCTAssertNil(MockMosaicService.blockBid(in: parsedFixture(), suffix: "99"))
    }

    func testBlockBidNonNumericSuffixPassesThrough() {
        // A bid-shaped suffix is the stale-proof editor address — passed
        // through verbatim, no parse needed (server parity).
        XCTAssertEqual(
            MockMosaicService.blockBid(
                in: [], suffix: "33333333-3333-3333-3333-333333333333"
            ),
            "33333333-3333-3333-3333-333333333333"
        )
    }

    // MARK: - The relay gate routes (no more silent no-op)

    func testRelaySetBlockPropertyRoutesThroughEngineSeam() async throws {
        let service = MockMosaicService()
        service.attach(backend: .relay)
        var captured: (slug: String, bid: String, key: String, value: String)?
        service.onLocalPropertySet = { slug, bid, key, value in
            captured = (slug, bid, key, value)
            return true
        }
        // Bid-suffix address: resolvable without a local note file.
        try await service.setBlockProperty(
            blockId: "daily-note:44444444-4444-4444-4444-444444444444",
            key: "status",
            value: "done"
        )
        XCTAssertEqual(captured?.slug, "daily-note")
        XCTAssertEqual(captured?.bid, "44444444-4444-4444-4444-444444444444")
        XCTAssertEqual(captured?.key, "status")
        XCTAssertEqual(captured?.value, "done")
    }

    func testRelaySetBlockPropertyThrowsWhenEngineRejects() async {
        // The seam reporting false (engine closed / bid not found) must
        // THROW — the caller keeps the triaged row instead of dropping it
        // over a write that never landed.
        let service = MockMosaicService()
        service.attach(backend: .relay)
        service.onLocalPropertySet = { _, _, _, _ in false }
        do {
            try await service.setBlockProperty(
                blockId: "daily-note:44444444-4444-4444-4444-444444444444",
                key: "status",
                value: "done"
            )
            XCTFail("expected a throw when the engine rejects the write")
        } catch {
            // expected
        }
    }

    func testRelaySetBlockPropertyThrowsWhenSeamUnwired() async {
        // No seam (shell never wired it) is indistinguishable from a
        // failed write — must throw, never silently succeed.
        let service = MockMosaicService()
        service.attach(backend: .relay)
        do {
            try await service.setBlockProperty(
                blockId: "daily-note:44444444-4444-4444-4444-444444444444",
                key: "status",
                value: "done"
            )
            XCTFail("expected a throw when no engine seam is wired")
        } catch {
            // expected
        }
    }

    func testRelaySetBlockPropertyThrowsOnMalformedAddress() async {
        let service = MockMosaicService()
        service.attach(backend: .relay)
        service.onLocalPropertySet = { _, _, _, _ in
            XCTFail("seam must not fire for a malformed address")
            return true
        }
        do {
            try await service.setBlockProperty(
                blockId: "no-colon-address", key: "status", value: "done"
            )
            XCTFail("expected a throw on a malformed block address")
        } catch {
            // expected
        }
    }

    func testMockSetBlockPropertyStaysInert() async throws {
        // `.mock` keeps dropping the write silently (design-time seed) —
        // the relay routing must not leak into it.
        let service = MockMosaicService()
        service.onLocalPropertySet = { _, _, _, _ in
            XCTFail("seam must not fire in mock mode")
            return true
        }
        try await service.setBlockProperty(
            blockId: "daily-note:0", key: "status", value: "done"
        )
    }

    func testRelaySaveInboxDslRoutesThroughNoteWriteSeam() async throws {
        let service = MockMosaicService()
        service.attach(backend: .relay)
        var captured: (slug: String, title: String, content: String)?
        service.onLocalNoteWrite = { slug, title, content, _ in
            captured = (slug, title, content)
        }
        let dsl = "kind:block -has:status -is:heading"
        try await service.saveInboxDsl(slug: "inbox-work", dsl: dsl)
        XCTAssertEqual(captured?.slug, "inbox-work")
        XCTAssertEqual(captured?.title, "Inbox Work")
        XCTAssertTrue(
            captured?.content.contains("query:: \(dsl)") == true,
            "saved-filter note carries the new DSL: \(captured?.content ?? "nil")"
        )
    }
}
