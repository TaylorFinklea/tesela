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

    // MARK: - FIX 5: chip value/label/icon formatting per resolved def

    private func makeDef(
        name: String,
        valueType: PropertyType,
        choices: [String] = [],
        chipIcon: String? = nil,
        chipLabelMode: ChipLabelMode? = nil,
        chipShortLabel: String? = nil,
        chipValueFormat: ChipValueFormat? = nil
    ) -> PropertyDef {
        PropertyDef(
            name: name, valueType: valueType, choices: choices, def: nil, show: nil,
            hideByDefault: false, hideEmpty: true,
            chipIcon: chipIcon, chipLabelMode: chipLabelMode,
            chipShortLabel: chipShortLabel, chipValueFormat: chipValueFormat,
            chordKey: nil, valueChordKeys: [:], choiceColors: [:], nlTriggers: []
        )
    }

    /// A `.date`-typed def with no explicit `chip_value_format` defaults to
    /// month-day, so `[[2026-05-13]]` renders "May 13" (not the literal link).
    func testDateDefDefaultsToMonthDayFormat() {
        let due = makeDef(name: "Due", valueType: .date)
        XCTAssertEqual(ChipFormat.valueFormat(for: due), .monthDay)
        XCTAssertEqual(ChipFormat.formattedValue("[[2026-05-13]]", def: due), "May 13")
        // A bare ISO date formats identically.
        XCTAssertEqual(ChipFormat.formattedValue("2026-05-13", def: due), "May 13")
    }

    /// A non-date def with no format default keeps the raw value (truncated),
    /// so a custom text/number prop shows verbatim.
    func testTextDefKeepsRawValue() {
        let points = makeDef(name: "Points", valueType: .number)
        XCTAssertEqual(ChipFormat.valueFormat(for: points), .value)
        XCTAssertEqual(ChipFormat.formattedValue("8", def: points), "8")
    }

    /// A select def with `chip_value_format: bars` ranks the value across its
    /// choices (low→high), so the highest choice fills all three segments.
    func testSelectBarsFormatRanksByChoice() {
        let energy = makeDef(
            name: "Energy", valueType: .select,
            choices: ["low", "medium", "high"], chipValueFormat: .bars
        )
        XCTAssertEqual(ChipFormat.formattedValue("high", def: energy), "▰▰▰")
        XCTAssertEqual(ChipFormat.formattedValue("low", def: energy), "▰▱▱")
    }

    /// `chip_label_mode` + `chip_short_label` drive the label text; `icon`/`none`
    /// suppress it. Derived mode is `icon` when a `chip_icon` is set, else `full`.
    func testLabelModeAndShortLabel() {
        let full = makeDef(name: "Status", valueType: .select)
        XCTAssertEqual(ChipFormat.labelMode(for: full), .full)
        XCTAssertEqual(ChipFormat.labelText(for: full, fallbackKey: "status"), "Status")

        let short = makeDef(
            name: "Priority", valueType: .select,
            chipLabelMode: .short, chipShortLabel: "Pri"
        )
        XCTAssertEqual(ChipFormat.labelText(for: short, fallbackKey: "priority"), "Pri")

        let iconOnly = makeDef(name: "Deadline", valueType: .date, chipIcon: "flag")
        XCTAssertEqual(ChipFormat.labelMode(for: iconOnly), .icon)
        XCTAssertNil(ChipFormat.labelText(for: iconOnly, fallbackKey: "deadline"))
    }

    /// `chip_icon` resolves a known Tabler name to an SF Symbol; an emoji /
    /// unknown name falls back to raw text (mirror web `resolveChipIcon`).
    func testChipIconResolution() {
        XCTAssertEqual(ChipIconRegistry.resolve("calendar").symbol, "calendar")
        XCTAssertEqual(ChipIconRegistry.resolve("flag").symbol, "flag.fill")
        // Tabler `bulb`/`lightbulb` both map to the SF lightbulb.
        XCTAssertEqual(ChipIconRegistry.resolve("lightbulb").symbol, "lightbulb")
        // An emoji is not a known name → returned verbatim as the emoji branch.
        let emoji = ChipIconRegistry.resolve("📅")
        XCTAssertNil(emoji.symbol)
        XCTAssertEqual(emoji.emoji, "📅")
        // No icon → both nil.
        XCTAssertNil(ChipIconRegistry.resolve(nil).symbol)
        XCTAssertNil(ChipIconRegistry.resolve(nil).emoji)
    }

    /// `chip_value_format: iso` strips the `[[ ]]` link wrapper from a date.
    func testIsoFormatStripsLinkBrackets() {
        let raw = makeDef(name: "Created", valueType: .date, chipValueFormat: .iso)
        XCTAssertEqual(ChipFormat.formattedValue("[[2026-05-13]]", def: raw), "2026-05-13")
    }
}
