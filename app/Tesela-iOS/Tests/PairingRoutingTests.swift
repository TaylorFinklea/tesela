import XCTest
@testable import Tesela

/// Locks the pairing adoption routing + cached-code invalidation rules
/// (audit A6/A13, commits 07ab601 / 182210d / 3acbeb0).
///
/// The last three iOS sync failures all lived here: a loopback embed
/// pairing set `.http(127.0.0.1)` (unreachable), the replacement routed
/// to Mock (fake data), and a transient rebuild error deleted the cached
/// pairing code (bricking `.relay`, which has no Mac HTTP to refetch it).
@MainActor
final class PairingRoutingTests: XCTestCase {

    private func record(url: String, relayUrl: String?) -> PairingCodeRecord {
        PairingCodeRecord(
            groupIdHex: String(repeating: "ab", count: 16),
            groupKeyHex: String(repeating: "cd", count: 32),
            deviceIdHex: String(repeating: "ef", count: 16),
            url: url,
            displayName: "test-node",
            version: 2,
            relayUrl: relayUrl
        )
    }

    // MARK: isRelayOnlyPairing — adopt routing

    func testLoopbackInviterWithRelayPairsViaRelay() {
        // The Tauri desktop embed: loopback URL the phone can never reach.
        XCTAssertTrue(RelayTicker.isRelayOnlyPairing(record(url: "http://127.0.0.1:7474", relayUrl: "https://relay.example")))
        XCTAssertTrue(RelayTicker.isRelayOnlyPairing(record(url: "http://localhost:7474", relayUrl: "https://relay.example")))
        XCTAssertTrue(RelayTicker.isRelayOnlyPairing(record(url: "http://[::1]:7474", relayUrl: "https://relay.example")))
        XCTAssertTrue(RelayTicker.isRelayOnlyPairing(record(url: "http://0.0.0.0:7474", relayUrl: "https://relay.example")))
        XCTAssertTrue(RelayTicker.isRelayOnlyPairing(record(url: "", relayUrl: "https://relay.example")))
    }

    func testReachableInviterPairsHttpDirect() {
        // LAN / Tailscale node with a real URL → thin HTTP client.
        XCTAssertFalse(RelayTicker.isRelayOnlyPairing(record(url: "http://192.168.1.20:7474", relayUrl: "https://relay.example")))
        XCTAssertFalse(RelayTicker.isRelayOnlyPairing(record(url: "http://100.64.0.5:7474", relayUrl: "https://relay.example")))
    }

    func testNoRelayUrlNeverRoutesToRelay() {
        // LAN-only inviter (relayUrl nil/empty): even a loopback URL can't
        // pair via relay — there is no relay to tick against.
        XCTAssertFalse(RelayTicker.isRelayOnlyPairing(record(url: "http://127.0.0.1:7474", relayUrl: nil)))
        XCTAssertFalse(RelayTicker.isRelayOnlyPairing(record(url: "http://127.0.0.1:7474", relayUrl: "")))
        XCTAssertFalse(RelayTicker.isRelayOnlyPairing(record(url: "", relayUrl: nil)))
    }

    func testHostnameMerelyContainingLocalhostIsNotLoopback() {
        // "//localhost" (with the scheme slashes) is the loopback marker —
        // a real host that merely ends in "...localhost.example" must not match.
        XCTAssertFalse(RelayTicker.isRelayOnlyPairing(record(url: "http://my-localhost.example:7474", relayUrl: "https://relay.example")))
    }

    // MARK: BackendSettings mode→backend mapping

    func testRelayModeResolvesToRelayBackendIgnoringServerURL() {
        XCTAssertEqual(BackendSettings.resolveBackend(mode: .relay, serverURL: "http://192.168.1.20:7474"), .relay)
        XCTAssertEqual(BackendSettings.resolveBackend(mode: .relay, serverURL: ""), .relay)
        XCTAssertEqual(BackendSettings.resolveBackend(mode: .relay, serverURL: "not a url"), .relay)
    }

    func testHttpModeWithValidURLResolvesToHttp() {
        XCTAssertEqual(
            BackendSettings.resolveBackend(mode: .http, serverURL: "http://100.64.0.5:7474"),
            .http(URL(string: "http://100.64.0.5:7474")!)
        )
        // Whitespace is trimmed before parsing.
        XCTAssertEqual(
            BackendSettings.resolveBackend(mode: .http, serverURL: "  http://10.0.0.5:7474  "),
            .http(URL(string: "http://10.0.0.5:7474")!)
        )
    }

    func testHttpModeWithUnusableURLFallsBackToMock() {
        XCTAssertEqual(BackendSettings.resolveBackend(mode: .http, serverURL: ""), .mock)
        XCTAssertEqual(BackendSettings.resolveBackend(mode: .http, serverURL: "   "), .mock)
        XCTAssertEqual(BackendSettings.resolveBackend(mode: .http, serverURL: "no-scheme-or-host"), .mock)
    }

    func testMockModeResolvesToMock() {
        XCTAssertEqual(BackendSettings.resolveBackend(mode: .mock, serverURL: "http://10.0.0.5:7474"), .mock)
    }

    // MARK: isDefinitivePairingFailure — cached-code invalidation

    func testInvalidPairingCodeIsDefinitive() {
        // The cached blob doesn't even decode — retrying can never succeed.
        XCTAssertTrue(RelayTicker.isDefinitivePairingFailure(
            FfiSyncError.InvalidPairingCode(message: "bad base64")
        ))
    }

    func testCryptoVerifyFailuresAreDefinitive() {
        // `register_or_recover` / `verify_registration` hijack messages —
        // the relay's stored registration doesn't verify under our key.
        XCTAssertTrue(RelayTicker.isDefinitivePairingFailure(
            FfiSyncError.Other(message: "stored registration does not verify under the group key")
        ))
        XCTAssertTrue(RelayTicker.isDefinitivePairingFailure(
            FfiSyncError.Other(message: "relay registration HIJACK detected for group")
        ))
    }

    func testTransientErrorsKeepTheCachedCode() {
        // Network-ish failures MUST keep the cache: `.relay` mode has no
        // Mac HTTP to refetch the code from — deleting it bricks sync
        // until the user re-scans the QR (audit A6).
        XCTAssertFalse(RelayTicker.isDefinitivePairingFailure(
            FfiSyncError.Other(message: "error sending request for url (https://relay.example/registration): connection refused")
        ))
        XCTAssertFalse(RelayTicker.isDefinitivePairingFailure(
            FfiSyncError.Other(message: "relay returned 503 Service Unavailable")
        ))
        XCTAssertFalse(RelayTicker.isDefinitivePairingFailure(
            FfiSyncError.Other(message: "operation timed out")
        ))
        // Non-FFI errors (URLSession etc.) are never definitive.
        XCTAssertFalse(RelayTicker.isDefinitivePairingFailure(
            URLError(.timedOut)
        ))
    }
}
