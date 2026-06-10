import XCTest
@testable import Tesela

/// Locks the relay cursor persistence schema (audit A5, commit ddd8def).
///
/// Relay seqs are a per-relay, per-group namespace, so the persisted
/// cursor keys MUST be scoped `relay.{in,out}boundCursor*.<relayUrl>|<groupIdHex>`
/// — a global cursor replayed against a different relay/group (re-pair,
/// relay DB wipe, the HA→CF migration) silently black-holes inbound
/// forever. These tests pin the exact key strings (changing them orphans
/// every device's persisted cursors) and the one-shot legacy migration.
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
        // The migration looks for exactly these pre-scoping keys.
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

    // MARK: Legacy migration

    func testLegacyCursorsAdoptedByFirstScopeThenRemoved() {
        defaults.set(Int64(42), forKey: RelayTicker.legacyInboundCursorKey)
        defaults.set(Int64(7), forKey: RelayTicker.legacyOutboundCursorKey)

        let scope = RelayTicker.cursorScope(relayUrl: "https://relay.example", groupIdHex: "aa")
        RelayTicker.migrateLegacyCursors(toScope: scope, defaults: defaults)

        // Adopted under the scoped keys (in-place upgrades keep progress)…
        XCTAssertEqual(defaults.object(forKey: RelayTicker.inboundCursorKey(scope: scope)) as? Int64, 42)
        XCTAssertEqual(defaults.object(forKey: RelayTicker.outboundCursorKey(scope: scope)) as? Int64, 7)
        // …and the bare keys are GONE so no later pairing re-adopts them.
        XCTAssertNil(defaults.object(forKey: RelayTicker.legacyInboundCursorKey))
        XCTAssertNil(defaults.object(forKey: RelayTicker.legacyOutboundCursorKey))
    }

    func testLegacyCursorsAdoptedOnlyOnce() {
        defaults.set(Int64(42), forKey: RelayTicker.legacyInboundCursorKey)

        let first = RelayTicker.cursorScope(relayUrl: "https://relay-a.example", groupIdHex: "aa")
        RelayTicker.migrateLegacyCursors(toScope: first, defaults: defaults)
        let second = RelayTicker.cursorScope(relayUrl: "https://relay-b.example", groupIdHex: "bb")
        RelayTicker.migrateLegacyCursors(toScope: second, defaults: defaults)

        // The first pairing post-upgrade owns the legacy progress; a later
        // DIFFERENT identity starts fresh (cursor 0 → snapshot bootstrap).
        XCTAssertEqual(defaults.object(forKey: RelayTicker.inboundCursorKey(scope: first)) as? Int64, 42)
        XCTAssertNil(defaults.object(forKey: RelayTicker.inboundCursorKey(scope: second)))
    }

    func testLegacyMigrationNeverOverwritesAnExistingScopedCursor() {
        let scope = RelayTicker.cursorScope(relayUrl: "https://relay.example", groupIdHex: "aa")
        defaults.set(Int64(100), forKey: RelayTicker.inboundCursorKey(scope: scope))
        defaults.set(Int64(42), forKey: RelayTicker.legacyInboundCursorKey)

        RelayTicker.migrateLegacyCursors(toScope: scope, defaults: defaults)

        // Scoped progress wins; the stale bare key is still removed.
        XCTAssertEqual(defaults.object(forKey: RelayTicker.inboundCursorKey(scope: scope)) as? Int64, 100)
        XCTAssertNil(defaults.object(forKey: RelayTicker.legacyInboundCursorKey))
    }

    func testMigrationWithNoLegacyKeysIsANoOp() {
        let scope = RelayTicker.cursorScope(relayUrl: "https://relay.example", groupIdHex: "aa")
        RelayTicker.migrateLegacyCursors(toScope: scope, defaults: defaults)
        XCTAssertNil(defaults.object(forKey: RelayTicker.inboundCursorKey(scope: scope)))
        XCTAssertNil(defaults.object(forKey: RelayTicker.outboundCursorKey(scope: scope)))
    }
}
