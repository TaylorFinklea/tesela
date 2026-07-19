import XCTest
@testable import Tesela

final class PropertyEditingTests: XCTestCase {
    func testMultiSelectDeltaPreservesMembersAndEmitsIndependentChanges() {
        XCTAssertEqual(PropertyEditing.multiSelectValues("alpha, beta, alpha"), ["alpha", "beta"])
        XCTAssertEqual(
            PropertyEditing.multiSelectDelta(
                currentValue: "alpha, beta",
                selected: ["beta", "gamma"]
            ),
            PropertyListDelta(current: ["alpha", "beta"], add: ["gamma"], remove: ["alpha"])
        )
    }

    func testCheckboxToggleIsCanonical() {
        XCTAssertTrue(PropertyEditing.isChecked("TRUE"))
        XCTAssertEqual(PropertyEditing.toggledCheckboxValue("true"), "false")
        XCTAssertEqual(PropertyEditing.toggledCheckboxValue("false"), "true")
    }

    func testLinksNormalizeAndRejectUnsafeSchemes() {
        XCTAssertEqual(
            PropertyEditing.linkURL(valueType: .url, value: "example.com")?.absoluteString,
            "https://example.com"
        )
        XCTAssertNil(PropertyEditing.linkURL(valueType: .url, value: "javascript:alert(1)"))
        XCTAssertEqual(
            PropertyEditing.linkURL(valueType: .email, value: "hello@example.com")?.absoluteString,
            "mailto:hello@example.com"
        )
        XCTAssertEqual(
            PropertyEditing.linkURL(valueType: .phone, value: "+1 (312) 555-0199")?.absoluteString,
            "tel:+13125550199"
        )
    }
}
