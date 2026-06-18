import XCTest
@testable import Tesela

/// Pure codec tests for the keyboard toolbar preference (`@AppStorage`
/// round-trip). These tests cover the encode / decode / de-dup / unknown
/// handling of the `KeyboardToolbarItem` enum so that the user-facing
/// toolbar customization stays stable while view polish continues. The
/// tests are intentionally header-only — they do not touch the toolbar
/// UI, edit sync, or any other view-layer code.
final class KeyboardToolbarItemTests: XCTestCase {

    // MARK: - Default ordering

    func testDefaultOrderIsPaletteThroughMic() {
        XCTAssertEqual(
            defaultKeyboardToolbarItems,
            [.commandPalette, .slashCommand, .backlink, .dedent, .indent, .cycleStatus, .mic]
        )
    }

    func testDefaultOrderExcludesPinnedHideAndOmitsOptionalItems() {
        // The Hide-keyboard button is pinned to the trailing edge by
        // `BlockRow` and never appears in the user-configurable default.
        // `.date` and `.tags` are valid cases but are not part of the
        // default seed.
        XCTAssertFalse(defaultKeyboardToolbarItems.contains(.hideKeyboard))
        XCTAssertFalse(defaultKeyboardToolbarItems.contains(.date))
        XCTAssertFalse(defaultKeyboardToolbarItems.contains(.tags))
    }

    func testCaseIterableOrderMatchesDeclaration() {
        // CaseIterable is order-sensitive: any new case slotted in
        // *between* existing ones would shift these indices. Lock the
        // order so a refactor cannot quietly reorder saved layouts.
        XCTAssertEqual(
            Array(KeyboardToolbarItem.allCases),
            [
                .hideKeyboard,
                .slashCommand,
                .backlink,
                .dedent,
                .indent,
                .cycleStatus,
                .mic,
                .date,
                .tags,
                .commandPalette,
            ]
        )
    }

    func testDefaultRawMatchesEncodedDefault() {
        XCTAssertEqual(
            defaultKeyboardToolbarItemsRaw,
            encodeKeyboardToolbarItems(defaultKeyboardToolbarItems)
        )
        XCTAssertEqual(
            defaultKeyboardToolbarItemsRaw,
            "palette,slash,backlink,dedent,indent,status,mic"
        )
    }

    // MARK: - Encode / decode round-trip

    func testEncodeJoinsRawValuesWithCommas() {
        XCTAssertEqual(
            encodeKeyboardToolbarItems([.slashCommand, .mic]),
            "slash,mic"
        )
        XCTAssertEqual(
            encodeKeyboardToolbarItems([.indent, .dedent, .indent]),
            "indent,dedent,indent"
        )
    }

    func testDecodeRoundTripsEveryCase() {
        for item in KeyboardToolbarItem.allCases {
            let raw = encodeKeyboardToolbarItems([item])
            XCTAssertEqual(decodeKeyboardToolbarItems(raw), [item], "round-trip failed for \(item)")
        }
    }

    func testDecodeRoundTripsTheDefaultSeed() {
        let raw = defaultKeyboardToolbarItemsRaw
        XCTAssertEqual(decodeKeyboardToolbarItems(raw), defaultKeyboardToolbarItems)
    }

    func testDecodeRoundTripsAnArbitraryReordering() {
        let custom: [KeyboardToolbarItem] = [.mic, .indent, .slashCommand, .dedent]
        let raw = encodeKeyboardToolbarItems(custom)
        XCTAssertEqual(decodeKeyboardToolbarItems(raw), custom)
    }

    // MARK: - Duplicate removal

    func testDecodeDropsDuplicatesPreservingFirstOccurrence() {
        XCTAssertEqual(
            decodeKeyboardToolbarItems("slash,slash,backlink"),
            [.slashCommand, .backlink]
        )
        XCTAssertEqual(
            decodeKeyboardToolbarItems("status,slash,status,mic,slash"),
            [.cycleStatus, .slashCommand, .mic]
        )
    }

    func testDecodeDeduplicatesConsecutiveAndNonConsecutive() {
        XCTAssertEqual(
            decodeKeyboardToolbarItems("slash,slash,slash"),
            [.slashCommand]
        )
        XCTAssertEqual(
            decodeKeyboardToolbarItems("indent,dedent,indent,dedent,indent"),
            [.indent, .dedent]
        )
    }

    // MARK: - Unknown value handling

    func testDecodeDropsUnknownRawValues() {
        XCTAssertEqual(
            decodeKeyboardToolbarItems("slash,foo,backlink,bar"),
            [.slashCommand, .backlink]
        )
        XCTAssertEqual(
            decodeKeyboardToolbarItems("nope,slash,also-nope,mic"),
            [.slashCommand, .mic]
        )
    }

    func testDecodeReturnsEmptyForOnlyUnknownValues() {
        XCTAssertEqual(decodeKeyboardToolbarItems("foo,bar,baz"), [])
        XCTAssertEqual(decodeKeyboardToolbarItems("datepicker,voice"), [])
    }

    func testDecodeIgnoresEmptyPiecesFromConsecutiveOrEdgeCommas() {
        // Swift's `split(separator:)` omits empty subsequences by default,
        // so `,,` / `,slash,` / `slash,,` all behave as the trimmed list.
        XCTAssertEqual(decodeKeyboardToolbarItems(",,"), [])
        XCTAssertEqual(decodeKeyboardToolbarItems(",slash,"), [.slashCommand])
        XCTAssertEqual(decodeKeyboardToolbarItems("slash,,mic"), [.slashCommand, .mic])
    }

    // MARK: - Empty / edge cases

    func testDecodeReturnsEmptyForEmptyString() {
        XCTAssertEqual(decodeKeyboardToolbarItems(""), [])
    }

    func testDecodeSingleItem() {
        XCTAssertEqual(decodeKeyboardToolbarItems("mic"), [.mic])
        XCTAssertEqual(decodeKeyboardToolbarItems("hide"), [.hideKeyboard])
    }

    func testEncodeEmptyArrayProducesEmptyString() {
        XCTAssertEqual(encodeKeyboardToolbarItems([]), "")
    }

    // MARK: - ID + label stability

    func testIdMatchesRawValue() {
        // `Identifiable.id` is used as the SwiftUI `ForEach` key; if it
        // ever diverges from the raw value, list diffing would churn on
        // every encode/decode.
        for item in KeyboardToolbarItem.allCases {
            XCTAssertEqual(item.id, item.rawValue, "id diverged for \(item)")
        }
    }

    func testLabelsAreNonEmpty() {
        for item in KeyboardToolbarItem.allCases {
            XCTAssertFalse(item.label.isEmpty, "empty label for \(item)")
        }
    }
}
