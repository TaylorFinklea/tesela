import XCTest
@testable import Tesela

/// Pins which block chip groups render while a block is being edited.
///
/// 2026-06-24 device-test finding: setting a date on iOS gave no feedback
/// while the block was focused — the user thought it hadn't worked and only
/// saw the chip after navigating away. Cause: the whole chip row was gated
/// on `!isEditing`. Tags / inline properties belong hidden during edit (they
/// live in the editable prose and would duplicate), but DATE chips
/// (scheduled / deadline / recurring) are structured properties not in the
/// edited text, so they must stay visible while editing.
@MainActor
final class BlockRowChipVisibilityTests: XCTestCase {

    func testDateChipsStayVisibleWhileEditing() {
        let vis = BlockRow.chipVisibility(
            hasTags: false, hasDate: true, hasProps: false, isEditing: true)
        XCTAssertTrue(vis.dates, "a date must stay visible while its block is focused")
    }

    func testTagsAndPropsHiddenWhileEditing() {
        let vis = BlockRow.chipVisibility(
            hasTags: true, hasDate: false, hasProps: true, isEditing: true)
        XCTAssertFalse(vis.tags, "tags are inline in the prose during edit — don't duplicate")
        XCTAssertFalse(vis.props, "inline properties stay hidden during edit")
    }

    func testEverythingVisibleWhenNotEditing() {
        let vis = BlockRow.chipVisibility(
            hasTags: true, hasDate: true, hasProps: true, isEditing: false)
        XCTAssertTrue(vis.tags)
        XCTAssertTrue(vis.dates)
        XCTAssertTrue(vis.props)
    }

    func testNothingVisibleWhenAbsent() {
        let vis = BlockRow.chipVisibility(
            hasTags: false, hasDate: false, hasProps: false, isEditing: false)
        XCTAssertFalse(vis.tags)
        XCTAssertFalse(vis.dates)
        XCTAssertFalse(vis.props)
    }
}
