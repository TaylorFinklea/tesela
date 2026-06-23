import XCTest
import SwiftUI
@testable import Tesela

/// Phase 5.6 — chip-color resolution. The web `DisplayChip` (Phase 4) tints a
/// select VALUE chip from the Property page's `choice_colors`, keyed by the
/// lowercased choice. iOS mirrors that: the resolved `PropertyDef.choiceColors`
/// drives the `PropertyChip` tint via `TagPalette.resolveOverride` (the same
/// hex/named-hue parser the tag pills use), falling back to the muted style
/// when a choice has no color.
///
/// These tests exercise the resolution path BlockRow's private `chipTint`
/// composes: (1) the registry resolves `choice_colors` off the Property page
/// onto the resolved def; (2) the CSS string maps to a concrete color; (3)
/// uncolored / unknown choices resolve to nil (→ unchanged muted chip).
final class ChipColorResolutionTests: XCTestCase {

    // MARK: - Fixtures

    private func note(_ title: String, _ noteType: String, _ custom: [String: Any]) -> RegistryNote {
        RegistryNote(title: title, noteType: noteType, custom: custom)
    }

    /// A select Property page carrying `choice_colors` (keys lowercased by the
    /// parser). `done` is a named hue; `blocked` a hex; `todo` deliberately
    /// has no color (falls back to muted).
    private var statusPage: RegistryNote {
        note("Status", "Property", [
            "value_type": "select",
            "choices": ["todo", "doing", "done", "blocked"],
            "choice_colors": ["done": "green", "blocked": "#E8697F", "doing": "amber"],
        ])
    }

    /// A multi-select Property page — color resolves by the FIRST matching
    /// choice in a `, `-joined value (mirror of the web behavior).
    private var labelsPage: RegistryNote {
        note("Labels", "Property", [
            "value_type": "multi-select",
            "choices": ["urgent", "chill"],
            "choice_colors": ["urgent": "coral"],
        ])
    }

    private var taskTag: RegistryNote {
        note("Task", "Tag", [
            "tag_properties": ["Status", "Labels"],
        ])
    }

    private func registry() -> PropertyRegistry {
        PropertyRegistry.build(from: [statusPage, labelsPage, taskTag])
    }

    private func def(_ reg: PropertyRegistry, _ tag: String, _ name: String) -> PropertyDef? {
        reg.resolvedDefs(forTag: tag).first { $0.name.lowercased() == name.lowercased() }
    }

    // MARK: - Registry carries choice_colors onto the resolved def

    func testResolvedDefCarriesLowercasedChoiceColors() {
        guard let status = def(registry(), "Task", "Status") else {
            return XCTFail("Status not resolved for Task")
        }
        XCTAssertEqual(status.valueType, .select)
        XCTAssertEqual(status.choiceColors["done"], "green")
        XCTAssertEqual(status.choiceColors["blocked"], "#E8697F")
        XCTAssertEqual(status.choiceColors["doing"], "amber")
        // `todo` is a valid choice but has no declared color.
        XCTAssertNil(status.choiceColors["todo"])
    }

    // MARK: - CSS string → concrete color for a value WITH a color

    func testNamedHueChoiceColorResolvesToConcreteColor() {
        guard let status = def(registry(), "Task", "Status") else {
            return XCTFail("Status not resolved")
        }
        // "done" → "green" → the curated palette green.
        guard let css = status.choiceColors["done"],
              let hex = TagPalette.resolveOverride(css) else {
            return XCTFail("expected a resolvable color for done")
        }
        XCTAssertEqual(hex, TagPalette.named["green"])
    }

    func testHexChoiceColorResolvesToThatHex() {
        guard let status = def(registry(), "Task", "Status") else {
            return XCTFail("Status not resolved")
        }
        guard let css = status.choiceColors["blocked"],
              let hex = TagPalette.resolveOverride(css) else {
            return XCTFail("expected a resolvable color for blocked")
        }
        XCTAssertEqual(hex, 0xE8697F)
    }

    // MARK: - Uncolored / unknown → nil (chip stays muted)

    func testUncoloredChoiceHasNoColor() {
        guard let status = def(registry(), "Task", "Status") else {
            return XCTFail("Status not resolved")
        }
        // The value "todo" is a real choice but carries no color entry → nil.
        XCTAssertNil(status.choiceColors["todo"])
    }

    func testEmptyChoiceColorsWhenPropertyDeclaresNone() {
        // A select Property with NO choice_colors resolves to an empty map,
        // so every value falls back to the muted chip.
        let plain = note("Stage", "Property", [
            "value_type": "select",
            "choices": ["a", "b"],
        ])
        let tag = note("Thing", "Tag", ["tag_properties": ["Stage"]])
        let reg = PropertyRegistry.build(from: [plain, tag])
        guard let stage = def(reg, "Thing", "Stage") else {
            return XCTFail("Stage not resolved")
        }
        XCTAssertTrue(stage.choiceColors.isEmpty)
    }

    // MARK: - Multi-select colors by the first matching choice

    func testMultiSelectColorsByFirstMatchingChoice() {
        guard let labels = def(registry(), "Task", "Labels") else {
            return XCTFail("Labels not resolved")
        }
        XCTAssertEqual(labels.valueType, .multiSelect)
        // A value "urgent, chill" should pick "urgent" (the first declared).
        let value = "urgent, chill"
        let parts = value.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces).lowercased() }
        let firstHit = parts.compactMap { labels.choiceColors[$0] }.first
        XCTAssertEqual(firstHit, "coral")
        XCTAssertEqual(firstHit.flatMap(TagPalette.resolveOverride), TagPalette.named["coral"])
    }

    // MARK: - The built-in mock registry (no colors) keeps chips muted

    func testBuiltinTaskStatusHasNoChoiceColors() {
        // The mock built-ins seed Task→Status with choices but no colors, so
        // mock-backend chips stay muted until a user adds choice_colors.
        let reg = PropertyRegistry.buildBuiltins()
        guard let status = reg.resolvedDefs(forTag: "Task").first(where: {
            $0.name.lowercased() == "status"
        }) else {
            return XCTFail("Status not resolved for built-in Task")
        }
        XCTAssertTrue(status.choiceColors.isEmpty)
    }
}
