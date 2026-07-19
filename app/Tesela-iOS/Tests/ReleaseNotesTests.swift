import XCTest
@testable import Tesela

@MainActor
final class ReleaseNotesTests: XCTestCase {
    private func canonicalData() throws -> Data {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("release-notes/releases.json")
        return try Data(contentsOf: url)
    }

    private func catalog() throws -> ReleaseNotesCatalog {
        try ReleaseNotesCatalogSource.decode(canonicalData())
    }

    func testCanonicalCatalogLoadsWithCurrentAndOlderIOSReleases() throws {
        let catalog = try catalog()

        XCTAssertEqual(catalog.schemaVersion, 1)
        XCTAssertEqual(catalog.current.ios, "2026-07-19.ios-1.1-81")
        XCTAssertEqual(
            catalog.history(for: .ios).map(\.id),
            [
                "2026-07-19.ios-1.1-81",
                "2026-07-15.ios-1.1-80",
                "2026-07-14.ios-1.1-79",
                "2026-07-08.ios-1.1-75",
            ]
        )
    }

    func testBundledCatalogMatchesCanonicalBytes() throws {
        let bundledURL = try XCTUnwrap(
            Bundle.main.url(forResource: "ReleaseNotes", withExtension: "json")
        )
        XCTAssertEqual(try Data(contentsOf: bundledURL), try canonicalData())
    }

    func testSeenDecisionCoversMissingUnknownOlderAndCurrent() throws {
        let catalog = try catalog()

        XCTAssertTrue(catalog.shouldPresentCurrent(for: .ios, lastSeen: nil))
        XCTAssertTrue(catalog.shouldPresentCurrent(for: .ios, lastSeen: "unknown"))
        XCTAssertTrue(
            catalog.shouldPresentCurrent(for: .ios, lastSeen: "2026-07-15.ios-1.1-80")
        )
        XCTAssertFalse(
            catalog.shouldPresentCurrent(for: .ios, lastSeen: "2026-07-19.ios-1.1-81")
        )
    }

    func testDowngradeDoesNotReopenAnOlderCurrentRelease() throws {
        var json = try XCTUnwrap(
            JSONSerialization.jsonObject(with: canonicalData()) as? [String: Any]
        )
        var current = try XCTUnwrap(json["current"] as? [String: Any])
        current["ios"] = "2026-07-14.ios-1.1-79"
        json["current"] = current
        let downgraded = try ReleaseNotesCatalogSource.decode(
            JSONSerialization.data(withJSONObject: json)
        )

        XCTAssertFalse(
            downgraded.shouldPresentCurrent(
                for: .ios,
                lastSeen: "2026-07-15.ios-1.1-80"
            )
        )
    }

    func testPresenterWaitsForOnboardingAndMarksOnlyAfterCurrentRenders() throws {
        let suiteName = "ReleaseNotesTests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let presenter = ReleaseNotesPresenter(catalog: try catalog(), defaults: defaults)

        presenter.autoPresentIfNeeded(onboardingComplete: false)
        XCTAssertNil(presenter.presentation)
        XCTAssertNil(defaults.string(forKey: presenter.seenKey))

        presenter.autoPresentIfNeeded(onboardingComplete: true)
        XCTAssertEqual(presenter.presentation?.current?.id, "2026-07-19.ios-1.1-81")
        XCTAssertNil(defaults.string(forKey: presenter.seenKey))

        presenter.markCurrentRendered()
        XCTAssertEqual(
            defaults.string(forKey: presenter.seenKey),
            "2026-07-19.ios-1.1-81"
        )

        presenter.presentation = nil
        presenter.autoPresentIfNeeded(onboardingComplete: true)
        XCTAssertNil(presenter.presentation)
    }

    func testManualPresentationFailsSoftWhenCatalogIsUnavailable() throws {
        let suiteName = "ReleaseNotesTests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let presenter = ReleaseNotesPresenter(catalog: nil, defaults: defaults)

        presenter.autoPresentIfNeeded(onboardingComplete: true)
        XCTAssertNil(presenter.presentation)

        presenter.presentCurrent()
        XCTAssertNotNil(presenter.presentation)
        XCTAssertNil(presenter.presentation?.current)

        presenter.markCurrentRendered()
        XCTAssertNil(defaults.string(forKey: presenter.seenKey))
    }

    func testAppVersionLabelIsDynamicAndComplete() {
        XCTAssertEqual(
            ReleaseNotesAppVersion.displayName(marketing: "1.1", build: "80"),
            "Tesela 1.1 (80)"
        )
        XCTAssertEqual(
            ReleaseNotesAppVersion.displayName(marketing: "1.1", build: nil),
            "Tesela 1.1"
        )
    }
}
