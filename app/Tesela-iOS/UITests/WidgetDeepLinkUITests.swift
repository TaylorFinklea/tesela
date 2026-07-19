import XCTest

final class WidgetDeepLinkUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments += ["-onboardingComplete", "YES"]
        app.launch()
    }

    func testAgendaWidgetURLSelectsAgendaOnColdLaunch() throws {
        app.terminate()
        app.open(try XCTUnwrap(URL(string: "tesela://agenda")))

        assertSelectedTab("Agenda")
    }

    func testWidgetURLsRouteBetweenTabsWhileRunning() throws {
        app.open(try XCTUnwrap(URL(string: "tesela://agenda")))
        assertSelectedTab("Agenda")

        app.open(try XCTUnwrap(URL(string: "tesela://views")))
        assertSelectedTab("Views")
    }

    private func assertSelectedTab(_ name: String) {
        let tab = app.tabBars.buttons[name]
        XCTAssertTrue(tab.waitForExistence(timeout: 10), "Expected the \(name) tab to exist")
        XCTAssertTrue(tab.isSelected, "Expected the \(name) tab to be selected")
    }
}
