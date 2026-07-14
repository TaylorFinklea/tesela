import XCTest
@testable import Tesela

/// Locks the relay cursor persistence schema (audit A5, commit ddd8def).
///
/// Relay seqs are a per-relay, per-group namespace, so the persisted
/// cursor keys MUST be scoped `relay.{in,out}boundCursor*.<relayUrl>|<groupIdHex>`
/// — a global cursor replayed against a different relay/group (re-pair,
/// relay DB wipe, the HA→CF migration) silently black-holes inbound
/// forever. These tests pin the exact key strings (changing them orphans
/// every device's persisted cursors) and fail-closed legacy quarantine.
@MainActor
final class RelayCursorScopingTests: XCTestCase {

    /// Throwaway defaults so the tests never touch the host app's
    /// `UserDefaults.standard` (which carries real pairing state on a
    /// dev simulator).
    private var suiteName: String!
    private var defaults: UserDefaults!

    override func setUp() {
        super.setUp()
        suiteName = "tesela-tests-\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suiteName)
    }

    override func tearDown() {
        defaults.removePersistentDomain(forName: suiteName)
        defaults = nil
        suiteName = nil
        super.tearDown()
    }

    // MARK: Key derivation

    func testScopedKeySchemeIsExact() {
        // Pin the exact persisted strings: these are the on-disk schema.
        let scope = RelayTicker.cursorScope(
            relayUrl: "https://relay.example.com",
            groupIdHex: "00112233445566778899aabbccddeeff"
        )
        XCTAssertEqual(scope, "https://relay.example.com|00112233445566778899aabbccddeeff")
        XCTAssertEqual(
            RelayTicker.inboundCursorKey(scope: scope),
            "relay.inboundCursorSeq.https://relay.example.com|00112233445566778899aabbccddeeff"
        )
        XCTAssertEqual(
            RelayTicker.outboundCursorKey(scope: scope),
            "relay.outboundCursorNtp.https://relay.example.com|00112233445566778899aabbccddeeff"
        )
    }

    func testLegacyKeysAreTheBareStrings() {
        // The quarantine looks for exactly these pre-scoping keys.
        XCTAssertEqual(RelayTicker.legacyInboundCursorKey, "relay.inboundCursorSeq")
        XCTAssertEqual(RelayTicker.legacyOutboundCursorKey, "relay.outboundCursorNtp")
    }

    func testDifferentRelayOrGroupYieldsDifferentKeys() {
        let a = RelayTicker.cursorScope(relayUrl: "https://relay-a.example", groupIdHex: "aa")
        let sameRelayOtherGroup = RelayTicker.cursorScope(relayUrl: "https://relay-a.example", groupIdHex: "bb")
        let otherRelaySameGroup = RelayTicker.cursorScope(relayUrl: "https://relay-b.example", groupIdHex: "aa")
        XCTAssertNotEqual(RelayTicker.inboundCursorKey(scope: a), RelayTicker.inboundCursorKey(scope: sameRelayOtherGroup))
        XCTAssertNotEqual(RelayTicker.inboundCursorKey(scope: a), RelayTicker.inboundCursorKey(scope: otherRelaySameGroup))
        XCTAssertNotEqual(RelayTicker.outboundCursorKey(scope: a), RelayTicker.outboundCursorKey(scope: sameRelayOtherGroup))
        // Inbound vs outbound never collide for the same scope.
        XCTAssertNotEqual(RelayTicker.inboundCursorKey(scope: a), RelayTicker.outboundCursorKey(scope: a))
    }

    // MARK: Legacy quarantine

    func testLegacyCursorsAreNotAdoptedIntoFreshScopedStore() async throws {
        defaults.set(Int64(42), forKey: RelayTicker.legacyInboundCursorKey)
        defaults.set(Int64(7), forKey: RelayTicker.legacyOutboundCursorKey)

        let documents = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: documents) }
        let engineScope = MosaicEngineScope(groupIdHex: "aa")
        let scopedRoot = engineScope.rootURL(documentsURL: documents)
        XCTAssertFalse(FileManager.default.fileExists(atPath: scopedRoot.path))
        try await RelayTicker.prepareStorage(for: engineScope, documentsURL: documents)

        let scope = RelayTicker.cursorScope(relayUrl: "https://relay.example", groupIdHex: "aa")
        RelayTicker.quarantineLegacyCursors(defaults: defaults)

        XCTAssertTrue(FileManager.default.fileExists(atPath: scopedRoot.path))
        XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: scopedRoot.path), [])
        XCTAssertNil(defaults.object(forKey: RelayTicker.inboundCursorKey(scope: scope)))
        XCTAssertNil(defaults.object(forKey: RelayTicker.outboundCursorKey(scope: scope)))
        XCTAssertNil(defaults.object(forKey: RelayTicker.legacyInboundCursorKey))
        XCTAssertNil(defaults.object(forKey: RelayTicker.legacyOutboundCursorKey))
    }

    func testNoScopeEverAdoptsAQuarantinedLegacyCursor() {
        defaults.set(Int64(42), forKey: RelayTicker.legacyInboundCursorKey)

        let first = RelayTicker.cursorScope(relayUrl: "https://relay-a.example", groupIdHex: "aa")
        RelayTicker.quarantineLegacyCursors(defaults: defaults)
        let second = RelayTicker.cursorScope(relayUrl: "https://relay-b.example", groupIdHex: "bb")
        RelayTicker.quarantineLegacyCursors(defaults: defaults)

        XCTAssertNil(defaults.object(forKey: RelayTicker.inboundCursorKey(scope: first)))
        XCTAssertNil(defaults.object(forKey: RelayTicker.inboundCursorKey(scope: second)))
    }

    func testLegacyQuarantineNeverRemovesAnExistingScopedCursor() {
        let scope = RelayTicker.cursorScope(relayUrl: "https://relay.example", groupIdHex: "aa")
        defaults.set(Int64(100), forKey: RelayTicker.inboundCursorKey(scope: scope))
        defaults.set(Int64(42), forKey: RelayTicker.legacyInboundCursorKey)

        RelayTicker.quarantineLegacyCursors(defaults: defaults)

        XCTAssertEqual(defaults.object(forKey: RelayTicker.inboundCursorKey(scope: scope)) as? Int64, 100)
        XCTAssertNil(defaults.object(forKey: RelayTicker.legacyInboundCursorKey))
    }

    func testQuarantineWithNoLegacyKeysIsANoOp() {
        let scope = RelayTicker.cursorScope(relayUrl: "https://relay.example", groupIdHex: "aa")
        RelayTicker.quarantineLegacyCursors(defaults: defaults)
        XCTAssertNil(defaults.object(forKey: RelayTicker.inboundCursorKey(scope: scope)))
        XCTAssertNil(defaults.object(forKey: RelayTicker.outboundCursorKey(scope: scope)))
    }
}
