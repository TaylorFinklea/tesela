import XCTest
@testable import Tesela

final class BlockContextMenuAvailabilityTests: XCTestCase {
    func testReadOnlyBlockDoesNotExposeAnInertContextMenu() {
        let modifier = ActionableBlockContextMenuModifier(
            blockId: "block-a",
            onAction: nil
        )

        XCTAssertFalse(modifier.isEnabled)
    }

    func testActionableBlockExposesItsContextMenu() {
        let modifier = ActionableBlockContextMenuModifier(
            blockId: "block-a",
            onAction: { _ in }
        )

        XCTAssertTrue(modifier.isEnabled)
    }
}
