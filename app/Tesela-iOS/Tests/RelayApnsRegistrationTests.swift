import XCTest
@testable import Tesela

/// The APNs token must be (re-)registered with the CURRENT relay identity,
/// not keyed by token alone. 2026-06-24: after the HA→CF relay migration the
/// iPhone's token stayed registered with the OLD relay (registration was
/// once-per-session, token-only), so the CF Worker had NO token to
/// background-push — the app never woke in the background and web edits/deletes
/// looked stuck for hours until a manual reopen (a fresh session re-registered).
/// This is the same migration class the inbound-cursor scoping already fixed;
/// the registration key must carry the relay scope so a relay change forces a
/// re-register. See decisions.md 2026-06-24.
@MainActor
final class RelayApnsRegistrationTests: XCTestCase {

    func testSameTokenOnDifferentRelayReRegisters() {
        let token = "98a40d1a"
        let cf = RelayTicker.apnsRegistrationKey(token: token, scope: "https://cf.example|grp")
        let ha = RelayTicker.apnsRegistrationKey(token: token, scope: "https://ha.example|grp")
        XCTAssertNotEqual(cf, ha, "same token on a new relay must re-register (the HA→CF gap)")
    }

    func testSameTokenAndScopeIsStable() {
        let a = RelayTicker.apnsRegistrationKey(token: "t", scope: "s")
        let b = RelayTicker.apnsRegistrationKey(token: "t", scope: "s")
        XCTAssertEqual(a, b, "no needless re-register when nothing changed")
    }

    func testRotatedTokenReRegisters() {
        XCTAssertNotEqual(
            RelayTicker.apnsRegistrationKey(token: "old", scope: "s"),
            RelayTicker.apnsRegistrationKey(token: "new", scope: "s"),
            "an Apple token rotation must re-register")
    }
}
