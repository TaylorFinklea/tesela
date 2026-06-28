import XCTest
@testable import Tesela

/// Phase 3 iOS presence foundation: the PRES codec is byte-compatible with the
/// web (so iOS↔web cursors interoperate), and the remote-cursor store applies /
/// filters / expires peers correctly.
final class LoroPresenceCodecTests: XCTestCase {
    func testPresenceFrameRoundTrips() {
        let f = LoroPresence.Frame(
            peer: "p1", color: "#ff8800", name: "A",
            slug: "2026-06-27", bid: "abababab-abab-abab-abab-abababababab", offset: 7)
        let bytes = LoroPresence.encode(f)
        XCTAssertTrue(LoroPresence.isPresenceFrame(bytes))
        XCTAssertEqual(LoroPresence.decode(bytes), f)
    }

    func testDecodeRejectsNonPresFrame() {
        let tlr2 = Data([0x54, 0x4c, 0x52, 0x32, 1, 2, 3]) // "TLR2"…
        XCTAssertNil(LoroPresence.decode(tlr2))
        XCTAssertFalse(LoroPresence.isPresenceFrame(tlr2))
    }

    /// A web-style frame (plain offset, no name) must decode on iOS — proves
    /// cross-platform wire compatibility with web/src/lib/loro/presence.ts.
    func testDecodesWebFrame() {
        var bytes = Data([0x50, 0x52, 0x45, 0x53]) // "PRES"
        bytes.append(
            ##"{"peer":"web","color":"#3b82f6","slug":"2026-06-27","bid":"b1","offset":3}"##
                .data(using: .utf8)!)
        let f = LoroPresence.decode(bytes)
        XCTAssertEqual(f?.peer, "web")
        XCTAssertEqual(f?.offset, 3)
        XCTAssertNil(f?.name)
    }
}

@MainActor
final class RemoteCursorStoreTests: XCTestCase {
    private func frame(peer: String = "other", slug: String = "d", bid: String = "b1", offset: Int = 4)
        -> LoroPresence.Frame
    {
        LoroPresence.Frame(peer: peer, color: "#3b82f6", name: nil, slug: slug, bid: bid, offset: offset)
    }

    func testApplyAndQueryByBlock() {
        let store = RemoteCursorStore()
        store.apply(frame(), now: Date(timeIntervalSince1970: 1000))
        let got = store.cursors(forSlug: "d", bid: "b1", now: Date(timeIntervalSince1970: 1000))
        XCTAssertEqual(got.count, 1)
        XCTAssertEqual(got.first?.offset, 4)
        XCTAssertEqual(store.cursors(forSlug: "d", bid: "other", now: Date(timeIntervalSince1970: 1000)).count, 0)
    }

    func testOwnPeerIgnored() {
        let store = RemoteCursorStore()
        store.apply(frame(peer: store.localPeer), now: Date(timeIntervalSince1970: 1000))
        XCTAssertEqual(store.cursors(forSlug: "d", bid: "b1", now: Date(timeIntervalSince1970: 1000)).count, 0)
    }

    func testStaleExpiresAndPrunes() {
        let store = RemoteCursorStore()
        store.apply(frame(), now: Date(timeIntervalSince1970: 1000))
        XCTAssertEqual(store.cursors(forSlug: "d", bid: "b1", now: Date(timeIntervalSince1970: 1009)).count, 1)
        XCTAssertEqual(store.cursors(forSlug: "d", bid: "b1", now: Date(timeIntervalSince1970: 1011)).count, 0)
        XCTAssertTrue(store.pruneStale(now: Date(timeIntervalSince1970: 1011)))
        XCTAssertFalse(store.pruneStale(now: Date(timeIntervalSince1970: 1011)))
    }

    func testColorDeterministic() {
        XCTAssertEqual(RemoteCursorStore.color(for: "abc"), RemoteCursorStore.color(for: "abc"))
    }
}
