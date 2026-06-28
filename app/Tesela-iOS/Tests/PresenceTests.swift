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

/// Relay-mode presence (Option B): the pure FFI seal/open/headers the
/// `PresenceRelaySocket` calls must byte-interoperate with the desktop bridge.
/// These exercise the Swift side of the FFI: an inner PRES frame seals → opens
/// → decodes back to the same caret, a wrong group key fails the AEAD tag, and
/// the upgrade-GET headers sign non-empty under a 32/16/16-byte identity.
final class PresenceRelayFfiTests: XCTestCase {
    private let groupKey = Data(repeating: 0xAB, count: 32)
    private let groupId = Data(repeating: 0x11, count: 16)
    private let deviceId = Data(repeating: 0x22, count: 16)

    func testSealOpenRoundTripsThroughLoroPresence() {
        let frame = LoroPresence.Frame(
            peer: "p1", color: "#22c55e", name: nil,
            slug: "2026-06-28", bid: "abababab-abab-abab-abab-abababababab", offset: 12)
        let inner = LoroPresence.encode(frame)
        let outer = presenceSeal(groupKey: groupKey, groupId: groupId, inner: inner)
        XCTAssertFalse(outer.isEmpty)
        XCTAssertNotEqual(outer, inner) // sealed, not the raw PRES bytes

        let opened = presenceOpen(groupKey: groupKey, groupId: groupId, outer: outer)
        XCTAssertNotNil(opened)
        XCTAssertEqual(LoroPresence.decode(opened!), frame)
    }

    func testOpenFailsUnderWrongGroupKey() {
        let inner = LoroPresence.encode(
            LoroPresence.Frame(peer: "p", color: "#ef4444", name: nil, slug: "d", bid: "b", offset: 1))
        let outer = presenceSeal(groupKey: groupKey, groupId: groupId, inner: inner)
        let wrongKey = Data(repeating: 0xCD, count: 32)
        XCTAssertNil(presenceOpen(groupKey: wrongKey, groupId: groupId, outer: outer))
    }

    func testWsHeadersSignNonEmpty() {
        let h = presenceWsHeaders(groupKey: groupKey, groupId: groupId, deviceId: deviceId)
        XCTAssertFalse(h.macB64.isEmpty)
        XCTAssertFalse(h.nonceB64.isEmpty)
        XCTAssertGreaterThan(h.ts, 0)
        XCTAssertEqual(h.groupHex, String(repeating: "11", count: 16))
        XCTAssertEqual(h.deviceHex, String(repeating: "22", count: 16))
    }
}
