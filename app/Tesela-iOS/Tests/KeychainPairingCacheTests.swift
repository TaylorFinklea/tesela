import XCTest
@testable import Tesela

/// tesela-tp0.2: the cached pairing code (group id + group key + relay
/// URL) moved from plaintext `UserDefaults` to the Keychain. These tests
/// pin the round-trip, the one-shot legacy-`UserDefaults` migration, and
/// `clear()` — the same shape as `RelayCursorScopingTests`' migration
/// coverage, but for the key material itself rather than cursor state.
final class KeychainPairingCacheTests: XCTestCase {

    private let legacyDefaultsKey = "relay.cachedPairingCode"

    override func setUp() {
        super.setUp()
        KeychainPairingCache.clear()
    }

    override func tearDown() {
        KeychainPairingCache.clear()
        super.tearDown()
    }

    func testSaveThenLoadRoundTrips() {
        XCTAssertNil(KeychainPairingCache.load())
        KeychainPairingCache.save("fake-pairing-code-payload")
        XCTAssertEqual(KeychainPairingCache.load(), "fake-pairing-code-payload")
    }

    func testSaveOverwritesPreviousValue() {
        KeychainPairingCache.save("first-code")
        KeychainPairingCache.save("second-code")
        XCTAssertEqual(KeychainPairingCache.load(), "second-code")
    }

    func testClearRemovesTheStoredValue() {
        KeychainPairingCache.save("fake-pairing-code-payload")
        KeychainPairingCache.clear()
        XCTAssertNil(KeychainPairingCache.load())
    }

    /// A pre-cutover install left the pairing code in plaintext
    /// `UserDefaults`. The first `load()` post-upgrade must adopt it into
    /// the Keychain and delete the plaintext copy — so the key material
    /// doesn't linger somewhere readable outside the Keychain.
    func testLoadMigratesLegacyPlaintextDefaultsAndDeletesIt() {
        UserDefaults.standard.set("legacy-plaintext-code", forKey: legacyDefaultsKey)
        defer { UserDefaults.standard.removeObject(forKey: legacyDefaultsKey) }

        let loaded = KeychainPairingCache.load()
        XCTAssertEqual(loaded, "legacy-plaintext-code")
        XCTAssertNil(
            UserDefaults.standard.string(forKey: legacyDefaultsKey),
            "the plaintext copy must be deleted once migrated into the Keychain"
        )

        // Second load hits the Keychain directly — no re-migration needed.
        XCTAssertEqual(KeychainPairingCache.load(), "legacy-plaintext-code")
    }

    func testSaveClearsAnyStaleLegacyDefaultsCopy() {
        UserDefaults.standard.set("stale-legacy-code", forKey: legacyDefaultsKey)
        defer { UserDefaults.standard.removeObject(forKey: legacyDefaultsKey) }

        KeychainPairingCache.save("fresh-code")
        XCTAssertNil(UserDefaults.standard.string(forKey: legacyDefaultsKey))
        XCTAssertEqual(KeychainPairingCache.load(), "fresh-code")
    }
}
